//! Jendela Pengaturan (F-27): data yang ditampilkan dan perubahan yang disimpan di `config.json`.

use serde::Serialize;
use tauri::State;

use crate::config::{Config, ConfigStore};
use crate::hotkey::{HotkeyState, HotkeyStatus};

/// Pintasan Tanya Nuenly. Tetap sampai hotkey bisa diubah (F-03).
const HOTKEY: &str = "Ctrl+Space";
const MAX_MODEL_NAME: usize = 200;

/// Bentuknya sama dengan `SettingsView` di `app/ui/src/windows/settings/backend.ts`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    /// Model pilihan pengguna; `None` berarti dipilih otomatis sesuai kemampuan PC.
    pub model: Option<String>,
    pub hotkey: &'static str,
    pub hotkey_status: HotkeyStatus,
    pub ollama_address: String,
    /// Aplikasi yang tidak pernah dibaca, apa pun modenya.
    pub blocked_apps: &'static [&'static str],
}

fn view(config: &Config, hotkey: HotkeyStatus) -> SettingsView {
    SettingsView {
        model: config.model.clone(),
        hotkey: HOTKEY,
        hotkey_status: hotkey,
        ollama_address: nuenly_llm::ollama::DEFAULT_ADDRESS.to_string(),
        blocked_apps: nuenly_core::ALWAYS_BLOCKED,
    }
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri menyuntikkan state sebagai nilai.
pub fn settings_get(store: State<'_, ConfigStore>, hotkey: State<'_, HotkeyState>) -> SettingsView {
    view(&store.get(), hotkey.status)
}

/// Menyimpan model pilihan; `None` kembali ke pilihan otomatis.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Argumen command Tauri selalu berupa nilai.
pub fn settings_set_model(
    store: State<'_, ConfigStore>,
    hotkey: State<'_, HotkeyState>,
    model: Option<String>,
) -> Result<SettingsView, String> {
    if model.as_deref().is_some_and(|name| !valid_model_name(name)) {
        return Err("Nama model tidak dikenal.".to_owned());
    }
    store
        .update(|config| config.model = model)
        .map(|config| view(&config, hotkey.status))
        .map_err(|_| "Pilihan model tidak bisa disimpan. Coba lagi.".to_owned())
}

/// Nama model Ollama, misalnya `gpt-oss:20b` atau `hf.co/org/model:Q4_K_M`.
fn valid_model_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_MODEL_NAME
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._:/-".contains(&b))
}

#[cfg(test)]
mod tests {
    use super::{valid_model_name, view};
    use crate::config::Config;
    use crate::hotkey::HotkeyStatus;

    #[test]
    fn accepts_ollama_model_names() {
        for name in [
            "gpt-oss:20b",
            "gemma3:4b",
            "hf.co/org/model:Q4_K_M",
            "llama3.2",
        ] {
            assert!(valid_model_name(name), "{name}");
        }
    }

    #[test]
    fn rejects_anything_else() {
        let long = "a".repeat(201);
        for name in [
            "",
            " ",
            "gpt oss",
            "model\n",
            "../x;rm",
            "<script>",
            long.as_str(),
        ] {
            assert!(!valid_model_name(name), "{name:?}");
        }
    }

    #[test]
    fn shows_the_fixed_loopback_address_and_blocked_apps() {
        let settings = view(&Config::default(), HotkeyStatus::Taken);
        assert_eq!(settings.model, None);
        assert_eq!(settings.hotkey_status, HotkeyStatus::Taken);
        assert_eq!(settings.ollama_address, "127.0.0.1:11434");
        assert!(settings.blocked_apps.contains(&"keepass.exe"));
    }
}
