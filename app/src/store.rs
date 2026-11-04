//! Store Nuenly di disk (F-28, ADR 0013): `nuenly.db` terenkripsi SQLCipher di folder data
//! lokal aplikasi. Kuncinya ada di `nuenly.key`, terkunci DPAPI untuk akun Windows ini.

use std::io;
use std::path::Path;
use std::sync::Arc;

use anyhow::Context as _;
use nuenly_store::{AuditEntry, Store, StoreError, StoreKey};

pub const DB_FILE: &str = "nuenly.db";
pub const KEY_FILE: &str = "nuenly.key";
/// Akhiran untuk store dan kunci yang tidak bisa dipakai lagi. Disingkirkan, bukan dihapus,
/// dan hanya pasangan terakhir yang disimpan.
const UNUSABLE_SUFFIX: &str = "tidak-terbaca";
/// Memisahkan blob DPAPI kunci store dari blob lain milik akun yang sama.
const KEY_PURPOSE: &[u8] = b"nuenly-store-key-v1";

/// Membuka store di `dir`, atau membuat store dan kunci baru bila belum ada.
///
/// Store yang tidak bisa dibuka lagi disingkirkan bersama kuncinya, lalu diganti store kosong.
/// Ini terjadi bila kuncinya hilang, dibuat akun Windows lain, atau tidak cocok. Setelan kembali
/// ke bawaan, jadi onboarding muncul lagi.
///
/// # Errors
///
/// Folder atau berkas tidak bisa dibaca atau ditulis, atau store berasal dari versi Nuenly yang
/// lebih baru. Store tidak diubah dalam kasus ini.
pub fn open(dir: &Path) -> anyhow::Result<Arc<Store>> {
    std::fs::create_dir_all(dir).context("folder data tidak bisa dibuat")?;
    let db = dir.join(DB_FILE);
    let key_file = dir.join(KEY_FILE);

    if let Some(key) = saved_key(&key_file)? {
        match Store::open(&db, &key) {
            Err(StoreError::Unreadable) => {
                tracing::warn!("store tidak cocok dengan kuncinya dan dibuat ulang");
            }
            result => return result.map(Arc::new).context("store tidak bisa dibuka"),
        }
    }
    // Tanpa kunci yang cocok, isi store lama tidak bisa dibaca lagi.
    for file in [&db, &key_file] {
        if file.exists() {
            let mut unusable = file.as_os_str().to_owned();
            unusable.push(format!(".{UNUSABLE_SUFFIX}"));
            std::fs::rename(file, unusable).context("store lama tidak bisa disingkirkan")?;
        }
    }

    let key = StoreKey::generate()?;
    save_key(&key_file, &key)?;
    Store::open(&db, &key)
        .map(Arc::new)
        .context("store baru tidak bisa dibuat")
}

/// Mencatat kejadian audit di thread blocking tanpa menunggu. Gagal mencatat tidak menggagalkan
/// fitur yang dicatat, hanya meninggalkan peringatan di log.
pub fn record(store: &Arc<Store>, entry: AuditEntry) {
    let store = Arc::clone(store);
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(err) = store.record(&entry) {
            tracing::warn!(error = %err, "audit tidak tersimpan");
        }
    });
}

/// Kunci tersimpan, atau `None` bila belum ada atau tidak bisa dibuka DPAPI.
///
/// # Errors
///
/// Berkas kunci ada tetapi tidak bisa dibaca, misalnya sedang dikunci program lain. Store
/// dibiarkan, agar kunci yang masih benar tidak tertimpa.
fn saved_key(path: &Path) -> io::Result<Option<StoreKey>> {
    let blob = match std::fs::read(path) {
        Ok(blob) => blob,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };
    let key = unprotect(&blob)
        .map_err(|err| tracing::warn!(error = %err, "kunci store tidak bisa dibuka DPAPI"))
        .ok()
        .and_then(|bytes| StoreKey::from_bytes(&bytes).ok());
    Ok(key)
}

