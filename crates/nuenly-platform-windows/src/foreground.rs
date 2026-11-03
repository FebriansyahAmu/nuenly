//! Aplikasi di latar depan untuk ganti mode otomatis (F-14), lewat
//! `SetWinEventHook(EVENT_SYSTEM_FOREGROUND)`.
//!
//! Windows memberi tahu setiap kali jendela lain naik ke latar depan, jadi tidak ada polling.
//! Yang dibaca hanya identitas aplikasinya: nama executable dan deskripsinya dari metadata file,
//! tanpa isi atau judul jendela. Jendela Nuenly sendiri dan permukaan shell Windows (taskbar,
//! menu Start, Alt+Tab, desktop) dilewati, agar membuka popup tray tidak dianggap pindah
//! aplikasi.

use std::cell::RefCell;
use std::io;
use std::sync::mpsc;
use std::thread;

use nuenly_core::AppIdentity;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows::Win32::UI::WindowsAndMessaging::{
    CHILDID_SELF, EVENT_SYSTEM_FOREGROUND, GetClassNameW, GetMessageW, MSG, OBJID_WINDOW,
    PostThreadMessageW, WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS, WM_QUIT,
};

use crate::process::{name_of, process_path};
use crate::window::{display_name, foreground, window_process};

/// Kelas jendela shell Windows yang bukan aplikasi: taskbar, area notifikasi, Alt+Tab, Task
/// View, dan desktop.
const SHELL_CLASSES: &[&str] = &[
    "Shell_TrayWnd",
    "Shell_SecondaryTrayWnd",
    "NotifyIconOverflowWindow",
    "TopLevelWindowForOverflowXamlIsland",
    "XamlExplorerHostIslandWindow",
    "ForegroundStaging",
    "MultitaskingViewFrame",
    "Progman",
    "WorkerW",
];

/// Proses shell yang jendelanya bukan aplikasi: menu Start, pencarian, dan panel sistem.
const SHELL_PROCESSES: &[&str] = &[
    "startmenuexperiencehost.exe",
    "searchhost.exe",
    "searchapp.exe",
    "shellexperiencehost.exe",
];

/// Batas nama kelas jendela di Windows, termasuk nol penutup.
const CLASS_CAPACITY: usize = 256;

type OnChange = Box<dyn Fn(AppIdentity)>;

thread_local! {
    /// Penerima perubahan di thread pelacak. Callback WinEvent tidak membawa data milik
    /// pemasangnya, jadi penerimanya disimpan di thread yang memasang hook.
    static ON_CHANGE: RefCell<Option<OnChange>> = const { RefCell::new(None) };
}

/// Pelacak aplikasi di latar depan. Berhenti saat di-drop.
#[derive(Debug)]
pub struct ForegroundWatch {
    thread_id: u32,
}

impl ForegroundWatch {
    /// Memanggil `on_change` setiap kali jendela aplikasi lain naik ke latar depan.
    ///
    /// `on_change` berjalan di thread pelacak, jadi perubahan berikutnya menunggu sampai
    /// panggilan sebelumnya selesai.
    ///
    /// # Errors
    ///
    /// [`io::Error`] bila thread tidak bisa dibuat atau Windows menolak memasang hook.
    pub fn start(on_change: impl Fn(AppIdentity) + Send + 'static) -> io::Result<Self> {
        let (ready, started) = mpsc::sync_channel(1);
        thread::Builder::new()
            .name("nuenly-foreground".to_owned())
            .spawn(move || {
                // SAFETY: GetCurrentThreadId tidak punya prasyarat.
                let thread_id = unsafe { GetCurrentThreadId() };
                ON_CHANGE.with_borrow_mut(|slot| *slot = Some(Box::new(on_change)));
                // SAFETY: `on_event` cocok dengan WINEVENTPROC dan hidup selama program. Dengan
                // WINEVENT_OUTOFCONTEXT tanpa modul, Windows memanggilnya di thread ini saat
                // antreannya dipompa `listen`.
                let hook = unsafe {
                    SetWinEventHook(
                        EVENT_SYSTEM_FOREGROUND,
                        EVENT_SYSTEM_FOREGROUND,
                        None,
                        Some(on_event),
                        0,
                        0,
                        WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
                    )
                };
                if hook.is_invalid() {
                    let _ = ready.send(Err(io::Error::other(
                        "hook jendela latar depan tidak bisa dipasang",
                    )));
                    return;
                }
                let _ = ready.send(Ok(thread_id));
                listen();
                // SAFETY: hook ini dipasang di thread yang sama dan hanya dilepas sekali di sini.
                let _ = unsafe { UnhookWinEvent(hook) };
            })?;
        let thread_id = started
            .recv()
            .map_err(|_| io::Error::other("thread pelacak latar depan berhenti"))??;
        Ok(Self { thread_id })
    }
}

