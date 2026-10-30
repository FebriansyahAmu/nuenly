//! Hotkey global Tanya Nuenly (F-03, US-1): Ctrl+Space.

use serde::Serialize;
use tauri::AppHandle;

/// Bentuknya sama dengan `HotkeyStatus` di `app/ui/src/windows/settings/backend.ts`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum HotkeyStatus {
    Active,
    /// Kombinasinya sudah dipakai aplikasi lain.
    Taken,
    /// Gagal dipasang karena alasan lain, atau OS ini belum didukung.
    Failed,
}

/// State terkelola Tauri. Hotkey tetap terdaftar selama state ini hidup.
pub struct HotkeyState {
    pub status: HotkeyStatus,
    #[cfg(windows)]
    _hotkey: Option<nuenly_platform_windows::hotkey::Hotkey>,
}

/// Mendaftarkan Ctrl+Space untuk Tanya Nuenly.
#[cfg(windows)]
pub fn register(app: &AppHandle) -> HotkeyState {
    use nuenly_platform_windows::hotkey::{Hotkey, HotkeyError, Shortcut};

    let app = app.clone();
    match Hotkey::register(Shortcut::CTRL_SPACE, move || crate::ask::on_hotkey(&app)) {
        Ok(hotkey) => HotkeyState {
            status: HotkeyStatus::Active,
            _hotkey: Some(hotkey),
        },
        Err(err) => HotkeyState {
            status: if matches!(err, HotkeyError::Taken) {
                HotkeyStatus::Taken
            } else {
                HotkeyStatus::Failed
            },
            _hotkey: None,
        },
    }
}

#[cfg(not(windows))]
pub fn register(_app: &AppHandle) -> HotkeyState {
    HotkeyState {
        status: HotkeyStatus::Failed,
    }
}
