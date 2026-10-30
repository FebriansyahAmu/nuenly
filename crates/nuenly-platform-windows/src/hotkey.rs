//! Hotkey global lewat `RegisterHotKey` (F-03).
//!
//! Windows hanya memberi tahu saat kombinasi yang didaftarkan ditekan. Tombol lain tidak pernah
//! sampai ke Nuenly, jadi ini bukan keylogger.

use std::io;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use nuenly_core::{Key, Shortcut};
use windows::Win32::Foundation::{ERROR_HOTKEY_ALREADY_REGISTERED, LPARAM, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN, RegisterHotKey,
    UnregisterHotKey, VK_F1, VK_SPACE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetMessageW, MSG, PostThreadMessageW, WM_HOTKEY, WM_QUIT,
};

/// ID hotkey di thread-nya. Setiap [`Hotkey`] punya thread sendiri, jadi ID-nya selalu sama.
const ID: i32 = 1;

/// Kegagalan mendaftarkan hotkey.
#[derive(Debug, thiserror::Error)]
pub enum HotkeyError {
    /// Kombinasi itu sudah didaftarkan aplikasi lain.
    #[error("kombinasi tombol sudah dipakai aplikasi lain")]
    Taken,
    #[error("kombinasi tombol tidak didukung")]
    Unsupported,
    #[error("hotkey tidak bisa didaftarkan: {0}")]
    Os(#[from] io::Error),
}

impl From<windows::core::Error> for HotkeyError {
    fn from(err: windows::core::Error) -> Self {
        if err.code() == ERROR_HOTKEY_ALREADY_REGISTERED.to_hresult() {
            Self::Taken
        } else {
            Self::Os(err.into())
        }
    }
}

/// Hotkey yang sedang terdaftar. Dilepas saat di-drop.
#[derive(Debug)]
pub struct Hotkey {
    thread_id: u32,
    /// Terisi setelah thread hotkey melepas kombinasinya. `None` setelah [`Hotkey::release`].
    released: Option<mpsc::Receiver<()>>,
}

impl Hotkey {
    /// Mendaftarkan `shortcut` dan memanggil `on_press` setiap kali kombinasi itu ditekan.
    /// Menahan tombol tidak memicu ulang.
    ///
    /// `on_press` berjalan di thread hotkey, jadi tekanan berikutnya menunggu sampai panggilan
    /// sebelumnya selesai.
    ///
    /// # Errors
    ///
    /// [`HotkeyError::Taken`] bila kombinasi itu sudah dipakai aplikasi lain,
    /// [`HotkeyError::Unsupported`] bila tombolnya tidak dikenal, atau [`HotkeyError::Os`] bila
    /// Windows menolaknya karena alasan lain atau thread tidak bisa dibuat.
    pub fn register(
        shortcut: Shortcut,
        on_press: impl Fn() + Send + 'static,
    ) -> Result<Self, HotkeyError> {
        let (modifiers, key) = win32(shortcut).ok_or(HotkeyError::Unsupported)?;
        let (ready, registered) = mpsc::sync_channel(1);
        let (unregistered, released) = mpsc::sync_channel(1);
        thread::Builder::new()
            .name("nuenly-hotkey".to_owned())
            .spawn(move || {
                // SAFETY: GetCurrentThreadId tidak punya prasyarat.
                let thread_id = unsafe { GetCurrentThreadId() };
                // SAFETY: tanpa jendela, Windows mengirim WM_HOTKEY ke antrean pesan thread ini,
                // yang dibuat oleh panggilan ini juga.
                let result =
                    unsafe { RegisterHotKey(None, ID, modifiers | MOD_NOREPEAT, u32::from(key)) };
                if let Err(err) = result {
                    let _ = ready.send(Err(HotkeyError::from(err)));
                    return;
                }
                let _ = ready.send(Ok(thread_id));
                listen(&on_press);
                // SAFETY: hotkey ini didaftarkan di thread yang sama dengan ID yang sama.
                let _ = unsafe { UnregisterHotKey(None, ID) };
                let _ = unregistered.send(());
            })?;
        let thread_id = registered
            .recv()
            .map_err(|_| io::Error::other("thread hotkey berhenti"))??;
        Ok(Self {
            thread_id,
            released: Some(released),
        })
    }

