//! Validasi struktur nomor, agar deret angka biasa tidak dianggap data pribadi.

use zeroize::Zeroizing;

/// Nilai digit ASCII di `written`, berurutan.
pub(crate) fn digits(written: &str) -> Zeroizing<Vec<u8>> {
    Zeroizing::new(
        written
            .bytes()
            .filter(u8::is_ascii_digit)
            .map(|b| b - b'0')
            .collect(),
    )
}

/// Kode provinsi Kemendagri yang dipakai di NIK, termasuk provinsi baru di Papua.
fn province(code: u8) -> bool {
    matches!(code, 11..=19 | 21 | 31..=36 | 51..=53 | 61..=65 | 71..=76 | 81 | 82 | 91..=96)
}

/// Nomor 16 digit berstruktur NIK atau nomor KK: kode provinsi yang ada, kode kabupaten dan
/// kecamatan bukan 00, tanggal (lahir, +40 untuk perempuan, atau tanggal terbit KK) dan bulan yang
/// mungkin, serta nomor urut bukan 0000. Tahun tidak bisa diperiksa karena hanya dua digit.
pub(crate) fn nik(digits: &[u8]) -> bool {
    let [p1, p2, k1, k2, c1, c2, d1, d2, m1, m2, _, _, s @ ..] = digits else {
        return false;
    };
    if s.len() != 4 {
        return false;
    }
    let pair = |a: u8, b: u8| a * 10 + b;
    let day = pair(*d1, *d2);
    let day = if day > 40 { day - 40 } else { day };
    province(pair(*p1, *p2))
        && pair(*k1, *k2) != 0
        && pair(*c1, *c2) != 0
        && (1..=31).contains(&day)
        && (1..=12).contains(&pair(*m1, *m2))
        && s != [0, 0, 0, 0]
}

/// Nomor kartu pembayaran: 13–19 digit, diawali 2–6, dan lolos checksum Luhn.
pub(crate) fn card(digits: &[u8]) -> bool {
    (13..=19).contains(&digits.len())
        && digits.first().is_some_and(|first| (2..=6).contains(first))
        && luhn(digits)
}

fn luhn(digits: &[u8]) -> bool {
    let sum: u32 = digits
        .iter()
        .rev()
        .enumerate()
        .map(|(index, &digit)| {
            let digit = u32::from(digit);
            if index % 2 == 1 {
                let doubled = digit * 2;
                if doubled > 9 { doubled - 9 } else { doubled }
            } else {
                digit
            }
        })
        .sum();
    sum.is_multiple_of(10)
}

/// Tanggal yang mungkin: hari 1–31 dan bulan 1–12, dalam urutan hari-bulan atau bulan-hari.
/// Untuk tanggal dengan nama bulan, cukup harinya.
pub(crate) fn date(written: &str) -> bool {
    let groups: Vec<&str> = written
        .split(|c: char| !c.is_ascii_digit())
        .filter(|group| !group.is_empty())
        .collect();
    let value = |group: &str| group.parse::<u32>().unwrap_or(0);
    if written.chars().any(char::is_alphabetic) {
        return groups
            .iter()
            .find(|group| group.len() <= 2)
            .is_some_and(|day| (1..=31).contains(&value(day)));
    }
    let [first, second, third] = groups.as_slice() else {
        return false;
    };
    let (day, month) = if first.len() == 4 {
        (value(third), value(second))
    } else {
        (value(first), value(second))
    };
    let day_month = |day: u32, month: u32| (1..=31).contains(&day) && (1..=12).contains(&month);
    day_month(day, month) || day_month(month, day)
}

#[cfg(test)]
mod tests {
    use super::{card, date, digits, nik};

    #[test]
    fn nik_follows_the_population_number_layout() {
        // Kode kabupaten 99 tidak ada, jadi nomor-nomor ini bukan NIK sungguhan.
        assert!(nik(&digits("3299994501900001")));
        // Perempuan: tanggal lahir + 40.
        assert!(nik(&digits("3299997112900001")));
        for invalid in [
            "9999994501900001", // provinsi tidak ada
            "3200994501900001", // kabupaten 00
            "3299004501900001", // kecamatan 00
            "3299997201900001", // tanggal 32
            "3299990113900001", // bulan 13
            "3299994501900000", // nomor urut 0000
            "329999450190001",  // 15 digit
        ] {
            assert!(!nik(&digits(invalid)), "{invalid}");
        }
    }

    #[test]
    fn cards_pass_the_luhn_checksum() {
        assert!(card(&digits("4111 1111 1111 1111")));
        assert!(card(&digits("5555555555554444")));
        assert!(!card(&digits("4111 1111 1111 1112")));
        // Diawali 9 bukan kartu pembayaran meskipun lolos Luhn.
        assert!(!card(&digits("9111111111111111")));
        assert!(!card(&digits("411111111111")));
    }

    #[test]
    fn dates_need_a_possible_day_and_month() {
        for valid in [
            "12-03-1985",
            "3/12/85",
            "2026-11-09",
            "12 Maret 1985",
            "March 12, 1985",
            "12/31/2026",
        ] {
            assert!(date(valid), "{valid}");
        }
        for invalid in ["32-01-1985", "45.30.12", "2026-13-40", "00/00/2000"] {
            assert!(!date(invalid), "{invalid}");
        }
    }
}
