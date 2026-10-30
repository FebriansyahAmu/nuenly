//! Ikon tray dan popup-nya (F-25, US-3). Ikon selalu terlihat selama Nuenly berjalan.

use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

#[cfg(debug_assertions)]
use tauri::menu::Submenu;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{
    AppHandle, LogicalPosition, LogicalSize, Manager, PhysicalPosition, PhysicalRect, PhysicalSize,
    Rect,
};

use crate::status::{self, AppStatus};
#[cfg(debug_assertions)]
use crate::toast;
use crate::{ask, onboarding};

pub const ID: &str = "nuenly";
/// Label jendela popup tray.
pub const POPUP: &str = "tray";

const ASK: &str = "ask";
const SETTINGS: &str = "settings";
const QUIT: &str = "quit";

/// Ukuran popup dalam piksel logis, sesuai desain 04.
const POPUP_SIZE: LogicalSize<f64> = LogicalSize::new(320.0, 400.0);
const POPUP_MARGIN: f64 = 8.0;
/// Klik ikon tray yang menutup popup (karena popup kehilangan fokus) tidak boleh langsung
/// membukanya lagi.
const REOPEN_GUARD: Duration = Duration::from_millis(300);

/// Kapan popup terakhir disembunyikan karena kehilangan fokus.
#[derive(Default)]
pub struct PopupState(Mutex<Option<Instant>>);

/// Memasang ikon tray. Klik kiri membuka popup, atau onboarding bila persetujuan privasi belum
/// ada; klik kanan membuka menu cadangan.
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

    let mut tray = TrayIconBuilder::with_id(ID)
        .tooltip(status::tooltip(
            &app.state::<AppStatus>().view(),
            onboarding::accepted(app),
        ))
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                rect,
                ..
            } = event
                && onboarding::ensure_accepted(tray.app_handle())
            {
                let _ = toggle_popup(tray.app_handle(), rect);
            }
        })
        .on_menu_event(|app, event| match event.id().as_ref() {
            QUIT => app.exit(0),
            ASK => {
                let _ = ask::open(app);
            }
            SETTINGS => show_settings(app),
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

/// Dipanggil saat popup kehilangan fokus.
pub fn popup_blurred(app: &AppHandle) {
    if let Some(popup) = app.get_webview_window(POPUP) {
        let _ = popup.hide();
    }
    let state = app.state::<PopupState>();
    *state.0.lock().unwrap_or_else(PoisonError::into_inner) = Some(Instant::now());
}

fn toggle_popup(app: &AppHandle, icon: Rect) -> tauri::Result<()> {
    let Some(popup) = app.get_webview_window(POPUP) else {
        return Ok(());
    };
    if popup.is_visible()? {
        return popup.hide();
    }
    let state = app.state::<PopupState>();
    let just_closed = state
        .0
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .is_some_and(|hidden| hidden.elapsed() < REOPEN_GUARD);
    if just_closed {
        return Ok(());
    }

    // Di Windows posisi ikon tray sudah dalam piksel fisik, jadi skala 1.0 tidak mengubahnya.
    let icon = PhysicalRect {
        position: icon.position.to_physical::<i32>(1.0),
        size: icon.size.to_physical::<u32>(1.0),
    };
    let (x, y) = (f64::from(icon.position.x), f64::from(icon.position.y));
    let monitor = match app.monitor_from_point(x, y)? {
        Some(monitor) => Some(monitor),
        None => app.primary_monitor()?,
    };
    if let Some(monitor) = monitor {
        let scale = monitor.scale_factor();
        let size = POPUP_SIZE.to_physical::<u32>(scale);
        let margin = LogicalPosition::new(POPUP_MARGIN, 0.0)
            .to_physical::<i32>(scale)
            .x;
        let origin = popup_origin(&icon, monitor.work_area(), size, margin);
        popup.set_size(size)?;
        popup.set_position(origin)?;
    }
    popup.show()?;
    popup.set_focus()
}

/// Posisi popup di dekat ikon tray: di atas ikon bila taskbar di bawah, di bawahnya bila
/// taskbar di atas, dan selalu di dalam work area.
fn popup_origin(
    icon: &PhysicalRect<i32, u32>,
    area: &PhysicalRect<i32, u32>,
    size: PhysicalSize<u32>,
    margin: i32,
) -> PhysicalPosition<i32> {
    let (icon_x, icon_y) = (i64::from(icon.position.x), i64::from(icon.position.y));
    let (area_x, area_y) = (i64::from(area.position.x), i64::from(area.position.y));
    let (width, height) = (i64::from(size.width), i64::from(size.height));
    let margin = i64::from(margin);

    let x = clamp(
        icon_x + i64::from(icon.size.width) / 2 - width / 2,
        area_x + margin,
        area_x + i64::from(area.size.width) - width - margin,
    );
    let y = clamp(
        icon_y - height - margin,
        area_y + margin,
        area_y + i64::from(area.size.height) - height - margin,
    );
    PhysicalPosition::new(
        i32::try_from(x).unwrap_or(i32::MAX),
        i32::try_from(y).unwrap_or(i32::MAX),
    )
}

/// Seperti `i64::clamp`, tetapi tidak panik bila area lebih kecil dari popup.
fn clamp(value: i64, min: i64, max: i64) -> i64 {
    value.min(max).max(min)
}

pub fn show_settings(app: &AppHandle) {
    if let Some(popup) = app.get_webview_window(POPUP) {
        let _ = popup.hide();
    }
    if let Some(window) = app.get_webview_window(SETTINGS) {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri menyuntikkan handle sebagai nilai.
pub fn open_settings(app: AppHandle) {
    show_settings(&app);
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri menyuntikkan handle sebagai nilai.
pub fn app_quit(app: AppHandle) {
    app.exit(0);
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

#[cfg(test)]
mod tests {
    use tauri::{PhysicalPosition, PhysicalRect, PhysicalSize};

    use super::popup_origin;

    fn rect(x: i32, y: i32, width: u32, height: u32) -> PhysicalRect<i32, u32> {
        PhysicalRect {
            position: PhysicalPosition::new(x, y),
            size: PhysicalSize::new(width, height),
        }
    }

    const POPUP: PhysicalSize<u32> = PhysicalSize::new(320, 400);

    #[test]
    fn opens_above_a_bottom_taskbar() {
        let origin = popup_origin(&rect(1800, 1048, 24, 24), &rect(0, 0, 1920, 1040), POPUP, 8);
        assert_eq!(origin, PhysicalPosition::new(1592, 632));
    }

    #[test]
    fn opens_below_a_top_taskbar() {
        let origin = popup_origin(&rect(100, 8, 24, 24), &rect(0, 40, 1920, 1040), POPUP, 8);
        assert_eq!(origin, PhysicalPosition::new(8, 48));
    }

    #[test]
    fn stays_inside_a_work_area_smaller_than_the_popup() {
        let origin = popup_origin(&rect(50, 50, 24, 24), &rect(0, 0, 200, 200), POPUP, 8);
        assert_eq!(origin, PhysicalPosition::new(8, 8));
    }
}
