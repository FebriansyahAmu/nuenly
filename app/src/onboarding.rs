//! Onboarding (F-26, US-5): selamat datang, privasi, dan model. Nuenly tidak membaca apa pun
//! sebelum pengguna menekan "Saya mengerti" di langkah privasi.

use tauri::{AppHandle, Manager, State};

use crate::config::ConfigStore;
use crate::ollama::{self, OllamaStatus};
use crate::status::{self, AppStatus};
use crate::toast::{self, Toast, ToastContent};

pub const LABEL: &str = "onboarding";

/// Versi pemberitahuan privasi di langkah 2. Naikkan bila Nuenly mulai membaca hal baru, agar
/// pengguna yang sudah setuju diminta menyetujuinya lagi.
pub const PRIVACY_VERSION: u32 = 1;

const OLLAMA_DOWNLOAD: &str = "https://ollama.com/download/windows";

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

#[tauri::command]
pub async fn onboarding_ollama() -> OllamaStatus {
    tauri::async_runtime::spawn_blocking(ollama::detect)
        .await
        .unwrap_or(OllamaStatus::Missing)
}

/// Membuka halaman unduhan Ollama. Tautannya tetap; frontend tidak bisa memilih alamat lain.
#[tauri::command]
pub fn onboarding_open_download() -> Result<(), String> {
    open_https(OLLAMA_DOWNLOAD).map_err(|_| "Halaman unduhan tidak bisa dibuka.".to_owned())
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

#[cfg(windows)]
fn open_https(url: &str) -> std::io::Result<()> {
    nuenly_platform_windows::shell::open_https(url)
}

#[cfg(not(windows))]
fn open_https(_url: &str) -> std::io::Result<()> {
    Err(std::io::Error::other(
        "membuka tautan belum didukung di OS ini",
    ))
}
