//! Pembungkus untuk teks yang dibaca dari layar.

use std::fmt;

use zeroize::Zeroizing;

/// Teks yang berasal dari aplikasi lain: isi field, teks terpilih, judul jendela, atau label.
///
/// Isinya tidak pernah tampil lewat `Debug`, dan tipe ini sengaja tidak mengimplementasikan
/// `Display`, jadi teks pengguna tidak bisa masuk log atau pesan error tanpa sengaja.
/// Buffernya dihapus (zeroize) saat di-drop.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct SensitiveText(Zeroizing<String>);

impl SensitiveText {
    /// Membungkus teks dari layar.
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self(Zeroizing::new(text.into()))
    }

    /// Isi teks. Jangan teruskan hasilnya ke log, `tracing`, atau pesan error.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// `true` bila teksnya kosong.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Jumlah karakter Unicode.
    #[must_use]
    pub fn char_count(&self) -> usize {
        self.0.chars().count()
    }
}

impl fmt::Debug for SensitiveText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SensitiveText(<{} karakter>)", self.char_count())
    }
}

#[cfg(test)]
mod tests {
    use super::SensitiveText;

    #[test]
    fn debug_hides_content() {
        let text = SensitiveText::new("rahasia sintetis");
        let debug = format!("{text:?}");
        assert!(!debug.contains("rahasia"));
        assert_eq!(debug, "SensitiveText(<16 karakter>)");
    }

    #[test]
    fn expose_returns_content() {
        let text = SensitiveText::new("teks sintetis");
        assert_eq!(text.expose(), "teks sintetis");
        assert!(!text.is_empty());
    }
}
