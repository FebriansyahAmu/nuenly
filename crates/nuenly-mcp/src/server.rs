//! Konfigurasi server MCP dan cara menjalankannya.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;

use nuenly_core::ActivePolicy;
use serde::{Deserialize, Serialize};

/// Variabel lingkungan yang diwariskan ke server stdio: yang dibutuhkan proses Windows biasa
/// (folder sistem, `PATH`, folder sementara dan profil). Variabel lain milik pengguna, misalnya
/// kunci API atau token, tidak ikut.
pub const INHERITED_ENV: &[&str] = &[
    "SystemRoot",
    "SystemDrive",
    "windir",
    "ComSpec",
    "PATH",
    "PATHEXT",
    "TEMP",
    "TMP",
    "USERPROFILE",
    "HOMEDRIVE",
    "HOMEPATH",
    "APPDATA",
    "LOCALAPPDATA",
    "ProgramData",
    "ProgramFiles",
    "ProgramFiles(x86)",
    "ProgramW6432",
    "CommonProgramFiles",
    "NUMBER_OF_PROCESSORS",
    "PROCESSOR_ARCHITECTURE",
    "OS",
    "HOME",
    "LANG",
];

/// ID server dari konfigurasi pengguna, misalnya `berkas-kantor`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ServerId(String);

/// ID server tidak valid: kosong, lebih dari 64 karakter, atau memuat selain `a-z`, `0-9`, `-`.
#[derive(Debug, thiserror::Error)]
#[error("ID server MCP tidak valid")]
pub struct InvalidServerId;

impl ServerId {
    /// # Errors
    ///
    /// [`InvalidServerId`] bila formatnya salah.
    pub fn new(id: &str) -> Result<Self, InvalidServerId> {
        let valid = !id.is_empty()
            && id.len() <= 64
            && id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        if valid {
            Ok(Self(id.to_owned()))
        } else {
            Err(InvalidServerId)
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ServerId {
    type Error = InvalidServerId;

    fn try_from(id: String) -> Result<Self, Self::Error> {
        Self::new(&id)
    }
}

impl From<ServerId> for String {
    fn from(id: ServerId) -> Self {
        id.0
    }
}

impl fmt::Display for ServerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Satu server MCP yang ditambahkan pengguna.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServerSpec {
    pub id: ServerId,
    /// Nama yang tampil di Pengaturan dan dialog izin, misalnya "Berkas kantor".
    pub label: String,
    /// ID mode tempat server ini dipakai (F-16: server terikat mode). Kosong berarti tidak
    /// dipakai di mode mana pun.
    pub modes: Vec<String>,
    pub transport: Transport,
}

impl ServerSpec {
    /// `true` bila server ini boleh dipakai di mode aktif.
    #[must_use]
    pub fn usable_in(&self, policy: &ActivePolicy) -> bool {
        self.modes.iter().any(|mode| mode == policy.mode.as_str())
    }
}

/// Cara Nuenly berbicara dengan server. Streamable HTTP menyusul bersama TLS.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum Transport {
    /// Proses lokal yang dijalankan Nuenly, berbicara lewat stdin/stdout.
    Stdio {
        command: PathBuf,
        #[serde(default)]
        args: Vec<String>,
        /// Variabel tambahan untuk server ini saja, di atas [`INHERITED_ENV`].
        #[serde(default)]
        env: BTreeMap<String, String>,
        #[serde(default)]
        cwd: Option<PathBuf>,
    },
}

/// Lingkungan proses server: hanya [`INHERITED_ENV`] dari `parent` (nama tanpa beda huruf besar,
/// seperti di Windows), lalu `extra` dari konfigurasi server.
pub(crate) fn environment(
    parent: impl IntoIterator<Item = (OsString, OsString)>,
    extra: &BTreeMap<String, String>,
) -> Vec<(OsString, OsString)> {
    let mut env: Vec<(OsString, OsString)> = parent
        .into_iter()
        .filter(|(name, _)| {
            name.to_str().is_some_and(|name| {
                INHERITED_ENV
                    .iter()
                    .any(|inherited| inherited.eq_ignore_ascii_case(name))
            })
        })
        .collect();
    for (name, value) in extra {
        env.retain(|(existing, _)| {
            !existing
                .to_str()
                .is_some_and(|existing| existing.eq_ignore_ascii_case(name))
        });
        env.push((name.into(), value.into()));
    }
    env
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::ffi::OsString;

    use nuenly_core::{ActivePolicy, ModeId};

    use super::{ServerId, ServerSpec, Transport, environment};

    fn spec() -> ServerSpec {
        ServerSpec {
            id: ServerId::new("berkas-kantor").unwrap(),
            label: "Berkas kantor".to_owned(),
            modes: vec!["all".to_owned()],
            transport: Transport::Stdio {
                command: "npx".into(),
                args: vec!["-y".to_owned(), "server-contoh".to_owned()],
                env: BTreeMap::new(),
                cwd: None,
            },
        }
    }

    #[test]
    fn server_ids_are_short_slugs() {
        assert!(ServerId::new("berkas-kantor-2").is_ok());
        for bad in ["", "Berkas", "berkas kantor", "../x", &"a".repeat(65)] {
            assert!(ServerId::new(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn servers_are_bound_to_modes() {
        let spec = spec();
        assert!(spec.usable_in(&ActivePolicy::new(ModeId::new("all").unwrap())));
        assert!(!spec.usable_in(&ActivePolicy::new(ModeId::new("medical").unwrap())));
    }

    #[test]
    fn specs_round_trip_through_json() {
        let json = serde_json::to_value(spec()).unwrap();
        assert_eq!(json["transport"]["type"], "stdio");
        assert_eq!(serde_json::from_value::<ServerSpec>(json).unwrap(), spec());
        let bad = serde_json::json!({
            "id": "Bukan Slug", "label": "x", "modes": [],
            "transport": { "type": "stdio", "command": "x" }
        });
        assert!(serde_json::from_value::<ServerSpec>(bad).is_err());
    }

    #[test]
    fn only_basic_variables_are_inherited() {
        let parent = [
            ("Path", r"C:\Windows"),
            ("SYSTEMROOT", r"C:\Windows"),
            ("OPENAI_API_KEY", "rahasia-sintetis"),
            ("GITHUB_TOKEN", "rahasia-sintetis"),
            ("TEMP", r"C:\Temp"),
        ]
        .map(|(name, value)| (OsString::from(name), OsString::from(value)));
        let extra = BTreeMap::from([
            ("CONTOH_FOLDER".to_owned(), r"C:\Contoh".to_owned()),
            ("temp".to_owned(), r"D:\Lain".to_owned()),
        ]);
        let env = environment(parent, &extra);
        let names: Vec<_> = env.iter().map(|(name, _)| name.to_str().unwrap()).collect();
        assert_eq!(names, ["Path", "SYSTEMROOT", "CONTOH_FOLDER", "temp"]);
        assert!(env.iter().all(|(_, value)| value != "rahasia-sintetis"));
    }
}
