//! Setelan yang bertahan antar-sesi, disimpan sebagai `config.json` di folder konfigurasi
//! aplikasi. Isinya tidak pernah memuat teks pengguna.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use serde::{Deserialize, Serialize};

pub const FILE_NAME: &str = "config.json";

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    /// Versi pemberitahuan privasi yang disetujui di onboarding; 0 berarti belum pernah.
    pub privacy_accepted: u32,
    /// Model Ollama pilihan pengguna; `None` berarti dipilih otomatis sesuai kemampuan PC.
    pub model: Option<String>,
    /// Pintasan Tanya Nuenly, misalnya `Alt+Space`; `None` berarti bawaan (Ctrl+Space).
    pub hotkey: Option<String>,
}

/// State terkelola Tauri: setelan di memori beserta lokasinya di disk.
pub struct ConfigStore {
    path: PathBuf,
    current: Mutex<Config>,
}

impl ConfigStore {
    /// Memuat setelan dari `path`. Berkas yang tidak ada atau rusak dibaca sebagai setelan
    /// bawaan, jadi onboarding muncul lagi dan persetujuan diminta ulang.
    pub fn load(path: PathBuf) -> Self {
        let current = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self {
            path,
            current: Mutex::new(current),
        }
    }

    pub fn get(&self) -> Config {
        self.lock().clone()
    }

    /// Mengubah setelan lalu menyimpannya. Perubahan baru berlaku di memori setelah tersimpan.
    pub fn update(&self, change: impl FnOnce(&mut Config)) -> io::Result<Config> {
        let mut current = self.lock();
        let mut next = current.clone();
        change(&mut next);
        save(&self.path, &next)?;
        current.clone_from(&next);
        Ok(next)
    }

    fn lock(&self) -> MutexGuard<'_, Config> {
        self.current.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Menulis ke berkas sementara lalu mengganti nama, agar `config.json` tidak tertinggal
/// setengah tertulis bila proses berhenti di tengah jalan.
fn save(path: &Path, config: &Config) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let json = serde_json::to_vec_pretty(config).map_err(io::Error::other)?;
    let temp = path.with_extension("json.tmp");
    std::fs::write(&temp, json)?;
    std::fs::rename(&temp, path)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{Config, ConfigStore, FILE_NAME};

    /// Folder uji sementara yang dihapus setelah test selesai.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("nuenly-config-{}-{name}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            Self(dir)
        }

        fn file(&self) -> PathBuf {
            self.0.join(FILE_NAME)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn nothing_is_accepted_without_a_file() {
        let dir = TempDir::new("kosong");
        assert_eq!(ConfigStore::load(dir.file()).get(), Config::default());
    }

    #[test]
    fn saved_changes_survive_a_restart() {
        let dir = TempDir::new("simpan");
        let store = ConfigStore::load(dir.file());

        store.update(|config| config.privacy_accepted = 1).unwrap();

        assert_eq!(store.get().privacy_accepted, 1);
        assert_eq!(ConfigStore::load(dir.file()).get().privacy_accepted, 1);
        assert!(!dir.file().with_extension("json.tmp").exists());
    }

    #[test]
    fn a_corrupt_file_falls_back_to_defaults() {
        let dir = TempDir::new("rusak");
        std::fs::create_dir_all(&dir.0).unwrap();
        std::fs::write(dir.file(), b"{\"privacyAccepted\": 1").unwrap();

        assert_eq!(ConfigStore::load(dir.file()).get(), Config::default());
    }

    #[test]
    fn unknown_fields_from_a_newer_version_are_ignored() {
        let dir = TempDir::new("baru");
        std::fs::create_dir_all(&dir.0).unwrap();
        std::fs::write(dir.file(), br#"{"privacyAccepted": 2, "fiturBaru": true}"#).unwrap();

        assert_eq!(ConfigStore::load(dir.file()).get().privacy_accepted, 2);
    }

    #[test]
    fn a_failed_save_leaves_the_setting_unchanged() {
        let dir = TempDir::new("gagal");
        // Folder dengan nama berkas setelan membuat penyimpanan gagal.
        std::fs::create_dir_all(dir.file()).unwrap();
        let store = ConfigStore::load(dir.file());

        assert!(store.update(|config| config.privacy_accepted = 1).is_err());
        assert_eq!(store.get().privacy_accepted, 0);
    }
}
