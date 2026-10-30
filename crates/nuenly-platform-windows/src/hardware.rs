//! RAM dan GPU, untuk memilih model dan panjang konteks (F-13).

use std::io;

use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE, DXGI_ERROR_NOT_FOUND, IDXGIFactory1,
};
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

/// RAM fisik, dalam byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Memory {
    pub total: u64,
    pub available: u64,
}

/// Satu GPU perangkat keras.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gpu {
    pub name: String,
    /// Memori khusus GPU (VRAM), dalam byte. GPU terintegrasi biasanya hanya ratusan MB.
    pub dedicated_memory: u64,
}

/// # Errors
///
/// Error OS bila Windows tidak memberikan status memori.
pub fn memory() -> io::Result<Memory> {
    let mut status = MEMORYSTATUSEX {
        dwLength: u32::try_from(size_of::<MEMORYSTATUSEX>()).unwrap_or(u32::MAX),
        ..MEMORYSTATUSEX::default()
    };
    // SAFETY: `status` adalah MEMORYSTATUSEX milik kita dengan `dwLength` terisi, sesuai syarat
    // GlobalMemoryStatusEx.
    unsafe { GlobalMemoryStatusEx(&raw mut status) }?;
    Ok(Memory {
        total: status.ullTotalPhys,
        available: status.ullAvailPhys,
    })
}

/// GPU perangkat keras lewat DXGI, tanpa adapter perangkat lunak (Microsoft Basic Render Driver).
///
/// # Errors
///
/// Error OS bila DXGI tidak tersedia.
pub fn gpus() -> io::Result<Vec<Gpu>> {
    // SAFETY: CreateDXGIFactory1 tidak punya prasyarat dan tidak butuh COM diinisialisasi.
    let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1() }?;
    let mut gpus = Vec::new();
    for index in 0.. {
        // SAFETY: `factory` valid; indeks di luar jangkauan dijawab DXGI_ERROR_NOT_FOUND.
        let adapter = match unsafe { factory.EnumAdapters1(index) } {
            Ok(adapter) => adapter,
            Err(error) if error.code() == DXGI_ERROR_NOT_FOUND => break,
            Err(error) => return Err(error.into()),
        };
        // SAFETY: `adapter` baru didapat dari factory dan masih hidup.
        let desc = unsafe { adapter.GetDesc1() }?;
        if desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0.cast_unsigned() != 0 {
            continue;
        }
        let end = desc
            .Description
            .iter()
            .position(|&unit| unit == 0)
            .unwrap_or(desc.Description.len());
        gpus.push(Gpu {
            name: String::from_utf16_lossy(&desc.Description[..end]),
            dedicated_memory: u64::try_from(desc.DedicatedVideoMemory).unwrap_or(u64::MAX),
        });
    }
    Ok(gpus)
}

#[cfg(test)]
mod tests {
    use super::{gpus, memory};

    #[test]
    fn reads_physical_memory() {
        let memory = memory().unwrap();
        assert!(memory.total > 0);
        assert!(memory.available <= memory.total);
    }

    #[test]
    fn lists_hardware_gpus() {
        // Mesin CI tanpa GPU tetap lolos; yang dites hanya bahwa DXGI bisa ditanya.
        for gpu in gpus().unwrap() {
            assert_ne!(gpu.name, "");
        }
    }
}
