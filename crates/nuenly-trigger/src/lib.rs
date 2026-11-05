//! Pemicu saran proaktif Nuenly (F-09, US-2): kapan field yang sedang diisi pantas diperiksa.
//!
//! Masukannya sinyal tanpa teks dari platform (fokus, isi berubah, caret bergerak), dengan waktu
//! dari pemanggil. Keluarannya [`Trigger`]: pengguna berhenti mengetik (idle) atau meninggalkan
//! field yang baru diisi (blur). Tidak ada timer berkala: pemanggil memasang satu timer ke
//! [`TriggerEngine::wake_at`] lalu memanggil [`TriggerEngine::tick`]. Isi field baru dibaca
//! setelah ada pemicu, sekali.
//!
//! Suggestion ranker (F-24) menyusul di crate ini.

#![forbid(unsafe_code)]

mod engine;

pub use engine::{Input, Reason, Trigger, TriggerConfig, TriggerEngine};
