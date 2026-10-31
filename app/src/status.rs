//! Status Nuenly yang tampil di tray (F-25, US-3): mode aktif dan jeda.

use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use nuenly_core::ActivePolicy;
use nuenly_modes::ModeManager;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::onboarding;

/// Lama jeda dari tombol "Jeda 15 menit".
const PAUSE: Duration = Duration::from_mins(15);

/// Mode untuk jendela, dari mode pack. Jendela memakai `id` hanya untuk memilih mode, tidak
/// untuk bercabang.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ModeView {
    pub id: String,
    pub label: String,
    /// Warna aksen mode untuk sentuhan kecil di UI (titik logo, segmen aktif).
    pub accent: String,
}

/// Status untuk jendela; dikirim lewat `status_get` dan event `status:changed`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusView {
    pub modes: Vec<ModeView>,
    pub active_mode: String,
    /// Akhir jeda dalam milidetik sejak epoch Unix; `None` bila tidak dijeda.
    pub paused_until_ms: Option<u64>,
}

/// Logika status tanpa Tauri agar bisa dites.
#[derive(Debug)]
pub struct Status {
    modes: ModeManager,
    paused_until: Option<SystemTime>,
}

impl Status {
    pub fn new(modes: ModeManager) -> Self {
        Self {
            modes,
            paused_until: None,
        }
    }

    /// Mengganti mode; `false` bila `id` tidak dikenal.
    pub fn set_mode(&mut self, id: &str) -> bool {
        self.modes.activate(id)
    }

    pub fn pause(&mut self, now: SystemTime, duration: Duration) {
        self.paused_until = now.checked_add(duration);
    }

    pub fn resume(&mut self) {
        self.paused_until = None;
    }

    /// Kebijakan mode aktif, dari mode pack.
    pub fn policy(&self) -> ActivePolicy {
        self.modes.active().policy().clone()
    }

    /// Prompt sistem Tanya Nuenly mode aktif.
    pub fn ask_prompt(&self) -> &'static str {
        self.modes.active().ask_prompt()
    }

    /// Akhir jeda dalam milidetik sejak epoch Unix; jeda yang sudah lewat dianggap selesai.
    fn paused_until_ms(&self, now: SystemTime) -> Option<u64> {
        self.paused_until
            .filter(|until| *until > now)
            .and_then(|until| until.duration_since(UNIX_EPOCH).ok())
            .map(|since_epoch| u64::try_from(since_epoch.as_millis()).unwrap_or(u64::MAX))
    }

    /// Status pada waktu `now`.
    pub fn view(&self, now: SystemTime) -> StatusView {
        let modes = self
            .modes
            .packs()
            .iter()
            .map(|pack| ModeView {
                id: pack.id().as_str().to_owned(),
                label: pack.label().to_owned(),
                accent: pack.accent().to_owned(),
            })
            .collect();
        StatusView {
            modes,
            active_mode: self.modes.active().id().as_str().to_owned(),
            paused_until_ms: self.paused_until_ms(now),
        }
    }
}

/// State terkelola Tauri.
pub struct AppStatus(Mutex<Status>);

impl AppStatus {
    pub fn new(modes: ModeManager) -> Self {
        Self(Mutex::new(Status::new(modes)))
    }

    fn lock(&self) -> MutexGuard<'_, Status> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn update<T>(&self, change: impl FnOnce(&mut Status) -> T) -> (T, StatusView) {
        let mut status = self.lock();
        let result = change(&mut status);
        (result, status.view(SystemTime::now()))
    }

    pub fn view(&self) -> StatusView {
        self.update(|_| ()).1
    }

    /// Kebijakan baca saat ini, atau `None` selama dijeda: Nuenly tidak membaca apa pun.
    pub fn reading_policy(&self) -> Option<ActivePolicy> {
        let status = self.lock();
        if status.paused_until_ms(SystemTime::now()).is_some() {
            return None;
        }
        Some(status.policy())
    }

    /// Prompt sistem Tanya Nuenly mode aktif.
    pub fn ask_prompt(&self) -> &'static str {
        self.lock().ask_prompt()
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
        .map_or("", |mode| mode.label.as_str());
    format!("Nuenly · {label}")
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, UNIX_EPOCH};

    use nuenly_modes::ModeManager;

    use super::{Status, tooltip};

    fn status() -> Status {
        Status::new(ModeManager::official().unwrap())
    }

    #[test]
    fn starts_in_the_first_mode_and_not_paused() {
        let view = status().view(UNIX_EPOCH);
        assert_eq!(view.active_mode, "all");
        assert_eq!(view.paused_until_ms, None);
        assert_eq!(tooltip(&view, true), "Nuenly · Umum");
    }

    #[test]
    fn windows_receive_the_modes_from_the_mode_packs() {
        let view = serde_json::to_value(status().view(UNIX_EPOCH)).unwrap();
        assert_eq!(
            view["modes"],
            serde_json::json!([
                { "id": "all", "label": "Umum", "accent": "#4f46e5" },
                { "id": "medical", "label": "Medical", "accent": "#0f766e" },
            ])
        );
        assert_eq!(view["activeMode"], "all");
    }

    #[test]
    fn inactive_until_privacy_is_accepted() {
        let mut status = status();
        status.pause(UNIX_EPOCH, Duration::from_mins(15));
        assert_eq!(
            tooltip(&status.view(UNIX_EPOCH), false),
            "Nuenly · belum aktif"
        );
    }

    #[test]
    fn policy_and_prompt_follow_the_active_mode() {
        let mut status = status();
        let umum_prompt = status.ask_prompt();
        assert!(status.policy().context_preview);

        assert!(status.set_mode("medical"));
        assert!(!status.set_mode("tidak-ada"));
        assert_eq!(status.view(UNIX_EPOCH).active_mode, "medical");
        assert_eq!(status.policy().mode.as_str(), "medical");
        assert!(!status.policy().context_preview);
        assert_ne!(status.ask_prompt(), umum_prompt);
    }

    #[test]
    fn pause_ends_on_time_or_on_resume() {
        let now = UNIX_EPOCH + Duration::from_secs(1_000);
        let mut status = status();
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
