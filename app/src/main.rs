//! `nuenly.exe`: shell Tauri yang memuat jendela, ikon tray, dan command IPC.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]

mod ask;
mod clipboard;
mod toast;
mod tray;

use anyhow::Context as _;
use tauri::WindowEvent;

fn main() -> anyhow::Result<()> {
    #[cfg(windows)]
    nuenly_platform_windows::harden_dll_search().context("gagal membatasi pencarian DLL")?;

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            clipboard::clipboard_write_text,
            toast::toast_layout,
        ])
        .setup(|app| {
            tray::create(app.handle())?;
            Ok(())
        })
        .on_window_event(|window, event| match event {
            // Nuenly hidup di tray: menutup jendela hanya menyembunyikannya.
            WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                let _ = window.hide();
            }
            // Tanya Nuenly tertutup sendiri saat pengguna pindah ke jendela lain.
            WindowEvent::Focused(false) if window.label() == ask::LABEL => {
                let _ = window.hide();
            }
            _ => {}
        })
        .run(tauri::generate_context!())
        .context("Nuenly berhenti karena error")
}
