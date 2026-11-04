//! Catatan audit: apa yang dikirim Nuenly ke model, dari aplikasi dan mode apa, tanpa teks
//! pengguna. Jenis kejadian lain (saran, umpan balik, tool call) menyusul bersama fiturnya.

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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AskOutcome {
    Answered,
    /// Jendela ditutup sebelum jawaban selesai.
    Stopped,
    Failed,
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

    use super::{AskOutcome, AuditEvent, from_millis, millis};

    #[test]
    fn events_are_stored_as_tagged_json() {
        let event = AuditEvent::Ask {
            model: "gemma3:4b".to_owned(),
            context_chars: 120,
            outcome: AskOutcome::Answered,
        };
        assert_eq!(
            serde_json::to_value(&event).unwrap(),
            serde_json::json!({
                "kind": "ask",
                "model": "gemma3:4b",
                "contextChars": 120,
                "outcome": "answered",
            })
        );
    }

    #[test]
    fn times_round_trip_to_the_millisecond() {
        let at = SystemTime::UNIX_EPOCH + Duration::from_millis(1_793_750_400_123);
        assert_eq!(from_millis(millis(at)), at);
        assert_eq!(millis(SystemTime::UNIX_EPOCH - Duration::from_secs(1)), 0);
    }
}
