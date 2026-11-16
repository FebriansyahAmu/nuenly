//! Pembacaan konteks dari aplikasi aktif untuk Tanya Nuenly (F-02), dan Ganti teks (F-23) di
//! tempat konteks itu dibaca. Bacaan selalu lewat `nuenly_core::capture_first`, jadi filter
//! aplikasi diperiksa sebelum isi jendela dibaca. Saran saat mengetik membaca lewat pemantaunya
//! sendiri (`crate::typing`).

#[cfg(windows)]
use std::time::Duration;

use nuenly_core::{ActivePolicy, CaptureKind, CaptureOutcome, ContextError, SensitiveText};
#[cfg(windows)]
use {
    nuenly_core::capture_first,
    nuenly_platform_windows::{ReplaceReason, UiaContextSource, clipboard::Saved},
};

use crate::replace::{Refused, ReplaceFailure};

/// Batas tunggu satu pembacaan (teks terpilih atau isi field). Target PRD baca UIA ≤ 50 ms; sisanya cadangan untuk
/// aplikasi yang lambat menjawab, sebelum Tanya Nuenly tetap dibuka tanpa konteks. Dipakai juga
/// pemicu saran.
#[cfg(windows)]
pub const READ_TIMEOUT: Duration = Duration::from_millis(250);

/// State terkelola Tauri. Tanpa sumber bila OS ini belum didukung atau UI Automation gagal
/// dimulai.
pub struct ContextReader {
    #[cfg(windows)]
    source: Option<UiaContextSource>,
}

/// Isi clipboard sebelum Ganti teks, untuk dikembalikan setelah aplikasi asal menempel.
pub struct Restore(#[cfg(windows)] Saved);

impl Restore {
    /// Mengembalikan teks clipboard sebelumnya, kecuali pengguna sudah menyalin hal lain.
    pub fn restore(self) {
        #[cfg(windows)]
        let _ = self.0.restore();
    }
}

impl ContextReader {
    /// Di Windows: thread UI Automation khusus. Membuatnya tidak membaca apa pun.
    pub fn new() -> Self {
        Self {
            #[cfg(windows)]
            source: UiaContextSource::spawn(READ_TIMEOUT).ok(),
        }
    }

    /// Membaca teks terpilih di aplikasi aktif bila `policy` mengizinkan, untuk "Uji saran" di
    /// menu tray build debug. Blocking, paling lama sekitar batas tunggu sumbernya.
    #[cfg(debug_assertions)]
    pub fn selection(&self, policy: &ActivePolicy) -> Result<CaptureOutcome, ContextError> {
        self.capture(policy, &[CaptureKind::SelectedText])
    }

    /// Konteks Tanya Nuenly: teks terpilih, atau isi field fokus bila tidak ada yang dipilih.
    /// Blocking, paling lama sekitar dua kali batas tunggu sumbernya.
    pub fn for_ask(&self, policy: &ActivePolicy) -> Result<CaptureOutcome, ContextError> {
        self.capture(policy, ASK_KINDS)
    }

    #[cfg(windows)]
    fn capture(
        &self,
        policy: &ActivePolicy,
        kinds: &[CaptureKind],
    ) -> Result<CaptureOutcome, ContextError> {
        match &self.source {
            Some(source) => capture_first(source, policy, kinds),
            None => Err(ContextError::Unsupported),
        }
    }

    #[cfg(not(windows))]
    #[allow(clippy::unused_self)]
    fn capture(
        &self,
        _policy: &ActivePolicy,
        _kinds: &[CaptureKind],
    ) -> Result<CaptureOutcome, ContextError> {
        Err(ContextError::Unsupported)
    }

    /// Target Ganti teks dari bacaan terakhir yang berhasil, bila teksnya bisa diganti: teks
    /// terpilih, atau seluruh isi kolom. Panggil tepat setelah [`for_ask`](Self::for_ask).
    #[allow(clippy::unused_self)]
    pub fn replace_target(&self) -> Option<u64> {
        #[cfg(windows)]
        return self
            .source
            .as_ref()
            .and_then(UiaContextSource::replace_target);
        #[cfg(not(windows))]
        None
    }

    /// Melupakan target Ganti teks tanpa menunggu, saat sesi Tanya Nuenly selesai.
    #[allow(clippy::unused_self)]
    pub fn forget_target(&self) {
        #[cfg(windows)]
        if let Some(source) = &self.source {
            source.forget_target();
        }
    }

    /// Ganti teks: menempel `replacement` di tempat target `id` dibaca, hanya bila teks di sana
    /// masih `expected`. Blocking, paling lama beberapa detik.
    #[allow(clippy::unused_self, clippy::needless_pass_by_value)]
    pub fn replace(
        &self,
        id: u64,
        expected: SensitiveText,
        replacement: SensitiveText,
    ) -> Result<Restore, Refused> {
        #[cfg(windows)]
        {
            let Some(source) = &self.source else {
                return Err(Refused::before_switch(ReplaceFailure::Unavailable));
            };
            source
                .replace(id, expected, replacement)
                .map(Restore)
                .map_err(|err| Refused {
                    failure: match err.reason {
                        ReplaceReason::Gone => ReplaceFailure::Unavailable,
                        ReplaceReason::Changed => ReplaceFailure::Changed,
                        ReplaceReason::Moved => ReplaceFailure::Moved,
                        ReplaceReason::Failed => ReplaceFailure::Failed,
                    },
                    switched: err.switched,
                })
        }
        #[cfg(not(windows))]
        {
            let _ = (id, expected, replacement);
            Err(Refused::before_switch(ReplaceFailure::Unavailable))
        }
    }
}

/// Urutan baca Tanya Nuenly: teks terpilih lebih dulu, lalu isi field fokus.
pub const ASK_KINDS: &[CaptureKind] = &[CaptureKind::SelectedText, CaptureKind::FocusedField];
