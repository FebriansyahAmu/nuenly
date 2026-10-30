//! Status Nuenly yang tampil di tray (F-25, US-3): mode aktif dan jeda.

use std::sync::{Mutex, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::onboarding;

/// Lama jeda dari tombol "Jeda 15 menit".
const PAUSE: Duration = Duration::from_mins(15);

/// Mode yang tersedia. Sementara ditulis di sini; `ModeManager` (F-14) akan memuatnya dari
/// mode pack. Modul lain membaca kebijakan, tidak bercabang berdasarkan `id`.
const MODES: [ModeInfo; 2] = [
    ModeInfo {
        id: "all",
        label: "Umum",
        accent: "#4f46e5",
    },
    ModeInfo {
        id: "medical",
        label: "Medical",
        accent: "#0f766e",
    },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct ModeInfo {
    pub id: &'static str,
    pub label: &'static str,
    /// Warna aksen mode untuk sentuhan kecil di UI (titik logo, segmen aktif).
    pub accent: &'static str,
}

/// Status untuk jendela; dikirim lewat `status_get` dan event `status:changed`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusView {
    pub modes: &'static [ModeInfo],
    pub active_mode: &'static str,
    /// Akhir jeda dalam milidetik sejak epoch Unix; `None` bila tidak dijeda.
    pub paused_until_ms: Option<u64>,
}

/// Logika status tanpa Tauri agar bisa dites.
#[derive(Debug, Default)]
pub struct Status {
    active: usize,
    paused_until: Option<SystemTime>,
}

impl Status {
    /// Mengganti mode; `false` bila `id` tidak dikenal.
    pub fn set_mode(&mut self, id: &str) -> bool {
        match MODES.iter().position(|mode| mode.id == id) {
            Some(index) => {
                self.active = index;
                true
            }
            None => false,
        }
    }

    pub fn pause(&mut self, now: SystemTime, duration: Duration) {
        self.paused_until = now.checked_add(duration);
    }

    pub fn resume(&mut self) {
        self.paused_until = None;
    }

    /// Status pada waktu `now`; jeda yang sudah lewat dianggap selesai.
    pub fn view(&self, now: SystemTime) -> StatusView {
        let paused_until_ms = self
            .paused_until
            .filter(|until| *until > now)
            .and_then(|until| until.duration_since(UNIX_EPOCH).ok())
            .map(|since_epoch| u64::try_from(since_epoch.as_millis()).unwrap_or(u64::MAX));
        StatusView {
            modes: &MODES,
            active_mode: MODES.get(self.active).map_or(MODES[0].id, |mode| mode.id),
            paused_until_ms,
        }
    }
}

/// State terkelola Tauri.
#[derive(Default)]
pub struct AppStatus(Mutex<Status>);

impl AppStatus {
    fn update<T>(&self, change: impl FnOnce(&mut Status) -> T) -> (T, StatusView) {
        let mut status = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let result = change(&mut status);
        (result, status.view(SystemTime::now()))
    }

    pub fn view(&self) -> StatusView {
        self.update(|_| ()).1
    }
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri menyuntikkan state sebagai nilai.
pub fn status_get(status: State<'_, AppStatus>) -> StatusView {
    status.view()
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Argumen command Tauri selalu berupa nilai.
pub fn status_set_mode(
    app: AppHandle,
    status: State<'_, AppStatus>,
    id: String,
) -> Result<StatusView, String> {
    let (known, view) = status.update(|status| status.set_mode(&id));
    if !known {
        return Err("Mode tidak dikenal.".to_owned());
    }
    broadcast(&app, &view);
    Ok(view)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Argumen command Tauri selalu berupa nilai.
pub fn status_pause(app: AppHandle, status: State<'_, AppStatus>) -> StatusView {
    let ((), view) = status.update(|status| status.pause(SystemTime::now(), PAUSE));
    broadcast(&app, &view);

    // Setelah jeda berakhir, siarkan status baru agar tray dan jendela lain ikut berubah.
    std::thread::spawn(move || {
        std::thread::sleep(PAUSE);
        let view = app.state::<AppStatus>().view();
        if view.paused_until_ms.is_none() {
            broadcast(&app, &view);
        }
    });
    view
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Argumen command Tauri selalu berupa nilai.
pub fn status_resume(app: AppHandle, status: State<'_, AppStatus>) -> StatusView {
    let ((), view) = status.update(Status::resume);
    broadcast(&app, &view);
    view
}

/// Mengirim status ke semua jendela dan memperbarui tooltip ikon tray.
pub fn broadcast(app: &AppHandle, view: &StatusView) {
    let _ = app.emit("status:changed", view);
    if let Some(tray) = app.tray_by_id(crate::tray::ID) {
        let _ = tray.set_tooltip(Some(tooltip(view, onboarding::accepted(app))));
    }
}

/// Tooltip ikon tray, misalnya "Nuenly · Umum" atau "Nuenly · dijeda". Sebelum persetujuan
/// privasi di onboarding: "Nuenly · belum aktif".
pub fn tooltip(view: &StatusView, accepted: bool) -> String {
    if !accepted {
        return "Nuenly · belum aktif".to_owned();
    }
    if view.paused_until_ms.is_some() {
        return "Nuenly · dijeda".to_owned();
    }
    let label = view
        .modes
        .iter()
        .find(|mode| mode.id == view.active_mode)
        .map_or("", |mode| mode.label);
    format!("Nuenly · {label}")
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, UNIX_EPOCH};

    use super::{Status, tooltip};

    #[test]
    fn starts_in_the_first_mode_and_not_paused() {
        let view = Status::default().view(UNIX_EPOCH);
        assert_eq!(view.active_mode, "all");
        assert_eq!(view.paused_until_ms, None);
        assert_eq!(tooltip(&view, true), "Nuenly · Umum");
    }

    #[test]
    fn inactive_until_privacy_is_accepted() {
        let mut status = Status::default();
        status.pause(UNIX_EPOCH, Duration::from_mins(15));
        assert_eq!(
            tooltip(&status.view(UNIX_EPOCH), false),
            "Nuenly · belum aktif"
        );
    }

    #[test]
    fn switches_only_to_known_modes() {
        let mut status = Status::default();
        assert!(status.set_mode("medical"));
        assert!(!status.set_mode("tidak-ada"));
        assert_eq!(status.view(UNIX_EPOCH).active_mode, "medical");
    }

    #[test]
    fn pause_ends_on_time_or_on_resume() {
        let now = UNIX_EPOCH + Duration::from_secs(1_000);
        let mut status = Status::default();
        status.pause(now, Duration::from_mins(15));

        let paused = status.view(now);
        assert_eq!(paused.paused_until_ms, Some(1_900_000));
        assert_eq!(tooltip(&paused, true), "Nuenly · dijeda");
        assert_eq!(
            status.view(now + Duration::from_mins(15)).paused_until_ms,
            None
        );

        status.resume();
        assert_eq!(status.view(now).paused_until_ms, None);
    }
}
