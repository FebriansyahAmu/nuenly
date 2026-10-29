//! Menulis teks ke clipboard Windows.

use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::core::w;
use zeroize::Zeroizing;

/// `CF_UNICODETEXT` dari winuser.h; ditulis langsung agar tidak perlu fitur `Win32_System_Ole`.
const CF_UNICODETEXT: u32 = 13;

/// Menyalin teks ke clipboard tanpa ikut ke riwayat clipboard (Win+V) maupun sinkronisasi
/// clipboard cloud, karena isinya bisa berasal dari data pengguna atau pasien.
///
/// # Errors
///
/// Error OS bila clipboard sedang dipakai aplikasi lain atau memori tidak bisa dialokasikan.
pub fn write_text(text: &str) -> std::io::Result<()> {
    let bytes: Zeroizing<Vec<u8>> = Zeroizing::new(
        text.encode_utf16()
            .chain(std::iter::once(0))
            .flat_map(u16::to_ne_bytes)
            .collect(),
    );

    // SAFETY: OpenClipboard tanpa jendela pemilik tidak punya prasyarat. Clipboard ditutup
    // oleh `OpenGuard` di semua jalur keluar.
    unsafe { OpenClipboard(None) }?;
    let _open = OpenGuard;

    // SAFETY: clipboard sedang dibuka oleh thread ini.
    unsafe { EmptyClipboard() }?;
    set_data(CF_UNICODETEXT, &bytes)?;

    // Nilai DWORD 0 pada format ini berarti "jangan simpan di riwayat" dan "jangan unggah".
    for name in [
        w!("CanIncludeInClipboardHistory"),
        w!("CanUploadToCloudClipboard"),
    ] {
        // SAFETY: `name` adalah string UTF-16 statis yang diakhiri nol.
        let format = unsafe { RegisterClipboardFormatW(name) };
        if format != 0 {
            set_data(format, &0u32.to_ne_bytes())?;
        }
    }
    Ok(())
}

/// Menyalin `bytes` ke memori global baru lalu menyerahkannya ke clipboard yang sedang dibuka.
fn set_data(format: u32, bytes: &[u8]) -> std::io::Result<()> {
    // SAFETY: GlobalAlloc tidak punya prasyarat. Memori dibebaskan di sini bila tidak berhasil
    // diserahkan ke clipboard.
    let memory = unsafe { GlobalAlloc(GMEM_MOVEABLE, bytes.len()) }?;

    // SAFETY: `memory` baru dialokasikan dan belum dikunci.
    let target = unsafe { GlobalLock(memory) }.cast::<u8>();
    if target.is_null() {
        let error = std::io::Error::last_os_error();
        free(memory);
        return Err(error);
    }
    // SAFETY: `target` menunjuk ke blok sebesar `bytes.len()` milik kita yang tidak tumpang
    // tindih dengan `bytes`.
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), target, bytes.len()) };
    // SAFETY: melepas kunci yang diambil di atas. Hasilnya error berkode 0 saat kunci habis,
    // jadi diabaikan.
    let _ = unsafe { GlobalUnlock(memory) };

    // SAFETY: clipboard terbuka dan `memory` adalah blok GMEM_MOVEABLE yang tidak dikunci. Bila
    // berhasil, kepemilikannya pindah ke sistem dan tidak boleh dibebaskan lagi.
    if let Err(error) = unsafe { SetClipboardData(format, Some(HANDLE(memory.0))) } {
        free(memory);
        return Err(error.into());
    }
    Ok(())
}

fn free(memory: HGLOBAL) {
    // SAFETY: `memory` dialokasikan GlobalAlloc dan belum diserahkan ke clipboard.
    let _ = unsafe { GlobalFree(Some(memory)) };
}

/// Menutup clipboard saat keluar dari `write_text`.
struct OpenGuard;

impl Drop for OpenGuard {
    fn drop(&mut self) {
        // SAFETY: hanya dibuat setelah OpenClipboard berhasil di thread ini.
        let _ = unsafe { CloseClipboard() };
    }
}

#[cfg(test)]
mod tests {
    use super::write_text;

    // Menimpa clipboard pengguna dan butuh sesi desktop interaktif, jadi tidak jalan di CI.
    // Jalankan manual: cargo test -p nuenly-platform-windows -- --ignored
    #[test]
    #[ignore = "menimpa clipboard pengguna"]
    fn writes_text_to_clipboard() {
        write_text("uji clipboard Nuenly · teks sintetis").unwrap();
    }
}
