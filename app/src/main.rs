//! `nuenly.exe`: shell Tauri yang memuat jendela, ikon tray, dan command IPC.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]

mod ask;
mod clipboard;
mod config;
mod ollama;
mod onboarding;
mod settings;
mod status;
mod toast;
mod tray;

use anyhow::Context as _;
use tauri::{Manager, WindowEvent};

fn main() -> anyhow::Result<()> {
    #[cfg(windows)]
    nuenly_platform_windows::harden_dll_search().context("gagal membatasi pencarian DLL")?;

    tauri::Builder::default()
        .manage(status::AppStatus::default())
        .manage(tray::PopupState::default())
        .invoke_handler(tauri::generate_handler![
            clipboard::clipboard_write_text,
            ollama::ollama_open_download,
            ollama::ollama_status,
            onboarding::onboarding_accept,
            onboarding::onboarding_finish,
            settings::settings_get,
            settings::settings_set_model,
            status::status_get,
            status::status_pause,
            status::status_resume,
            status::status_set_mode,
            toast::toast_layout,
            tray::app_quit,
            tray::open_settings,
        ])
        .setup(|app| {
            let config = app.path().app_config_dir()?.join(config::FILE_NAME);
            app.manage(config::ConfigStore::load(config));
            tray::create(app.handle())?;
            // Nuenly tidak membaca apa pun sampai pemberitahuan privasi disetujui.
            if !onboarding::accepted(app.handle()) {
                onboarding::show(app.handle());
            }
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
            // Popup tray juga menutup sendiri, seperti flyout Windows.
            WindowEvent::Focused(false) if window.label() == tray::POPUP => {
                tray::popup_blurred(window.app_handle());
            }
            _ => {}
        })
        .run(tauri::generate_context!())
        .context("Nuenly berhenti karena error")
}
