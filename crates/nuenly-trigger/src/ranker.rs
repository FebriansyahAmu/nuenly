//! [`Ranker`] (F-24): saran mana yang layak tampil, dari tingkat keyakinan model, kemiripan dengan
//! saran yang baru tampil, dan tanggapan pengguna di aplikasi yang sama.

use std::collections::{HashMap, VecDeque};
use std::hash::{BuildHasher, Hasher, RandomState};
use std::time::{Duration, Instant};

use nuenly_core::{Confidence, ProcessName, SensitiveText, Suggestion};

/// Keyakinan minimum bawaan: saran yang diragukan model sendiri tidak ditampilkan.
const BASE_THRESHOLD: Confidence = Confidence::Medium;
/// Keyakinan minimum setelah pengguna sering menolak saran di aplikasi itu.
const RAISED_THRESHOLD: Confidence = Confidence::High;
/// Banyaknya tanggapan terakhir per aplikasi yang dihitung.
const WINDOW: usize = 10;
/// Tekanan (tolak dikurangi terima) di [`WINDOW`] yang menaikkan ambang.
const RAISE_AT: i32 = 3;
/// Saran yang sama tidak tampil lagi di aplikasi yang sama selama ini.
const REPEAT_AFTER: Duration = Duration::from_mins(30);
/// Batas saran yang diingat per aplikasi untuk mencegah saran berulang.
const MAX_REMEMBERED: usize = 32;

/// Tanggapan pengguna atas saran yang tampil.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Feedback {
    /// Poin disalin.
    Copied,
    /// Rinciannya dilihat.
    Viewed,
    /// Hilang sendiri tanpa disentuh; netral.
    Expired,
    /// "Abaikan", atau ditutup tanpa melihat rinciannya.
    Dismissed,
    /// 👎.
    Disliked,
}

impl Feedback {
    /// Sumbangan ke tekanan: positif berarti menolak, negatif berarti menerima.
    fn pressure(self) -> i32 {
        match self {
            Self::Copied => -2,
            Self::Viewed => -1,
            Self::Expired => 0,
            Self::Dismissed => 1,
            Self::Disliked => 2,
        }
    }
}

/// Keputusan untuk satu saran.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Show,
    /// Keyakinan model di bawah ambang aplikasi ini.
    BelowThreshold,
    /// Saran yang sama sudah tampil di aplikasi ini dalam [`REPEAT_AFTER`] terakhir.
    Repeated,
}

#[derive(Debug, Default)]
struct AppHistory {
    /// Tanggapan terakhir, paling lama di depan.
    feedback: VecDeque<Feedback>,
    /// Sidik saran yang tampil beserta waktunya.
    shown: VecDeque<(u64, Instant)>,
}

/// Ranker untuk satu pengguna. Tanggapan dihitung per aplikasi (`None` untuk aplikasi yang tidak
/// diketahui), dan semua waktu berasal dari pemanggil.
#[derive(Debug, Default)]
pub struct Ranker {
    apps: HashMap<Option<ProcessName>, AppHistory>,
    /// Sidik saran memakai SipHash berkunci acak per proses, jadi teks saran tidak disimpan.
    hasher: RandomState,
}

impl Ranker {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Keyakinan minimum saran di `app` saat ini. Naik ke [`Confidence::High`] bila tanggapan
    /// terakhir di aplikasi itu lebih banyak menolak, dan turun lagi setelah saran diterima.
    #[must_use]
    pub fn threshold(&self, app: Option<&ProcessName>) -> Confidence {
        let pressure: i32 = self.history(app).map_or(0, |history| {
            history.feedback.iter().map(|f| f.pressure()).sum()
        });
        if pressure >= RAISE_AT {
            RAISED_THRESHOLD
        } else {
            BASE_THRESHOLD
        }
    }

    #[must_use]
    pub fn judge(
        &self,
        app: Option<&ProcessName>,
        suggestion: &Suggestion,
        now: Instant,
    ) -> Verdict {
        if suggestion.confidence() < self.threshold(app) {
            return Verdict::BelowThreshold;
        }
        let print = self.fingerprint(suggestion);
        let repeated = self.history(app).is_some_and(|history| {
            history.shown.iter().any(|(seen, at)| {
                *seen == print && now.saturating_duration_since(*at) < REPEAT_AFTER
            })
        });
        if repeated {
            Verdict::Repeated
        } else {
            Verdict::Show
        }
    }

    /// Saran tampil di `app`; saran yang sama ditahan selama [`REPEAT_AFTER`].
    pub fn shown(&mut self, app: Option<&ProcessName>, suggestion: &Suggestion, now: Instant) {
        let print = self.fingerprint(suggestion);
        let history = self.apps.entry(app.cloned()).or_default();
        history
            .shown
            .retain(|(_, at)| now.saturating_duration_since(*at) < REPEAT_AFTER);
        if history.shown.len() == MAX_REMEMBERED {
            history.shown.pop_front();
        }
        history.shown.push_back((print, now));
    }

    /// Tanggapan pengguna atas saran yang tampil di `app`.
    pub fn react(&mut self, app: Option<&ProcessName>, feedback: Feedback) {
        let history = self.apps.entry(app.cloned()).or_default();
        if history.feedback.len() == WINDOW {
            history.feedback.pop_front();
        }
        history.feedback.push_back(feedback);
    }

    /// Melupakan semua tanggapan dan saran yang diingat, misalnya setelah riwayat dihapus.
    pub fn forget(&mut self) {
        self.apps.clear();
    }

    fn history(&self, app: Option<&ProcessName>) -> Option<&AppHistory> {
        self.apps.get(&app.cloned())
    }

