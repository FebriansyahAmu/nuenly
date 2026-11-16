//! Ganti teks (F-23, US-1, ADR 0022): jawaban Tanya Nuenly ditempel lewat clipboard ke tempat
//! teks konteksnya dibaca. Ini satu-satunya aksi tulis Nuenly ke aplikasi lain: hanya atas klik
//! pengguna, sekali per jawaban, dan hanya bila teks di sana belum berubah sejak dibaca.

use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use nuenly_core::SensitiveText;
use nuenly_store::{AuditEntry, AuditEvent, ReplaceOutcome, ReplaceSource, Store};
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::ask::{self, AskState};
use crate::context::ContextReader;
use crate::status::AppStatus;
use crate::toast::{self, Toast, ToastContent};
use crate::{clipboard, logging, store, typing};

/// Jeda sebelum isi clipboard sebelumnya dikembalikan, agar aplikasi asal sempat menempel jawaban
/// lebih dulu.
const RESTORE_DELAY: Duration = Duration::from_secs(1);
/// Lama pesan di toast setelah Ganti teks gagal di aplikasi asal.
const NOTICE_MS: u64 = 8000;

/// Kenapa Ganti teks tidak menempel jawaban. Bentuknya sama dengan `ReplaceFailure` di
/// `app/ui/src/windows/ask/tauri.ts`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ReplaceFailure {
    /// Tidak ada yang bisa diganti: jawaban belum selesai atau sudah dipakai, konteks dihapus,
    /// atau teksnya bagian dari dokumen panjang.
    Unavailable,
    /// Teks di aplikasi asal sudah berubah sejak dibaca.
    Changed,
    /// Jendela atau kolom asal tidak bisa dikembalikan.
    Moved,
    Failed,
}

/// Ganti teks berhenti karena `failure`. `switched` berarti jendela asal sudah dikembalikan ke
/// depan, jadi jendela Tanya Nuenly sudah tertutup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Refused {
    pub failure: ReplaceFailure,
    pub switched: bool,
}

impl Refused {
    pub fn before_switch(failure: ReplaceFailure) -> Self {
        Self {
            failure,
            switched: false,
        }
    }
}

/// Bahan Ganti teks dari sesi Tanya Nuenly.
pub struct Replacement {
    pub target: u64,
    pub source: ReplaceSource,
    /// Teks yang dibaca saat hotkey; harus masih sama di aplikasi asal.
    pub expected: SensitiveText,
    /// Jawaban yang ditempel.
    pub text: SensitiveText,
}

/// Tombol "Ganti teks" di Tanya Nuenly.
///
/// Bila teks di aplikasi asal sudah berubah sebelum jendela asal dikembalikan, hasilnya error dan
/// Tanya Nuenly tetap terbuka (Salin masih bisa). Bila gagal setelah itu, jawaban disalin ke
/// clipboard dan pesannya tampil di toast. Setiap percobaan dicatat di audit tanpa teks.
#[tauri::command]
pub async fn ask_replace(app: AppHandle) -> Result<(), ReplaceFailure> {
    let started = Instant::now();
    let replacement = app.state::<AskState>().replacement()?;
    let (helped, mode) = app.state::<AppStatus>().audit_scope();
    let source = replacement.source;
    let answer = replacement.text.clone();
    let handle = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        handle.state::<ContextReader>().replace(
            replacement.target,
            replacement.expected,
            replacement.text,
        )
    })
    .await
    .unwrap_or(Err(Refused {
        failure: ReplaceFailure::Failed,
        switched: true,
    }));

    let outcome = outcome(result.as_ref().err());
    store::record(
        &app.state::<Arc<Store>>(),
        AuditEntry {
            at: SystemTime::now(),
            app: helped,
            mode,
            event: AuditEvent::Replace { source, outcome },
        },
    );
    tracing::info!(
        source = ?source,
        outcome = ?outcome,
        switched = result.as_ref().err().is_none_or(|refused| refused.switched),
        total_ms = logging::millis(started.elapsed()),
        "Ganti teks"
    );

    match result {
        Ok(restore) => {
            close(&app);
            // Tempelan memicu perubahan isi kolom; teks dari Nuenly tidak boleh memicu saran.
            typing::reset(&app);
            let later = app.clone();
            std::thread::spawn(move || {
                std::thread::sleep(RESTORE_DELAY);
                restore.restore();
                typing::reset(&later);
            });
            Ok(())
        }
        Err(refused) if refused.switched => {
            close(&app);
            let copied = clipboard::write(answer.expose()).is_ok();
            notify(&app, refused.failure, copied);
            Err(refused.failure)
        }
        Err(refused) => Err(refused.failure),
    }
}

