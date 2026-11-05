//! `--watch`: mencatat sinyal mengetik dari `WinEvent` dan UIA, tanpa membaca isi kolom, lalu
//! menandai saat pengguna berhenti mengetik. Setiap sumber juga diumpankan ke mesin pemicu
//! F-09 (`nuenly-trigger`) tersendiri, jadi terlihat sumber mana yang memicu di saat yang tepat.

use std::collections::{BTreeMap, HashMap};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result};
use nuenly_core::ProactivePolicy;
use nuenly_platform_windows::activity::{Activity, Signal, Source, UiaActivity, WinEventActivity};
use nuenly_trigger::{Input, Reason, Trigger, TriggerConfig, TriggerEngine};

use crate::args::Args;

pub fn run(args: &Args) -> Result<()> {
    let (sender, inbox) = mpsc::channel::<Activity>();
    let win_sender = sender.clone();
    let win = WinEventActivity::start(move |activity| {
        let _ = win_sender.send(activity);
    })
    .context("gagal memasang hook WinEvent")?;
    let _uia = UiaActivity::start(move |activity| {
        let _ = sender.send(activity);
    })
    .context("gagal memasang handler UIA")?;

    eprintln!(
        "Mencatat selama {} detik. Ketik di beberapa aplikasi; isi kolom tidak dibaca.",
        args.seconds.as_secs()
    );
    let started = Instant::now();
    let deadline = started + args.seconds;
    let mut tracker = Tracker::new(args.idle);
    let mut triggers = Triggers::new(args.idle);
    let mut names = Names::default();
    loop {
        let now = Instant::now();
        if now >= deadline {
            break;
        }
        let wake = [tracker.wake_at(), triggers.wake_at()]
            .into_iter()
            .flatten()
            .fold(deadline, Instant::min);
        let (ended, fired) = match inbox.recv_timeout(wake.saturating_duration_since(now)) {
            Ok(activity) => (tracker.push(activity), triggers.push(activity)),
            Err(RecvTimeoutError::Timeout) => {
                let now = Instant::now();
                (tracker.tick(now), triggers.tick(now))
            }
            Err(RecvTimeoutError::Disconnected) => break,
        };
        if let Some(burst) = ended {
            print_burst(&burst, started, &mut names, args.idle);
        }
        for (source, trigger) in fired {
            print_trigger(source, &trigger, started, &mut names);
        }
    }
    if let Some(burst) = tracker.finish() {
        print_burst(&burst, started, &mut names, args.idle);
    }
    print_totals(&tracker, &mut names, win.ignored());
    triggers.print_totals();
    Ok(())
}

/// Satu mesin pemicu F-09 per sumber sinyal, dengan nilai bawaan produk kecuali jeda idle.
/// Batas per jam tidak tercapai karena tidak ada saran yang benar-benar tampil.
struct Triggers {
    engines: [(Source, TriggerEngine); 2],
    counts: BTreeMap<(Source, &'static str), u32>,
}

impl Triggers {
    fn new(idle: Duration) -> Self {
        let config = TriggerConfig {
            idle,
            ..TriggerConfig::new(&ProactivePolicy::default())
        };
        Self {
            engines: [Source::WinEvent, Source::Uia]
                .map(|source| (source, TriggerEngine::new(config))),
            counts: BTreeMap::new(),
        }
    }

    fn push(&mut self, activity: Activity) -> Vec<(Source, Trigger)> {
        let input = input(activity);
        let fired = self
            .engines
            .iter_mut()
            .filter(|(source, _)| *source == activity.source)
            .filter_map(|(source, engine)| Some((*source, engine.push(input, activity.at)?)))
            .collect();
        self.count(fired)
    }

    fn tick(&mut self, now: Instant) -> Vec<(Source, Trigger)> {
        let fired = self
            .engines
            .iter_mut()
            .filter_map(|(source, engine)| Some((*source, engine.tick(now)?)))
            .collect();
        self.count(fired)
    }

    fn wake_at(&self) -> Option<Instant> {
        self.engines
            .iter()
            .filter_map(|(_, engine)| engine.wake_at())
            .min()
    }

