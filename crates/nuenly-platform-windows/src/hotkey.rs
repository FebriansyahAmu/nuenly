//! Hotkey global lewat `RegisterHotKey` (F-03).
//!
//! Windows hanya memberi tahu saat kombinasi yang didaftarkan ditekan. Tombol lain tidak pernah
//! sampai ke Nuenly, jadi ini bukan keylogger.

use std::io;
use std::sync::mpsc;
use std::thread;

use windows::Win32::Foundation::{ERROR_HOTKEY_ALREADY_REGISTERED, LPARAM, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    HOT_KEY_MODIFIERS, MOD_CONTROL, MOD_NOREPEAT, RegisterHotKey, UnregisterHotKey, VK_SPACE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetMessageW, MSG, PostThreadMessageW, WM_HOTKEY, WM_QUIT,
};

/// ID hotkey di thread-nya. Setiap [`Hotkey`] punya thread sendiri, jadi ID-nya selalu sama.
const ID: i32 = 1;

/// Kombinasi tombol untuk hotkey global.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shortcut {
    modifiers: HOT_KEY_MODIFIERS,
    key: u16,
}

impl Shortcut {
    /// Ctrl+Space, pintasan bawaan Tanya Nuenly.
    pub const CTRL_SPACE: Self = Self {
        modifiers: MOD_CONTROL,
        key: VK_SPACE.0,
    };
}

/// Kegagalan mendaftarkan hotkey.
#[derive(Debug, thiserror::Error)]
pub enum HotkeyError {
    /// Kombinasi itu sudah didaftarkan aplikasi lain.
    #[error("kombinasi tombol sudah dipakai aplikasi lain")]
    Taken,
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
    /// [`HotkeyError::Taken`] bila kombinasi itu sudah dipakai aplikasi lain, atau
    /// [`HotkeyError::Os`] bila Windows menolaknya karena alasan lain atau thread tidak bisa
    /// dibuat.
    pub fn register(
        shortcut: Shortcut,
        on_press: impl Fn() + Send + 'static,
    ) -> Result<Self, HotkeyError> {
        let (ready, registered) = mpsc::sync_channel(1);
        thread::Builder::new()
            .name("nuenly-hotkey".to_owned())
            .spawn(move || {
                // SAFETY: GetCurrentThreadId tidak punya prasyarat.
                let thread_id = unsafe { GetCurrentThreadId() };
                // SAFETY: tanpa jendela, Windows mengirim WM_HOTKEY ke antrean pesan thread ini,
                // yang dibuat oleh panggilan ini juga.
                let result = unsafe {
                    RegisterHotKey(
                        None,
                        ID,
                        shortcut.modifiers | MOD_NOREPEAT,
                        u32::from(shortcut.key),
                    )
                };
                if let Err(err) = result {
                    let _ = ready.send(Err(HotkeyError::from(err)));
                    return;
                }
                let _ = ready.send(Ok(thread_id));
                listen(&on_press);
                // SAFETY: hotkey ini didaftarkan di thread yang sama dengan ID yang sama.
                let _ = unsafe { UnregisterHotKey(None, ID) };
            })?;
        let thread_id = registered
            .recv()
            .map_err(|_| io::Error::other("thread hotkey berhenti"))??;
        Ok(Self { thread_id })
    }
}

impl Drop for Hotkey {
    /// Meminta thread hotkey berhenti tanpa menunggunya, agar tidak macet bila `on_press` sedang
    /// menunggu thread yang men-drop.
    fn drop(&mut self) {
        // SAFETY: tanpa prasyarat; bila thread-nya sudah berhenti, panggilan ini hanya gagal.
        let _ = unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) };
    }
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
    use windows::Win32::UI::Input::KeyboardAndMouse::{MOD_ALT, MOD_CONTROL, MOD_SHIFT, VK_F24};

    use super::{HOT_KEY_MODIFIERS, Hotkey, HotkeyError, Shortcut};

    /// Kombinasi yang hampir pasti tidak dipakai siapa pun, agar test tidak mengganggu desktop.
    const UNUSED: Shortcut = Shortcut {
        modifiers: HOT_KEY_MODIFIERS(MOD_CONTROL.0 | MOD_ALT.0 | MOD_SHIFT.0),
        key: VK_F24.0,
    };

    #[test]
    fn taken_shortcut_is_refused_until_released() {
        let first = Hotkey::register(UNUSED, || {}).unwrap();
        let err = Hotkey::register(UNUSED, || {}).unwrap_err();
        assert!(matches!(err, HotkeyError::Taken), "{err:?}");

        drop(first);
        // Thread lama melepas hotkey setelah menerima WM_QUIT; tunggu sebentar.
        let again = (0..50).find_map(|_| {
            std::thread::sleep(std::time::Duration::from_millis(10));
            Hotkey::register(UNUSED, || {}).ok()
        });
        assert!(again.is_some());
    }
}