    /// Melepas hotkey dan menunggu sampai Windows benar-benar melepasnya, paling lama
    /// `timeout`, agar kombinasi yang sama bisa langsung didaftarkan lagi. `false` bila belum
    /// terlepas, misalnya karena `on_press` masih berjalan.
    #[must_use]
    pub fn release(mut self, timeout: Duration) -> bool {
        let released = self.released.take();
        self.quit();
        released.is_some_and(|released| released.recv_timeout(timeout).is_ok())
    }

    fn quit(&self) {
        // SAFETY: tanpa prasyarat; bila thread-nya sudah berhenti, panggilan ini hanya gagal.
        let _ = unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) };
    }
}

impl Drop for Hotkey {
    /// Meminta thread hotkey berhenti tanpa menunggunya, agar tidak macet bila `on_press` sedang
    /// menunggu thread yang men-drop.
    fn drop(&mut self) {
        if self.released.is_some() {
            self.quit();
        }
    }
}

/// Pengubah dan kode tombol virtual Windows untuk `shortcut`.
fn win32(shortcut: Shortcut) -> Option<(HOT_KEY_MODIFIERS, u16)> {
    let mut modifiers = HOT_KEY_MODIFIERS(0);
    for (on, flag) in [
        (shortcut.ctrl(), MOD_CONTROL),
        (shortcut.alt(), MOD_ALT),
        (shortcut.shift(), MOD_SHIFT),
        (shortcut.win(), MOD_WIN),
    ] {
        if on {
            modifiers |= flag;
        }
    }
    // Kode tombol huruf dan angka sama dengan ASCII-nya; F1–F24 berurutan.
    let key = match shortcut.key() {
        Key::Space => VK_SPACE.0,
        Key::Letter(letter @ b'A'..=b'Z') => u16::from(letter),
        Key::Digit(digit @ b'0'..=b'9') => u16::from(digit),
        Key::Function(number @ 1..=24) => VK_F1.0 + u16::from(number) - 1,
        _ => return None,
    };
    Some((modifiers, key))
}

/// Loop pesan thread hotkey sampai `WM_QUIT`.
fn listen(on_press: &impl Fn()) {
    let mut message = MSG::default();
    // GetMessageW mengembalikan 0 untuk WM_QUIT dan -1 bila gagal; keduanya mengakhiri loop.
    // SAFETY: `message` adalah MSG yang bisa ditulis dan hidup selama setiap panggilan.
    while unsafe { GetMessageW(&raw mut message, None, 0, 0) }.0 > 0 {
        // Thread ini hanya mendaftarkan satu hotkey, jadi setiap WM_HOTKEY berasal darinya.
        if message.message == WM_HOTKEY {
            on_press();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use nuenly_core::Shortcut;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        MOD_ALT, MOD_CONTROL, MOD_SHIFT, VK_F24, VK_K,
    };

    use super::{Hotkey, HotkeyError, win32};

    /// Kombinasi yang hampir pasti tidak dipakai siapa pun, agar test tidak mengganggu desktop.
    fn unused() -> Shortcut {
        Shortcut::parse("Ctrl+Alt+Shift+F24").unwrap()
    }

    #[test]
    fn maps_shortcuts_to_windows_keys() {
        let (modifiers, key) = win32(unused()).unwrap();
        assert_eq!(modifiers, MOD_CONTROL | MOD_ALT | MOD_SHIFT);
        assert_eq!(key, VK_F24.0);
        assert_eq!(win32(Shortcut::parse("Alt+K").unwrap()).unwrap().1, VK_K.0);
    }

    #[test]
    fn taken_shortcut_is_refused_until_released() {
        let first = Hotkey::register(unused(), || {}).unwrap();
        let err = Hotkey::register(unused(), || {}).unwrap_err();
        assert!(matches!(err, HotkeyError::Taken), "{err:?}");

        assert!(first.release(Duration::from_secs(1)));
        assert!(Hotkey::register(unused(), || {}).is_ok());
    }
}
