//! Hasil pembacaan konteks dari aplikasi aktif.

use crate::app::AppIdentity;
use crate::text::SensitiveText;

/// Apa yang dibaca dari aplikasi aktif.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum CaptureKind {
    /// Teks yang sedang dipilih pengguna.
    SelectedText,
    /// Isi field yang sedang fokus.
    FocusedField,
}

/// Jenis kontrol tempat teks dibaca.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ControlKind {
    /// Field teks, satu baris atau beberapa baris.
    Edit,
    /// Dokumen, misalnya badan email atau editor teks kaya.
    Document,
    /// Kontrol lain yang mengekspos teks.
    Other,
}

/// Kotak pembatas di layar, dalam piksel fisik.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Field tempat teks dibaca.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldInfo {
    /// Label field, misalnya "Anamnesis". Bisa memuat isi layar, jadi diperlakukan sensitif.
    pub label: Option<SensitiveText>,
    pub control: ControlKind,
    /// Field kata sandi. Snapshot dari field ini dibuang oleh [`capture`](crate::capture).
    pub is_password: bool,
    pub bounds: Option<Rect>,
}

/// Konteks yang dibaca dari aplikasi aktif pada satu waktu.
///
/// Semua teks dari layar dibungkus [`SensitiveText`], jadi `Debug` aman untuk log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextSnapshot {
    pub app: AppIdentity,
    pub window_title: SensitiveText,
    pub kind: CaptureKind,
    /// Field asal teks. `None` untuk teks terpilih yang tidak berada di field.
    pub field: Option<FieldInfo>,
    pub text: SensitiveText,
}

#[cfg(test)]
mod tests {
    use super::{CaptureKind, ContextSnapshot, ControlKind, FieldInfo};
    use crate::app::{AppIdentity, ProcessName};
    use crate::text::SensitiveText;

    #[test]
    fn debug_hides_all_screen_text() {
        let snapshot = ContextSnapshot {
            app: AppIdentity {
                process: ProcessName::new("medapp.exe").unwrap(),
                display_name: "MedApp".to_owned(),
            },
            window_title: SensitiveText::new("Rawat Jalan - Tn. Sintetis"),
            kind: CaptureKind::FocusedField,
            field: Some(FieldInfo {
                label: Some(SensitiveText::new("Anamnesis")),
                control: ControlKind::Edit,
                is_password: false,
                bounds: None,
            }),
            text: SensitiveText::new("Pasien sintetis mengeluh nyeri kepala"),
        };

        let debug = format!("{snapshot:?}");
        for secret in ["Sintetis", "Anamnesis", "nyeri"] {
            assert!(!debug.contains(secret), "Debug memuat teks layar");
        }
        assert!(debug.contains("medapp.exe"));
    }
}
