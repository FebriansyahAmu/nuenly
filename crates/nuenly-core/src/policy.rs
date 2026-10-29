//! Kebijakan yang berlaku untuk mode aktif.

use std::time::Duration;

use crate::app::{ForegroundWindow, ProcessName};
use crate::error::InvalidName;

/// Aplikasi yang tidak pernah dibaca, apa pun isi mode pack atau kebijakan admin:
/// Nuenly sendiri, dialog kredensial dan UAC Windows, layar kunci, dan pengelola kata sandi.
pub const ALWAYS_BLOCKED: &[&str] = &[
    "nuenly.exe",
    "credentialuibroker.exe",
    "consent.exe",
    "logonui.exe",
    "lockapp.exe",
    "keepass.exe",
    "keepassxc.exe",
    "1password.exe",
    "bitwarden.exe",
];

/// ID mode internal dari manifest mode pack, misalnya `all` atau `medical`.
///
/// Dipakai untuk label dan kunci cache. Modul tidak boleh bercabang berdasarkan nilai ini;
/// perbedaan perilaku antar-mode selalu dinyatakan lewat field [`ActivePolicy`].
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ModeId(String);

impl ModeId {
    /// # Errors
    ///
    /// [`InvalidName`] bila kosong, lebih dari 64 karakter, atau memuat karakter selain
    /// `a-z`, `0-9`, dan `-`.
    pub fn new(id: &str) -> Result<Self, InvalidName> {
        let valid = !id.is_empty()
            && id.len() <= 64
            && id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        if valid {
            Ok(Self(id.to_owned()))
        } else {
            Err(InvalidName::new("ID mode"))
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Filter aplikasi yang diperiksa sebelum Nuenly membaca isi jendela apa pun (F-01).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AppFilter {
    /// Bila `Some`, hanya aplikasi di daftar ini yang boleh dibaca.
    pub allow: Option<Vec<ProcessName>>,
    /// Aplikasi yang tidak dibaca, sebagai tambahan [`ALWAYS_BLOCKED`].
    pub block: Vec<ProcessName>,
}

impl AppFilter {
    /// `true` bila isi jendela aplikasi ini boleh dibaca.
    #[must_use]
    pub fn permits(&self, process: &ProcessName) -> bool {
        if ALWAYS_BLOCKED.contains(&process.as_str()) || self.block.contains(process) {
            return false;
        }
        self.allow
            .as_ref()
            .is_none_or(|allow| allow.contains(process))
    }
}

/// Ke mana teks boleh dikirim untuk diproses model.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Egress {
    /// Hanya model lokal di perangkat ini.
    #[default]
    LocalOnly,
    /// Model cloud boleh dipakai bila pengguna mengaktifkannya, selalu lewat PII guard.
    CloudWithRedaction,
}

/// Seberapa ketat PII/PHI guard menyaring teks sebelum keluar perangkat.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RedactionLevel {
    #[default]
    Standard,
    Strict,
}

/// Batas saran proaktif lewat toast.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProactivePolicy {
    pub enabled: bool,
    /// Paling banyak saran yang tampil per jam.
    pub max_per_hour: u32,
    /// Toast biasa hilang setelah selang ini; hitungan berhenti saat kursor di atasnya.
    pub toast_timeout: Duration,
    /// Saran penting tetap tampil sampai ditutup pengguna.
    pub sticky_important: bool,
}

impl Default for ProactivePolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            max_per_hour: 6,
            toast_timeout: Duration::from_secs(8),
            sticky_important: false,
        }
    }
}

/// Kebijakan untuk mode aktif. Dibuat `ModeManager` dari mode pack dan kebijakan admin;
/// modul lain hanya membacanya.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActivePolicy {
    pub mode: ModeId,
    pub apps: AppFilter,
    pub egress: Egress,
    pub redaction: RedactionLevel,
    pub proactive: ProactivePolicy,
}

impl ActivePolicy {
    /// Kebijakan dengan nilai bawaan yang aman: hanya model lokal, redaksi standar,
    /// dan semua aplikasi boleh dibaca kecuali [`ALWAYS_BLOCKED`].
    #[must_use]
    pub fn new(mode: ModeId) -> Self {
        Self {
            mode,
            apps: AppFilter::default(),
            egress: Egress::default(),
            redaction: RedactionLevel::default(),
            proactive: ProactivePolicy::default(),
        }
    }

    /// Izin membaca isi jendela ini, atau `None` bila filter aplikasi menolaknya.
    #[must_use]
    pub fn permit<'a>(&self, window: &'a ForegroundWindow) -> Option<ReadPermit<'a>> {
        self.apps
            .permits(&window.app.process)
            .then_some(ReadPermit { window })
    }
}

/// Bukti bahwa [`ActivePolicy`] mengizinkan pembacaan satu jendela.
///
/// Hanya bisa dibuat lewat [`ActivePolicy::permit`], dan
/// [`ContextSource::read`](crate::ContextSource::read) mensyaratkannya, jadi tidak ada
/// jalur baca yang melewati filter aplikasi.
#[derive(Debug)]
pub struct ReadPermit<'a> {
    window: &'a ForegroundWindow,
}

impl ReadPermit<'_> {
    /// Jendela yang boleh dibaca.
    #[must_use]
    pub fn window(&self) -> &ForegroundWindow {
        self.window
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{ALWAYS_BLOCKED, ActivePolicy, AppFilter, Egress, ModeId, RedactionLevel};
    use crate::app::ProcessName;

    fn name(raw: &str) -> ProcessName {
        ProcessName::new(raw).unwrap()
    }

    #[test]
    fn always_blocked_entries_are_normalized() {
        for entry in ALWAYS_BLOCKED {
            assert_eq!(name(entry).as_str(), *entry);
        }
    }

    #[test]
    fn always_blocked_wins_over_allowlist() {
        let filter = AppFilter {
            allow: Some(vec![name("keepass.exe"), name("nuenly.exe")]),
            block: Vec::new(),
        };
        assert!(!filter.permits(&name("KeePass.exe")));
        assert!(!filter.permits(&name("nuenly.exe")));
    }

    #[test]
    fn blocklist_blocks() {
        let filter = AppFilter {
            allow: None,
            block: vec![name("game.exe")],
        };
        assert!(!filter.permits(&name("game.exe")));
        assert!(filter.permits(&name("outlook.exe")));
    }

    #[test]
    fn allowlist_restricts() {
        let filter = AppFilter {
            allow: Some(vec![name("medapp.exe")]),
            block: Vec::new(),
        };
        assert!(filter.permits(&name("medapp.exe")));
        assert!(!filter.permits(&name("outlook.exe")));
    }

    #[test]
    fn mode_id_validates_format() {
        assert_eq!(ModeId::new("medical").unwrap().as_str(), "medical");
        assert!(ModeId::new("nuenly-all-2").is_ok());
        for bad in ["", "Medical", "mode umum", "all/../x", &"a".repeat(65)] {
            assert!(ModeId::new(bad).is_err());
        }
    }

    #[test]
    fn new_policy_uses_safe_defaults() {
        let policy = ActivePolicy::new(ModeId::new("all").unwrap());
        assert_eq!(policy.egress, Egress::LocalOnly);
        assert_eq!(policy.redaction, RedactionLevel::Standard);
        assert_eq!(policy.apps, AppFilter::default());
        assert_eq!(policy.proactive.max_per_hour, 6);
        assert_eq!(policy.proactive.toast_timeout, Duration::from_secs(8));
    }
}
