//! [`ContextSource`] Windows: identitas jendela lewat Win32, isinya lewat UI Automation di satu
//! thread COM khusus.

use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use nuenly_core::{
    Browser, BrowserPage, CaptureKind, ContextError, ContextSnapshot, ContextSource,
    ForegroundWindow, ReadPermit, SensitiveText,
};

use crate::clipboard::Saved;
use crate::uia::{ReplaceError, ReplaceReason, Uia};
use crate::window;

/// Batas waktu Ganti teks, dari memeriksa teks sampai menempel.
const REPLACE_TIMEOUT: Duration = Duration::from_secs(3);
/// Ctrl+V tidak dikirim lagi bila sisa waktunya kurang dari ini, agar tempel tidak terjadi setelah
/// pemanggil berhenti menunggu.
const PASTE_MARGIN: Duration = Duration::from_millis(250);

/// Pekerjaan untuk thread UIA yang memegang `C`.
pub(crate) type Job<C> = Box<dyn FnOnce(&mut C) + Send>;

pub(crate) const THREAD_ERROR: ContextError = ContextError::Platform {
    operation: "thread UIA",
    code: 0,
};

/// Mengirim `job` ke thread UIA lewat `send`, lalu menunggu hasilnya paling lama `timeout`.
/// `send` mengembalikan `false` bila thread-nya sudah berhenti.
///
/// Pekerjaan yang sudah ditinggal pemanggil tidak dikerjakan, agar jendela yang mungkin sudah
/// berganti tidak ikut dibaca.
pub(crate) fn run_until<C: 'static, T: Send + 'static>(
    send: impl FnOnce(Job<C>) -> bool,
    timeout: Duration,
    job: impl FnOnce(&mut C) -> Result<T, ContextError> + Send + 'static,
) -> Result<T, ContextError> {
    let deadline = Instant::now() + timeout;
    let (reply, result) = mpsc::sync_channel(1);
    let delivered = send(Box::new(move |context| {
        if Instant::now() < deadline {
            let _ = reply.send(job(context));
        }
    }));
    if !delivered {
        return Err(THREAD_ERROR);
    }
    result
        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .map_err(|_| ContextError::Timeout)?
}

/// Sumber konteks dari UI Automation.
///
/// Semua panggilan UIA berjalan berurutan di thread `nuenly-uia`, karena memanggilnya dari
/// thread UI bisa macet saat jendela milik proses ini sendiri ikut dibaca. Pemanggil menunggu
/// paling lama `timeout`; aplikasi yang hang hanya membuat hasilnya terlambat lalu dibuang.
pub struct UiaContextSource {
    jobs: mpsc::Sender<Job<Uia>>,
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
        let (jobs, queue) = mpsc::channel::<Job<Uia>>();
        let (ready, started) = mpsc::sync_channel(1);
        thread::Builder::new()
            .name("nuenly-uia".to_owned())
            .spawn(move || {
                let mut uia = match Uia::new() {
                    Ok(uia) => uia,
                    Err(err) => {
                        let _ = ready.send(Err(err));
                        return;
                    }
                };
                let _ = ready.send(Ok(()));
                // Berhenti saat UiaContextSource di-drop.
                for job in queue {
                    job(&mut uia);
                }
            })
            .map_err(|_| THREAD_ERROR)?;
        started.recv().map_err(|_| THREAD_ERROR)??;
        Ok(Self { jobs, timeout })
    }

    /// ID target Ganti teks dari bacaan terakhir yang berhasil, bila teksnya bisa diganti: teks
    /// terpilih, atau seluruh isi kolom yang terbaca utuh. Bacaan berikutnya menggantinya.
    #[must_use]
    pub fn replace_target(&self) -> Option<u64> {
        self.run(|uia| Ok(uia.target())).ok().flatten()
    }

    /// Melupakan target Ganti teks tanpa menunggu, misalnya saat Tanya Nuenly ditutup.
    pub fn forget_target(&self) {
        let _ = self
            .jobs
            .send(Box::new(|uia: &mut Uia| uia.forget_target()));
    }

    /// Ganti teks (F-23): menempel `replacement` di tempat target `id` dibaca, hanya bila teks di
    /// sana masih `expected`. Dipanggil atas klik pengguna selagi jendela Nuenly di depan.
    ///
    /// Hasilnya isi clipboard sebelumnya; kembalikan dengan [`Saved::restore`] setelah aplikasi
    /// asal sempat menempel. Tempel tidak pernah dikirim setelah pemanggil berhenti menunggu.
    ///
    /// # Errors
    ///
    /// [`ReplaceError`]. Bila batas tunggu habis, jendela asal dianggap sudah di depan.
    pub fn replace(
        &self,
        id: u64,
        expected: SensitiveText,
        replacement: SensitiveText,
    ) -> Result<Saved, ReplaceError> {
        let paste_by = Instant::now() + REPLACE_TIMEOUT.saturating_sub(PASTE_MARGIN);
        run_until(
            |work| self.jobs.send(work).is_ok(),
            REPLACE_TIMEOUT,
            move |uia: &mut Uia| Ok(uia.replace(id, &expected, replacement.expose(), paste_by)),
        )
        .unwrap_or_else(|_| {
            Err(ReplaceError {
                reason: ReplaceReason::Failed,
                switched: true,
            })
        })
    }

    /// Menjalankan `job` di thread UIA dan menunggu hasilnya paling lama `timeout`.
    fn run<T: Send + 'static>(
        &self,
        job: impl FnOnce(&Uia) -> Result<T, ContextError> + Send + 'static,
    ) -> Result<T, ContextError> {
        run_until(
            |work| self.jobs.send(work).is_ok(),
            self.timeout,
            move |uia: &mut Uia| job(uia),
        )
    }
}

