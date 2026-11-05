//! [`TriggerEngine`]: debounce sampai pengguna diam, pemicu blur, jarak antar-pemeriksaan, dan
//! batas saran per jam.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use nuenly_core::ProactivePolicy;

/// Rentang batas frekuensi.
const HOUR: Duration = Duration::from_hours(1);

/// Sinyal dari platform, tanpa teks. `pid` adalah proses pemilik elemen yang sedang fokus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Input {
    /// Fokus pindah ke elemen lain, termasuk ke aplikasi lain.
    Focus { pid: u32 },
    /// Isi elemen yang sedang fokus berubah. Perubahan di elemen lain (jam, progres, status)
    /// harus disaring platform; yang datang dari proses lain diabaikan di sini.
    Edit { pid: u32 },
    /// Caret atau seleksi di elemen yang sedang fokus bergerak tanpa perubahan isi.
    Move { pid: u32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    /// Pengguna berhenti mengetik selama [`TriggerConfig::idle`].
    Idle,
    /// Pengguna meninggalkan field yang baru diisi.
    Blur,
}

/// Saatnya memeriksa field yang diisi di proses `pid`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Trigger {
    pub pid: u32,
    pub reason: Reason,
    pub at: Instant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TriggerConfig {
    /// Diam selama ini setelah mengetik berarti berhenti mengetik. Nilai bawaan sementara
    /// sampai hasil spike sinyal mengetik masuk (ADR 0012).
    pub idle: Duration,
    /// Jarak minimum antar-pemeriksaan karena idle di field yang sama, agar jeda di tengah
    /// kalimat tidak membuat model bekerja berulang-ulang. Blur tidak menunggu jarak ini.
    pub cooldown: Duration,
    /// Perubahan isi minimum sejak pemeriksaan terakhir; satu-dua ketikan belum cukup.
    pub min_edits: u32,
    /// Saran yang boleh tampil per jam; 0 berarti pemicu mati.
    pub max_per_hour: u32,
}

impl TriggerConfig {
    /// Nilai bawaan dengan batas dari kebijakan mode aktif.
    #[must_use]
    pub fn new(policy: &ProactivePolicy) -> Self {
        Self {
            idle: Duration::from_millis(1500),
            cooldown: Duration::from_secs(15),
            min_edits: 3,
            max_per_hour: if policy.enabled {
                policy.max_per_hour
            } else {
                0
            },
        }
    }
}

/// Field yang sedang diisi.
#[derive(Debug)]
struct Field {
    pid: u32,
    /// Perubahan isi sejak pemeriksaan terakhir.
    edits: u32,
    last_input: Instant,
    last_trigger: Option<Instant>,
}

/// Mesin pemicu untuk satu pengguna. Semua waktu berasal dari pemanggil, jadi perilakunya
/// bisa dites tanpa menunggu.
#[derive(Debug)]
pub struct TriggerEngine {
    config: TriggerConfig,
    field: Option<Field>,
    /// Waktu saran yang tampil dalam satu jam terakhir.
    shown: VecDeque<Instant>,
}

impl TriggerEngine {
    #[must_use]
    pub fn new(config: TriggerConfig) -> Self {
        Self {
            config,
            field: None,
            shown: VecDeque::new(),
        }
    }

    /// Mengganti batas, misalnya saat mode berganti. Saran yang sudah tampil tetap dihitung.
    pub fn set_config(&mut self, config: TriggerConfig) {
        self.config = config;
    }

    /// Mencatat satu sinyal pada waktu `at`. Hasilnya pemicu blur bila sinyal ini menandai
    /// pengguna meninggalkan field yang baru diisi.
    pub fn push(&mut self, input: Input, at: Instant) -> Option<Trigger> {
        match input {
            Input::Focus { pid } => {
                let left = self.leave(at);
                self.field = Some(Field {
                    pid,
                    edits: 0,
                    last_input: at,
                    last_trigger: None,
                });
                left
            }
            Input::Edit { pid } => {
                if let Some(field) = self.focused(pid) {
                    field.edits = field.edits.saturating_add(1);
                    field.last_input = at;
                }
                None
            }
            Input::Move { pid } => {
                if let Some(field) = self.focused(pid) {
                    field.last_input = at;
                }
                None
            }
        }
    }

    /// Memeriksa apakah pengguna sudah berhenti mengetik pada waktu `now`. Dipanggil oleh timer
    /// yang dipasang ke [`wake_at`](Self::wake_at).
    pub fn tick(&mut self, now: Instant) -> Option<Trigger> {
        if self.wake_at().is_none_or(|wake| now < wake) {
            return None;
        }
        let field = self.field.as_mut()?;
        field.edits = 0;
        field.last_trigger = Some(now);
        let pid = field.pid;
        self.fire(pid, Reason::Idle, now)
    }

