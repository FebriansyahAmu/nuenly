//! Ikon tray. Selalu terlihat selama Nuenly berjalan (US-3).

use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager};

use crate::ask;

const ASK: &str = "ask";
const SETTINGS: &str = "settings";
const QUIT: &str = "quit";

/// Memasang ikon tray dengan menu sementara; popup tray sesuai desain menyusul (F-25).
pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let menu = Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, ASK, "Tanya Nuenly", true, None::<&str>)?,
            &MenuItem::with_id(app, SETTINGS, "Pengaturan", true, None::<&str>)?,
            &MenuItem::with_id(app, QUIT, "Keluar", true, None::<&str>)?,
        ],
    )?;

    let mut tray = TrayIconBuilder::with_id("nuenly")
        .tooltip("Nuenly")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id().as_ref() {
            QUIT => app.exit(0),
            ASK => {
                let _ = ask::open(app);
            }
            SETTINGS => show(app, SETTINGS),
            _ => {}
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

fn show(app: &AppHandle, label: &str) {
    if let Some(window) = app.get_webview_window(label) {
        let _ = window.show();
        let _ = window.set_focus();
    }
}
