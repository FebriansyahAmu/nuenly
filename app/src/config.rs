//! Setelan yang bertahan antar-sesi, disimpan di store (`nuenly-store`) sebagai setelan
//! `config`. Isinya tidak pernah memuat teks pengguna.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use nuenly_store::{Store, StoreError};
use serde::{Deserialize, Serialize};

/// Berkas setelan sebelum ada store, di folder konfigurasi aplikasi. Dipindahkan ke store saat
/// pertama kali dibuka.
pub const LEGACY_FILE: &str = "config.json";
/// Nama setelan di store.
const SETTING: &str = "config";

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    /// Versi pemberitahuan privasi yang disetujui di onboarding; 0 berarti belum pernah.
    pub privacy_accepted: u32,
    /// Model Ollama pilihan pengguna; `None` berarti dipilih otomatis sesuai kemampuan PC.
    pub model: Option<String>,
    /// Pintasan Tanya Nuenly, misalnya `Alt+Space`; `None` berarti bawaan (Ctrl+Space).
    pub hotkey: Option<String>,
    /// Mode pilihan pengguna per aplikasi: nama executable → ID mode, misalnya
    /// `"klinik.exe": "medical"`. Aplikasi lain memakai mode pertama.
    pub app_modes: BTreeMap<String, String>,
    /// Saran saat mengetik (US-2).
    pub suggest: SuggestConfig,
}

/// Pilihan saran yang boleh dipakai pengguna, per jam. Yang terbesar sama dengan batas PRD.
pub const SUGGEST_LIMITS: &[u32] = &[1, 3, 6];

/// Setelan saran saat mengetik. Mode pack tetap bisa mematikan saran atau membatasinya lebih
/// rendah.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SuggestConfig {
    pub enabled: bool,
    /// Saran paling banyak per jam, salah satu dari [`SUGGEST_LIMITS`].
    pub per_hour: u32,
}

impl Default for SuggestConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            per_hour: 6,
        }
    }
}

impl SuggestConfig {
    /// Batas per jam yang berlaku dengan batas mode pack `pack_limit`. Nilai tersimpan yang tidak
    /// dikenal memakai pilihan terkecil.
    pub fn limit(self, pack_limit: u32) -> u32 {
        let own = if SUGGEST_LIMITS.contains(&self.per_hour) {
            self.per_hour
        } else {
            SUGGEST_LIMITS[0]
        };
        own.min(pack_limit)
    }
}

/// State terkelola Tauri: setelan di memori beserta store tempatnya disimpan.
pub struct ConfigStore {
    store: Arc<Store>,
    current: Mutex<Config>,
}

impl ConfigStore {
    /// Memuat setelan dari store. Bila store belum punya setelan, isi `legacy` (`config.json`
    /// dari versi sebelum store) dipindahkan ke store lalu berkasnya dihapus. Setelan yang
    /// tidak bisa dibaca menjadi setelan bawaan, jadi onboarding muncul lagi dan persetujuan
    /// diminta ulang.
    pub fn load(store: Arc<Store>, legacy: &Path) -> Self {
        let current = match store.setting::<Config>(SETTING) {
            Ok(Some(config)) => config,
            Ok(None) => import(&store, legacy),
            Err(err) => {
                tracing::warn!(error = %err, "setelan tidak bisa dibaca; memakai bawaan");
                Config::default()
            }
        };
        Self {
            store,
            current: Mutex::new(current),
        }
    }

    pub fn get(&self) -> Config {
        self.lock().clone()
    }

    /// Mengubah setelan lalu menyimpannya. Perubahan baru berlaku di memori setelah tersimpan.
    pub fn update(&self, change: impl FnOnce(&mut Config)) -> Result<Config, StoreError> {
        let mut current = self.lock();
        let mut next = current.clone();
        change(&mut next);
        self.store.set_setting(SETTING, &next)?;
        current.clone_from(&next);
        Ok(next)
    }

    fn lock(&self) -> MutexGuard<'_, Config> {
        self.current.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Setelan dari `config.json` lama. Berkasnya dihapus setelah isinya tersimpan di store; yang
/// tidak bisa dibaca dianggap setelan bawaan.
fn import(store: &Store, legacy: &Path) -> Config {
    let Ok(bytes) = std::fs::read(legacy) else {
        return Config::default();
    };
    let config = serde_json::from_slice(&bytes).unwrap_or_default();
    match store.set_setting(SETTING, &config) {
        Ok(()) => {
            let _ = std::fs::remove_file(legacy);
        }
        Err(err) => tracing::warn!(error = %err, "setelan lama tidak bisa dipindahkan"),
    }
    config
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use nuenly_store::{KEY_LEN, Store, StoreKey};

    use super::{Config, ConfigStore, LEGACY_FILE, SuggestConfig};

    /// Folder uji sementara berisi store yang dihapus setelah test selesai.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("nuenly-config-{}-{name}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn store(&self) -> Arc<Store> {
            let key = StoreKey::from_bytes(&[9; KEY_LEN]).unwrap();
            Arc::new(Store::open(&self.0.join("nuenly.db"), &key).unwrap())
        }

        fn legacy(&self) -> PathBuf {
            self.0.join(LEGACY_FILE)
        }

        fn load(&self) -> ConfigStore {
            ConfigStore::load(self.store(), &self.legacy())
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn nothing_is_accepted_on_a_new_store() {
        let dir = TempDir::new("kosong");
        assert_eq!(dir.load().get(), Config::default());
    }

    #[test]
    fn saved_changes_survive_a_restart() {
        let dir = TempDir::new("simpan");
        let config = dir.load();

        config.update(|config| config.privacy_accepted = 1).unwrap();
        assert_eq!(config.get().privacy_accepted, 1);
        drop(config);

        assert_eq!(dir.load().get().privacy_accepted, 1);
    }

    #[test]
    fn the_old_config_file_moves_into_the_store() {
        let dir = TempDir::new("pindah");
        std::fs::write(
            dir.legacy(),
            br#"{"privacyAccepted": 3, "model": "gemma3:4b", "fiturBaru": true}"#,
        )
        .unwrap();

        let config = dir.load().get();
        assert_eq!(config.privacy_accepted, 3);
        assert_eq!(config.model.as_deref(), Some("gemma3:4b"));
        assert!(!dir.legacy().exists());
        assert_eq!(dir.load().get(), config);
    }

    #[test]
    fn suggestions_are_on_by_default_and_never_exceed_the_mode_pack() {
        let config: Config = serde_json::from_str(r#"{"privacyAccepted": 4}"#).unwrap();
        assert_eq!(config.suggest, SuggestConfig::default());
        assert!(config.suggest.enabled);
        assert_eq!(config.suggest.limit(6), 6);
        assert_eq!(config.suggest.limit(2), 2);

        let fewer = SuggestConfig {
            enabled: true,
            per_hour: 3,
        };
        assert_eq!(fewer.limit(6), 3);
        // Nilai yang tidak ditawarkan di Pengaturan, misalnya hasil edit tangan.
        let odd = SuggestConfig {
            enabled: true,
            per_hour: 500,
        };
        assert_eq!(odd.limit(6), 1);
    }

    #[test]
    fn a_corrupt_old_file_falls_back_to_defaults() {
        let dir = TempDir::new("rusak");
        std::fs::write(dir.legacy(), b"{\"privacyAccepted\": 1").unwrap();

        assert_eq!(dir.load().get(), Config::default());
        assert!(!dir.legacy().exists());
    }
}
