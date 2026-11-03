//! Ganti mode otomatis per aplikasi (F-14): mode mengikuti aplikasi di latar depan lewat event
//! Windows, tanpa polling. Yang dibaca hanya identitas aplikasinya, tidak pernah isi jendelanya.

use tauri::AppHandle;
#[cfg(windows)]
use {
    nuenly_core::AppIdentity,
    nuenly_platform_windows::foreground::{self, ForegroundWatch},
    tauri::Manager,
};

#[cfg(windows)]
use crate::onboarding;
#[cfg(windows)]
use crate::status::{self, AppStatus};

/// Pelacak yang tetap hidup selama Nuenly berjalan. `None` bila Windows menolak memasangnya;
/// Nuenly tetap jalan, hanya modenya tidak berganti otomatis.
#[cfg(windows)]
struct ForegroundState {
    _watch: Option<ForegroundWatch>,
}

/// Mulai mengikuti aplikasi di latar depan. Butuh `AppStatus` dan `ConfigStore` sudah terdaftar.
#[cfg(windows)]
pub fn watch(app: &AppHandle) {
    let handle = app.clone();
    let watch = ForegroundWatch::start(move |identity| follow(&handle, identity)).ok();
    app.manage(ForegroundState { _watch: watch });
    sync(app);
}

/// Menyamakan mode dengan aplikasi di latar depan saat ini: tepat sebelum teks terpilih dibaca,
/// setelah persetujuan privasi, dan setelah jeda berakhir.
#[cfg(windows)]
pub fn sync(app: &AppHandle) {
    if let Some(identity) = foreground::current_app() {
        follow(app, identity);
    }
}

#[cfg(windows)]
fn follow(app: &AppHandle, identity: AppIdentity) {
    // Sebelum "Saya mengerti" di onboarding, Nuenly tidak mengikuti aplikasi apa pun.
    if !onboarding::accepted(app) {
        return;
    }
    if let Some(view) = app.state::<AppStatus>().follow(identity) {
        status::broadcast(app, &view);
    }
}

#[cfg(not(windows))]
pub fn watch(_app: &AppHandle) {}

#[cfg(not(windows))]
pub fn sync(_app: &AppHandle) {}
