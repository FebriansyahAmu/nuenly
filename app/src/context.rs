//! Pembacaan konteks dari aplikasi aktif (F-02). Selalu lewat `nuenly_core::capture`, jadi
//! filter aplikasi diperiksa sebelum isi jendela dibaca.

#[cfg(windows)]
use std::time::Duration;

use nuenly_core::{
    ActivePolicy, CaptureKind, CaptureOutcome, ContextError, ContextSource, capture,
};

/// Batas tunggu membaca teks terpilih. Target PRD baca UIA ≤ 50 ms; sisanya cadangan untuk
/// aplikasi yang lambat menjawab, sebelum Tanya Nuenly tetap dibuka tanpa konteks.
#[cfg(windows)]
const READ_TIMEOUT: Duration = Duration::from_millis(250);

/// State terkelola Tauri. `None` bila OS ini belum punya sumber konteks.
pub struct ContextReader(Option<Box<dyn ContextSource>>);

impl ContextReader {
    /// Di Windows: thread UI Automation khusus. Membuatnya tidak membaca apa pun.
    pub fn new() -> Self {
        #[cfg(windows)]
        let source = nuenly_platform_windows::UiaContextSource::spawn(READ_TIMEOUT)
            .ok()
            .map(|source| Box::new(source) as Box<dyn ContextSource>);
        #[cfg(not(windows))]
        let source = None;
        Self(source)
    }

    /// Membaca teks terpilih di aplikasi aktif bila `policy` mengizinkan. Blocking, paling lama
    /// sekitar batas tunggu sumbernya.
    pub fn selection(&self, policy: &ActivePolicy) -> Result<CaptureOutcome, ContextError> {
        match &self.0 {
            Some(source) => capture(source.as_ref(), policy, CaptureKind::SelectedText),
            None => Err(ContextError::Unsupported),
        }
    }
}
