//! Satu mode pack: manifest `mode.toml` dan prompt di `prompts/`.

use std::ops::RangeInclusive;
use std::time::Duration;

use nuenly_core::{ActivePolicy, AppFilter, ModeId, ProactivePolicy, ProcessName, RedactionLevel};
use serde::Deserialize;

use crate::error::{ModeError, invalid};

/// Panjang maksimum label mode di UI, dalam karakter.
const MAX_LABEL_CHARS: usize = 32;
/// Panjang maksimum prompt, dalam karakter. Prompt sistem ikut di setiap permintaan, jadi prompt
/// yang panjang mengurangi ruang teks layar di `num_ctx` 4096.
const MAX_PROMPT_CHARS: usize = 4000;
const MAX_SUGGESTIONS_PER_HOUR: u32 = 60;
const TOAST_TIMEOUT_SECS: RangeInclusive<u64> = 1..=120;

/// Satu mode: label dan warna aksen untuk UI, kebijakan, dan prompt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModePack {
    label: String,
    accent: String,
    policy: ActivePolicy,
    ask_prompt: &'static str,
}

impl ModePack {
    /// Membaca mode pack dari isi `mode.toml` dan `prompts/ask.txt`. Kebijakan yang tidak
    /// ditulis memakai bawaan [`ActivePolicy::new`].
    ///
    /// Field yang tidak dikenal ditolak agar salah ketik tidak diam-diam jatuh ke nilai bawaan.
    /// Tujuan kirim teks (`egress`) sengaja tidak bisa diatur mode pack: selalu hanya model lokal
    /// sampai PII guard ada (Fase 2).
    ///
    /// # Errors
    ///
    /// [`ModeError::Manifest`] bila `manifest` tidak sesuai format, atau [`ModeError::Invalid`]
    /// bila ada nilai di luar batas.
    pub fn parse(manifest: &str, ask_prompt: &'static str) -> Result<Self, ModeError> {
        let manifest: Manifest = toml::from_str(manifest)
            .map_err(|err| ModeError::Manifest(err.message().to_owned()))?;
        let section = manifest.policy;

        let mut policy = ActivePolicy::new(ModeId::new(&manifest.id).map_err(|_| invalid("id"))?);
        policy.context_preview = section.context_preview;
        policy.redaction = match section.redaction {
            Redaction::Standard => RedactionLevel::Standard,
            Redaction::Strict => RedactionLevel::Strict,
        };
        policy.apps = AppFilter {
            allow: section.apps.allow.as_deref().map(processes).transpose()?,
            block: processes(&section.apps.block)?,
        };
        policy.proactive = section.proactive.policy()?;

        Ok(Self {
            label: label(&manifest.label)?,
            accent: accent(&manifest.accent)?,
            policy,
            ask_prompt: prompt(ask_prompt)?,
        })
    }

    #[must_use]
    pub fn id(&self) -> &ModeId {
        &self.policy.mode
    }

    /// Nama mode untuk pengguna, misalnya "Umum".
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Warna aksen `#rrggbb` (huruf kecil) untuk sentuhan kecil di UI.
    #[must_use]
    pub fn accent(&self) -> &str {
        &self.accent
    }

    #[must_use]
    pub fn policy(&self) -> &ActivePolicy {
        &self.policy
    }

