//! Onboarding (F-26, US-5): selamat datang, privasi, dan model. Nuenly tidak membaca apa pun
//! sebelum pengguna menekan "Saya mengerti" di langkah privasi.

use tauri::{AppHandle, Manager, State};

use crate::config::ConfigStore;
use crate::status::{self, AppStatus};
use crate::toast::{self, Toast, ToastContent};

pub const LABEL: &str = "onboarding";

/// Versi pemberitahuan privasi di langkah 2. Naikkan bila Nuenly mulai membaca hal baru, agar
/// pengguna yang sudah setuju diminta menyetujuinya lagi.
pub const PRIVACY_VERSION: u32 = 1;

/// Apakah pengguna sudah menyetujui pemberitahuan privasi versi ini.
pub fn accepted(app: &AppHandle) -> bool {
    app.try_state::<ConfigStore>()
        .is_some_and(|store| store.get().privacy_accepted >= PRIVACY_VERSION)
}

/// Gerbang untuk setiap jalur yang membaca konteks. Bila persetujuan belum ada, onboarding
/// dibuka dan hasilnya `false`.
pub fn ensure_accepted(app: &AppHandle) -> bool {
    let accepted = accepted(app);
    if !accepted {
        show(app);
    }
    accepted
}

pub fn show(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// "Saya mengerti" di langkah privasi.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri menyuntikkan handle dan state sebagai nilai.
pub fn onboarding_accept(
    app: AppHandle,
    store: State<'_, ConfigStore>,
    status: State<'_, AppStatus>,
) -> Result<(), String> {
    store
        .update(|config| config.privacy_accepted = PRIVACY_VERSION)
        .map_err(|_| "Persetujuan tidak bisa disimpan. Coba lagi.".to_owned())?;
    status::broadcast(&app, &status.view());
    Ok(())
}

/// Menutup onboarding, lalu memberi tahu bahwa Nuenly kini berjalan di tray.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri menyuntikkan handle sebagai nilai.
pub fn onboarding_finish(app: AppHandle) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window(LABEL) {
        window.hide()?;
    }
    if !accepted(&app) {
        return Ok(());
    }
    toast::show(
        &app,
        &Toast {
            id: "onboarding-selesai".to_owned(),
            duration_ms: Some(6000),
            content: ToastContent::Success {
                message: "Nuenly sudah aktif. Ikonnya ada di tray, dekat jam.".to_owned(),
            },
        },
    )
}
