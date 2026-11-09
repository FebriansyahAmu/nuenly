//! PII/PHI guard Nuenly (F-15): menyamarkan data pribadi dan data kesehatan sebelum teks keluar
//! perangkat, lalu mengembalikannya di jawaban.
//!
//! Setiap jalur yang mengirim teks pengguna keluar perangkat (server kantor, Claude API, argumen
//! tool MCP) wajib lewat [`Guard::redact`] lebih dulu, dengan level dari
//! [`ActivePolicy::redaction`](nuenly_core::ActivePolicy). Model lokal di `127.0.0.1` tidak
//! keluar perangkat.
//!
//! ```text
//! teks layar ─▶ Guard::redact ─▶ susun prompt ─▶ kirim ─▶ jawaban ─▶ Redacted::restore ─▶ jendela
//! ```
//!
//! Data diganti nama samaran seperti `[NAMA-1]` atau `[NIK-1]`; nilai yang sama mendapat nama
//! samaran yang sama, jadi model tetap tahu dua kemunculan itu orang yang sama. Daftar
//! pasangannya hanya di memori ([`Redacted`]) dan tidak pernah dikirim.
//!
//! Level standar menyamarkan email, telepon, NIK/KK yang strukturnya sah, NPWP, kartu BPJS,
//! kartu pembayaran (Luhn), alamat jalan bernomor, nama setelah sapaan singkat (`Tn.`, `Ny.`,
//! `a.n.`), dan nilai setelah label kamus (nama, alamat, tanggal lahir, NIK, BPJS, rekam medis,
//! rekening, telepon). Level ketat menambah semua tanggal lengkap, semua deret angka 8 digit atau
//! lebih, RT/RW, nama jalan tanpa nomor, dan nama setelah sapaan lain (`dr.`, `Ibu`, `Pak`).
//!
//! Batasnya: nama tanpa label atau sapaan ("Budi datang kemarin") tidak dikenali, begitu pula
//! nomor berformat asing tanpa label. Karena itu Medical tetap hanya memakai model lokal.

#![forbid(unsafe_code)]

mod detect;
mod kind;
mod normalize;
mod patterns;
mod redacted;
mod validate;

use nuenly_core::{RedactionLevel, SensitiveText};

pub use kind::Kind;
pub use redacted::{Findings, Redacted, Restorer};

#[derive(Debug, thiserror::Error)]
pub enum GuardError {
    /// Pola bawaan tidak bisa dikompilasi. Ini bug, dan test menangkapnya.
    #[error("pola PII guard tidak valid")]
    Pattern(#[from] regex::Error),
}

/// Penyaring data pribadi. Polanya dikompilasi sekali; simpan satu per aplikasi.
#[derive(Debug)]
pub struct Guard {
    detector: detect::Detector,
}

impl Guard {
    /// # Errors
    ///
    /// [`GuardError::Pattern`] bila pola bawaan tidak bisa dikompilasi.
    pub fn new() -> Result<Self, GuardError> {
        Ok(Self {
            detector: detect::Detector::new()?,
        })
    }

    /// Menyamarkan data pribadi di `text` sesuai `level`. Teks dinormalisasi lebih dulu
    /// (karakter tak terlihat dibuang, angka lebar penuh jadi ASCII), dan hasil normalisasi itu
    /// yang dikirim.
    #[must_use]
    pub fn redact(&self, text: &SensitiveText, level: RedactionLevel) -> Redacted {
        let normalized = normalize::normalize(text.expose());
        let spans = self.detector.spans(&normalized, level);
        Redacted::new(&normalized, &spans)
    }
}
