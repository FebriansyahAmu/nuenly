//! Command clipboard. Jendela toast tidak pernah fokus sehingga tidak bisa memakai clipboard
//! browser, dan penulisan lewat Rust juga mengecualikan teks dari riwayat dan clipboard cloud.

/// Menyalin teks ke clipboard. Pesan error sengaja umum dan tidak memuat isi teks.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Argumen command Tauri selalu dideserialisasi sebagai nilai.
pub fn clipboard_write_text(text: String) -> Result<(), String> {
    write(&text).map_err(|_| "Teks tidak bisa disalin.".to_owned())
}

#[cfg(windows)]
fn write(text: &str) -> std::io::Result<()> {
    nuenly_platform_windows::clipboard::write_text(text)
}

#[cfg(not(windows))]
fn write(_text: &str) -> std::io::Result<()> {
    Err(std::io::Error::other("clipboard belum didukung di OS ini"))
}
