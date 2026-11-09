//! Jenis data pribadi yang disamarkan dan nama samarannya.

/// Jenis data pribadi. Urutannya juga urutan prioritas bila dua temuan dimulai di tempat yang
/// sama dengan panjang yang sama.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    /// Nomor rekam medis atau nomor registrasi pasien.
    MedicalRecord,
    /// NIK atau nomor kartu keluarga.
    Nik,
    Npwp,
    /// Nomor kartu BPJS Kesehatan atau JKN-KIS.
    Bpjs,
    /// Nomor kartu pembayaran.
    Card,
    /// Nomor rekening bank atau virtual account.
    Account,
    Phone,
    Email,
    /// Tanggal lahir, termasuk tempat lahir di depannya.
    BirthDate,
    /// Tanggal lengkap lain (level ketat).
    Date,
    Name,
    Address,
    /// Deret angka panjang lain (level ketat).
    Number,
}

impl Kind {
    /// Label di nama samaran, misalnya `NIK` untuk `[NIK-1]`.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::MedicalRecord => "RM",
            Self::Nik => "NIK",
            Self::Npwp => "NPWP",
            Self::Bpjs => "BPJS",
            Self::Card => "KARTU",
            Self::Account => "REKENING",
            Self::Phone => "TELEPON",
            Self::Email => "EMAIL",
            Self::BirthDate => "TGL-LAHIR",
            Self::Date => "TANGGAL",
            Self::Name => "NAMA",
            Self::Address => "ALAMAT",
            Self::Number => "ANGKA",
        }
    }

    /// Bentuk baku nilai, agar penulisan yang berbeda untuk data yang sama (spasi, tanda hubung,
    /// huruf besar, `+62`) mendapat satu nama samaran.
    pub(crate) fn canonical(self, value: &str) -> String {
        match self {
            Self::Nik | Self::Npwp | Self::Bpjs | Self::Card | Self::Account | Self::Number => {
                value.chars().filter(char::is_ascii_digit).collect()
            }
            Self::Phone => {
                // Nomor lokal diawali 0, jadi 62 di depan selalu kode negara Indonesia.
                let digits: String = value.chars().filter(char::is_ascii_digit).collect();
                match digits.strip_prefix("62") {
                    Some(local) => format!("0{local}"),
                    None => digits,
                }
            }
            Self::MedicalRecord => value
                .chars()
                .filter(char::is_ascii_alphanumeric)
                .map(|c| c.to_ascii_lowercase())
                .collect(),
            Self::Email | Self::BirthDate | Self::Date | Self::Name | Self::Address => value
                .split_whitespace()
                .map(str::to_lowercase)
                .collect::<Vec<_>>()
                .join(" "),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Kind;

    #[test]
    fn different_spellings_of_one_value_share_a_canonical_form() {
        assert_eq!(
            Kind::Phone.canonical("+62 812-0000-1234"),
            Kind::Phone.canonical("0812 0000 1234")
        );
        assert_eq!(
            Kind::Nik.canonical("3299 9945 0190 0001"),
            "3299994501900001"
        );
        assert_eq!(
            Kind::Name.canonical("BUDI  Santoso"),
            Kind::Name.canonical("budi santoso")
        );
        assert_eq!(Kind::MedicalRecord.canonical("RM-00.12"), "rm0012");
    }

    #[test]
    fn labels_are_upper_case_words() {
        for kind in [Kind::BirthDate, Kind::Name, Kind::MedicalRecord] {
            assert!(
                kind.label()
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c == '-')
            );
        }
    }
}
