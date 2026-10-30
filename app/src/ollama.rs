//! Pemeriksaan Ollama untuk onboarding (F-26) dan Pengaturan › Model AI (F-27): apakah Ollama
//! berjalan di PC ini, versinya, dan model yang sudah diunduh.

use std::ffi::OsStr;
use std::path::Path;

use nuenly_llm::OllamaProvider;
use serde::Serialize;
use tauri::State;

const EXE: &str = if cfg!(windows) {
    "ollama.exe"
} else {
    "ollama"
};
const DOWNLOAD_PAGE: &str = "https://ollama.com/download/windows";

/// Bentuknya sama dengan `OllamaStatus` di `app/ui/src/lib/ollama.ts`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OllamaStatus {
    /// Ollama menjawab di 127.0.0.1:11434.
    Running,
    /// Program Ollama ada, tetapi belum dijalankan.
    Installed,
    #[default]
    Missing,
}

/// Bentuknya sama dengan `OllamaInfo` di `app/ui/src/lib/ollama.ts`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OllamaInfo {
    pub status: OllamaStatus,
    pub version: Option<String>,
    /// Nama model yang berjalan di PC ini, urut abjad; model cloud tidak termasuk. Kosong bila
    /// Ollama tidak berjalan.
    pub models: Vec<String>,
}

pub async fn detect(provider: &OllamaProvider) -> OllamaInfo {
    if let Ok(version) = provider.version().await {
        // Model cloud meneruskan teks ke luar PC, jadi tidak ditawarkan (dan ditolak provider).
        let models = provider
            .models()
            .await
            .map(|models| {
                models
                    .into_iter()
                    .filter(|model| !model.remote)
                    .map(|model| model.name)
                    .collect()
            })
            .unwrap_or_default();
        return OllamaInfo {
            status: OllamaStatus::Running,
            version: Some(version),
            models,
        };
    }
    let installed = tauri::async_runtime::spawn_blocking(|| {
        installed(
            std::env::var_os("LOCALAPPDATA").as_deref(),
            std::env::var_os("PATH").as_deref(),
        )
    })
    .await
    .unwrap_or(false);
    OllamaInfo {
        status: if installed {
            OllamaStatus::Installed
        } else {
            OllamaStatus::Missing
        },
        ..OllamaInfo::default()
    }
}

/// Selalu berhasil; `Result` hanya karena command async yang memakai `State` wajib mengembalikannya.
#[tauri::command]
pub async fn ollama_status(provider: State<'_, OllamaProvider>) -> Result<OllamaInfo, String> {
    Ok(detect(&provider).await)
}

/// Membuka halaman unduhan Ollama. Tautannya tetap; frontend tidak bisa memilih alamat lain.
#[tauri::command]
pub fn ollama_open_download() -> Result<(), String> {
    open_https(DOWNLOAD_PAGE).map_err(|_| "Halaman unduhan tidak bisa dibuka.".to_owned())
}

#[cfg(windows)]
fn open_https(url: &str) -> std::io::Result<()> {
    nuenly_platform_windows::shell::open_https(url)
}

#[cfg(not(windows))]
fn open_https(_url: &str) -> std::io::Result<()> {
    Err(std::io::Error::other(
        "membuka tautan belum didukung di OS ini",
    ))
}

/// Mencari program Ollama di lokasi pemasangan bawaan (`%LOCALAPPDATA%\Programs\Ollama`) dan
/// di `PATH`.
fn installed(local_app_data: Option<&OsStr>, path: Option<&OsStr>) -> bool {
    let default = local_app_data.map(|dir| Path::new(dir).join("Programs").join("Ollama"));
    let on_path = path.into_iter().flat_map(std::env::split_paths);
    default
        .into_iter()
        .chain(on_path)
        .any(|dir| dir.join(EXE).is_file())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{EXE, installed};

    #[test]
    fn finds_ollama_in_the_default_location_or_on_path() {
        let root: PathBuf =
            std::env::temp_dir().join(format!("nuenly-ollama-{}", std::process::id()));
        let local = root.join("local");
        let default = local.join("Programs").join("Ollama");
        let bin = root.join("bin");
        std::fs::create_dir_all(&default).unwrap();
        std::fs::create_dir_all(&bin).unwrap();

        let found = installed(Some(local.as_os_str()), Some(bin.as_os_str()));
        assert!(!found);

        std::fs::write(bin.join(EXE), b"").unwrap();
        assert!(installed(None, Some(bin.as_os_str())));

        std::fs::write(default.join(EXE), b"").unwrap();
        assert!(installed(Some(local.as_os_str()), None));

        std::fs::remove_dir_all(&root).unwrap();
    }
}
