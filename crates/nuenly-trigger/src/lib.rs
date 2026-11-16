//! Pemicu dan ranker saran proaktif Nuenly (F-09, F-24, US-2).
//!
//! [`TriggerEngine`] memutuskan kapan field yang sedang diisi pantas diperiksa. Masukannya sinyal
//! tanpa teks dari platform (fokus, isi berubah, caret bergerak), dengan waktu dari pemanggil.
//! Keluarannya [`Trigger`]: pengguna berhenti mengetik (idle) atau meninggalkan field yang baru
//! diisi (blur). Tidak ada timer berkala: pemanggil memasang satu timer ke
//! [`TriggerEngine::wake_at`] lalu memanggil [`TriggerEngine::tick`]. Isi field baru dibaca
//! setelah ada pemicu, sekali.
//!
//! [`Ranker`] memutuskan apakah saran hasil pemeriksaan layak tampil: keyakinan model di atas
//! ambang aplikasi itu, dan bukan saran yang baru saja tampil. Tanggapan pengguna menyetel
//! ambangnya.

#![forbid(unsafe_code)]

mod engine;
mod ranker;

pub use engine::{Input, Reason, Trigger, TriggerConfig, TriggerEngine};
pub use ranker::{Feedback, Ranker, Verdict};