    /// Kapan [`tick`](Self::tick) perlu dipanggil bila tidak ada sinyal baru; `None` bila
    /// tidak ada yang ditunggu.
    #[must_use]
    pub fn wake_at(&self) -> Option<Instant> {
        if self.config.max_per_hour == 0 {
            return None;
        }
        let field = self
            .field
            .as_ref()
            .filter(|field| field.edits >= self.config.min_edits)?;
        let idle = field.last_input + self.config.idle;
        Some(
            field
                .last_trigger
                .map_or(idle, |last| idle.max(last + self.config.cooldown)),
        )
    }

    /// Saran dari sebuah pemicu benar-benar tampil pada waktu `at`. Hanya saran yang tampil
    /// yang dihitung untuk batas per jam; pemeriksaan tanpa saran tidak.
    pub fn shown(&mut self, at: Instant) {
        self.forget_before(at);
        self.shown.push_back(at);
    }

    /// Melupakan field yang sedang diisi tanpa memicu apa pun, misalnya saat Nuenly dijeda.
    pub fn reset(&mut self) {
        self.field = None;
    }

    /// Field yang sedang fokus, bila milik proses `pid`.
    fn focused(&mut self, pid: u32) -> Option<&mut Field> {
        self.field.as_mut().filter(|field| field.pid == pid)
    }

    /// Fokus meninggalkan field; pemicu blur bila field itu diisi sejak pemeriksaan terakhir.
    fn leave(&mut self, at: Instant) -> Option<Trigger> {
        let field = self.field.take()?;
        if field.edits < self.config.min_edits {
            return None;
        }
        self.fire(field.pid, Reason::Blur, at)
    }

    /// Pemicu, kecuali batas saran per jam sudah tercapai.
    fn fire(&mut self, pid: u32, reason: Reason, at: Instant) -> Option<Trigger> {
        self.forget_before(at);
        let limit = usize::try_from(self.config.max_per_hour).unwrap_or(usize::MAX);
        (self.shown.len() < limit).then_some(Trigger { pid, reason, at })
    }