    fn count(&mut self, fired: Vec<(Source, Trigger)>) -> Vec<(Source, Trigger)> {
        for (source, trigger) in &fired {
            *self
                .counts
                .entry((*source, reason_name(trigger.reason)))
                .or_default() += 1;
        }
        fired
    }

    fn print_totals(&self) {
        let parts: Vec<String> = self
            .counts
            .iter()
            .map(|((source, reason), count)| format!("{} {reason} {count}", source_name(*source)))
            .collect();
        println!(
            "Pemicu F-09: {}",
            if parts.is_empty() {
                "-".to_owned()
            } else {
                parts.join(", ")
            }
        );
    }
}

/// Sinyal platform sebagai masukan mesin pemicu.
fn input(activity: Activity) -> Input {
    let pid = activity.pid;
    match activity.signal {
        Signal::Focus => Input::Focus { pid },
        Signal::ValueChanged | Signal::TextChanged => Input::Edit { pid },
        Signal::SelectionChanged | Signal::CaretMoved => Input::Move { pid },
    }
}

fn print_trigger(source: Source, trigger: &Trigger, started: Instant, names: &mut Names) {
    println!(
        "+{:>6}  {}  → pemicu {} ({})",
        seconds(trigger.at.saturating_duration_since(started)),
        names.of(trigger.pid),
        reason_name(trigger.reason),
        source_name(source)
    );
}

fn reason_name(reason: Reason) -> &'static str {
    match reason {
        Reason::Idle => "idle",
        Reason::Blur => "blur",
    }
}

fn source_name(source: Source) -> &'static str {
    match source {
        Source::WinEvent => "WinEvent",
        Source::Uia => "UIA",
    }
}

/// Kenapa satu rangkaian ketikan dianggap selesai.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    /// Tidak ada sinyal selama jeda idle.
    Idle,
    /// Fokus pindah ke elemen lain.
    Focus,
    /// Sinyal datang dari aplikasi lain.
    OtherApp,
    /// Pencatatan selesai.
    Finished,
}

/// Rangkaian sinyal dari satu aplikasi tanpa jeda sepanjang idle.
#[derive(Debug, PartialEq, Eq)]
pub struct Burst {
    pub pid: u32,
    pub first: Instant,
    pub last: Instant,
    pub end: End,
    pub counts: BTreeMap<(Source, Signal), u64>,
}

impl Burst {
    /// Ada perubahan isi, bukan hanya caret atau seleksi yang bergerak.
    fn edited(&self) -> bool {
        self.counts
            .keys()
            .any(|(_, signal)| matches!(signal, Signal::ValueChanged | Signal::TextChanged))
    }
}

/// Mengelompokkan sinyal menjadi rangkaian ketikan dan menghitung semuanya.
pub struct Tracker {
    idle: Duration,
    burst: Option<Burst>,
    totals: BTreeMap<(u32, Source, Signal), u64>,
}

impl Tracker {
    pub fn new(idle: Duration) -> Self {
        Self {
            idle,
            burst: None,
            totals: BTreeMap::new(),
        }
    }

    /// Mencatat satu sinyal. Hasilnya rangkaian sebelumnya bila sinyal ini menutupnya.
    pub fn push(&mut self, activity: Activity) -> Option<Burst> {
        *self
            .totals
            .entry((activity.pid, activity.source, activity.signal))
            .or_default() += 1;

        if activity.signal == Signal::Focus {
            return self.close(End::Focus);
        }
        let ended = match &self.burst {
            Some(burst) if burst.pid != activity.pid => self.close(End::OtherApp),
            Some(burst) if activity.at.saturating_duration_since(burst.last) >= self.idle => {
                self.close(End::Idle)
            }
            _ => None,
        };
        let burst = self.burst.get_or_insert_with(|| Burst {
            pid: activity.pid,
            first: activity.at,
            last: activity.at,
            end: End::Idle,
            counts: BTreeMap::new(),
        });
        burst.last = burst.last.max(activity.at);
        *burst
            .counts
            .entry((activity.source, activity.signal))
            .or_default() += 1;
        ended
    }