impl ContextSource for UiaContextSource {
    fn foreground_window(&self) -> Result<Option<ForegroundWindow>, ContextError> {
        window::foreground_window()
    }

    fn browser_page(
        &self,
        permit: &ReadPermit<'_>,
        browser: Browser,
    ) -> Result<BrowserPage, ContextError> {
        let window = permit.window().clone();
        self.run(move |uia| uia.browser_page(&window, browser))
    }

    fn read(
        &self,
        permit: &ReadPermit<'_>,
        kind: CaptureKind,
    ) -> Result<Option<ContextSnapshot>, ContextError> {
        let window = permit.window().clone();
        match kind {
            CaptureKind::SelectedText => self.run(move |uia| uia.read_selection(&window)),
            CaptureKind::FocusedField => self.run(move |uia| uia.read_field(&window)),
            _ => Err(ContextError::Unsupported),
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
    use std::time::Duration;

    use nuenly_core::{ContextError, SensitiveText};

    use super::UiaContextSource;
    use crate::uia::{ReplaceError, ReplaceReason};

    /// Dua klien UIA yang dibuat bersamaan di satu proses bisa gagal dengan `E_FAIL`. Aplikasi
    /// membuatnya satu per satu saat start, jadi test yang membuat klien UIA dibuat bergiliran.
    static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

    /// Giliran membuat klien UIA, untuk test di modul mana pun.
    pub(crate) fn uia_turn() -> MutexGuard<'static, ()> {
        ONE_AT_A_TIME.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn spawn(timeout: Duration) -> (MutexGuard<'static, ()>, UiaContextSource) {
        let guard = uia_turn();
        (guard, UiaContextSource::spawn(timeout).unwrap())
    }

    #[test]
    fn an_unknown_target_is_refused_before_any_window_is_touched() {
        let (_guard, source) = spawn(Duration::from_secs(5));
        assert_eq!(source.replace_target(), None);
        let result = source.replace(
            42,
            SensitiveText::new("teks sintetis"),
            SensitiveText::new("jawaban sintetis"),
        );
        assert_eq!(
            result.err(),
            Some(ReplaceError {
                reason: ReplaceReason::Gone,
                switched: false,
            })
        );
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
