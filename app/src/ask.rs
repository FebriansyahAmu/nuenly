//! Jendela Tanya Nuenly (F-22, US-1): overlay redup di layar kerja tempat kursor berada.

use tauri::{AppHandle, Emitter, Manager};

use crate::onboarding;

pub const LABEL: &str = "ask";

/// Membuka Tanya Nuenly di monitor tempat kursor berada dan memulai sesi baru. Sebelum
/// persetujuan privasi, yang dibuka adalah onboarding.
///
/// Jendelanya sudah dibuat (tersembunyi) saat start, jadi membuka hanya memindahkan,
/// memberi tahu frontend, lalu menampilkan; target PRD ≤ 150 ms.
pub fn open(app: &AppHandle) -> tauri::Result<()> {
    // Tanya Nuenly membaca teks terpilih, jadi menunggu "Saya mengerti" di onboarding.
    if !onboarding::ensure_accepted(app) {
        return Ok(());
    }
    let Some(window) = app.get_webview_window(LABEL) else {
        return Ok(());
    };

    let cursor = app.cursor_position()?;
    let monitor = match app.monitor_from_point(cursor.x, cursor.y)? {
        Some(monitor) => Some(monitor),
        None => app.primary_monitor()?,
    };
    if let Some(monitor) = monitor {
        let area = monitor.work_area();
        window.set_position(area.position)?;
        window.set_size(area.size)?;
    }

    // Konteks (asal teks terpilih) menyusul di alur hotkey, langkah 7. Teksnya tidak pernah
    // dikirim ke jendela; frontend hanya menerima asal dan nama aplikasinya.
    app.emit_to(LABEL, "ask:open", ())?;
    window.show()?;
    window.set_focus()
}