    /// Sidik judul dan isi saran, tanpa beda huruf besar, spasi, dan tanda baca.
    fn fingerprint(&self, suggestion: &Suggestion) -> u64 {
        let mut hasher = self.hasher.build_hasher();
        for part in [suggestion.title(), suggestion.body()] {
            write_words(&mut hasher, part);
        }
        hasher.finish()
    }
}

/// Kata-kata `text` dalam huruf kecil, dipisah satu penanda, tanpa menyalin teksnya.
fn write_words(hasher: &mut impl Hasher, text: &SensitiveText) {
    let mut started = false;
    let mut gap = false;
    for c in text.expose().chars() {
        if c.is_alphanumeric() {
            if gap {
                hasher.write_u8(b' ');
                gap = false;
            }
            for lower in c.to_lowercase() {
                hasher.write_u32(u32::from(lower));
            }
            started = true;
        } else if started {
            gap = true;
        }
    }
    // Penutup agar judul dan isi tidak bisa bergeser di antara keduanya.
    hasher.write_u8(0xff);
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use nuenly_core::{Confidence, ProcessName, Suggestion};

    use super::{Feedback, MAX_REMEMBERED, REPEAT_AFTER, Ranker, Verdict};

    fn app(name: &str) -> ProcessName {
        ProcessName::new(name).unwrap()
    }

    fn suggestion(title: &str, confidence: Confidence) -> Suggestion {
        Suggestion::new(title, "Isi sintetis.", &["Poin sintetis."], confidence).unwrap()
    }

    #[test]
    fn doubtful_suggestions_are_held() {
        let ranker = Ranker::new();
        let notepad = app("notepad.exe");
        let now = Instant::now();
        assert_eq!(ranker.threshold(Some(&notepad)), Confidence::Medium);
        for (confidence, verdict) in [
            (Confidence::Low, Verdict::BelowThreshold),
            (Confidence::Medium, Verdict::Show),
            (Confidence::High, Verdict::Show),
        ] {
            let candidate = suggestion("Email ini bisa lebih singkat", confidence);
            assert_eq!(ranker.judge(Some(&notepad), &candidate, now), verdict);
        }
    }

    #[test]
    fn the_same_suggestion_waits_before_showing_again() {
        let mut ranker = Ranker::new();
        let notepad = app("notepad.exe");
        let start = Instant::now();
        let shorter = suggestion("Email ini bisa lebih singkat", Confidence::High);
        ranker.shown(Some(&notepad), &shorter, start);

        // Beda huruf besar, spasi, dan tanda baca tetap dianggap sama.
        let again = suggestion("email ini   bisa lebih singkat!", Confidence::High);
        assert_eq!(
            ranker.judge(Some(&notepad), &again, start),
            Verdict::Repeated
        );
        // Aplikasi lain dan saran lain tidak terpengaruh.
        assert_eq!(
            ranker.judge(Some(&app("winword.exe")), &again, start),
            Verdict::Show
        );
        let other = suggestion("Ada salah ketik", Confidence::High);
        assert_eq!(ranker.judge(Some(&notepad), &other, start), Verdict::Show);

        assert_eq!(
            ranker.judge(Some(&notepad), &again, start + REPEAT_AFTER),
            Verdict::Show
        );
    }

    #[test]
    fn rejections_raise_the_threshold_and_acceptance_lowers_it() {
        let mut ranker = Ranker::new();
        let notepad = app("notepad.exe");
        let now = Instant::now();
        let medium = suggestion("Ada salah ketik", Confidence::Medium);

        ranker.react(Some(&notepad), Feedback::Disliked);
        ranker.react(Some(&notepad), Feedback::Expired);
        assert_eq!(ranker.threshold(Some(&notepad)), Confidence::Medium);
        ranker.react(Some(&notepad), Feedback::Dismissed);
        assert_eq!(ranker.threshold(Some(&notepad)), Confidence::High);
        assert_eq!(
            ranker.judge(Some(&notepad), &medium, now),
            Verdict::BelowThreshold
        );
        // Hanya aplikasi itu yang terpengaruh.
        assert_eq!(
            ranker.threshold(Some(&app("winword.exe"))),
            Confidence::Medium
        );
        assert_eq!(ranker.threshold(None), Confidence::Medium);

        ranker.react(Some(&notepad), Feedback::Copied);
        assert_eq!(ranker.threshold(Some(&notepad)), Confidence::Medium);
    }

    #[test]
    fn only_recent_reactions_count() {
        let mut ranker = Ranker::new();
        let notepad = app("notepad.exe");
        for _ in 0..3 {
            ranker.react(Some(&notepad), Feedback::Disliked);
        }
        assert_eq!(ranker.threshold(Some(&notepad)), Confidence::High);
        for _ in 0..10 {
            ranker.react(Some(&notepad), Feedback::Expired);
        }
        assert_eq!(ranker.threshold(Some(&notepad)), Confidence::Medium);
    }

    #[test]
    fn memory_is_bounded_and_can_be_forgotten() {
        let mut ranker = Ranker::new();
        let notepad = app("notepad.exe");
        let now = Instant::now();
        let first = suggestion("Saran pertama", Confidence::High);
        ranker.shown(Some(&notepad), &first, now);
        for index in 0..MAX_REMEMBERED {
            let later = suggestion(&format!("Saran {index}"), Confidence::High);
            ranker.shown(Some(&notepad), &later, now + Duration::from_secs(1));
        }
        assert_eq!(ranker.judge(Some(&notepad), &first, now), Verdict::Show);

        ranker.react(Some(&notepad), Feedback::Disliked);
        ranker.react(Some(&notepad), Feedback::Disliked);
        ranker.forget();
        assert_eq!(ranker.threshold(Some(&notepad)), Confidence::Medium);
    }
}
