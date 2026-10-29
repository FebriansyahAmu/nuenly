//! Error crate ini. Pesannya hanya memuat teks statis dan angka, tidak pernah isi layar.

/// Kegagalan saat membaca konteks dari platform.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ContextError {
    /// Jendela lain naik ke latar depan selagi dibaca.
    #[error("jendela di latar depan berganti saat dibaca")]
    ForegroundChanged,
    /// Aplikasi tidak mengekspos teksnya ke platform.
    #[error("aplikasi ini tidak mengekspos teks lewat antarmuka aksesibilitas")]
    Unsupported,
    /// Platform tidak menjawab dalam batas waktu.
    #[error("pembacaan konteks melewati batas waktu")]
    Timeout,
    /// Panggilan OS gagal. `operation` sengaja `&'static str` agar tidak ada teks dinamis
    /// yang masuk ke pesan error.
    #[error("panggilan platform gagal: {operation} (kode {code:#010x})")]
    Platform { operation: &'static str, code: i32 },
}

/// Nama atau ID yang tidak memenuhi format.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{what} tidak valid")]
pub struct InvalidName {
    what: &'static str,
}

impl InvalidName {
    pub(crate) fn new(what: &'static str) -> Self {
        Self { what }
    }
}
