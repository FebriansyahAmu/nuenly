//! Input keyboard tiruan, hanya untuk Ganti teks (F-23): satu Ctrl+V ke jendela asal setelah
//! pengguna mengklik tombolnya. Tidak ada jalur lain yang mengirim input.

use std::io;

use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput,
    VIRTUAL_KEY, VK_CONTROL, VK_V,
};

/// Mengirim Ctrl+V ke elemen yang sedang fokus di jendela latar depan.
///
/// # Errors
///
/// Error OS bila Windows tidak menerima keempat event. Input yang diblokir UIPI (misalnya ke
/// aplikasi yang berjalan sebagai administrator) tidak terdeteksi di sini.
pub(crate) fn paste() -> io::Result<()> {
    let inputs = [
        key(VK_CONTROL, false),
        key(VK_V, false),
        key(VK_V, true),
        key(VK_CONTROL, true),
    ];
    let size = i32::try_from(size_of::<INPUT>()).map_err(io::Error::other)?;
    // SAFETY: `inputs` adalah array INPUT keyboard yang terinisialisasi penuh dan hidup selama
    // panggilan, dan `size` adalah ukuran satu INPUT seperti yang diminta SendInput.
    let sent = unsafe { SendInput(&inputs, size) };
    if usize::try_from(sent).is_ok_and(|sent| sent == inputs.len()) {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

fn key(code: VIRTUAL_KEY, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: code,
                wScan: 0,
                dwFlags: if up {
                    KEYEVENTF_KEYUP
                } else {
                    KEYBD_EVENT_FLAGS(0)
                },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}
