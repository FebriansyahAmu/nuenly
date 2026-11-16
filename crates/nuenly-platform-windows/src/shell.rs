//! Membuka tautan di browser bawaan pengguna.

use std::io;

use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::{HSTRING, PCWSTR, w};

/// Membuka tautan `https://` di browser bawaan, misalnya halaman unduhan Ollama.
///
/// Panggil dari thread UI (COM sudah diinisialisasi), seperti command Tauri yang tidak `async`.
///
/// # Errors
///
/// `InvalidInput` bila tautan bukan `https://`; error OS bila Windows gagal membukanya.
pub fn open_https(url: &str) -> io::Result<()> {
    if !url.starts_with("https://") || url.chars().any(char::is_control) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "hanya tautan https",
        ));
    }
    let url = HSTRING::from(url);
    // SAFETY: "open" dan `url` adalah string UTF-16 berakhiran nol yang hidup selama panggilan;
    // tanpa jendela induk, parameter, dan folder kerja.
    let result = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            &url,
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    // Menurut dokumentasi ShellExecuteW, nilai di atas 32 berarti berhasil.
    if result.0.addr() > 32 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(test)]
mod tests {
    use super::open_https;

    #[test]
    fn refuses_anything_but_https() {
        for url in [
            "http://ollama.com",
            "file:///C:/Windows/System32/calc.exe",
            "calc.exe",
            "https://ollama.com/\r\nx",
        ] {
            assert_eq!(
                open_https(url).unwrap_err().kind(),
                std::io::ErrorKind::InvalidInput
            );
        }
    }
}
