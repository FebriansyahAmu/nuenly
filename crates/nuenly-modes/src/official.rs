//! Mode pack resmi dari folder `modes/`. Isinya ikut di dalam binary, jadi selalu tersedia dan
//! tidak berubah tanpa mengganti binary (ADR 0008).

use crate::{ModeError, ModePack, PackFiles, SuggestFiles};

/// Isi berkas tiap mode pack resmi, dalam urutan tampil di UI. Yang pertama adalah mode awal.
const OFFICIAL: [(&str, PackFiles); 2] = [
    (
        include_str!("../../../modes/all/mode.toml"),
        PackFiles {
            ask: include_str!("../../../modes/all/prompts/ask.txt"),
            suggest: Some(SuggestFiles {
                prompt: include_str!("../../../modes/all/prompts/suggest.txt"),
                schema: include_str!("../../../modes/all/schemas/suggest.json"),
            }),
        },
    ),
    (
        include_str!("../../../modes/medical/mode.toml"),
        // Saran kelengkapan dokumentasi menyusul di Fase 3.
        PackFiles {
            ask: include_str!("../../../modes/medical/prompts/ask.txt"),
            suggest: None,
        },
    ),
];

pub(crate) fn packs() -> Result<Vec<ModePack>, ModeError> {
    OFFICIAL
        .iter()
        .map(|(manifest, files)| ModePack::parse(manifest, *files))
        .collect()
}

#[cfg(test)]
mod tests {
    use nuenly_core::{Egress, RedactionLevel};

    use crate::ModeManager;

    #[test]
    fn umum_comes_first_and_medical_second() {
        let modes = ModeManager::official().unwrap();
        let summary: Vec<_> = modes
            .packs()
            .iter()
            .map(|pack| (pack.id().as_str(), pack.label(), pack.accent()))
            .collect();
        assert_eq!(
            summary,
            [
                ("all", "Umum", "#4f46e5"),
                ("medical", "Medical", "#0f766e")
            ]
        );
        assert_eq!(modes.active().id().as_str(), "all");
    }

    #[test]
    fn umum_shows_the_selected_text_and_medical_does_not() {
        let mut modes = ModeManager::official().unwrap();
        let umum = modes.active().policy().clone();
        assert!(umum.context_preview);
        assert_eq!(umum.redaction, RedactionLevel::Standard);
        assert!(!umum.proactive.sticky_important);

        assert!(modes.choose("medical", None));
        let medical = modes.active().policy();
        assert!(!medical.context_preview);
        assert_eq!(medical.redaction, RedactionLevel::Strict);
        assert!(medical.proactive.sticky_important);
    }

    #[test]
    fn every_official_mode_stays_on_this_pc_and_reads_every_app() {
        for pack in ModeManager::official().unwrap().packs() {
            let policy = pack.policy();
            assert_eq!(policy.egress, Egress::LocalOnly);
            assert_eq!(policy.apps.allow, None);
            // Berkas dibuka dengan akhir baris LF (.gitattributes); CR akan ikut terkirim ke model.
            assert!(!pack.ask_prompt().contains('\r'));
            if let Some(suggest) = pack.suggest() {
                assert!(!suggest.prompt().contains('\r'));
            }
        }
    }

    #[test]
    fn umum_suggests_and_medical_waits_for_phase_3() {
        let mut modes = ModeManager::official().unwrap();
        assert!(modes.active().policy().proactive.enabled);
        assert!(modes.active().suggest().is_some());

        assert!(modes.choose("medical", None));
        assert!(!modes.active().policy().proactive.enabled);
        assert_eq!(modes.active().suggest(), None);
    }
}
