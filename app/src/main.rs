//! `nuenly.exe`: shell Tauri yang memuat jendela, ikon tray, dan command IPC.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]

mod answer;
mod ask;
mod audit;
mod clipboard;
mod config;
mod context;
mod foreground;
mod hardware;
mod hotkey;
mod logging;
mod ollama;
mod onboarding;
mod settings;
mod status;
mod store;
mod suggest;
mod toast;
mod tray;

use std::sync::Arc;

use anyhow::Context as _;
use tauri::{Manager, WindowEvent};

fn main() -> anyhow::Result<()> {
    logging::init();
    #[cfg(windows)]
    nuenly_platform_windows::harden_dll_search().context("gagal membatasi pencarian DLL")?;

    tauri::Builder::default()
        .manage(ask::AskState::default())
        .manage(hardware::HardwareState::default())
        .manage(suggest::SuggestState::default())
        .manage(tray::PopupState::default())
        .invoke_handler(tauri::generate_handler![
            answer::ask_send,
            audit::audit_clear,
            audit::audit_list,
            clipboard::clipboard_write_text,
            ollama::ollama_open_download,
            ollama::ollama_status,
            onboarding::onboarding_accept,
            onboarding::onboarding_finish,
            settings::settings_get,
            settings::settings_set_hotkey,
            settings::settings_set_model,
            status::status_get,
            status::status_pause,
            status::status_resume,
            status::status_set_mode,
            suggest::toast_action,
            toast::toast_layout,
            tray::app_quit,
            tray::open_settings,
        ])
        .setup(|app| {
            let store = store::open(&app.path().app_local_data_dir()?)?;
            let legacy = app.path().app_config_dir()?.join(config::LEGACY_FILE);
            let config = config::ConfigStore::load(Arc::clone(&store), &legacy);
            let mut modes = nuenly_modes::ModeManager::official()?;
            let saved = config.get();
            modes.restore(
                saved
                    .app_modes
                    .iter()
                    .map(|(app, id)| (app.as_str(), id.as_str())),
            );
            app.manage(store);
            app.manage(config);
            app.manage(status::AppStatus::new(modes));
            app.manage(nuenly_llm::OllamaProvider::new()?);
            app.manage(context::ContextReader::new());
            // Didaftarkan setelah semua state yang dipakai alur hotkey tersedia.
            let shortcut = hotkey::configured(&saved);
            app.manage(hotkey::HotkeyState::register(app.handle(), shortcut));
            tray::create(app.handle())?;
            foreground::watch(app.handle());
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
            // Tanya Nuenly tertutup sendiri saat pengguna pindah ke jendela lain, termasuk saat
            // disembunyikan lewat Esc.
            WindowEvent::Focused(false) if window.label() == ask::LABEL => {
                let _ = window.hide();
                ask::closed(window.app_handle());
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
