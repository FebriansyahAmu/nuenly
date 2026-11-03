//! Pemeriksaan Ollama untuk onboarding (F-26) dan Pengaturan › Model AI (F-27): apakah Ollama
//! berjalan di PC ini, versinya, model yang sudah diunduh, dan rencana `ResourceProfiler`
//! (F-13) untuk model yang dipakai.

use std::ffi::OsStr;
use std::net::IpAddr;
use std::path::Path;

use nuenly_context::{Candidate, ModelPlan, Placement};
use nuenly_llm::ollama::{DEFAULT_ADDRESS, OllamaModel};
use nuenly_llm::{LlmError, OllamaProvider};
use serde::Serialize;
use tauri::State;

use crate::config::ConfigStore;
use crate::hardware::{HardwareInfo, HardwareState};

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
    /// Ollama berjalan, tetapi juga bisa dihubungi dari jaringan, jadi tidak dipakai.
    Exposed,
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
    /// Rencana untuk model pilihan pengguna, atau pilihan otomatis. `None` bila belum ada model
    /// yang cocok.
    pub plan: Option<PlanView>,
    pub hardware: HardwareInfo,
}

/// Bentuknya sama dengan `PlanView` di `app/ui/src/lib/ollama.ts`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanView {
    pub model: String,
    /// Dipilih `ResourceProfiler` karena pengguna memilih "Otomatis".
    pub automatic: bool,
    pub placement: &'static str,
    pub num_ctx: u32,
    pub keep_alive_secs: u64,
    pub kv_q8_recommended: bool,
}

impl PlanView {
    fn new(plan: ModelPlan, automatic: bool) -> Self {
        Self {
            placement: match plan.placement {
                Placement::Gpu => "gpu",
                Placement::Split => "split",
                Placement::Cpu => "cpu",
                Placement::TooLarge => "tooLarge",
            },
            model: plan.model,
            automatic,
            num_ctx: plan.num_ctx.get(),
            keep_alive_secs: plan.keep_alive.as_secs(),
            kv_q8_recommended: plan.kv_q8_recommended,
        }
    }
}

/// Model yang berjalan di PC ini. Model cloud meneruskan teks ke luar PC, jadi tidak ditawarkan
/// (dan ditolak provider).
pub async fn local_models(provider: &OllamaProvider) -> Result<Vec<OllamaModel>, LlmError> {
    Ok(provider
        .models()
        .await?
        .into_iter()
        .filter(|model| !model.remote)
        .collect())
}

/// Rencana `ResourceProfiler` untuk `preferred` (`None` = otomatis) di antara `models`.
pub fn plan(
    hardware: &HardwareInfo,
    models: &[OllamaModel],
    preferred: Option<&str>,
) -> Option<ModelPlan> {
    let candidates: Vec<Candidate<'_>> = models
        .iter()
        .map(|model| Candidate {
            name: &model.name,
            size: model.size,
        })
        .collect();
    hardware.profiler().plan(&candidates, preferred)
}

/// Memeriksa Ollama, lalu merencanakan `preferred` (`None` = otomatis) untuk PC ini.
pub async fn detect(
    provider: &OllamaProvider,
    hardware: HardwareInfo,
    preferred: Option<&str>,
) -> OllamaInfo {
    if let Ok(version) = provider.version().await {
        if exposed().await {
            return OllamaInfo {
                status: OllamaStatus::Exposed,
                version: Some(version),
                hardware,
                ..OllamaInfo::default()
            };
        }
        let local = local_models(provider).await.unwrap_or_default();
        let plan =
            plan(&hardware, &local, preferred).map(|plan| PlanView::new(plan, preferred.is_none()));
        return OllamaInfo {
            status: OllamaStatus::Running,
            version: Some(version),
            models: local.into_iter().map(|model| model.name).collect(),
            plan,
            hardware,
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
        hardware,
        ..OllamaInfo::default()
    }
}

/// Ollama bisa dihubungi komputer lain di jaringan: ada socket di port Ollama yang mendengarkan
/// di luar loopback, biasanya karena `OLLAMA_HOST=0.0.0.0`. API Ollama tanpa autentikasi, jadi
/// siapa pun di jaringan bisa memakai, menghapus, atau mengganti modelnya. Setiap jalur yang
/// mengirim teks ke Ollama wajib memeriksanya lebih dulu (ADR 0011).
pub async fn exposed() -> bool {
    let port = DEFAULT_ADDRESS.port();
    if let Ok(Ok(addresses)) = tauri::async_runtime::spawn_blocking(move || listeners(port)).await {
        return reachable_from_network(&addresses);
    }
    // Kegagalan membaca tabel tidak bisa dipicu dari jaringan, jadi Nuenly tetap jalan.
    tracing::warn!("tabel socket tidak terbaca; Ollama dianggap hanya untuk PC ini");
    false
}

/// `true` bila ada alamat selain loopback, termasuk `0.0.0.0` dan `::`.
fn reachable_from_network(addresses: &[IpAddr]) -> bool {
    addresses
        .iter()
        .any(|address| !address.to_canonical().is_loopback())
}

#[cfg(windows)]
fn listeners(port: u16) -> std::io::Result<Vec<IpAddr>> {
    nuenly_platform_windows::network::listening_addresses(port)
}

#[cfg(not(windows))]
fn listeners(_port: u16) -> std::io::Result<Vec<IpAddr>> {
    Err(std::io::Error::other("belum didukung di OS ini"))
}

/// Selalu berhasil; `Result` hanya karena command async yang memakai `State` wajib mengembalikannya.
#[tauri::command]
pub async fn ollama_status(
    provider: State<'_, OllamaProvider>,
    hardware: State<'_, HardwareState>,
    store: State<'_, ConfigStore>,
) -> Result<OllamaInfo, String> {
    let preferred = store.get().model;
    let hardware = hardware.get().await;
    Ok(detect(&provider, hardware, preferred.as_deref()).await)
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
    use std::net::IpAddr;
    use std::path::PathBuf;

    use super::{EXE, OllamaStatus, detect, installed, reachable_from_network};

    #[test]
    fn only_loopback_is_private() {
        let parse = |addresses: &[&str]| -> Vec<IpAddr> {
            addresses
                .iter()
                .map(|address| address.parse().unwrap())
                .collect()
        };
        for private in [
            &[][..],
            &["127.0.0.1"],
            &["::1"],
            &["::ffff:127.0.0.1"],
            &["127.0.0.1", "::1"],
        ] {
            assert!(!reachable_from_network(&parse(private)), "{private:?}");
        }
        for exposed in [
            &["0.0.0.0"][..],
            &["::"],
            &["192.168.1.10"],
            &["127.0.0.1", "0.0.0.0"],
        ] {
            assert!(reachable_from_network(&parse(exposed)), "{exposed:?}");
        }
    }

    // Ollama dan GPU sungguhan di PC pengembang; tidak memuat model.
    // Jalankan manual: cargo test -p nuenly --bin nuenly plans_for_this_pc -- --ignored --nocapture
    #[test]
    #[ignore = "butuh Ollama yang berjalan"]
    fn plans_for_this_pc() {
        let provider = nuenly_llm::OllamaProvider::new().unwrap();
        let hardware = crate::hardware::detect();
        for preferred in [None, Some("gemma4:e4b")] {
            let info =
                tauri::async_runtime::block_on(detect(&provider, hardware.clone(), preferred));
            assert_eq!(info.status, OllamaStatus::Running);
            println!("{preferred:?}: {:#?}\n{:#?}", info.hardware, info.plan);
        }
    }

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
