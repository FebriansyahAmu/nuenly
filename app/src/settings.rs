//! Jendela Pengaturan (F-27): data yang ditampilkan dan perubahan yang disimpan di `config.json`.

use nuenly_core::Shortcut;
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::config::{Config, ConfigStore};
use crate::hotkey::{HotkeyState, HotkeyStatus};

const MAX_MODEL_NAME: usize = 200;

/// Bentuknya sama dengan `SettingsView` di `app/ui/src/windows/settings/backend.ts`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    /// Model pilihan pengguna; `None` berarti dipilih otomatis sesuai kemampuan PC.
    pub model: Option<String>,
    /// Pintasan Tanya Nuenly yang sedang dipakai, misalnya `Ctrl+Space`.
    pub hotkey: String,
    pub hotkey_status: HotkeyStatus,
    pub ollama_address: String,
    /// Aplikasi yang tidak pernah dibaca, apa pun modenya.
    pub blocked_apps: &'static [&'static str],
}

/// Kenapa pintasan tidak bisa diganti; bentuknya sama dengan `HotkeyFailure` di
/// `app/ui/src/windows/settings/backend.ts`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum HotkeyFailure {
    /// Bukan kombinasi yang didukung, misalnya tanpa Ctrl maupun Alt.
    Invalid,
    /// Sudah dipakai aplikasi lain; pintasan lama tetap dipakai.
    Taken,
    /// Windows menolaknya; pintasan lama tetap dipakai.
    Failed,
    /// Sudah dipakai sekarang, tetapi tidak tersimpan untuk sesi berikutnya.
    NotSaved,
}

fn view(config: &Config, (hotkey, status): (Shortcut, HotkeyStatus)) -> SettingsView {
    SettingsView {
        model: config.model.clone(),
        hotkey: hotkey.to_string(),
        hotkey_status: status,
        ollama_address: nuenly_llm::ollama::DEFAULT_ADDRESS.to_string(),
        blocked_apps: nuenly_core::ALWAYS_BLOCKED,
    }
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri menyuntikkan state sebagai nilai.
pub fn settings_get(store: State<'_, ConfigStore>, hotkey: State<'_, HotkeyState>) -> SettingsView {
    view(&store.get(), hotkey.current())
}

/// Mengganti pintasan Tanya Nuenly; `None` kembali ke Ctrl+Space.
#[tauri::command]
pub async fn settings_set_hotkey(
    app: AppHandle,
    store: State<'_, ConfigStore>,
    shortcut: Option<String>,
) -> Result<SettingsView, HotkeyFailure> {
    let shortcut = match shortcut.as_deref() {
        Some(text) => Shortcut::parse(text).map_err(|_| HotkeyFailure::Invalid)?,
        None => Shortcut::CTRL_SPACE,
    };
    // Melepas pintasan lama menunggu thread hotkey, jadi dikerjakan di luar thread UI.
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        handle.state::<HotkeyState>().replace(&handle, shortcut)
    })
    .await
    .map_err(|_| HotkeyFailure::Failed)?
    .map_err(|status| match status {
        HotkeyStatus::Taken => HotkeyFailure::Taken,
        _ => HotkeyFailure::Failed,
    })?;

    let saved = (shortcut != Shortcut::CTRL_SPACE).then(|| shortcut.to_string());
    let config = store
        .update(|config| config.hotkey = saved)
        .map_err(|_| HotkeyFailure::NotSaved)?;
    Ok(view(&config, app.state::<HotkeyState>().current()))
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
        .map(|config| view(&config, hotkey.current()))
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
    use nuenly_core::Shortcut;

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
        let alt_space = Shortcut::parse("Alt+Space").unwrap();
        let settings = view(&Config::default(), (alt_space, HotkeyStatus::Taken));
        assert_eq!(settings.model, None);
        assert_eq!(settings.hotkey, "Alt+Space");
        assert_eq!(settings.hotkey_status, HotkeyStatus::Taken);
        assert_eq!(settings.ollama_address, "127.0.0.1:11434");
        assert!(settings.blocked_apps.contains(&"keepass.exe"));
    }
}
