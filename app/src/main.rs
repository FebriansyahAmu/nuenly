//! `nuenly.exe`: shell Tauri yang memuat jendela, ikon tray, dan command IPC.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]

mod tray;

use anyhow::Context as _;
use tauri::WindowEvent;

fn main() -> anyhow::Result<()> {
    #[cfg(windows)]
    nuenly_platform_windows::harden_dll_search().context("gagal membatasi pencarian DLL")?;

    tauri::Builder::default()
        .setup(|app| {
            tray::create(app.handle())?;
            Ok(())
        })
        .on_window_event(|window, event| {
            // Nuenly hidup di tray: menutup jendela hanya menyembunyikannya.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .context("Nuenly berhenti karena error")
}