    /// Menutup rangkaian yang sudah diam selama jeda idle pada waktu `now`.
    pub fn tick(&mut self, now: Instant) -> Option<Burst> {
        let quiet = self
            .burst
            .as_ref()
            .is_some_and(|burst| now.saturating_duration_since(burst.last) >= self.idle);
        if quiet { self.close(End::Idle) } else { None }
    }

    /// Kapan rangkaian yang berjalan akan dianggap selesai bila tidak ada sinyal baru.
    pub fn wake_at(&self) -> Option<Instant> {
        self.burst.as_ref().map(|burst| burst.last + self.idle)
    }

    pub fn finish(&mut self) -> Option<Burst> {
        self.close(End::Finished)
    }

    fn close(&mut self, end: End) -> Option<Burst> {
        self.burst.take().map(|burst| Burst { end, ..burst })
    }
}

/// Nama proses dari PID, disimpan agar tidak dibaca ulang.
#[derive(Default)]
struct Names(HashMap<u32, String>);

impl Names {
    fn of(&mut self, pid: u32) -> String {
        self.0
            .entry(pid)
            .or_insert_with(|| {
                nuenly_platform_windows::process_name(pid)
                    .map_or_else(|_| "?".to_owned(), |name| name.as_str().to_owned())
            })
            .clone()
    }
}

fn print_burst(burst: &Burst, started: Instant, names: &mut Names, idle: Duration) {
    // Caret atau seleksi yang bergerak tanpa perubahan isi bukan mengetik.
    if !burst.edited() {
        return;
    }
    let end = match burst.end {
        End::Idle => format!("berhenti mengetik (diam {})", seconds(idle)),
        End::Focus => "fokus pindah".to_owned(),
        End::OtherApp => "pindah aplikasi".to_owned(),
        End::Finished => "pencatatan selesai".to_owned(),
    };
    println!(
        "+{:>6}  {}  {end} setelah {} · {}",
        seconds(burst.last.saturating_duration_since(started)),
        names.of(burst.pid),
        seconds(burst.last.saturating_duration_since(burst.first)),
        counts(burst.counts.iter().map(|(&key, &count)| (key, count)))
    );
}

fn print_totals(tracker: &Tracker, names: &mut Names, ignored: u64) {
    let mut by_app: BTreeMap<String, BTreeMap<(Source, Signal), u64>> = BTreeMap::new();
    for (&(pid, source, signal), &count) in &tracker.totals {
        *by_app
            .entry(names.of(pid))
            .or_default()
            .entry((source, signal))
            .or_default() += count;
    }
    println!("\nRekap sinyal per aplikasi:");
    if by_app.is_empty() {
        println!("  -");
    }
    for (app, counts_by_kind) in by_app {
        println!("  {app}: {}", counts(counts_by_kind.into_iter()));
    }
    println!("LOCATIONCHANGE bukan caret yang dibuang (gerakan mouse dan lainnya): {ignored}");
}

/// Misalnya `WinEvent: nilai 31, caret 33 | UIA: nilai 31`.
fn counts(items: impl Iterator<Item = ((Source, Signal), u64)>) -> String {
    let mut sources: BTreeMap<Source, Vec<String>> = BTreeMap::new();
    for ((source, signal), count) in items {
        sources
            .entry(source)
            .or_default()
            .push(format!("{} {count}", signal_name(signal)));
    }
    sources
        .into_iter()
        .map(|(source, parts)| format!("{}: {}", source_name(source), parts.join(", ")))
        .collect::<Vec<_>>()
        .join(" | ")
}

fn signal_name(signal: Signal) -> &'static str {
    match signal {
        Signal::Focus => "fokus",
        Signal::ValueChanged => "nilai",
        Signal::TextChanged => "teks",
        Signal::SelectionChanged => "seleksi",
        Signal::CaretMoved => "caret",
    }
}

