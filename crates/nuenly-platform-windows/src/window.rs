//! Jendela di latar depan dan identitas aplikasinya, tanpa membaca isi jendela (F-01).

use std::ffi::c_void;
use std::path::Path;

use nuenly_core::{AppIdentity, ContextError, ForegroundWindow, SensitiveText, WindowHandle};
use windows::Win32::Foundation::HWND;
use windows::Win32::Storage::FileSystem::{
    GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
};
use windows::core::HSTRING;
use zeroize::Zeroizing;

use crate::process::{name_of, process_path};

/// Batas panjang nama aplikasi untuk pengguna, dalam karakter.
const MAX_DISPLAY_NAME: usize = 64;
/// Batas panjang judul jendela yang dibaca, dalam unit UTF-16.
const MAX_TITLE: usize = 32_767;

/// Jendela di latar depan, atau `None` bila tidak ada (misalnya desktop atau layar kunci).
///
/// Hanya membaca identitas prosesnya: nama executable dan deskripsi dari metadata file, tanpa
/// menyentuh isi jendela.
///
/// # Errors
///
/// [`ContextError::Platform`] bila proses pemilik jendela tidak bisa dibaca.
pub fn foreground_window() -> Result<Option<ForegroundWindow>, ContextError> {
    let hwnd = foreground();
    let Some(pid) = window_process(hwnd) else {
        return Ok(None);
    };
    let path = process_path(pid)?;
    let process = name_of(&path)?;
    let display_name = file_description(&path).unwrap_or_else(|| file_stem(process.as_str()));
    Ok(Some(ForegroundWindow {
        handle: handle(hwnd),
        app: AppIdentity {
            process,
            display_name,
        },
    }))
}

pub(crate) fn foreground() -> HWND {
    // SAFETY: GetForegroundWindow tidak punya prasyarat dan hanya membaca state sistem.
    unsafe { GetForegroundWindow() }
}

pub(crate) fn handle(hwnd: HWND) -> WindowHandle {
    WindowHandle(hwnd.0.addr() as u64)
}

/// Judul native jendela. Teks layar, jadi hanya dipakai untuk memeriksa halaman browser.
pub(crate) fn title(hwnd: HWND) -> SensitiveText {
    // SAFETY: tanpa prasyarat; handle yang tidak valid hanya menghasilkan 0.
    let len = unsafe { GetWindowTextLengthW(hwnd) };
    let len = usize::try_from(len).unwrap_or(0).min(MAX_TITLE);
    if len == 0 {
        return SensitiveText::default();
    }
    let mut buffer = Zeroizing::new(vec![0u16; len + 1]);
    // SAFETY: `buffer` berisi `len + 1` elemen u16 yang bisa ditulis dan hidup selama panggilan;
    // fungsi menulis paling banyak sepanjang slice itu, termasuk nol penutup.
    let copied = unsafe { GetWindowTextW(hwnd, &mut buffer) };
    let copied = usize::try_from(copied).unwrap_or(0);
    SensitiveText::new(String::from_utf16_lossy(
        buffer.get(..copied).unwrap_or_default(),
    ))
}

/// ID proses pemilik jendela, atau `None` bila jendelanya tidak ada (lagi).
pub(crate) fn window_process(hwnd: HWND) -> Option<u32> {
    if hwnd.is_invalid() {
        return None;
    }
    let mut pid = 0;
    // SAFETY: `pid` adalah u32 yang bisa ditulis dan hidup selama panggilan. Handle yang sudah
    // tidak valid hanya membuat fungsi ini mengembalikan 0.
    unsafe { GetWindowThreadProcessId(hwnd, Some(&raw mut pid)) };
    (pid != 0).then_some(pid)
}

