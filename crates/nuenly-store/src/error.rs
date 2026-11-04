//! Error crate ini. Pesannya hanya memuat pesan SQLite tentang skema dan pesan parser JSON
//! tentang posisi, tidak pernah isi store.

use crate::KEY_LEN;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum StoreError {
    /// Kunci salah, atau berkasnya bukan store Nuenly.
    #[error("store tidak bisa dibuka dengan kunci ini")]
    Unreadable,
    /// Store dibuat versi Nuenly yang lebih baru; skemanya tidak dikenal versi ini.
    #[error("store memakai skema {0} dari versi Nuenly yang lebih baru")]
    Newer(u32),
    #[error("kunci store harus {KEY_LEN} byte")]
    KeyLength,
    #[error("generator acak sistem operasi tidak tersedia")]
    Random,
    #[error("isi store tidak sesuai format JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("SQLite: {0}")]
    Sqlite(#[from] rusqlite::Error),
}
