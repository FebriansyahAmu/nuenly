//! Format teks layar untuk output: disamarkan kecuali diminta.

use nuenly_core::SensitiveText;

/// Panjang maksimum teks yang dicetak per field, dalam karakter.
const MAX_SHOWN_CHARS: usize = 120;

/// `<n karakter>` bila `reveal` false. Selain itu teks dalam tanda kutip, dengan baris baru dan
/// tanda kutip di-escape, dipotong di [`MAX_SHOWN_CHARS`].
pub fn text(value: &SensitiveText, reveal: bool) -> String {
    let count = value.char_count();
    if !reveal {
        return format!("<{count} karakter>");
    }
    let shown: String = value.expose().chars().take(MAX_SHOWN_CHARS).collect();
    let suffix = if count > MAX_SHOWN_CHARS { "…" } else { "" };
    format!("{shown:?}{suffix}")
}

#[cfg(test)]
mod tests {
    use nuenly_core::SensitiveText;

    use super::{MAX_SHOWN_CHARS, text};

    #[test]
    fn hides_text_by_default() {
        let value = SensitiveText::new("Pasien sintetis");
        assert_eq!(text(&value, false), "<15 karakter>");
    }

    #[test]
    fn reveal_escapes_quotes_and_newlines() {
        let value = SensitiveText::new("baris \"satu\"\nbaris dua");
        assert_eq!(text(&value, true), r#""baris \"satu\"\nbaris dua""#);
    }

    #[test]
    fn reveal_truncates_long_text() {
        let value = SensitiveText::new("a".repeat(MAX_SHOWN_CHARS + 5));
        let shown = text(&value, true);
        assert!(shown.ends_with('…'));
        assert_eq!(shown.matches('a').count(), MAX_SHOWN_CHARS);
    }
}
