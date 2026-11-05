//! Riwayat aktivitas di Pengaturan › Privasi (F-28, US-4): catatan audit terbaru dari store,
//! dan menghapusnya atas permintaan pengguna. Catatan tidak pernah memuat teks pengguna.

use std::sync::Arc;
use std::time::UNIX_EPOCH;

use nuenly_store::{AuditEntry, AuditEvent, Store, StoreError};
use serde::Serialize;
use tauri::State;

/// Jumlah catatan terbaru yang ditampilkan.
const LIMIT: u32 = 50;

/// Satu catatan untuk jendela; bentuknya sama dengan `AuditItem` di
/// `app/ui/src/windows/settings/backend.ts`. Isi kejadian (`kind` dan field-nya) sama dengan
/// bentuk `AuditEvent` di store.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditItem {
    /// Waktu kejadian dalam milidetik sejak epoch Unix.
    pub at_ms: u64,
    /// Nama executable aplikasi yang dibantu.
    pub app: Option<String>,
    /// ID mode; jendela menerjemahkannya menjadi label mode.
    pub mode: String,
    #[serde(flatten)]
    pub event: AuditEvent,
}

impl From<AuditEntry> for AuditItem {
    fn from(entry: AuditEntry) -> Self {
        let at_ms = entry.at.duration_since(UNIX_EPOCH).map_or(0, |since| {
            u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
        });
        Self {
            at_ms,
            app: entry.app,
            mode: entry.mode,
            event: entry.event,
        }
    }
}

/// Catatan audit terbaru, yang terbaru lebih dulu.
#[tauri::command]
pub async fn audit_list(store: State<'_, Arc<Store>>) -> Result<Vec<AuditItem>, String> {
    let store = Arc::clone(&store);
    let entries = blocking(move || store.recent(LIMIT), "riwayat tidak bisa dibaca").await?;
    Ok(entries.into_iter().map(AuditItem::from).collect())
}

/// Menghapus semua catatan audit. Setelan tidak ikut terhapus.
#[tauri::command]
pub async fn audit_clear(store: State<'_, Arc<Store>>) -> Result<(), String> {
    let store = Arc::clone(&store);
    let removed = blocking(move || store.clear_audit(), "riwayat tidak bisa dihapus").await?;
    tracing::info!(removed, "riwayat aktivitas dihapus pengguna");
    Ok(())
}

/// Menjalankan panggilan store di thread blocking. Gagal menjadi pesan tetap untuk jendela;
/// rinciannya hanya di log.
async fn blocking<T: Send + 'static>(
    call: impl FnOnce() -> Result<T, StoreError> + Send + 'static,
    failure: &'static str,
) -> Result<T, String> {
    match tauri::async_runtime::spawn_blocking(call).await {
        Ok(Ok(result)) => Ok(result),
        Ok(Err(err)) => {
            tracing::warn!(error = %err, "{failure}");
            Err(failure.to_owned())
        }
        Err(err) => {
            tracing::warn!(error = %err, "{failure}");
            Err(failure.to_owned())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, UNIX_EPOCH};

    use nuenly_store::{AskOutcome, AuditEntry, AuditEvent};

    use super::AuditItem;

    #[test]
    fn items_carry_the_event_fields_beside_time_app_and_mode() {
        let item = AuditItem::from(AuditEntry {
            at: UNIX_EPOCH + Duration::from_millis(1_793_840_000_123),
            app: Some("notepad.exe".to_owned()),
            mode: "all".to_owned(),
            event: AuditEvent::Ask {
                model: "gemma3:4b".to_owned(),
                context_chars: 120,
                outcome: AskOutcome::Stopped,
            },
        });

        assert_eq!(
            serde_json::to_value(item).unwrap(),
            serde_json::json!({
                "atMs": 1_793_840_000_123_u64,
                "app": "notepad.exe",
                "mode": "all",
                "kind": "ask",
                "model": "gemma3:4b",
                "contextChars": 120,
                "outcome": "stopped",
            })
        );
    }

    #[test]
    fn times_before_the_epoch_become_zero() {
        let item = AuditItem::from(AuditEntry {
            at: UNIX_EPOCH - Duration::from_secs(1),
            app: None,
            mode: "medical".to_owned(),
            event: AuditEvent::Ask {
                model: "gemma3:4b".to_owned(),
                context_chars: 0,
                outcome: AskOutcome::Failed,
            },
        });
        assert_eq!(item.at_ms, 0);
    }
}
