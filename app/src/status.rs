//! Status Nuenly yang tampil di tray (F-25, US-3): mode aktif, aplikasi yang sedang dibantu,
//! dan jeda. Mode mengikuti aplikasi di latar depan (F-14).

use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use nuenly_core::{ActivePolicy, AppIdentity};
use nuenly_modes::{ModeManager, SuggestSpec};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::config::ConfigStore;
use crate::{foreground, onboarding};

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
    /// Nama aplikasi yang sedang dibantu, untuk "Membantu di …". Pilihan mode diingat untuk
    /// aplikasi ini. `None` bila belum ada, atau bila filter aplikasi tidak mengizinkannya.
    pub app_name: Option<String>,
    /// Akhir jeda dalam milidetik sejak epoch Unix; `None` bila tidak dijeda.
    pub paused_until_ms: Option<u64>,
}

/// Logika status tanpa Tauri agar bisa dites.
#[derive(Debug)]
pub struct Status {
    modes: ModeManager,
    /// Aplikasi terakhir di latar depan selain Nuenly dan shell Windows.
    app: Option<AppIdentity>,
    paused_until: Option<SystemTime>,
}

impl Status {
    pub fn new(modes: ModeManager) -> Self {
        Self {
            modes,
            app: None,
            paused_until: None,
        }
    }

    /// Aplikasi di latar depan berganti ke `app`, dan mode mengikuti pilihan yang diingat untuk
    /// aplikasi itu. `false` bila aplikasinya tetap sama.
    pub fn follow(&mut self, app: AppIdentity) -> bool {
        if self.app.as_ref() == Some(&app) {
            return false;
        }
        self.modes.follow(&app.process);
        self.app = Some(app);
        true
    }

    /// Mengganti mode dan mengingatnya untuk aplikasi yang sedang dibantu. `false` bila `id`
    /// tidak dikenal.
    pub fn set_mode(&mut self, id: &str) -> bool {
        let app = self.helped_app().map(|app| app.process.clone());
        self.modes.choose(id, app.as_ref())
    }

    /// Mode pilihan pengguna per aplikasi, untuk disimpan di setelan.
    pub fn remembered(&self) -> BTreeMap<String, String> {
        self.modes
            .remembered()
            .map(|(app, id)| (app.as_str().to_owned(), id.as_str().to_owned()))
            .collect()
    }

