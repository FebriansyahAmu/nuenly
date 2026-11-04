//! [`Store`]: satu koneksi SQLCipher di balik mutex. Semua panggilannya blocking dan singkat;
//! pemanggil async membungkusnya dengan `spawn_blocking`.

use std::path::Path;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::SystemTime;

use rusqlite::{Connection, ErrorCode, OptionalExtension as _};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::audit::{from_millis, millis};
use crate::{AuditEntry, RETENTION, StoreError, StoreKey};

/// Skema per versi. Store dengan `PRAGMA user_version` = n sudah menjalankan n migrasi
/// pertama. Migrasi yang sudah dirilis tidak boleh diubah; perubahan skema selalu menjadi
/// migrasi baru.
const MIGRATIONS: &[&str] = &[include_str!("../migrations/0001_awal.sql")];

pub struct Store {
    conn: Mutex<Connection>,
}

impl Store {
    /// Membuka store di `path`, atau membuat yang baru bila belum ada. Skemanya diperbarui dan
    /// audit yang melewati [`RETENTION`] dibuang.
    ///
    /// # Errors
    ///
    /// [`StoreError::Unreadable`] bila kunci salah atau berkasnya bukan store;
    /// [`StoreError::Newer`] bila skemanya dari versi Nuenly yang lebih baru.
    pub fn open(path: &Path, key: &StoreKey) -> Result<Self, StoreError> {
        let mut conn = Connection::open(path)?;
        // Log bawaan SQLCipher ditulis langsung ke stderr, di luar `tracing`. Kegagalan sudah
        // dilaporkan lewat `StoreError`.
        conn.execute_batch("PRAGMA cipher_log_level = NONE;")?;
        conn.execute_batch(&key.pragma())?;
        // Kunci yang salah baru ketahuan saat halaman pertama dibaca.
        conn.query_row("SELECT count(*) FROM sqlite_schema", [], |_| Ok(()))
            .map_err(|err| match err.sqlite_error_code() {
                Some(ErrorCode::NotADatabase) => StoreError::Unreadable,
                _ => err.into(),
            })?;
        // Baris yang dihapus ditimpa nol, jadi retensi benar-benar membuang isinya.
        conn.execute_batch("PRAGMA secure_delete = ON;")?;
        migrate(&mut conn)?;

        let store = Self {
            conn: Mutex::new(conn),
        };
        store.prune(SystemTime::now())?;
        Ok(store)
    }

    /// Setelan `name`, atau `None` bila belum pernah disimpan.
    ///
    /// # Errors
    ///
    /// Error SQLite, atau [`StoreError::Json`] bila isinya tidak cocok dengan `T`.
    pub fn setting<T: DeserializeOwned>(&self, name: &str) -> Result<Option<T>, StoreError> {
        let json: Option<String> = self
            .lock()
            .query_row(
                "SELECT value FROM settings WHERE name = ?1",
                [name],
                |row| row.get(0),
            )
            .optional()?;
        Ok(json.map(|json| serde_json::from_str(&json)).transpose()?)
    }

    /// Menyimpan setelan `name`, menggantikan nilai sebelumnya.
    ///
    /// # Errors
    ///
    /// Error SQLite, misalnya disk penuh.
    pub fn set_setting<T: Serialize + ?Sized>(
        &self,
        name: &str,
        value: &T,
    ) -> Result<(), StoreError> {
        let json = serde_json::to_string(value)?;
        self.lock().execute(
            "INSERT INTO settings (name, value) VALUES (?1, ?2) \
             ON CONFLICT (name) DO UPDATE SET value = excluded.value",
            (name, json),
        )?;
        Ok(())
    }

    /// Mencatat satu kejadian, sekaligus membuang catatan yang lebih tua dari [`RETENTION`]
    /// dihitung dari waktu kejadian itu.
    ///
    /// # Errors
    ///
    /// Error SQLite; catatan tidak tersimpan.
    pub fn record(&self, entry: &AuditEntry) -> Result<(), StoreError> {
        let event = serde_json::to_string(&entry.event)?;
        let mut conn = self.lock();
        let transaction = conn.transaction()?;
        transaction.execute(
            "INSERT INTO audit (at, app, mode, event) VALUES (?1, ?2, ?3, ?4)",
            (millis(entry.at), entry.app.as_deref(), &entry.mode, event),
        )?;
        transaction.execute("DELETE FROM audit WHERE at < ?1", [cutoff(entry.at)])?;
        transaction.commit()?;
        Ok(())
    }