    fn forget_before(&mut self, now: Instant) {
        while self.shown.front().is_some_and(|&shown| shown + HOUR <= now) {
            self.shown.pop_front();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use nuenly_core::ProactivePolicy;

    use super::{Input, Reason, Trigger, TriggerConfig, TriggerEngine};

    const NOTEPAD: u32 = 7;
    const WORD: u32 = 9;

    fn engine() -> TriggerEngine {
        TriggerEngine::new(TriggerConfig::new(&ProactivePolicy::default()))
    }

    fn ms(start: Instant, millis: u64) -> Instant {
        start + Duration::from_millis(millis)
    }

    /// Fokus ke field lalu mengetik `edits` kali, 100 ms sekali.
    fn type_in(engine: &mut TriggerEngine, start: Instant, pid: u32, edits: u64) -> Instant {
        engine.push(Input::Focus { pid }, start);
        for edit in 1..=edits {
            engine.push(Input::Edit { pid }, ms(start, edit * 100));
        }
        ms(start, edits * 100)
    }

    #[test]
    fn stopping_typing_triggers_once_after_the_idle_pause() {
        let start = Instant::now();
        let mut engine = engine();
        let last = type_in(&mut engine, start, NOTEPAD, 5);

        let wake = last + Duration::from_millis(1500);
        assert_eq!(engine.wake_at(), Some(wake));
        assert_eq!(engine.tick(last + Duration::from_millis(1499)), None);
        assert_eq!(
            engine.tick(wake),
            Some(Trigger {
                pid: NOTEPAD,
                reason: Reason::Idle,
                at: wake,
            })
        );
        assert_eq!(engine.wake_at(), None);
        assert_eq!(engine.tick(wake + Duration::from_secs(60)), None);
    }

    #[test]
    fn caret_movement_in_the_field_postpones_the_check() {
        let start = Instant::now();
        let mut engine = engine();
        let last = type_in(&mut engine, start, NOTEPAD, 5);

        let moved = last + Duration::from_secs(1);
        engine.push(Input::Move { pid: NOTEPAD }, moved);
        assert_eq!(engine.wake_at(), Some(moved + Duration::from_millis(1500)));
    }

    #[test]
    fn a_few_keystrokes_are_not_enough() {
        let start = Instant::now();
        let mut engine = engine();
        let last = type_in(&mut engine, start, NOTEPAD, 2);

        assert_eq!(engine.wake_at(), None);
        assert_eq!(engine.tick(last + Duration::from_secs(10)), None);
        assert_eq!(engine.push(Input::Focus { pid: WORD }, last), None);
    }

    #[test]
    fn leaving_a_filled_field_triggers_right_away() {
        let start = Instant::now();
        let mut engine = engine();
        let last = type_in(&mut engine, start, NOTEPAD, 4);

        let left = last + Duration::from_millis(300);
        assert_eq!(
            engine.push(Input::Focus { pid: WORD }, left),
            Some(Trigger {
                pid: NOTEPAD,
                reason: Reason::Blur,
                at: left,
            })
        );
        // Field baru belum diisi, jadi tidak ada yang ditunggu.
        assert_eq!(engine.wake_at(), None);
    }

    #[test]
    fn signals_from_other_processes_are_ignored() {
        let start = Instant::now();
        let mut engine = engine();
        engine.push(Input::Focus { pid: NOTEPAD }, start);
        // Aplikasi di latar belakang memperbarui angka atau progres.
        for edit in 1..=10 {
            engine.push(Input::Edit { pid: WORD }, ms(start, edit * 100));
            engine.push(Input::Move { pid: WORD }, ms(start, edit * 100));
        }
        assert_eq!(engine.wake_at(), None);
    }

    #[test]
    fn typing_before_any_focus_is_not_counted() {
        let start = Instant::now();
        let mut engine = engine();
        for edit in 1..=10 {
            engine.push(Input::Edit { pid: NOTEPAD }, ms(start, edit * 100));
        }
        assert_eq!(engine.wake_at(), None);
    }

    #[test]
    fn pauses_mid_sentence_do_not_check_again_before_the_cooldown() {
        let start = Instant::now();
        let mut engine = engine();
        let last = type_in(&mut engine, start, NOTEPAD, 5);
        let first = engine.tick(last + Duration::from_millis(1500)).unwrap().at;

        // Lanjut mengetik sebentar lalu diam lagi.
        for edit in 1..=5 {
            engine.push(
                Input::Edit { pid: NOTEPAD },
                first + Duration::from_secs(edit),
            );
        }
        assert_eq!(engine.wake_at(), Some(first + Duration::from_secs(15)));
        assert_eq!(engine.tick(first + Duration::from_secs(10)), None);
        assert_eq!(
            engine
                .tick(first + Duration::from_secs(15))
                .map(|trigger| trigger.reason),
            Some(Reason::Idle)
        );
    }

    #[test]
    fn no_more_triggers_after_the_hourly_limit() {
        let start = Instant::now();
        let mut engine = engine();
        for minute in 0..6 {
            engine.shown(start + Duration::from_mins(minute));
        }

        let after = start + Duration::from_mins(10);
        let last = type_in(&mut engine, after, NOTEPAD, 5);
        assert_eq!(engine.tick(last + Duration::from_millis(1500)), None);
        // Pemeriksaan yang ditahan batas tidak ditunggu lagi.
        assert_eq!(engine.wake_at(), None);

        // Satu jam setelah saran pertama, kuotanya terbuka lagi.
        let later = start + Duration::from_mins(61);
        let last = type_in(&mut engine, later, NOTEPAD, 5);
        assert!(engine.tick(last + Duration::from_millis(1500)).is_some());
    }

    #[test]
    fn a_disabled_policy_never_triggers() {
        let policy = ProactivePolicy {
            enabled: false,
            ..ProactivePolicy::default()
        };
        let start = Instant::now();
        let mut engine = TriggerEngine::new(TriggerConfig::new(&policy));
        let last = type_in(&mut engine, start, NOTEPAD, 10);

        assert_eq!(engine.wake_at(), None);
        assert_eq!(engine.push(Input::Focus { pid: WORD }, last), None);
    }

    #[test]
    fn reset_forgets_the_field_being_filled() {
        let start = Instant::now();
        let mut engine = engine();
        let last = type_in(&mut engine, start, NOTEPAD, 5);

        engine.reset();
        assert_eq!(engine.wake_at(), None);
        assert_eq!(engine.push(Input::Focus { pid: WORD }, last), None);
    }

    #[test]
    fn a_new_limit_applies_to_suggestions_already_shown() {
        let start = Instant::now();
        let mut engine = engine();
        engine.shown(start);
        engine.set_config(TriggerConfig {
            max_per_hour: 1,
            ..TriggerConfig::new(&ProactivePolicy::default())
        });

        let last = type_in(&mut engine, start + Duration::from_mins(1), NOTEPAD, 5);
        assert_eq!(engine.tick(last + Duration::from_millis(1500)), None);
    }
}
