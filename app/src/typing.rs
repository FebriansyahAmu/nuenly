//! Saran saat mengetik (US-2, F-09, ADR 0021).
//!
//! Sinyal UIA tanpa teks (fokus pindah, isi kolom berubah) masuk ke [`TriggerEngine`]. Saat
//! pengguna berhenti mengetik (idle) atau pindah ke kolom lain di aplikasi yang sama (blur), isi
//! kolom itu dibaca sekali lewat `nuenly_core::capture`, lalu diperiksa `suggest::run`. Tidak ada
//! polling: satu timer menunggu [`TriggerEngine::wake_at`].
//!
//! Pemantauan hanya berjalan setelah persetujuan privasi, saat tidak dijeda, saat saran
//! dinyalakan di Pengaturan, dan hanya untuk aplikasi yang modenya memberi saran.

#![cfg_attr(not(windows), allow(dead_code))]

use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Instant;

use nuenly_core::{CaptureKind, CaptureOutcome, ContextError, ContextSnapshot, ProactivePolicy};
use nuenly_trigger::{Input, Reason, Trigger, TriggerConfig, TriggerEngine};
use tauri::{AppHandle, Manager};
use tokio::sync::Notify;
#[cfg(windows)]
use {
    crate::config::ConfigStore,
    crate::context::READ_TIMEOUT,
    crate::status::AppStatus,
    crate::suggest::{self, Checked, SuggestState},
    crate::{foreground, logging, onboarding},
    nuenly_platform_windows::activity::{Activity, Signal, UiaActivity},
};

/// Isi kolom yang lebih pendek dari ini (tanpa spasi di tepi) belum layak diperiksa.
const MIN_CHARS: usize = 20;

/// Logika pemicu tanpa Tauri dan UIA agar bisa dites: mesin pemicu dengan batas per aplikasi,
/// lalu satu pemeriksaan pada satu waktu.
#[derive(Debug)]
pub struct Typing {
    engine: TriggerEngine,
    /// Pemeriksaan sedang berjalan.
    busy: bool,
    /// Pemicu terbaru yang menunggu pemeriksaan yang sedang berjalan selesai.
    pending: Option<Trigger>,
    /// Naik setiap [`reset`](Self::reset); pemeriksaan dari sebelum reset tidak dilanjutkan.
    generation: u64,
}

impl Default for Typing {
    fn default() -> Self {
        Self {
            engine: TriggerEngine::new(config(None)),
            busy: false,
            pending: None,
            generation: 0,
        }
    }
}

/// Nilai bawaan mesin pemicu dengan batas per jam `limit`; `None` mematikan pemicu.
fn config(limit: Option<u32>) -> TriggerConfig {
    TriggerConfig {
        max_per_hour: limit.unwrap_or(0),
        ..TriggerConfig::new(&ProactivePolicy::default())
    }
}

impl Typing {
    /// Fokus pindah ke elemen di proses `pid`. `limit` adalah batas saran per jam untuk aplikasi
    /// itu, atau `None` bila aplikasinya tidak diberi saran.
    ///
    /// Hasilnya pemicu blur untuk kolom yang baru ditinggalkan, hanya bila fokus tetap di
    /// aplikasi yang sama: setelah pindah aplikasi, kolom di jendela yang sudah di belakang tidak
    /// dibaca.
    pub fn focus(&mut self, pid: u32, limit: Option<u32>, at: Instant) -> Option<Trigger> {
        // Kolom yang ditinggalkan dinilai dengan batas aplikasinya sendiri.
        let left = self.engine.push(Input::Focus { pid }, at);
        self.engine.set_config(config(limit));
        left.filter(|trigger| trigger.pid == pid)
    }

    /// Isi kolom fokus di proses `pid` berubah.
    pub fn edit(&mut self, pid: u32, at: Instant) {
        let _ = self.engine.push(Input::Edit { pid }, at);
    }

    /// Caret atau seleksi di kolom fokus bergerak.
    pub fn moved(&mut self, pid: u32, at: Instant) {
        let _ = self.engine.push(Input::Move { pid }, at);
    }