/// `FileDescription` dari metadata versi executable, misalnya "Microsoft Outlook".
fn file_description(path: &str) -> Option<String> {
    let path = HSTRING::from(path);
    // SAFETY: `path` adalah string UTF-16 berakhiran nol yang hidup selama panggilan.
    let size = unsafe { GetFileVersionInfoSizeW(&path, None) };
    if size == 0 {
        return None;
    }
    let mut data = vec![0u8; size as usize];
    // SAFETY: `data` berisi `size` byte yang bisa ditulis, sesuai ukuran dari
    // GetFileVersionInfoSizeW untuk file yang sama.
    unsafe { GetFileVersionInfoW(&path, None, size, data.as_mut_ptr().cast()) }.ok()?;

    // Bahasa dan code page pertama dari tabel terjemahan, lalu Inggris (AS) Unicode.
    let mut tables = Vec::new();
    if let Some(&[lang_lo, lang_hi, page_lo, page_hi, ..]) =
        query(&data, &HSTRING::from("\\VarFileInfo\\Translation"), 1)
    {
        let language = u16::from_le_bytes([lang_lo, lang_hi]);
        let code_page = u16::from_le_bytes([page_lo, page_hi]);
        tables.push(format!("{language:04x}{code_page:04x}"));
    }
    tables.push("040904b0".to_owned());

    tables.iter().find_map(|table| {
        let key = HSTRING::from(format!("\\StringFileInfo\\{table}\\FileDescription"));
        // Panjang string dari VerQueryValueW dihitung dalam karakter UTF-16.
        let bytes = query(&data, &key, 2)?;
        let units: Vec<u16> = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes(*pair))
            .collect();
        clean(&String::from_utf16_lossy(&units))
    })
}

/// Mencari `block` di blok versi `data`. Panjang hasil `VerQueryValueW` dikalikan `unit` untuk
/// mendapat jumlah byte. Hasilnya `None` bila tidak ada atau berada di luar `data`.
fn query<'a>(data: &'a [u8], block: &HSTRING, unit: usize) -> Option<&'a [u8]> {
    let mut pointer: *mut c_void = std::ptr::null_mut();
    let mut len = 0u32;
    // SAFETY: `data` adalah blok versi utuh dari GetFileVersionInfoW dan `block` string UTF-16
    // berakhiran nol; keduanya hidup selama panggilan. `pointer` dan `len` bisa ditulis.
    let found =
        unsafe { VerQueryValueW(data.as_ptr().cast(), block, &raw mut pointer, &raw mut len) };
    if !found.as_bool() {
        return None;
    }
    // Penunjuk diubah menjadi rentang di `data`, jadi hasilnya dibaca tanpa `unsafe` dan
    // dipastikan tidak keluar dari blok.
    let start = pointer.addr().checked_sub(data.as_ptr().addr())?;
    let end = start.checked_add((len as usize).checked_mul(unit)?)?;
    data.get(start..end)
}

/// Merapikan nama dari metadata: berhenti di karakter nol, tanpa karakter kontrol, dan dipotong
/// ke [`MAX_DISPLAY_NAME`] karakter.
fn clean(raw: &str) -> Option<String> {
    let text = raw.split('\0').next().unwrap_or_default();
    let name: String = text
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_DISPLAY_NAME)
        .collect();
    let name = name.trim();
    (!name.is_empty()).then(|| name.to_owned())
}

/// Nama cadangan bila executable tidak punya deskripsi: `outlook.exe` menjadi `outlook`.
fn file_stem(process: &str) -> String {
    Path::new(process)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(process)
        .to_owned()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{clean, file_description, file_stem, foreground_window};

    #[test]
    fn reads_the_description_of_a_windows_program() {
        let windows = std::env::var_os("SystemRoot")
            .map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from);
        let explorer = windows.join("explorer.exe");
        let description = file_description(explorer.to_str().unwrap()).unwrap();
        assert_ne!(description, "");
    }

    #[test]
    fn missing_file_has_no_description() {
        assert_eq!(file_description(r"C:\tidak-ada\sintetis.exe"), None);
    }

    #[test]
    fn cleans_names_from_metadata() {
        assert_eq!(clean("Notepad\0sisa"), Some("Notepad".to_owned()));
        assert_eq!(clean("  Mail\r\n"), Some("Mail".to_owned()));
        assert_eq!(clean("\0"), None);
        assert_eq!(clean(&"a".repeat(100)).unwrap().len(), 64);
    }

    #[test]
    fn falls_back_to_the_file_stem() {
        assert_eq!(file_stem("outlook.exe"), "outlook");
        assert_eq!(file_stem("tanpa-ekstensi"), "tanpa-ekstensi");
    }

    // Di CI mungkin tidak ada jendela di latar depan, dan jendela milik proses terproteksi tidak
    // bisa dibaca; yang diuji hanya bahwa identitas yang terbaca punya nama.
    #[test]
    fn foreground_identity_has_a_name() {
        if let Ok(Some(window)) = foreground_window() {
            assert_ne!(window.app.display_name, "");
        }
    }
}