/// Menulis kunci terkunci DPAPI lewat berkas sementara, agar tidak tertinggal setengah.
fn save_key(path: &Path, key: &StoreKey) -> anyhow::Result<()> {
    let blob = protect(key.as_bytes()).context("kunci store tidak bisa dikunci DPAPI")?;
    let temp = path.with_extension("key.tmp");
    std::fs::write(&temp, blob).context("kunci store tidak bisa disimpan")?;
    std::fs::rename(&temp, path).context("kunci store tidak bisa disimpan")
}

#[cfg(windows)]
fn protect(key: &[u8]) -> io::Result<Vec<u8>> {
    nuenly_platform_windows::dpapi::protect(key, KEY_PURPOSE)
}

#[cfg(windows)]
fn unprotect(blob: &[u8]) -> io::Result<zeroize::Zeroizing<Vec<u8>>> {
    nuenly_platform_windows::dpapi::unprotect(blob, KEY_PURPOSE)
}

// Tanpa DPAPI kunci tidak bisa dilindungi, jadi store tidak dibuat sama sekali.
#[cfg(not(windows))]
fn protect(_key: &[u8]) -> io::Result<Vec<u8>> {
    Err(io::ErrorKind::Unsupported.into())
}

#[cfg(not(windows))]
fn unprotect(_blob: &[u8]) -> io::Result<zeroize::Zeroizing<Vec<u8>>> {
    Err(io::ErrorKind::Unsupported.into())
}

#[cfg(all(test, windows))]
mod tests {
    use std::path::PathBuf;
    use std::time::Duration;

    use super::{DB_FILE, KEY_FILE, UNUSABLE_SUFFIX, open};

    /// Folder uji sementara yang dihapus setelah test selesai.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir()
                .join(format!("nuenly-app-store-{}-{name}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            Self(dir)
        }

        fn unusable(&self, file: &str) -> PathBuf {
            self.0.join(format!("{file}.{UNUSABLE_SUFFIX}"))
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn retention_matches_the_privacy_notice() {
        // `READS` di `app/ui/src/lib/privacy.ts` menjanjikan catatan dihapus setelah 30 hari.
        assert_eq!(nuenly_store::RETENTION, Duration::from_hours(30 * 24));
    }

    #[test]
    fn the_store_reopens_with_its_saved_key() {
        let dir = TempDir::new("buka-lagi");
        let store = open(&dir.0).unwrap();
        store.set_setting("uji", "nilai sintetis").unwrap();
        drop(store);

        let store = open(&dir.0).unwrap();
        assert_eq!(
            store.setting::<String>("uji").unwrap().as_deref(),
            Some("nilai sintetis")
        );
        // Kunci di disk terkunci DPAPI, bukan 32 byte mentah.
        assert_ne!(std::fs::read(dir.0.join(KEY_FILE)).unwrap().len(), 32);
    }

    #[test]
    fn a_store_without_its_key_is_set_aside() {
        let dir = TempDir::new("tanpa-kunci");
        let store = open(&dir.0).unwrap();
        store.set_setting("uji", "nilai sintetis").unwrap();
        drop(store);
        std::fs::remove_file(dir.0.join(KEY_FILE)).unwrap();

        let store = open(&dir.0).unwrap();
        assert_eq!(store.setting::<String>("uji").unwrap(), None);
        assert!(dir.unusable(DB_FILE).exists());
    }

    #[test]
    fn a_damaged_key_sets_both_files_aside() {
        let dir = TempDir::new("kunci-rusak");
        drop(open(&dir.0).unwrap());
        std::fs::write(dir.0.join(KEY_FILE), b"bukan blob DPAPI").unwrap();

        drop(open(&dir.0).unwrap());
        assert!(dir.unusable(DB_FILE).exists());
        assert!(dir.unusable(KEY_FILE).exists());
        assert!(dir.0.join(DB_FILE).exists());
    }
}