    /// Lihat [`TriggerEngine::tick`].
    pub fn tick(&mut self, now: Instant) -> Option<Trigger> {
        self.engine.tick(now)
    }

    /// Lihat [`TriggerEngine::wake_at`].
    pub fn wake_at(&self) -> Option<Instant> {
        self.engine.wake_at()
    }

    /// Saran dari sebuah pemicu tampil; dihitung untuk batas per jam.
    pub fn shown(&mut self, at: Instant) {
        self.engine.shown(at);
    }

    /// Pemicu baru. `Some` bila pemeriksaannya boleh dimulai sekarang; selagi pemeriksaan lain
    /// berjalan, hanya pemicu terbaru yang disimpan.
    pub fn request(&mut self, trigger: Trigger) -> Option<Trigger> {
        if self.busy {
            self.pending = Some(trigger);
            return None;
        }
        self.busy = true;
        Some(trigger)
    }

    /// Pemeriksaan dari generasi `generation` selesai. Hasilnya pemicu berikutnya yang menunggu,
    /// yang langsung diperiksa oleh pemanggil.
    pub fn finished(&mut self, generation: u64) -> Option<Trigger> {
        if generation != self.generation {
            return None;
        }
        let next = self.pending.take();
        self.busy = next.is_some();
        next
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Melupakan kolom yang sedang diisi dan pemicu yang menunggu, misalnya saat dijeda atau mode
    /// berganti. Pemeriksaan yang sedang berjalan dihentikan pemanggil.
    pub fn reset(&mut self) {
        self.engine.reset();
        self.pending = None;
        self.busy = false;
        self.generation += 1;
    }
}

/// Bacaan untuk pemicu: kolom fokus saat idle, kolom yang baru ditinggalkan saat blur.
pub fn capture_kind(reason: Reason) -> CaptureKind {
    match reason {
        Reason::Idle => CaptureKind::FocusedField,
        Reason::Blur => CaptureKind::LeftField,
    }
}

/// Kenapa pemicu tidak diteruskan ke model. Hanya untuk log.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Skip {
    /// Jendela di depan sudah bukan milik aplikasi pemicu, atau tidak ada.
    Moved,
    /// Filter aplikasi menolak aplikasi itu.
    NotAllowed,
    /// Jendela privat atau situs perbankan.
    Sensitive,
    Password,
    /// Kolom kosong, tidak mengekspos teks, atau sudah dibaca.
    Empty,
    /// Terlalu pendek untuk diperiksa.
    Short,
    /// Gagal dibaca atau terlalu lama.
    Unreadable,
}

/// Teks kolom yang layak diperiksa dari hasil `capture`, atau alasan dilewati.
pub fn checkable(result: Result<CaptureOutcome, ContextError>) -> Result<ContextSnapshot, Skip> {
    match result {
        Ok(CaptureOutcome::Captured(snapshot)) => {
            let long_enough = snapshot.text.expose().trim().chars().nth(MIN_CHARS - 1);
            long_enough.map(|_| snapshot).ok_or(Skip::Short)
        }
        Ok(CaptureOutcome::NoWindow) | Err(ContextError::ForegroundChanged) => Err(Skip::Moved),
        Ok(CaptureOutcome::NotAllowed(_)) => Err(Skip::NotAllowed),
        Ok(CaptureOutcome::Sensitive(_)) => Err(Skip::Sensitive),
        Ok(CaptureOutcome::ProtectedField) => Err(Skip::Password),
        Ok(CaptureOutcome::Empty) => Err(Skip::Empty),
        Err(_) => Err(Skip::Unreadable),
    }
}

/// State terkelola Tauri.
#[derive(Default)]
pub struct TypingState {
    typing: Mutex<Typing>,
    /// Membangunkan timer saat waktu tunggu mesin pemicu mungkin berubah.
    wake: Notify,
    /// Pemeriksaan yang sedang berjalan, agar bisa dihentikan saat dijeda atau mode berganti.
    /// Urutan kunci: `runner` lalu `typing`.
    runner: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
    /// Pemantauan sinyal; `None` bila tidak berjalan.
    #[cfg(windows)]
    watch: Mutex<Option<UiaActivity>>,
}

