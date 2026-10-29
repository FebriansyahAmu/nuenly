//! Pengerasan proses yang dijalankan di awal `main`.

use windows::Win32::System::LibraryLoader::{
    LOAD_LIBRARY_SEARCH_DEFAULT_DIRS, SetDefaultDllDirectories,
};

/// Membatasi pencarian DLL ke folder aplikasi, System32, dan folder yang ditambahkan secara
/// eksplisit, sehingga DLL di folder kerja atau `PATH` tidak ikut dimuat (DLL hijacking).
///
/// Panggil di baris pertama `main`, sebelum ada DLL yang dimuat secara dinamis.
///
/// # Errors
///
/// Error OS bila Windows menolak pengaturan ini.
pub fn harden_dll_search() -> std::io::Result<()> {
    // SAFETY: SetDefaultDllDirectories hanya mengubah urutan pencarian DLL proses ini dan tidak
    // punya prasyarat selain flag yang valid.
    unsafe { SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_DEFAULT_DIRS) }
        .map_err(std::io::Error::from)
}

#[cfg(test)]
mod tests {
    use super::harden_dll_search;

    #[test]
    fn hardening_succeeds() {
        harden_dll_search().unwrap();
    }
}