    /// Aplikasi yang sedang dibantu: aplikasi terakhir di latar depan, bila mode aktif boleh
    /// membacanya. Aplikasi yang tidak pernah dibaca (misalnya pengelola kata sandi) tidak
    /// disebut dan tidak diingat modenya.
    fn helped_app(&self) -> Option<&AppIdentity> {
        let policy = self.modes.active().policy();
        self.app
            .as_ref()
            .filter(|app| policy.apps.permits(&app.process))
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

    /// Prompt dan schema saran proaktif mode aktif, atau `None` bila mode ini tidak memberi saran.
    #[cfg_attr(not(debug_assertions), allow(dead_code))] // Lihat `crate::suggest`.
    pub fn suggest_spec(&self) -> Option<SuggestSpec> {
        let pack = self.modes.active();
        pack.suggest()
            .filter(|_| pack.policy().proactive.enabled)
            .cloned()
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
            app_name: self.helped_app().map(|app| app.display_name.clone()),
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

    /// Prompt dan schema saran proaktif mode aktif, atau `None` bila mode ini tidak memberi saran.
    #[cfg_attr(not(debug_assertions), allow(dead_code))] // Lihat `crate::suggest`.
    pub fn suggest_spec(&self) -> Option<SuggestSpec> {
        self.lock().suggest_spec()
    }

    /// Nama executable aplikasi yang sedang dibantu dan ID mode aktif, untuk audit.
    pub fn audit_scope(&self) -> (Option<String>, String) {
        let status = self.lock();
        let app = status
            .helped_app()
            .map(|app| app.process.as_str().to_owned());
        (app, status.modes.active().id().as_str().to_owned())
    }

    /// Aplikasi di latar depan berganti. Selama dijeda Nuenly tidak mengikutinya. Hasilnya
    /// status baru bila ada yang berubah.
    pub fn follow(&self, app: AppIdentity) -> Option<StatusView> {
        let mut status = self.lock();
        let now = SystemTime::now();
        if status.paused_until_ms(now).is_some() || !status.follow(app) {
            return None;
        }
        Some(status.view(now))
    }
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri menyuntikkan state sebagai nilai.
pub fn status_get(status: State<'_, AppStatus>) -> StatusView {
    status.view()
}

/// Pilihan mode dari tray atau Pengaturan, diingat untuk aplikasi yang sedang dibantu.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Argumen command Tauri selalu berupa nilai.
pub fn status_set_mode(
    app: AppHandle,
    status: State<'_, AppStatus>,
    store: State<'_, ConfigStore>,
    id: String,
) -> Result<StatusView, String> {
    let (remembered, view) = status.update(|status| {
        let known = status.set_mode(&id);
        known.then(|| status.remembered())
    });
    let Some(remembered) = remembered else {
        return Err("Mode tidak dikenal.".to_owned());
    };
    // Bila gagal disimpan, pilihan tetap berlaku sampai Nuenly ditutup.
    let _ = store.update(|config| config.app_modes = remembered);
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
        if app.state::<AppStatus>().view().paused_until_ms.is_none() {
            resumed(&app);
        }
    });
    view
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Argumen command Tauri selalu berupa nilai.
pub fn status_resume(app: AppHandle, status: State<'_, AppStatus>) -> StatusView {
    status.update(Status::resume);
    resumed(&app)
}

/// Setelah jeda berakhir, mode kembali mengikuti aplikasi yang ada di depan saat ini.
fn resumed(app: &AppHandle) -> StatusView {
    foreground::sync(app);
    let view = app.state::<AppStatus>().view();
    broadcast(app, &view);
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
    use std::collections::BTreeMap;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use nuenly_core::{AppIdentity, ProcessName};
    use nuenly_modes::ModeManager;

    use super::{AppStatus, Status, tooltip};

    fn status() -> Status {
        Status::new(ModeManager::official().unwrap())
    }

    fn app(process: &str, display_name: &str) -> AppIdentity {
        AppIdentity {
            process: ProcessName::new(process).unwrap(),
            display_name: display_name.to_owned(),
        }
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
        assert_eq!(view["appName"], serde_json::Value::Null);
    }

    #[test]
    fn a_mode_chosen_in_an_app_returns_with_that_app() {
        let mut status = status();
        assert!(status.follow(app("klinik.exe", "Klinik Sintetis")));
        assert_eq!(
            status.view(UNIX_EPOCH).app_name.as_deref(),
            Some("Klinik Sintetis")
        );
        assert!(status.set_mode("medical"));

        assert!(status.follow(app("notepad.exe", "Notepad")));
        assert_eq!(status.view(UNIX_EPOCH).active_mode, "all");
        assert!(status.follow(app("klinik.exe", "Klinik Sintetis")));
        assert_eq!(status.view(UNIX_EPOCH).active_mode, "medical");
        assert_eq!(
            status.remembered(),
            BTreeMap::from([("klinik.exe".to_owned(), "medical".to_owned())])
        );
    }

    #[test]
    fn while_paused_app_changes_are_not_followed() {
        let status = AppStatus::new(ModeManager::official().unwrap());
        status.update(|status| status.pause(SystemTime::now(), Duration::from_mins(15)));
        assert_eq!(status.follow(app("notepad.exe", "Notepad")), None);

        status.update(Status::resume);
        let view = status.follow(app("notepad.exe", "Notepad")).unwrap();
        assert_eq!(view.app_name.as_deref(), Some("Notepad"));
    }

    #[test]
    fn the_same_app_again_changes_nothing() {
        let mut status = status();
        assert!(status.follow(app("notepad.exe", "Notepad")));
        assert!(!status.follow(app("notepad.exe", "Notepad")));
    }

    #[test]
    fn apps_that_are_never_read_are_not_named_or_remembered() {
        let mut status = status();
        status.follow(app("keepass.exe", "KeePass"));
        assert_eq!(status.view(UNIX_EPOCH).app_name, None);

        assert!(status.set_mode("medical"));
        assert_eq!(status.view(UNIX_EPOCH).active_mode, "medical");
        assert_eq!(status.remembered(), BTreeMap::new());
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
        assert!(status.suggest_spec().is_some());

        assert!(status.set_mode("medical"));
        assert!(!status.set_mode("tidak-ada"));
        assert_eq!(status.view(UNIX_EPOCH).active_mode, "medical");
        assert_eq!(status.policy().mode.as_str(), "medical");
        assert!(!status.policy().context_preview);
        assert_ne!(status.ask_prompt(), umum_prompt);
        // Medical belum memberi saran proaktif sampai Fase 3.
        assert_eq!(status.suggest_spec(), None);
        // Tanpa aplikasi yang sedang dibantu, pilihannya tidak diingat untuk siapa pun.
        assert_eq!(status.remembered(), BTreeMap::new());
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
