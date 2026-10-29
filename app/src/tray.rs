//! Ikon tray. Selalu terlihat selama Nuenly berjalan (US-3).

#[cfg(debug_assertions)]
use tauri::menu::Submenu;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager};

use crate::ask;
#[cfg(debug_assertions)]
use crate::toast;

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
    #[cfg(debug_assertions)]
    menu.insert(&test_toast_menu(app)?, 2)?;

    let mut tray = TrayIconBuilder::with_id("nuenly")
        .tooltip("Nuenly")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id().as_ref() {
            QUIT => app.exit(0),
            ASK => {
                let _ = ask::open(app);
            }
            SETTINGS => show(app, SETTINGS),
            #[cfg(debug_assertions)]
            id if id.starts_with(toast::demo::PREFIX) => show_test_toast(app, id),
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

/// Submenu "Uji toast" hanya di build debug, untuk spike fokus dan pengecekan tampilan.
#[cfg(debug_assertions)]
fn test_toast_menu(app: &AppHandle) -> tauri::Result<Submenu<tauri::Wry>> {
    let submenu = Submenu::new(app, "Uji toast (dev)", true)?;
    for (name, label) in toast::demo::ALL {
        let id = format!("{}{name}", toast::demo::PREFIX);
        submenu.append(&MenuItem::with_id(app, id, label, true, None::<&str>)?)?;
    }
    Ok(submenu)
}

#[cfg(debug_assertions)]
fn show_test_toast(app: &AppHandle, id: &str) {
    let demo = id
        .strip_prefix(toast::demo::PREFIX)
        .and_then(toast::demo::toast);
    if let Some(demo) = demo {
        let _ = toast::show(app, &demo);
    }
}