impl TypingState {
    fn typing(&self) -> MutexGuard<'_, Typing> {
        lock(&self.typing)
    }

    /// Menghentikan pemeriksaan yang sedang berjalan dan melupakan kolom yang sedang diisi.
    fn stop_checks(&self) {
        let mut runner = lock(&self.runner);
        if let Some(task) = runner.take() {
            task.abort();
        }
        self.typing().reset();
        self.wake.notify_one();
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Melupakan kolom yang sedang diisi dan menghentikan pemeriksaan yang berjalan, misalnya saat
/// mode berganti.
pub fn reset(app: &AppHandle) {
    if let Some(state) = app.try_state::<TypingState>() {
        state.stop_checks();
    }
}

/// Memulai timer pemicu, lalu pemantauan bila sudah boleh. Dipanggil sekali di `setup`, setelah
/// state yang dibaca [`refresh`] terdaftar.
#[cfg(windows)]
pub fn start(app: &AppHandle) {
    tauri::async_runtime::spawn(timer(app.clone()));
    refresh(app);
}

/// Menyalakan atau mematikan pemantauan sesuai persetujuan privasi, jeda, setelan, dan mode
/// pack. Memasang dan melepas handler UIA bisa menunggu aplikasi yang lambat, jadi dikerjakan di
/// thread lain.
#[cfg(windows)]
pub fn refresh(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || apply(&app));
}

#[cfg(not(windows))]
pub fn start(_app: &AppHandle) {}

#[cfg(not(windows))]
pub fn refresh(_app: &AppHandle) {}

/// Pemantauan boleh berjalan.
#[cfg(windows)]
fn wanted(app: &AppHandle) -> bool {
    let (Some(status), Some(config)) =
        (app.try_state::<AppStatus>(), app.try_state::<ConfigStore>())
    else {
        return false;
    };
    onboarding::accepted(app)
        && !status.paused()
        && config.get().suggest.enabled
        && status.any_suggests()
}

#[cfg(windows)]
fn apply(app: &AppHandle) {
    let Some(state) = app.try_state::<TypingState>() else {
        return;
    };
    // Kunci dipegang selama memasang atau melepas, agar dua `refresh` tidak membuat dua klien UIA
    // sekaligus.
    let mut watch = lock(&state.watch);
    if !wanted(app) {
        if watch.take().is_some() {
            tracing::info!("saran saat mengetik berhenti");
        }
        drop(watch);
        state.stop_checks();
        return;
    }
    if watch.is_some() {
        return;
    }
    let handle = app.clone();
    match UiaActivity::start(move |activity| on_activity(&handle, activity)) {
        Ok(started) => {
            *watch = Some(started);
            tracing::info!("saran saat mengetik berjalan");
        }
        Err(err) => tracing::warn!(error = %err, "saran saat mengetik tidak bisa dimulai"),
    }
}

/// Sinyal dari thread UIA aktivitas. Hasilnya untuk sinyal fokus: apakah perubahan isi elemen
/// itu dipantau. Tidak pernah memanggil UIA atau menunggu lama.
#[cfg(windows)]
fn on_activity(app: &AppHandle, activity: Activity) -> bool {
    let state = app.state::<TypingState>();
    let Activity {
        signal, pid, at, ..
    } = activity;
    let watch = match signal {
        Signal::Focus => {
            let limit = limit_for(app, pid);
            let left = state.typing().focus(pid, limit, at);
            if let Some(trigger) = left {
                request(app, &state, trigger);
            }
            limit.is_some()
        }
        Signal::TextChanged | Signal::ValueChanged => {
            state.typing().edit(pid, at);
            true
        }
        Signal::SelectionChanged | Signal::CaretMoved => {
            state.typing().moved(pid, at);
            true
        }
    };
    state.wake.notify_one();
    watch
}

