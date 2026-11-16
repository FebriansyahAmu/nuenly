//! Hotkey global Tanya Nuenly (F-03, US-1): bawaan Ctrl+Space, bisa diubah di Pengaturan › Umum.

use std::sync::{Mutex, MutexGuard, PoisonError};
#[cfg(windows)]
use std::time::Duration;

use nuenly_core::Shortcut;
use serde::Serialize;
use tauri::AppHandle;

use crate::config::Config;

/// Batas tunggu melepas pintasan lama sebelum memasang yang baru.
#[cfg(windows)]
const RELEASE_WAIT: Duration = Duration::from_secs(1);

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

/// Pintasan tersimpan, atau Ctrl+Space bila belum diubah atau isinya rusak.
pub fn configured(config: &Config) -> Shortcut {
    config
        .hotkey
        .as_deref()
        .and_then(|text| Shortcut::parse(text).ok())
        .unwrap_or(Shortcut::CTRL_SPACE)
}

/// State terkelola Tauri. Hotkey tetap terdaftar selama state ini hidup.
pub struct HotkeyState(Mutex<Current>);

struct Current {
    shortcut: Shortcut,
    status: HotkeyStatus,
    #[cfg(windows)]
    hotkey: Option<nuenly_platform_windows::hotkey::Hotkey>,
}

impl HotkeyState {
    /// Mendaftarkan `shortcut` saat Nuenly mulai.
    pub fn register(app: &AppHandle, shortcut: Shortcut) -> Self {
        let mut current = Current {
            shortcut,
            status: HotkeyStatus::Failed,
            #[cfg(windows)]
            hotkey: None,
        };
        current.install(app);
        Self(Mutex::new(current))
    }

    pub fn current(&self) -> (Shortcut, HotkeyStatus) {
        let current = self.lock();
        (current.shortcut, current.status)
    }

    /// Mengganti pintasan. Bila yang baru gagal dipasang, pintasan lama dipasang lagi agar Tanya
    /// Nuenly tetap bisa dibuka, dan hasilnya status kegagalan itu.
    ///
    /// Menunggu sampai pintasan lama benar-benar terlepas, jadi jangan dipanggil di thread UI.
    pub fn replace(&self, app: &AppHandle, shortcut: Shortcut) -> Result<(), HotkeyStatus> {
        let mut current = self.lock();
        if current.shortcut == shortcut && current.status == HotkeyStatus::Active {
            return Ok(());
        }
        let previous = current.shortcut;
        current.uninstall();
        current.shortcut = shortcut;
        current.install(app);
        if current.status == HotkeyStatus::Active {
            return Ok(());
        }

        let failure = current.status;
        current.shortcut = previous;
        current.install(app);
        Err(failure)
    }

    fn lock(&self) -> MutexGuard<'_, Current> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Current {
    #[cfg(windows)]
    fn install(&mut self, app: &AppHandle) {
        use nuenly_platform_windows::hotkey::{Hotkey, HotkeyError};

        let app = app.clone();
        let (hotkey, status) =
            match Hotkey::register(self.shortcut, move || crate::ask::on_hotkey(&app)) {
                Ok(hotkey) => (Some(hotkey), HotkeyStatus::Active),
                Err(HotkeyError::Taken) => (None, HotkeyStatus::Taken),
                Err(_) => (None, HotkeyStatus::Failed),
            };
        self.hotkey = hotkey;
        self.status = status;
    }

    #[cfg(windows)]
    fn uninstall(&mut self) {
        if let Some(hotkey) = self.hotkey.take() {
            // Bila belum terlepas dalam batas waktu, pemasangan berikutnya yang memberi tahu.
            let _ = hotkey.release(RELEASE_WAIT);
        }
    }

    #[cfg(not(windows))]
    fn install(&mut self, _app: &AppHandle) {
        self.status = HotkeyStatus::Failed;
    }

    #[cfg(not(windows))]
    fn uninstall(&mut self) {}
}

#[cfg(test)]
mod tests {
    use nuenly_core::Shortcut;

    use super::configured;
    use crate::config::Config;

    #[test]
    fn falls_back_to_ctrl_space() {
        assert_eq!(configured(&Config::default()), Shortcut::CTRL_SPACE);
        let broken = Config {
            hotkey: Some("Shift+K".to_owned()),
            ..Config::default()
        };
        assert_eq!(configured(&broken), Shortcut::CTRL_SPACE);
        let custom = Config {
            hotkey: Some("Alt+Space".to_owned()),
            ..Config::default()
        };
        assert_eq!(configured(&custom).to_string(), "Alt+Space");
    }
}
