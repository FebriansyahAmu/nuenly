//! Normalisasi sebelum deteksi, agar nomor tidak lolos hanya karena ditulis dengan karakter lain
//! yang tampak sama.

use zeroize::Zeroizing;

/// Membuang karakter tak terlihat, mengganti spasi dan tanda hubung khusus dengan versi ASCII,
/// dan mengubah huruf serta angka lebar penuh (fullwidth) menjadi ASCII. Teks hasilnya yang
/// dikirim keluar perangkat.
pub(crate) fn normalize(text: &str) -> Zeroizing<String> {
    let mut normalized = Zeroizing::new(String::with_capacity(text.len()));
    for c in text.chars() {
        match c {
            '\u{00AD}' | '\u{200B}'..='\u{200D}' | '\u{2060}' | '\u{FEFF}' => {}
            '\u{00A0}' | '\u{2007}' | '\u{202F}' | '\u{3000}' => normalized.push(' '),
            // Tanda hubung, figure dash, en dash, dan minus; em dash tetap.
            '\u{2010}'..='\u{2013}' | '\u{2212}' => normalized.push('-'),
            '\u{FF01}'..='\u{FF5E}' => {
                normalized.push(char::from_u32(u32::from(c) - 0xFEE0).unwrap_or(c));
            }
            _ => normalized.push(c),
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::normalize;

    #[test]
    fn look_alike_characters_become_ascii() {
        assert_eq!(
            normalize("０８１２\u{2011}0000\u{00A0}1234").as_str(),
            "0812-0000 1234"
        );
        assert_eq!(
            normalize("3299\u{200B}9945\u{FEFF}0190\u{00AD}0001").as_str(),
            "3299994501900001"
        );
        assert_eq!(normalize("Ｎａｍａ：Budi").as_str(), "Nama:Budi");
    }

    #[test]
    fn other_text_is_unchanged() {
        let text = "Keluhan: demam — 3 hari, nyeri ±2 cm. Ünïcödé 日本語";
        assert_eq!(normalize(text).as_str(), text);
    }
}
