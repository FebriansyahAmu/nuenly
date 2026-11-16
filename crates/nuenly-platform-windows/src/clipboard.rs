//! Menulis teks ke clipboard Windows, dan menyimpan lalu mengembalikan isi teks sebelumnya untuk
//! Ganti teks.

use nuenly_core::SensitiveText;
use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, GetClipboardSequenceNumber,
    IsClipboardFormatAvailable, OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
};
use windows::Win32::System::Memory::{
    GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock,
};
use windows::core::w;
use zeroize::Zeroizing;

/// `CF_UNICODETEXT` dari winuser.h; ditulis langsung agar tidak perlu fitur `Win32_System_Ole`.
const CF_UNICODETEXT: u32 = 13;

/// Menyalin teks ke clipboard tanpa ikut ke riwayat clipboard (Win+V) maupun sinkronisasi
/// clipboard cloud, karena isinya bisa berasal dari data pengguna atau pasien. Baris baru
/// ditulis sebagai CRLF, bentuk teks clipboard di Windows.
///
/// # Errors
///
/// Error OS bila clipboard sedang dipakai aplikasi lain atau memori tidak bisa dialokasikan.
pub fn write_text(text: &str) -> std::io::Result<()> {
    let bytes: Zeroizing<Vec<u8>> = Zeroizing::new(
        crlf(text)
            .encode_utf16()
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

/// `text` dengan setiap LF yang berdiri sendiri menjadi CRLF.
fn crlf(text: &str) -> Zeroizing<String> {
    let mut out = Zeroizing::new(String::with_capacity(text.len() + text.len() / 16));
    let mut previous = None;
    for c in text.chars() {
        if c == '\n' && previous != Some('\r') {
            out.push('\r');
        }
        out.push(c);
        previous = Some(c);
    }
    out
}

/// Isi teks clipboard sebelum Ganti teks menimpanya. Dikembalikan dengan [`Saved::restore`].
pub struct Saved {
    previous: Option<SensitiveText>,
    /// Nomor urut clipboard setelah jawaban ditulis; berubah bila ada yang menyalin lagi.
    sequence: u32,
}

impl Saved {
    /// Menyimpan teks clipboard saat ini, lalu menulis `replacement`.
    ///
    /// # Errors
    ///
    /// Error OS bila clipboard sedang dipakai aplikasi lain.
    pub(crate) fn replace_with(replacement: &str) -> std::io::Result<Self> {
        let previous = read_text()?;
        write_text(replacement)?;
        Ok(Self {
            previous,
            // SAFETY: tanpa prasyarat.
            sequence: unsafe { GetClipboardSequenceNumber() },
        })
    }

    /// Mengembalikan teks clipboard sebelumnya, kecuali clipboard sudah diubah lagi sejak jawaban
    /// ditulis (misalnya pengguna menyalin sesuatu). Clipboard yang sebelumnya tidak berisi teks
    /// tetap berisi jawaban.
    ///
    /// # Errors
    ///
    /// Error OS bila clipboard sedang dipakai aplikasi lain.
    pub fn restore(self) -> std::io::Result<()> {
        // SAFETY: tanpa prasyarat.
        let unchanged = unsafe { GetClipboardSequenceNumber() } == self.sequence;
        match self.previous {
            Some(previous) if unchanged => write_text(previous.expose()),
            _ => Ok(()),
        }
    }
}

/// Isi teks clipboard saat ini, atau `None` bila clipboard tidak berisi teks.
fn read_text() -> std::io::Result<Option<SensitiveText>> {
    // SAFETY: OpenClipboard tanpa jendela pemilik tidak punya prasyarat. Clipboard ditutup
    // oleh `OpenGuard` di semua jalur keluar.
    unsafe { OpenClipboard(None) }?;
    let _open = OpenGuard;

    // SAFETY: clipboard sedang dibuka oleh thread ini.
    if unsafe { IsClipboardFormatAvailable(CF_UNICODETEXT) }.is_err() {
        return Ok(None);
    }
    // SAFETY: clipboard terbuka dan formatnya tersedia. Handle-nya milik clipboard dan hanya
    // dipakai sebelum clipboard ditutup.
    let handle = unsafe { GetClipboardData(CF_UNICODETEXT) }?;
    let memory = HGLOBAL(handle.0);
    // SAFETY: `memory` adalah blok global milik clipboard yang sedang terbuka.
    let units = unsafe { GlobalSize(memory) } / size_of::<u16>();
    // SAFETY: seperti di atas; dikunci hanya untuk dibaca, lalu dilepas di bawah.
    let data = unsafe { GlobalLock(memory) }.cast::<u16>();
    if data.is_null() {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: blok terkunci berisi `units` unit UTF-16 dan tetap hidup sampai GlobalUnlock.
    let slice = unsafe { std::slice::from_raw_parts(data, units) };
    let end = slice.iter().position(|&unit| unit == 0).unwrap_or(units);
    let text = SensitiveText::new(String::from_utf16_lossy(&slice[..end]));
    // SAFETY: melepas kunci yang diambil di atas; hasil error berkode 0 saat kunci habis.
    let _ = unsafe { GlobalUnlock(memory) };
    Ok(Some(text))
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

/// Menutup clipboard saat keluar dari `write_text` atau `read_text`.
struct OpenGuard;

impl Drop for OpenGuard {
    fn drop(&mut self) {
        // SAFETY: hanya dibuat setelah OpenClipboard berhasil di thread ini.
        let _ = unsafe { CloseClipboard() };
    }
}

#[cfg(test)]
mod tests {
    use super::{crlf, write_text};

    #[test]
    fn line_breaks_become_crlf_once() {
        assert_eq!(
            crlf("satu\ndua\r\ntiga\n").as_str(),
            "satu\r\ndua\r\ntiga\r\n"
        );
        assert_eq!(crlf("tanpa baris baru").as_str(), "tanpa baris baru");
        assert_eq!(crlf("\n\n").as_str(), "\r\n\r\n");
    }

    // Menimpa clipboard pengguna dan butuh sesi desktop interaktif, jadi tidak jalan di CI.
    // Jalankan manual: cargo test -p nuenly-platform-windows -- --ignored
    #[test]
    #[ignore = "menimpa clipboard pengguna"]
    fn writes_text_to_clipboard() {
        write_text("uji clipboard Nuenly · teks sintetis").unwrap();
    }
}