impl Drop for ForegroundWatch {
    fn drop(&mut self) {
        // SAFETY: tanpa prasyarat; bila thread-nya sudah berhenti, panggilan ini hanya gagal.
        let _ = unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) };
    }
}

/// Aplikasi di latar depan saat ini, disaring sama seperti [`ForegroundWatch`]. `None` bila yang
/// di depan jendela Nuenly, shell Windows, atau tidak ada jendela.
#[must_use]
pub fn current_app() -> Option<AppIdentity> {
    app_of(foreground())
}

/// Callback `EVENT_SYSTEM_FOREGROUND`, di thread pelacak.
unsafe extern "system" fn on_event(
    _hook: HWINEVENTHOOK,
    _event: u32,
    hwnd: HWND,
    object: i32,
    child: i32,
    _thread: u32,
    _time: u32,
) {
    // Hanya jendelanya sendiri, bukan elemen di dalamnya. Peristiwa yang terlambat (jendela lain
    // sudah naik lagi) dilewati, karena peristiwa jendela yang baru menyusul.
    if object != OBJID_WINDOW.0 || child.cast_unsigned() != CHILDID_SELF || hwnd != foreground() {
        return;
    }
    if let Some(app) = app_of(hwnd) {
        ON_CHANGE.with_borrow(|slot| {
            if let Some(on_change) = slot {
                on_change(app);
            }
        });
    }
}

fn app_of(hwnd: HWND) -> Option<AppIdentity> {
    if is_shell_window(hwnd) {
        return None;
    }
    let pid = window_process(hwnd)?;
    if pid == std::process::id() {
        return None;
    }
    let path = process_path(pid).ok()?;
    let process = name_of(&path).ok()?;
    if SHELL_PROCESSES.contains(&process.as_str()) {
        return None;
    }
    let display_name = display_name(&path, &process);
    Some(AppIdentity {
        process,
        display_name,
    })
}

fn is_shell_window(hwnd: HWND) -> bool {
    let mut buffer = [0u16; CLASS_CAPACITY];
    // SAFETY: `buffer` bisa ditulis dan hidup selama panggilan; fungsi menulis paling banyak
    // sepanjang slice, termasuk nol penutup. Handle yang tidak valid hanya menghasilkan 0.
    let len = unsafe { GetClassNameW(hwnd, &mut buffer) };
    let len = usize::try_from(len).unwrap_or(0);
    let class = String::from_utf16_lossy(buffer.get(..len).unwrap_or_default());
    SHELL_CLASSES.contains(&class.as_str())
}

/// Memompa antrean pesan thread pelacak sampai `WM_QUIT`. Callback hook dipanggil di dalam
/// `GetMessageW`, jadi pesannya sendiri tidak perlu diproses.
fn listen() {
    let mut message = MSG::default();
    // GetMessageW mengembalikan 0 untuk WM_QUIT dan -1 bila gagal; keduanya mengakhiri loop.
    // SAFETY: `message` adalah MSG yang bisa ditulis dan hidup selama setiap panggilan.
    while unsafe { GetMessageW(&raw mut message, None, 0, 0) }.0 > 0 {}
}

#[cfg(test)]
mod tests {
    use nuenly_core::ProcessName;

    use super::{ForegroundWatch, SHELL_PROCESSES, current_app};

    #[test]
    fn shell_process_names_are_normalized() {
        for name in SHELL_PROCESSES {
            assert_eq!(ProcessName::new(name).unwrap().as_str(), *name);
        }
    }

    #[test]
    fn the_watch_starts_and_stops() {
        let watch = ForegroundWatch::start(|_| {}).unwrap();
        drop(watch);
    }

    // Di CI mungkin tidak ada jendela di latar depan; yang diuji hanya bahwa aplikasi yang
    // terbaca punya nama dan bukan proses test ini sendiri.
    #[test]
    fn the_current_app_is_never_this_process() {
        let own = crate::process_name(std::process::id()).unwrap();
        if let Some(app) = current_app() {
            assert_ne!(app.display_name, "");
            assert_ne!(app.process, own);
        }
    }
}
