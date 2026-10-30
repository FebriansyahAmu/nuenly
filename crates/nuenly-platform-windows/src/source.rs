//! [`ContextSource`] Windows: identitas jendela lewat Win32, isinya lewat UI Automation di satu
//! thread COM khusus.

use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use nuenly_core::{
    CaptureKind, ContextError, ContextSnapshot, ContextSource, ForegroundWindow, ReadPermit,
};

use crate::uia::Uia;
use crate::window;

type Job = Box<dyn FnOnce(&Uia) + Send>;

const THREAD_ERROR: ContextError = ContextError::Platform {
    operation: "thread UIA",
    code: 0,
};

/// Sumber konteks dari UI Automation.
///
/// Semua panggilan UIA berjalan berurutan di thread `nuenly-uia`, karena memanggilnya dari
/// thread UI bisa macet saat jendela milik proses ini sendiri ikut dibaca. Pemanggil menunggu
/// paling lama `timeout`; aplikasi yang hang hanya membuat hasilnya terlambat lalu dibuang.
pub struct UiaContextSource {
    jobs: mpsc::Sender<Job>,
    timeout: Duration,
}

impl UiaContextSource {
    /// Membuat thread UIA dan klien UI Automation-nya.
    ///
    /// # Errors
    ///
    /// [`ContextError::Platform`] bila thread tidak bisa dibuat atau UI Automation gagal
    /// diinisialisasi.
    pub fn spawn(timeout: Duration) -> Result<Self, ContextError> {
        let (jobs, queue) = mpsc::channel::<Job>();
        let (ready, started) = mpsc::sync_channel(1);
        thread::Builder::new()
            .name("nuenly-uia".to_owned())
            .spawn(move || {
                let uia = match Uia::new() {
                    Ok(uia) => uia,
                    Err(err) => {
                        let _ = ready.send(Err(err));
                        return;
                    }
                };
                let _ = ready.send(Ok(()));
                // Berhenti saat UiaContextSource di-drop.
                for job in queue {
                    job(&uia);
                }
            })
            .map_err(|_| THREAD_ERROR)?;
        started.recv().map_err(|_| THREAD_ERROR)??;
        Ok(Self { jobs, timeout })
    }

    /// Menjalankan `job` di thread UIA dan menunggu hasilnya paling lama `timeout`.
    fn run<T: Send + 'static>(
        &self,
        job: impl FnOnce(&Uia) -> Result<T, ContextError> + Send + 'static,
    ) -> Result<T, ContextError> {
        let deadline = Instant::now() + self.timeout;
        let (reply, result) = mpsc::sync_channel(1);
        self.jobs
            .send(Box::new(move |uia| {
                // Permintaan yang sudah ditinggal pemanggil tidak dikerjakan, agar jendela yang
                // mungkin sudah berganti tidak ikut dibaca.
                if Instant::now() < deadline {
                    let _ = reply.send(job(uia));
                }
            }))
            .map_err(|_| THREAD_ERROR)?;
        result
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .map_err(|_| ContextError::Timeout)?
    }
}

impl ContextSource for UiaContextSource {
    fn foreground_window(&self) -> Result<Option<ForegroundWindow>, ContextError> {
        window::foreground_window()
    }

    fn read(
        &self,
        permit: &ReadPermit<'_>,
        kind: CaptureKind,
    ) -> Result<Option<ContextSnapshot>, ContextError> {
        match kind {
            CaptureKind::SelectedText => {
                let window = permit.window().clone();
                self.run(move |uia| uia.read_selection(&window))
            }
            // Isi field fokus baru dibutuhkan saran proaktif (F-09, Fase 2).
            _ => Err(ContextError::Unsupported),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
    use std::time::Duration;

    use nuenly_core::ContextError;

    use super::UiaContextSource;

    /// Dua klien UIA yang dibuat bersamaan di satu proses bisa gagal dengan `E_FAIL`. Aplikasi
    /// hanya membuat satu, jadi test di sini dibuat bergiliran.
    static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

    fn spawn(timeout: Duration) -> (MutexGuard<'static, ()>, UiaContextSource) {
        let guard = ONE_AT_A_TIME.lock().unwrap_or_else(PoisonError::into_inner);
        (guard, UiaContextSource::spawn(timeout).unwrap())
    }

    #[test]
    fn jobs_run_on_the_uia_thread() {
        let (_guard, source) = spawn(Duration::from_secs(5));
        let name = source
            .run(|_| Ok(std::thread::current().name().map(str::to_owned)))
            .unwrap();
        assert_eq!(name.as_deref(), Some("nuenly-uia"));
    }

    #[test]
    fn slow_jobs_time_out_and_abandoned_jobs_are_skipped() {
        let (_guard, source) = spawn(Duration::from_millis(100));
        let slow = source.run(|_| {
            std::thread::sleep(Duration::from_millis(400));
            Ok(())
        });
        assert_eq!(slow, Err(ContextError::Timeout));

        // Antre di belakang pekerjaan lambat, jadi sudah lewat batas saat gilirannya tiba.
        let ran = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&ran);
        let abandoned = source.run(move |_| {
            flag.store(true, Ordering::SeqCst);
            Ok(())
        });
        assert_eq!(abandoned, Err(ContextError::Timeout));

        // Setelah antrean kosong, pekerjaan baru jalan lagi; urutannya menjamin pekerjaan yang
        // ditinggal sudah dilewati.
        std::thread::sleep(Duration::from_millis(600));
        assert_eq!(source.run(|_| Ok(())), Ok(()));
        assert!(!ran.load(Ordering::SeqCst));
    }
}
