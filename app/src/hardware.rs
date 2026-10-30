//! Spesifikasi PC untuk `ResourceProfiler` (F-13). Dibaca sekali, saat pertama dibutuhkan.

use std::sync::OnceLock;

use nuenly_context::{Hardware, ResourceProfiler};
use serde::Serialize;

/// Bentuknya sama dengan `HardwareInfo` di `app/ui/src/lib/ollama.ts`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HardwareInfo {
    pub ram_bytes: u64,
    /// VRAM GPU terbesar; 0 bila tidak ada GPU.
    pub vram_bytes: u64,
    pub gpu: Option<String>,
}

impl HardwareInfo {
    pub fn profiler(&self) -> ResourceProfiler {
        ResourceProfiler::new(Hardware {
            ram: self.ram_bytes,
            vram: self.vram_bytes,
        })
    }
}

/// State terkelola Tauri.
#[derive(Default)]
pub struct HardwareState(OnceLock<HardwareInfo>);

impl HardwareState {
    pub async fn get(&self) -> HardwareInfo {
        if let Some(info) = self.0.get() {
            return info.clone();
        }
        let info = tauri::async_runtime::spawn_blocking(detect)
            .await
            .unwrap_or_default();
        self.0.get_or_init(|| info).clone()
    }
}

/// Membaca spesifikasi PC. Blocking; di aplikasi lewat [`HardwareState::get`].
#[cfg(windows)]
pub fn detect() -> HardwareInfo {
    use nuenly_platform_windows::hardware;

    let gpu = hardware::gpus()
        .unwrap_or_default()
        .into_iter()
        .max_by_key(|gpu| gpu.dedicated_memory);
    HardwareInfo {
        ram_bytes: hardware::memory().map_or(0, |memory| memory.total),
        vram_bytes: gpu.as_ref().map_or(0, |gpu| gpu.dedicated_memory),
        gpu: gpu.map(|gpu| gpu.name),
    }
}

#[cfg(not(windows))]
pub fn detect() -> HardwareInfo {
    HardwareInfo::default()
}