    /// Prompt sistem Tanya Nuenly. Statis, tanpa nilai dinamis, agar prefix cache terpakai.
    #[must_use]
    pub fn ask_prompt(&self) -> &'static str {
        self.ask_prompt
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    id: String,
    label: String,
    accent: String,
    #[serde(default)]
    policy: PolicySection,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct PolicySection {
    context_preview: bool,
    redaction: Redaction,
    apps: AppsSection,
    proactive: ProactiveSection,
}

#[derive(Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Redaction {
    #[default]
    Standard,
    Strict,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct AppsSection {
    allow: Option<Vec<String>>,
    block: Vec<String>,
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ProactiveSection {
    enabled: bool,
    max_per_hour: u32,
    toast_timeout_secs: u64,
    sticky_important: bool,
}

impl Default for ProactiveSection {
    fn default() -> Self {
        let policy = ProactivePolicy::default();
        Self {
            enabled: policy.enabled,
            max_per_hour: policy.max_per_hour,
            toast_timeout_secs: policy.toast_timeout.as_secs(),
            sticky_important: policy.sticky_important,
        }
    }
}

impl ProactiveSection {
    fn policy(&self) -> Result<ProactivePolicy, ModeError> {
        if self.max_per_hour > MAX_SUGGESTIONS_PER_HOUR
            || !TOAST_TIMEOUT_SECS.contains(&self.toast_timeout_secs)
        {
            return Err(invalid("policy.proactive"));
        }
        Ok(ProactivePolicy {
            enabled: self.enabled,
            max_per_hour: self.max_per_hour,
            toast_timeout: Duration::from_secs(self.toast_timeout_secs),
            sticky_important: self.sticky_important,
        })
    }
}

fn label(raw: &str) -> Result<String, ModeError> {
    let label = raw.trim();
    let valid = !label.is_empty()
        && label.chars().count() <= MAX_LABEL_CHARS
        && !label.chars().any(char::is_control);
    if valid {
        Ok(label.to_owned())
    } else {
        Err(invalid("label"))
    }
}

/// Hanya `#RRGGBB`, karena nilainya dipasang sebagai variabel CSS di jendela.
fn accent(raw: &str) -> Result<String, ModeError> {
    match raw.strip_prefix('#') {
        Some(hex) if hex.len() == 6 && hex.bytes().all(|b| b.is_ascii_hexdigit()) => {
            Ok(raw.to_ascii_lowercase())
        }
        _ => Err(invalid("accent")),
    }
}

fn prompt(raw: &'static str) -> Result<&'static str, ModeError> {
    let prompt = raw.trim();
    if prompt.is_empty() || prompt.chars().count() > MAX_PROMPT_CHARS {
        return Err(invalid("prompts/ask.txt"));
    }
    Ok(prompt)
}

fn processes(names: &[String]) -> Result<Vec<ProcessName>, ModeError> {
    names
        .iter()
        .map(|name| ProcessName::new(name).map_err(|_| invalid("policy.apps")))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use nuenly_core::{ActivePolicy, Egress, ModeId, ProcessName, RedactionLevel};

    use super::ModePack;
    use crate::ModeError;

    const PROMPT: &str = "Kamu asisten uji. Teks layar ada di tag \"layar-\".";

    fn manifest(policy: &str) -> String {
        format!("id = \"uji\"\nlabel = \"Uji\"\naccent = \"#123ABC\"\n{policy}")
    }

    fn invalid(field: &'static str) -> Result<ModePack, ModeError> {
        Err(ModeError::Invalid { field })
    }

    #[test]
    fn missing_policy_uses_safe_defaults() {
        let pack = ModePack::parse(&manifest(""), PROMPT).unwrap();
        assert_eq!(pack.id().as_str(), "uji");
        assert_eq!(pack.label(), "Uji");
        assert_eq!(pack.accent(), "#123abc");
        assert_eq!(pack.ask_prompt(), PROMPT);
        assert_eq!(
            *pack.policy(),
            ActivePolicy::new(ModeId::new("uji").unwrap())
        );
    }

    #[test]
    fn reads_every_policy_field() {
        let pack = ModePack::parse(
            &manifest(
                "[policy]\n\
                 context_preview = true\n\
                 redaction = \"strict\"\n\
                 [policy.apps]\n\
                 allow = [\"AplikasiKlinik.exe\"]\n\
                 block = [\"game.exe\"]\n\
                 [policy.proactive]\n\
                 enabled = false\n\
                 max_per_hour = 3\n\
                 toast_timeout_secs = 12\n\
                 sticky_important = true\n",
            ),
            PROMPT,
        )
        .unwrap();
        let policy = pack.policy();
        assert!(policy.context_preview);
        assert_eq!(policy.redaction, RedactionLevel::Strict);
        assert_eq!(
            policy.apps.allow,
            Some(vec![ProcessName::new("aplikasiklinik.exe").unwrap()])
        );
        assert_eq!(policy.apps.block, [ProcessName::new("game.exe").unwrap()]);
        assert!(!policy.proactive.enabled);
        assert_eq!(policy.proactive.max_per_hour, 3);
        assert_eq!(policy.proactive.toast_timeout, Duration::from_secs(12));
        assert!(policy.proactive.sticky_important);
    }

    #[test]
    fn unknown_fields_are_refused() {
        // Termasuk tujuan kirim teks: mode pack tidak bisa membuka jalur ke model cloud.
        for policy in [
            "[policy]\negress = \"cloud\"\n",
            "[policy]\ncontext_preveiw = true\n",
            "warna = \"merah\"\n",
        ] {
            assert!(matches!(
                ModePack::parse(&manifest(policy), PROMPT),
                Err(ModeError::Manifest(_))
            ));
        }
        assert_eq!(
            ModePack::parse(&manifest(""), PROMPT)
                .unwrap()
                .policy()
                .egress,
            Egress::LocalOnly
        );
    }

    #[test]
    fn required_fields_and_formats_are_checked() {
        assert!(matches!(
            ModePack::parse("id = \"uji\"\nlabel = \"Uji\"\n", PROMPT),
            Err(ModeError::Manifest(_))
        ));
        assert!(matches!(
            ModePack::parse(&manifest("[policy]\nredaction = \"longgar\"\n"), PROMPT),
            Err(ModeError::Manifest(_))
        ));
        let with = |id: &str, label: &str, accent: &str| {
            ModePack::parse(
                &format!("id = \"{id}\"\nlabel = \"{label}\"\naccent = \"{accent}\"\n"),
                PROMPT,
            )
        };
        assert_eq!(with("Umum", "Umum", "#4f46e5"), invalid("id"));
        assert_eq!(with("uji", "  ", "#4f46e5"), invalid("label"));
        assert_eq!(with("uji", &"a".repeat(33), "#4f46e5"), invalid("label"));
        assert_eq!(with("uji", "Uji\\u0007", "#4f46e5"), invalid("label"));
        for accent in ["merah", "#12345", "#1234567", "#12345g", "red;x:url(y)"] {
            assert_eq!(with("uji", "Uji", accent), invalid("accent"));
        }
    }

    #[test]
    fn policy_values_are_bounded() {
        for policy in [
            "[policy.apps]\nblock = [\"\"]\n",
            "[policy.apps]\nallow = [\"  \"]\n",
            "[policy.proactive]\nmax_per_hour = 61\n",
            "[policy.proactive]\ntoast_timeout_secs = 0\n",
            "[policy.proactive]\ntoast_timeout_secs = 121\n",
        ] {
            assert!(matches!(
                ModePack::parse(&manifest(policy), PROMPT),
                Err(ModeError::Invalid { .. })
            ));
        }
    }

    #[test]
    fn prompt_is_trimmed_and_bounded() {
        let pack = ModePack::parse(&manifest(""), "\n  Kamu asisten uji.\n\n").unwrap();
        assert_eq!(pack.ask_prompt(), "Kamu asisten uji.");
        assert_eq!(
            ModePack::parse(&manifest(""), " \n "),
            invalid("prompts/ask.txt")
        );
        let long: &'static str = "a".repeat(4001).leak();
        assert_eq!(
            ModePack::parse(&manifest(""), long),
            invalid("prompts/ask.txt")
        );
    }
}