/// Batas saran per jam untuk aplikasi pemilik proses `pid`: pilihan pengguna, paling tinggi batas
/// mode yang dipakai aplikasi itu. `None` bila aplikasi itu tidak diberi saran.
#[cfg(windows)]
fn limit_for(app: &AppHandle, pid: u32) -> Option<u32> {
    let process = nuenly_platform_windows::process_name(pid).ok()?;
    let pack_limit = app.state::<AppStatus>().suggest_limit_for(&process)?;
    let limit = app.state::<ConfigStore>().get().suggest.limit(pack_limit);
    (limit > 0).then_some(limit)
}

/// Memulai pemeriksaan untuk `trigger`, atau menyimpannya bila pemeriksaan lain sedang berjalan.
#[cfg(windows)]
fn request(app: &AppHandle, state: &TypingState, trigger: Trigger) {
    let mut runner = lock(&state.runner);
    let (trigger, generation) = {
        let mut typing = state.typing();
        let Some(trigger) = typing.request(trigger) else {
            return;
        };
        (trigger, typing.generation())
    };
    *runner = Some(tauri::async_runtime::spawn(run_checks(
        app.clone(),
        trigger,
        generation,
    )));
}

/// Memeriksa pemicu satu per satu sampai tidak ada yang menunggu.
#[cfg(windows)]
async fn run_checks(app: AppHandle, first: Trigger, generation: u64) {
    let mut next = Some(first);
    while let Some(trigger) = next {
        check(&app, trigger).await;
        next = app.state::<TypingState>().typing().finished(generation);
    }
}

/// Timer tunggal mesin pemicu: tidur sampai `wake_at`, atau sampai sinyal baru mengubahnya.
#[cfg(windows)]
async fn timer(app: AppHandle) {
    let state = app.state::<TypingState>();
    loop {
        let wake = state.typing().wake_at();
        let Some(wake) = wake else {
            state.wake.notified().await;
            continue;
        };
        tokio::select! {
            () = tokio::time::sleep_until(tokio::time::Instant::from_std(wake)) => {
                let trigger = state.typing().tick(Instant::now());
                if let Some(trigger) = trigger {
                    request(&app, &state, trigger);
                }
            }
            () = state.wake.notified() => {}
        }
    }
}

