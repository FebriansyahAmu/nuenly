//! Catatan audit: apa yang dikirim Nuenly ke model, dari aplikasi dan mode apa, dan bagaimana
//! pengguna menanggapi saran, dan kapan Ganti teks menulis ke aplikasi lain, tanpa teks pengguna.
//! Tool call menyusul bersama fiturnya.

use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

/// Lama catatan audit disimpan. PRD meminta retensi pendek; catatan yang lebih tua dibuang
/// setiap kali store dibuka dan setiap kali ada catatan baru. Pemberitahuan privasi menyebut
/// angka ini.
pub const RETENTION: Duration = Duration::from_hours(30 * 24);

/// Satu kejadian yang dicatat.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuditEntry {
    pub at: SystemTime,
    /// Nama executable aplikasi yang dibantu, misalnya `notepad.exe`; `None` bila tidak
    /// diketahui.
    pub app: Option<String>,
    /// ID mode yang aktif saat itu.
    pub mode: String,
    pub event: AuditEvent,
}

/// Isi kejadian, disimpan sebagai JSON. Field-nya hanya nama, angka, dan pilihan tetap, tidak
/// pernah teks pengguna atau teks dari layar.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[non_exhaustive]
pub enum AuditEvent {
    /// Tanya Nuenly mengirim perintah, beserta teks terpilih bila ada, ke model lokal.
    #[serde(rename_all = "camelCase")]
    Ask {
        model: String,
        /// Jumlah karakter teks dari layar yang ikut dikirim; 0 berarti hanya perintah.
        context_chars: u32,
        outcome: AskOutcome,
    },
    /// Pemeriksaan saran proaktif mengirim teks yang sedang ditulis ke model lokal.
    #[serde(rename_all = "camelCase")]
    Suggest {
        model: String,
        /// Jumlah karakter teks dari layar yang ikut dikirim.
        context_chars: u32,
        outcome: SuggestOutcome,
    },
    /// Tanggapan pengguna atas saran yang tampil, untuk menyetel ambang saran (F-24).
    #[serde(rename_all = "camelCase")]
    Reaction {
        reaction: Reaction,
        /// Milidetik sejak saran tampil sampai ditanggapi.
        after_ms: u32,
    },
    /// Ganti teks: jawaban Tanya Nuenly ditempel ke aplikasi asal atas klik pengguna (F-23).
    #[serde(rename_all = "camelCase")]
    Replace {
        source: ReplaceSource,
        outcome: ReplaceOutcome,
    },
}

/// Teks yang diganti.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReplaceSource {
    /// Teks terpilih.
    Selection,
    /// Seluruh isi kolom.
    Field,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReplaceOutcome {
    /// Jawaban ditempel.
    Replaced,
    /// Teks di aplikasi asal sudah berubah sejak dibaca, jadi tidak diganti.
    Changed,
    /// Jendela atau kolom asal tidak bisa dikembalikan, atau tempel gagal.
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AskOutcome {
    Answered,
    /// Jendela ditutup sebelum jawaban selesai.
    Stopped,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SuggestOutcome {
    /// Saran tampil di toast.
    Shown,
    /// Ada saran, tetapi ditahan ranker: keyakinannya di bawah ambang, atau sama dengan saran yang
    /// baru tampil.
    Held,
    /// Model menilai tidak ada yang perlu disarankan.
    Nothing,
    /// Jawaban tidak sesuai schema setelah diulang, atau dibuang penyaring saran.
    Dropped,
    Failed,
}

/// Bagaimana saran yang tampil berakhir. Satu saran mendapat satu tanggapan.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Reaction {
    /// "Salin poin".
    Copied,
    /// "Lihat", lalu ditutup tanpa menyalin.
    Viewed,
    /// 👎: saran tidak membantu.
    Disliked,
    /// "Abaikan": bukan sekarang.
    Dismissed,
    /// Tombol tutup tanpa melihat rinciannya.
    Closed,
    /// Hilang sendiri setelah batas waktu toast.
    Expired,
}

/// Milidetik sejak epoch Unix, bentuk kolom `at`.
pub(crate) fn millis(at: SystemTime) -> i64 {
    at.duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |since| {
            i64::try_from(since.as_millis()).unwrap_or(i64::MAX)
        })
}

pub(crate) fn from_millis(millis: i64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_millis(u64::try_from(millis).unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use super::{
        AskOutcome, AuditEvent, Reaction, ReplaceOutcome, ReplaceSource, SuggestOutcome,
        from_millis, millis,
    };

    #[test]
    fn events_are_stored_as_tagged_json() {
        let cases = [
            (
                AuditEvent::Ask {
                    model: "gemma3:4b".to_owned(),
                    context_chars: 120,
                    outcome: AskOutcome::Answered,
                },
                serde_json::json!({
                    "kind": "ask",
                    "model": "gemma3:4b",
                    "contextChars": 120,
                    "outcome": "answered",
                }),
            ),
            (
                AuditEvent::Suggest {
                    model: "gemma3:4b".to_owned(),
                    context_chars: 300,
                    outcome: SuggestOutcome::Shown,
                },
                serde_json::json!({
                    "kind": "suggest",
                    "model": "gemma3:4b",
                    "contextChars": 300,
                    "outcome": "shown",
                }),
            ),
            (
                AuditEvent::Reaction {
                    reaction: Reaction::Disliked,
                    after_ms: 1800,
                },
                serde_json::json!({
                    "kind": "reaction",
                    "reaction": "disliked",
                    "afterMs": 1800,
                }),
            ),
            (
                AuditEvent::Replace {
                    source: ReplaceSource::Selection,
                    outcome: ReplaceOutcome::Changed,
                },
                serde_json::json!({
                    "kind": "replace",
                    "source": "selection",
                    "outcome": "changed",
                }),
            ),
        ];
        for (event, json) in cases {
            assert_eq!(serde_json::to_value(&event).unwrap(), json);
            assert_eq!(serde_json::from_value::<AuditEvent>(json).unwrap(), event);
        }
    }

    #[test]
    fn times_round_trip_to_the_millisecond() {
        let at = SystemTime::UNIX_EPOCH + Duration::from_millis(1_793_750_400_123);
        assert_eq!(from_millis(millis(at)), at);
        assert_eq!(millis(SystemTime::UNIX_EPOCH - Duration::from_secs(1)), 0);
    }
}
