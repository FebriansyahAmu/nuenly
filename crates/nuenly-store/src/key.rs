//! Kunci store: 32 byte acak yang dihapus dari memori saat di-drop.

use std::fmt;
use std::fmt::Write as _;

use zeroize::Zeroizing;

use crate::StoreError;

/// Panjang kunci SQLCipher mentah (AES-256).
pub const KEY_LEN: usize = 32;

/// Kunci enkripsi store. Tidak pernah tercetak di log: `Debug` hanya menulis nama tipenya.
pub struct StoreKey(Zeroizing<[u8; KEY_LEN]>);

impl StoreKey {
    /// Kunci baru dari generator acak sistem operasi.
    ///
    /// # Errors
    ///
    /// [`StoreError::Random`] bila generator acak tidak tersedia.
    pub fn generate() -> Result<Self, StoreError> {
        let mut key = Zeroizing::new([0; KEY_LEN]);
        getrandom::fill(key.as_mut_slice()).map_err(|_| StoreError::Random)?;
        Ok(Self(key))
    }

    /// Kunci yang tersimpan, misalnya setelah dibuka DPAPI.
    ///
    /// # Errors
    ///
    /// [`StoreError::KeyLength`] bila panjangnya bukan [`KEY_LEN`].
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, StoreError> {
        if bytes.len() != KEY_LEN {
            return Err(StoreError::KeyLength);
        }
        let mut key = Zeroizing::new([0; KEY_LEN]);
        key.copy_from_slice(bytes);
        Ok(Self(key))
    }

    /// Byte kunci, untuk dikunci DPAPI sebelum disimpan.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_slice()
    }

    /// `PRAGMA key` dengan kunci mentah (`x'…'`), jadi SQLCipher tidak menurunkannya lagi lewat
    /// PBKDF2. Kunci acak 256 bit tidak butuh penurunan, dan membuka store tetap cepat.
    pub(crate) fn pragma(&self) -> Zeroizing<String> {
        // Kapasitas cukup sejak awal agar tidak ada salinan lama yang tertinggal saat realokasi.
        let mut sql = Zeroizing::new(String::with_capacity(2 * KEY_LEN + 32));
        sql.push_str("PRAGMA key = \"x'");
        for byte in self.0.iter() {
            let _ = write!(sql, "{byte:02x}");
        }
        sql.push_str("'\";");
        sql
    }
}

impl fmt::Debug for StoreKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("StoreKey(..)")
    }
}

#[cfg(test)]
mod tests {
    use super::{KEY_LEN, StoreKey};
    use crate::StoreError;

    #[test]
    fn generated_keys_differ_and_round_trip() {
        let first = StoreKey::generate().unwrap();
        let second = StoreKey::generate().unwrap();
        assert_ne!(first.as_bytes(), second.as_bytes());

        let restored = StoreKey::from_bytes(first.as_bytes()).unwrap();
        assert_eq!(restored.as_bytes(), first.as_bytes());
    }

    #[test]
    fn keys_of_the_wrong_length_are_rejected() {
        assert!(matches!(
            StoreKey::from_bytes(&[7; KEY_LEN - 1]),
            Err(StoreError::KeyLength)
        ));
    }

    #[test]
    fn the_key_never_appears_in_debug_output() {
        let key = StoreKey::from_bytes(&[0xab; KEY_LEN]).unwrap();
        assert_eq!(format!("{key:?}"), "StoreKey(..)");
        assert!(key.pragma().starts_with("PRAGMA key = \"x'abab"));
        assert!(key.pragma().ends_with("ab'\";"));
    }
}