/// Membaca kolom untuk `trigger` lalu memeriksanya. Log tanpa teks: alasan pemicu, lama baca,
/// dan hasilnya; rincian pemeriksaan model dicatat `suggest::run`.
#[cfg(windows)]
async fn check(app: &AppHandle, trigger: Trigger) {
    let started = Instant::now();
    // Mode dipastikan milik aplikasi di depan sebelum kebijakannya dipakai.
    foreground::sync(app);
    let Some((policy, spec)) = app.state::<AppStatus>().suggest_scope() else {
        return;
    };
    let reader = lock(&app.state::<TypingState>().watch)
        .as_ref()
        .map(|watch| watch.reader(trigger.pid, READ_TIMEOUT));
    let Some(reader) = reader else {
        return;
    };
    let timeout = policy.proactive.toast_timeout;
    let kind = capture_kind(trigger.reason);
    let read =
        tauri::async_runtime::spawn_blocking(move || nuenly_core::capture(&reader, &policy, kind))
            .await;
    let read_ms = logging::millis(started.elapsed());
    let field = match read {
        Ok(result) => checkable(result),
        Err(_) => Err(Skip::Unreadable),
    };
    let context = match field {
        Ok(context) => context,
        Err(skip) => {
            tracing::info!(reason = ?trigger.reason, read_ms, skip = ?skip, "pemicu saran dilewati");
            return;
        }
    };

    let id = app.state::<SuggestState>().next_id();
    let checked = suggest::run(app, id, &spec, timeout, &context).await;
    if checked == Ok(Checked::Shown) {
        app.state::<TypingState>().typing().shown(Instant::now());
    }
    let held = match checked {
        Ok(Checked::Held(verdict)) => Some(verdict),
        _ => None,
    };
    tracing::info!(
        reason = ?trigger.reason,
        read_ms,
        check_ms = logging::millis(started.elapsed()),
        shown = checked == Ok(Checked::Shown),
        held = held.map(tracing::field::debug),
        "pemicu saran"
    );
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use nuenly_core::{
        ActivePolicy, AppFilter, AppIdentity, CaptureKind, ContextError, ControlKind,
        FakeContextSource, FieldInfo, ModeId, ProcessName, capture,
    };
    use nuenly_trigger::{Reason, Trigger};

    use super::{Skip, Typing, capture_kind, checkable};

    const NOTEPAD: u32 = 7;
    const WORD: u32 = 9;
    const LIMIT: Option<u32> = Some(6);
    const LONG: &str = "Rapat minggu depan digeser ke hari Kamis jam sembilan.";

    /// Fokus ke kolom lalu mengetik `edits` kali, 100 ms sekali. Hasilnya waktu ketikan terakhir.
    fn type_in(typing: &mut Typing, start: Instant, pid: u32, edits: u64) -> Instant {
        let _ = typing.focus(pid, LIMIT, start);
        for edit in 1..=edits {
            typing.edit(pid, start + Duration::from_millis(edit * 100));
        }
        start + Duration::from_millis(edits * 100)
    }

    fn trigger(pid: u32, reason: Reason, at: Instant) -> Trigger {
        Trigger { pid, reason, at }
    }

    #[test]
    fn stopping_typing_checks_the_focused_field() {
        let start = Instant::now();
        let mut typing = Typing::default();
        let last = type_in(&mut typing, start, NOTEPAD, 5);

        let wake = last + Duration::from_millis(1500);
        assert_eq!(typing.wake_at(), Some(wake));
        let idle = typing.tick(wake).unwrap();
        assert_eq!(idle, trigger(NOTEPAD, Reason::Idle, wake));
        assert_eq!(capture_kind(idle.reason), CaptureKind::FocusedField);
    }

    #[test]
    fn moving_to_another_field_in_the_same_app_checks_the_field_left() {
        let start = Instant::now();
        let mut typing = Typing::default();
        let last = type_in(&mut typing, start, NOTEPAD, 5);

        let blur = typing.focus(NOTEPAD, LIMIT, last).unwrap();
        assert_eq!(blur, trigger(NOTEPAD, Reason::Blur, last));
        assert_eq!(capture_kind(blur.reason), CaptureKind::LeftField);
    }

    #[test]
    fn switching_apps_skips_the_field_left_behind() {
        let start = Instant::now();
        let mut typing = Typing::default();
        let last = type_in(&mut typing, start, NOTEPAD, 5);

        assert_eq!(typing.focus(WORD, LIMIT, last), None);
        assert_eq!(typing.wake_at(), None);
    }

    #[test]
    fn apps_without_suggestions_never_trigger() {
        let start = Instant::now();
        let mut typing = Typing::default();
        let _ = typing.focus(NOTEPAD, None, start);
        for edit in 1..=10 {
            typing.edit(NOTEPAD, start + Duration::from_millis(edit * 100));
        }
        assert_eq!(typing.wake_at(), None);
        assert_eq!(typing.tick(start + Duration::from_secs(10)), None);
        assert_eq!(typing.focus(NOTEPAD, LIMIT, start), None);
    }

    #[test]
    fn the_hourly_limit_counts_suggestions_shown() {
        let start = Instant::now();
        let mut typing = Typing::default();
        let _ = typing.focus(NOTEPAD, Some(1), start);
        typing.shown(start);

        let last = start + Duration::from_secs(1);
        for edit in 1..=5 {
            typing.edit(NOTEPAD, last + Duration::from_millis(edit * 100));
        }
        assert_eq!(typing.tick(last + Duration::from_secs(5)), None);
        assert_eq!(typing.focus(NOTEPAD, Some(1), last), None);
    }

    #[test]
    fn one_check_at_a_time_keeps_only_the_latest_trigger() {
        let now = Instant::now();
        let mut typing = Typing::default();
        let first = trigger(NOTEPAD, Reason::Idle, now);
        let second = trigger(NOTEPAD, Reason::Blur, now);
        let third = trigger(WORD, Reason::Idle, now);

        assert_eq!(typing.request(first), Some(first));
        assert_eq!(typing.request(second), None);
        assert_eq!(typing.request(third), None);
        let generation = typing.generation();
        assert_eq!(typing.finished(generation), Some(third));
        assert_eq!(typing.finished(generation), None);
        assert_eq!(typing.request(first), Some(first));
    }

    #[test]
    fn pausing_forgets_the_field_and_waiting_triggers() {
        let start = Instant::now();
        let mut typing = Typing::default();
        let last = type_in(&mut typing, start, NOTEPAD, 5);
        let old = typing.generation();
        let now = trigger(NOTEPAD, Reason::Idle, last);
        assert!(typing.request(now).is_some());
        assert!(typing.request(now).is_none());

        typing.reset();
        assert_eq!(typing.wake_at(), None);
        // Pemeriksaan dari sebelum jeda tidak mengambil pemicu baru.
        assert_eq!(typing.finished(old), None);
        assert_eq!(typing.request(now), Some(now));
    }

    fn policy() -> ActivePolicy {
        ActivePolicy::new(ModeId::new("all").unwrap())
    }

    fn notepad() -> AppIdentity {
        AppIdentity {
            process: ProcessName::new("notepad.exe").unwrap(),
            display_name: "Notepad".to_owned(),
        }
    }

    fn field(is_password: bool) -> FieldInfo {
        FieldInfo {
            label: None,
            control: ControlKind::Edit,
            is_password,
            bounds: None,
        }
    }

    #[test]
    fn long_enough_fields_are_checked() {
        let source = FakeContextSource::new()
            .with_foreground(notepad(), "catatan.txt")
            .with_focused_field(field(false), LONG)
            .with_left_field(field(false), "Kolom sebelumnya berisi kalimat sintetis.");

        let idle = checkable(capture(&source, &policy(), capture_kind(Reason::Idle))).unwrap();
        assert_eq!(idle.text.expose(), LONG);
        let blur = checkable(capture(&source, &policy(), capture_kind(Reason::Blur))).unwrap();
        assert_eq!(blur.kind, CaptureKind::LeftField);
        assert_eq!(
            blur.text.expose(),
            "Kolom sebelumnya berisi kalimat sintetis."
        );
    }

    #[test]
    fn short_empty_and_password_fields_are_skipped() {
        let read = |source: &FakeContextSource| {
            checkable(capture(source, &policy(), CaptureKind::FocusedField)).map(|_| ())
        };
        let base = || FakeContextSource::new().with_foreground(notepad(), "catatan.txt");

        assert_eq!(
            read(&base().with_focused_field(field(false), "  oke siap   ")),
            Err(Skip::Short)
        );
        assert_eq!(read(&base()), Err(Skip::Empty));
        assert_eq!(
            read(&base().with_focused_field(field(true), LONG)),
            Err(Skip::Password)
        );
        assert_eq!(read(&FakeContextSource::new()), Err(Skip::Moved));
    }

    #[test]
    fn blocked_apps_are_never_read() {
        let mut policy = policy();
        policy.apps = AppFilter {
            allow: None,
            block: vec![ProcessName::new("notepad.exe").unwrap()],
        };
        let source = FakeContextSource::new()
            .with_foreground(notepad(), "catatan.txt")
            .with_focused_field(field(false), LONG);

        let result = checkable(capture(&source, &policy, CaptureKind::FocusedField));
        assert_eq!(result.map(|_| ()), Err(Skip::NotAllowed));
        assert_eq!(source.read_count(), 0);
    }

    #[test]
    fn read_failures_are_skipped() {
        assert_eq!(
            checkable(Err(ContextError::ForegroundChanged)).map(|_| ()),
            Err(Skip::Moved)
        );
        assert_eq!(
            checkable(Err(ContextError::Timeout)).map(|_| ()),
            Err(Skip::Unreadable)
        );
    }
}