    /// Catatan audit, yang terbaru lebih dulu, paling banyak `limit`.
    ///
    /// # Errors
    ///
    /// Error SQLite, atau [`StoreError::Json`] bila ada kejadian yang tidak dikenal versi ini.
    pub fn recent(&self, limit: u32) -> Result<Vec<AuditEntry>, StoreError> {
        let conn = self.lock();
        let mut statement = conn
            .prepare("SELECT at, app, mode, event FROM audit ORDER BY at DESC, id DESC LIMIT ?1")?;
        let rows = statement.query_map([limit], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        rows.map(|row| {
            let (at, app, mode, event) = row?;
            Ok(AuditEntry {
                at: from_millis(at),
                app,
                mode,
                event: serde_json::from_str(&event)?,
            })
        })
        .collect()
    }

    /// Membuang catatan audit yang lebih tua dari [`RETENTION`] pada waktu `now`. Hasilnya
    /// jumlah catatan yang dibuang.
    ///
    /// # Errors
    ///
    /// Error SQLite.
    pub fn prune(&self, now: SystemTime) -> Result<usize, StoreError> {
        Ok(self
            .lock()
            .execute("DELETE FROM audit WHERE at < ?1", [cutoff(now)])?)
    }

    fn lock(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Batas waktu audit yang disimpan pada waktu `now`, dalam milidetik.
fn cutoff(now: SystemTime) -> i64 {
    now.checked_sub(RETENTION).map_or(0, millis)
}

/// Versi skema terbaru yang dikenal versi ini.
fn latest() -> u32 {
    u32::try_from(MIGRATIONS.len()).unwrap_or(u32::MAX)
}

/// Menjalankan migrasi yang belum dijalankan, masing-masing dalam satu transaksi.
fn migrate(conn: &mut Connection) -> Result<(), StoreError> {
    let done: u32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if done > latest() {
        return Err(StoreError::Newer(done));
    }
    for (version, sql) in (1..).zip(MIGRATIONS).filter(|(version, _)| *version > done) {
        let transaction = conn.transaction()?;
        transaction.execute_batch(sql)?;
        transaction.pragma_update(None, "user_version", version)?;
        transaction.commit()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime};

    use super::{Store, latest};
    use crate::{AskOutcome, AuditEntry, AuditEvent, KEY_LEN, RETENTION, StoreError, StoreKey};

    /// Folder uji sementara yang dihapus setelah test selesai.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("nuenly-store-{}-{name}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn file(&self) -> PathBuf {
            self.0.join("nuenly.db")
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn key(byte: u8) -> StoreKey {
        StoreKey::from_bytes(&[byte; KEY_LEN]).unwrap()
    }

    fn ask(at: SystemTime, app: &str) -> AuditEntry {
        AuditEntry {
            at,
            app: Some(app.to_owned()),
            mode: "all".to_owned(),
            event: AuditEvent::Ask {
                model: "model-sintetis:1b".to_owned(),
                context_chars: 42,
                outcome: AskOutcome::Answered,
            },
        }
    }

    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        haystack
            .windows(needle.len())
            .any(|window| window == needle)
    }

    #[test]
    fn settings_survive_reopening() {
        let dir = TempDir::new("setelan");
        let store = Store::open(&dir.file(), &key(1)).unwrap();
        assert_eq!(store.setting::<String>("model").unwrap(), None);

        store.set_setting("model", "lama:1b").unwrap();
        store.set_setting("model", "baru:4b").unwrap();
        drop(store);

        let store = Store::open(&dir.file(), &key(1)).unwrap();
        assert_eq!(
            store.setting::<String>("model").unwrap().as_deref(),
            Some("baru:4b")
        );
    }

    #[test]
    fn the_file_on_disk_is_encrypted() {
        let dir = TempDir::new("enkripsi");
        let store = Store::open(&dir.file(), &key(2)).unwrap();
        store
            .set_setting("penanda", "penanda-setelan-sintetis")
            .unwrap();
        store
            .record(&ask(SystemTime::now(), "penanda-aplikasi.exe"))
            .unwrap();
        drop(store);

        let bytes = std::fs::read(dir.file()).unwrap();
        assert!(!bytes.starts_with(b"SQLite format 3"));
        assert!(!contains(&bytes, b"penanda-setelan-sintetis"));
        assert!(!contains(&bytes, b"penanda-aplikasi.exe"));
    }

    #[test]
    fn a_wrong_key_or_a_foreign_file_is_unreadable() {
        let dir = TempDir::new("kunci-salah");
        drop(Store::open(&dir.file(), &key(3)).unwrap());
        assert!(matches!(
            Store::open(&dir.file(), &key(4)),
            Err(StoreError::Unreadable)
        ));

        let other = dir.0.join("lain.db");
        std::fs::write(&other, b"bukan database Nuenly, hanya teks sintetis biasa").unwrap();
        assert!(matches!(
            Store::open(&other, &key(3)),
            Err(StoreError::Unreadable)
        ));
    }

    #[test]
    fn a_schema_from_a_newer_version_is_refused() {
        let dir = TempDir::new("skema-baru");
        let store = Store::open(&dir.file(), &key(5)).unwrap();
        let newer = latest() + 1;
        store
            .lock()
            .pragma_update(None, "user_version", newer)
            .unwrap();
        drop(store);

        assert!(matches!(
            Store::open(&dir.file(), &key(5)),
            Err(StoreError::Newer(version)) if version == newer
        ));
    }

    #[test]
    fn audit_lists_the_newest_first() {
        let dir = TempDir::new("audit");
        let store = Store::open(&dir.file(), &key(6)).unwrap();
        let now = SystemTime::UNIX_EPOCH + Duration::from_hours(500_000);
        for (offset, app) in [(0, "satu.exe"), (2, "tiga.exe"), (1, "dua.exe")] {
            store
                .record(&ask(now + Duration::from_secs(offset), app))
                .unwrap();
        }

        let recent = store.recent(2).unwrap();
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0], ask(now + Duration::from_secs(2), "tiga.exe"));
        assert_eq!(recent[1].app.as_deref(), Some("dua.exe"));
    }

    #[test]
    fn audit_older_than_the_retention_is_dropped() {
        let dir = TempDir::new("retensi");
        let store = Store::open(&dir.file(), &key(7)).unwrap();
        let now = SystemTime::now();
        let expired = now - RETENTION - Duration::from_secs(60);
        store.record(&ask(expired, "lama.exe")).unwrap();
        assert_eq!(store.recent(10).unwrap().len(), 1);

        // Catatan baru membuang yang lama.
        store.record(&ask(now, "baru.exe")).unwrap();
        let recent = store.recent(10).unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].app.as_deref(), Some("baru.exe"));

        // Membuka store juga membuang yang lama.
        store.record(&ask(expired, "lama.exe")).unwrap();
        drop(store);
        let store = Store::open(&dir.file(), &key(7)).unwrap();
        assert_eq!(store.recent(10).unwrap().len(), 1);
    }
}
