//! Error crate ini. Pesannya hanya teks statis dan kode status: pesan error dari server model
//! atau dari validasi schema bisa memuat potongan jawaban, jadi tidak pernah disimpan.

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum LlmError {
    /// Alamat server model lokal bukan loopback.
    #[error("alamat server model harus loopback")]
    NotLoopback,
    /// Tidak ada server yang menjawab di alamat itu.
    #[error("server model tidak bisa dihubungi")]
    Unreachable,
    #[error("server model tidak menjawab dalam batas waktu")]
    Timeout,
    #[error("model tidak ditemukan di server")]
    ModelNotFound,
    /// Model cloud: server model lokal akan meneruskan teks ke luar PC.
    #[error("model ini berjalan di luar PC dan tidak dipakai")]
    RemoteModel,
    #[error("server model menjawab dengan status HTTP {0}")]
    Status(u16),
    /// Server model melaporkan kegagalan di tengah jawaban.
    #[error("server model melaporkan kegagalan")]
    Server,
    /// Jawaban tidak sesuai protokol: JSON rusak, terlalu besar, atau terputus.
    #[error("jawaban server model tidak bisa dibaca")]
    Protocol,
    /// Jawaban model tetap tidak lolos JSON schema setelah diulang sekali.
    #[error("jawaban model tidak sesuai schema")]
    SchemaMismatch,
    #[error("JSON schema output tidak valid")]
    InvalidSchema,
}

impl From<reqwest::Error> for LlmError {
    fn from(error: reqwest::Error) -> Self {
        // Pesan reqwest bisa memuat URL; yang disimpan hanya jenisnya. Timeout saat menyambung
        // (Windows mengulang sambungan ke port tertutup) tetap berarti tidak bisa dihubungi.
        if error.is_connect() {
            Self::Unreachable
        } else if error.is_timeout() {
            Self::Timeout
        } else if let Some(status) = error.status() {
            Self::Status(status.as_u16())
        } else {
            Self::Protocol
        }
    }
}