/// Detik dengan satu angka desimal dan koma, misalnya "1,5 dtk".
fn seconds(duration: Duration) -> String {
    format!("{:.1} dtk", duration.as_secs_f64()).replace('.', ",")
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use nuenly_platform_windows::activity::{Activity, Signal, Source};

    use nuenly_trigger::Reason;

    use super::{End, Tracker, Triggers, seconds};

    const IDLE: Duration = Duration::from_millis(1500);

    fn activity(start: Instant, ms: u64, pid: u32, source: Source, signal: Signal) -> Activity {
        Activity {
            source,
            signal,
            pid,
            at: start + Duration::from_millis(ms),
        }
    }

    #[test]
    fn a_pause_longer_than_idle_ends_the_burst() {
        let start = Instant::now();
        let mut tracker = Tracker::new(IDLE);
        for ms in [0, 200, 400] {
            assert_eq!(
                tracker.push(activity(
                    start,
                    ms,
                    7,
                    Source::WinEvent,
                    Signal::ValueChanged
                )),
                None
            );
        }
        assert_eq!(tracker.wake_at(), Some(start + Duration::from_millis(1900)));
        assert_eq!(tracker.tick(start + Duration::from_millis(1800)), None);

        let burst = tracker.tick(start + Duration::from_millis(1900)).unwrap();
        assert_eq!(burst.end, End::Idle);
        assert_eq!(burst.last - burst.first, Duration::from_millis(400));
        assert_eq!(
            burst.counts.get(&(Source::WinEvent, Signal::ValueChanged)),
            Some(&3)
        );
        assert_eq!(tracker.wake_at(), None);
    }

    #[test]
    fn focus_and_other_apps_end_the_burst() {
        let start = Instant::now();
        let mut tracker = Tracker::new(IDLE);
        tracker.push(activity(start, 0, 7, Source::Uia, Signal::TextChanged));
        let ended = tracker
            .push(activity(
                start,
                100,
                9,
                Source::WinEvent,
                Signal::ValueChanged,
            ))
            .unwrap();
        assert_eq!((ended.pid, ended.end), (7, End::OtherApp));

        let ended = tracker
            .push(activity(start, 200, 9, Source::WinEvent, Signal::Focus))
            .unwrap();
        assert_eq!((ended.pid, ended.end), (9, End::Focus));
        assert_eq!(tracker.finish(), None);
    }

    #[test]
    fn caret_movement_alone_is_not_typing() {
        let start = Instant::now();
        let mut tracker = Tracker::new(IDLE);
        tracker.push(activity(start, 0, 7, Source::WinEvent, Signal::CaretMoved));
        assert!(!tracker.finish().unwrap().edited());

        tracker.push(activity(start, 0, 7, Source::WinEvent, Signal::CaretMoved));
        tracker.push(activity(start, 10, 7, Source::Uia, Signal::ValueChanged));
        assert!(tracker.finish().unwrap().edited());
    }

    #[test]
    fn each_source_feeds_its_own_trigger_engine() {
        let start = Instant::now();
        let mut triggers = Triggers::new(IDLE);
        triggers.push(activity(start, 0, 7, Source::Uia, Signal::Focus));
        for ms in [100, 200, 300] {
            triggers.push(activity(start, ms, 7, Source::Uia, Signal::TextChanged));
            triggers.push(activity(start, ms, 7, Source::Uia, Signal::CaretMoved));
        }
        let wake = start + Duration::from_millis(1800);
        assert_eq!(triggers.wake_at(), Some(wake));

        let fired = triggers.tick(wake);
        assert_eq!(fired.len(), 1);
        assert_eq!(fired[0].0, Source::Uia);
        assert_eq!(fired[0].1.reason, Reason::Idle);
        assert_eq!(
            triggers.counts.get(&(Source::Uia, "idle")).copied(),
            Some(1)
        );
        assert_eq!(triggers.counts.get(&(Source::WinEvent, "idle")), None);
    }

    #[test]
    fn seconds_use_a_decimal_comma() {
        assert_eq!(seconds(Duration::from_millis(1500)), "1,5 dtk");
    }
}
