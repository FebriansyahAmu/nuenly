//! Error crate ini. Pesannya hanya memuat nama field, ID mode, dan pesan parser TOML tentang
//! isi mode pack, tidak pernah teks pengguna.

/// Mode pack tidak bisa dipakai.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ModeError {
    /// `mode.toml` bukan TOML yang valid, kurang field wajib, atau memuat field yang tidak
    /// dikenal.
    #[error("mode.toml tidak sesuai format: {0}")]
    Manifest(String),
    /// Nilai di luar format atau batas yang diizinkan.
    #[error("{field} di mode pack tidak valid")]
    Invalid { field: &'static str },
    #[error("tidak ada mode pack")]
    NoModes,
    #[error("ID mode {0} dipakai lebih dari satu mode pack")]
    DuplicateId(String),
}

pub(crate) fn invalid(field: &'static str) -> ModeError {
    ModeError::Invalid { field }
}
