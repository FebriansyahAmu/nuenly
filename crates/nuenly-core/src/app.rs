//! Identitas aplikasi dan jendela di latar depan.

use std::fmt;

use crate::error::InvalidName;

/// Nama file executable yang dinormalisasi: tanpa folder dan huruf kecil, misalnya `outlook.exe`.
///
/// Berasal dari informasi proses, bukan dari isi layar, jadi aman dicatat di log.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProcessName(String);

impl ProcessName {
    /// Menormalisasi nama atau path executable.
    ///
    /// # Errors
    ///
    /// [`InvalidName`] bila nama filenya kosong.
    pub fn new(raw: &str) -> Result<Self, InvalidName> {
        let file = raw.rsplit(['\\', '/']).next().unwrap_or(raw).trim();
        if file.is_empty() {
            return Err(InvalidName::new("nama proses"));
        }
        Ok(Self(file.to_lowercase()))
    }

    /// Nama yang sudah dinormalisasi.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProcessName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Aplikasi pemilik jendela di latar depan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppIdentity {
    pub process: ProcessName,
    /// Nama untuk pengguna, misalnya "Mail". Diambil dari metadata executable, bukan dari layar.
    pub display_name: String,
}

/// Handle jendela dari platform (misalnya `HWND` di Windows), disimpan sebagai angka.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WindowHandle(pub u64);

/// Jendela di latar depan. Hanya identitasnya; isi jendela belum dibaca.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForegroundWindow {
    pub handle: WindowHandle,
    pub app: AppIdentity,
}

#[cfg(test)]
mod tests {
    use super::ProcessName;

    #[test]
    fn process_name_drops_folder_and_lowercases() {
        let name = ProcessName::new(r"C:\Program Files\Microsoft Office\OUTLOOK.EXE").unwrap();
        assert_eq!(name.as_str(), "outlook.exe");
        assert_eq!(
            ProcessName::new("/usr/bin/gedit").unwrap().as_str(),
            "gedit"
        );
    }

    #[test]
    fn process_name_rejects_empty() {
        assert!(ProcessName::new("").is_err());
        assert!(ProcessName::new(r"C:\Windows\").is_err());
        assert!(ProcessName::new("   ").is_err());
    }
}
