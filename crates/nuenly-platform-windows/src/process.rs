//! Nama executable dari ID proses.

use nuenly_core::{ContextError, ProcessName};
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::core::PWSTR;

/// Panjang path maksimum yang didukung, dalam unit UTF-16 (batas path panjang Windows).
const PATH_CAPACITY: u32 = 32_768;

/// Nama executable dari proses `pid`, misalnya `outlook.exe`.
///
/// Hanya membaca path image lewat `PROCESS_QUERY_LIMITED_INFORMATION`, yang tidak butuh hak
/// admin dan tidak menyentuh memori proses.
///
/// # Errors
///
/// [`ContextError::Platform`] bila proses tidak bisa dibuka atau path-nya tidak bisa dibaca.
pub fn process_name(pid: u32) -> Result<ProcessName, ContextError> {
    // SAFETY: OpenProcess tidak punya prasyarat pada argumennya. Handle yang berhasil dibuka
    // langsung dibungkus `ProcessHandle`, yang menutupnya tepat sekali saat di-drop.
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }
        .map(ProcessHandle)
        .map_err(win32_error("OpenProcess"))?;

    let mut buffer = vec![0u16; PATH_CAPACITY as usize];
    let mut len = PATH_CAPACITY;
    // SAFETY: `buffer` berisi `len` elemen u16 yang bisa ditulis dan hidup sampai panggilan
    // selesai. Fungsi menulis paling banyak `len` elemen, lalu mengisi `len` dengan jumlah
    // karakter yang ditulis.
    unsafe {
        QueryFullProcessImageNameW(
            handle.0,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &raw mut len,
        )
    }
    .map_err(win32_error("QueryFullProcessImageNameW"))?;

    let path = buffer
        .get(..len as usize)
        .map(String::from_utf16_lossy)
        .unwrap_or_default();
    ProcessName::new(&path).map_err(|_| ContextError::Platform {
        operation: "QueryFullProcessImageNameW",
        code: 0,
    })
}

/// Handle proses yang ditutup saat di-drop.
struct ProcessHandle(HANDLE);

impl Drop for ProcessHandle {
    fn drop(&mut self) {
        // SAFETY: handle berasal dari OpenProcess yang berhasil dan hanya ditutup di sini.
        // Kegagalan menutup tidak bisa ditangani dengan berarti, jadi diabaikan.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

fn win32_error(operation: &'static str) -> impl FnOnce(windows::core::Error) -> ContextError {
    move |err| ContextError::Platform {
        operation,
        code: err.code().0,
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;
    use std::path::Path;

    use super::process_name;

    #[test]
    fn reads_own_process_name() {
        let name = process_name(std::process::id()).unwrap();
        // ProcessName sudah huruf kecil, jadi perbandingan langsung dengan "exe" sudah tepat.
        assert_eq!(
            Path::new(name.as_str()).extension(),
            Some(OsStr::new("exe"))
        );
        assert_eq!(name.as_str(), name.as_str().to_lowercase());
    }

    #[test]
    fn rejects_system_idle_process() {
        assert!(process_name(0).is_err());
    }
}