/// Hasil untuk audit.
fn outcome(refused: Option<&Refused>) -> ReplaceOutcome {
    match refused.map(|refused| refused.failure) {
        None => ReplaceOutcome::Replaced,
        Some(ReplaceFailure::Changed) => ReplaceOutcome::Changed,
        Some(_) => ReplaceOutcome::Failed,
    }
}

/// Menutup Tanya Nuenly bila masih tampil; biasanya sudah tertutup sendiri saat jendela asal
/// kembali ke depan.
fn close(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(ask::LABEL) {
        let _ = window.hide();
    }
    ask::closed(app);
}

/// Pesan setelah jendela Tanya Nuenly tertutup.
fn notify(app: &AppHandle, failure: ReplaceFailure, copied: bool) {
    let _ = toast::show(
        app,
        &Toast {
            id: "ganti-teks".to_owned(),
            duration_ms: Some(NOTICE_MS),
            content: ToastContent::Success {
                message: message(failure, copied),
            },
        },
    );
}

fn message(failure: ReplaceFailure, copied: bool) -> String {
    let reason = match failure {
        ReplaceFailure::Changed => "Teksnya tidak diganti karena sudah berubah sejak dibaca.",
        _ => "Teksnya tidak bisa diganti di aplikasi ini.",
    };
    if copied {
        format!("Jawaban sudah disalin. {reason} Tempel sendiri dengan Ctrl+V bila masih perlu.")
    } else {
        reason.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use nuenly_store::ReplaceOutcome;

    use super::{Refused, ReplaceFailure, message, outcome};

    #[test]
    fn audit_tells_replaced_from_changed_and_failed() {
        assert_eq!(outcome(None), ReplaceOutcome::Replaced);
        let refused = |failure| Refused {
            failure,
            switched: true,
        };
        assert_eq!(
            outcome(Some(&refused(ReplaceFailure::Changed))),
            ReplaceOutcome::Changed
        );
        for failure in [
            ReplaceFailure::Unavailable,
            ReplaceFailure::Moved,
            ReplaceFailure::Failed,
        ] {
            assert_eq!(outcome(Some(&refused(failure))), ReplaceOutcome::Failed);
        }
    }

    #[test]
    fn the_toast_says_the_answer_was_copied_only_when_it_was() {
        assert!(message(ReplaceFailure::Changed, true).starts_with("Jawaban sudah disalin."));
        assert!(message(ReplaceFailure::Changed, true).contains("sudah berubah"));
        assert!(!message(ReplaceFailure::Moved, false).contains("disalin"));
    }

    #[test]
    fn failures_use_the_names_from_the_window() {
        let names: Vec<_> = [
            ReplaceFailure::Unavailable,
            ReplaceFailure::Changed,
            ReplaceFailure::Moved,
            ReplaceFailure::Failed,
        ]
        .iter()
        .map(|failure| serde_json::to_value(failure).unwrap())
        .collect();
        assert_eq!(
            names,
            [
                serde_json::json!("unavailable"),
                serde_json::json!("changed"),
                serde_json::json!("moved"),
                serde_json::json!("failed"),
            ]
        );
    }
}
