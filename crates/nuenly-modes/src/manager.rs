//! Daftar mode pack, mode yang sedang aktif, dan mode yang diingat per aplikasi.

use std::collections::BTreeMap;

use nuenly_core::{ModeId, ProcessName};

use crate::error::ModeError;
use crate::{ModePack, official};

/// Memegang mode pack yang tersedia dan memilih yang aktif. Modul lain membaca
/// [`ModePack::policy`] dan prompt mode aktif, bukan ID-nya.
///
/// Mode mengikuti aplikasi di latar depan: aplikasi yang pernah dipilihkan mode secara manual
/// memakai mode itu, aplikasi lain memakai mode pertama.
#[derive(Clone, Debug)]
pub struct ModeManager {
    packs: Vec<ModePack>,
    /// Selalu indeks yang valid di `packs`, begitu juga nilai di `remembered`.
    active: usize,
    /// Mode pilihan pengguna per aplikasi. Mode pertama tidak pernah disimpan di sini, karena
    /// sudah menjadi bawaan.
    remembered: BTreeMap<ProcessName, usize>,
}

impl ModeManager {
    /// Mode pack pertama menjadi mode awal dan mode bawaan untuk aplikasi yang belum diatur.
    ///
    /// # Errors
    ///
    /// [`ModeError::NoModes`] bila `packs` kosong, atau [`ModeError::DuplicateId`] bila dua mode
    /// pack memakai ID yang sama.
    pub fn new(packs: Vec<ModePack>) -> Result<Self, ModeError> {
        if packs.is_empty() {
            return Err(ModeError::NoModes);
        }
        for (index, pack) in packs.iter().enumerate() {
            if packs[..index].iter().any(|other| other.id() == pack.id()) {
                return Err(ModeError::DuplicateId(pack.id().as_str().to_owned()));
            }
        }
        Ok(Self {
            packs,
            active: 0,
            remembered: BTreeMap::new(),
        })
    }

    /// Mode pack resmi (Umum, lalu Medical) yang ikut di dalam binary.
    ///
    /// # Errors
    ///
    /// [`ModeError`] bila mode pack resmi rusak. Test crate ini memastikan hal itu tidak terjadi.
    pub fn official() -> Result<Self, ModeError> {
        Self::new(official::packs()?)
    }

    /// Semua mode, dalam urutan tampil di UI.
    #[must_use]
    pub fn packs(&self) -> &[ModePack] {
        &self.packs
    }

    #[must_use]
    pub fn active(&self) -> &ModePack {
        &self.packs[self.active]
    }

    /// Aplikasi di latar depan berganti ke `app`: mode yang diingat untuk aplikasi itu menjadi
    /// aktif, atau mode pertama bila belum ada.
    pub fn follow(&mut self, app: &ProcessName) {
        self.active = self.remembered.get(app).copied().unwrap_or(0);
    }

    /// Pilihan manual: `id` menjadi mode aktif dan diingat untuk `app`, bila ada. Memilih mode
    /// pertama melupakan pilihan untuk aplikasi itu. `false` bila `id` tidak dikenal.
    pub fn choose(&mut self, id: &str, app: Option<&ProcessName>) -> bool {
        let Some(index) = self.index_of(id) else {
            return false;
        };
        self.active = index;
        if let Some(app) = app {
            if index == 0 {
                self.remembered.remove(app);
            } else {
                self.remembered.insert(app.clone(), index);
            }
        }
        true
    }

    /// Pilihan yang diingat, untuk disimpan: aplikasi dan ID modenya.
    pub fn remembered(&self) -> impl Iterator<Item = (&ProcessName, &ModeId)> {
        self.remembered
            .iter()
            .map(|(app, &index)| (app, self.packs[index].id()))
    }

    /// Memulihkan pilihan yang tersimpan sebagai pasangan nama executable dan ID mode. Pasangan
    /// dengan nama yang tidak valid atau mode yang tidak dikenal (misalnya mode yang sudah
    /// dihapus) dilewati.
    pub fn restore<'a>(&mut self, remembered: impl IntoIterator<Item = (&'a str, &'a str)>) {
        for (app, id) in remembered {
            if let (Ok(app), Some(index)) = (ProcessName::new(app), self.index_of(id))
                && index != 0
            {
                self.remembered.insert(app, index);
            }
        }
    }

    fn index_of(&self, id: &str) -> Option<usize> {
        self.packs.iter().position(|pack| pack.id().as_str() == id)
    }
}

#[cfg(test)]
mod tests {
    use nuenly_core::ProcessName;

    use super::ModeManager;
    use crate::{ModeError, ModePack};

    /// Tidak ada pilihan yang diingat.
    const NONE: [(&str, &str); 0] = [];

    fn pack(id: &str) -> ModePack {
        ModePack::parse(
            &format!("id = \"{id}\"\nlabel = \"Uji\"\naccent = \"#000000\"\n"),
            "Kamu asisten uji.",
        )
        .unwrap()
    }

    fn modes() -> ModeManager {
        ModeManager::new(vec![pack("satu"), pack("dua")]).unwrap()
    }

    fn app(name: &str) -> ProcessName {
        ProcessName::new(name).unwrap()
    }

    fn active(modes: &ModeManager) -> &str {
        modes.active().id().as_str()
    }

    fn remembered(modes: &ModeManager) -> Vec<(&str, &str)> {
        modes
            .remembered()
            .map(|(app, id)| (app.as_str(), id.as_str()))
            .collect()
    }

    #[test]
    fn needs_at_least_one_mode_with_unique_ids() {
        assert_eq!(
            ModeManager::new(Vec::new()).unwrap_err(),
            ModeError::NoModes
        );
        assert_eq!(
            ModeManager::new(vec![pack("satu"), pack("dua"), pack("satu")]).unwrap_err(),
            ModeError::DuplicateId("satu".to_owned())
        );
    }

    #[test]
    fn starts_in_the_first_mode_and_switches_only_to_known_modes() {
        let mut modes = modes();
        assert_eq!(active(&modes), "satu");
        assert!(modes.choose("dua", None));
        assert!(!modes.choose("tiga", None));
        assert_eq!(active(&modes), "dua");
        assert_eq!(remembered(&modes), NONE);
    }

    #[test]
    fn a_manual_choice_is_remembered_for_that_app() {
        let mut modes = modes();
        assert!(modes.choose("dua", Some(&app("klinik.exe"))));

        modes.follow(&app("notepad.exe"));
        assert_eq!(active(&modes), "satu");
        modes.follow(&app("Klinik.exe"));
        assert_eq!(active(&modes), "dua");
        assert_eq!(remembered(&modes), [("klinik.exe", "dua")]);
    }

    #[test]
    fn choosing_the_first_mode_forgets_the_app() {
        let mut modes = modes();
        modes.choose("dua", Some(&app("klinik.exe")));
        modes.choose("satu", Some(&app("klinik.exe")));

        modes.follow(&app("klinik.exe"));
        assert_eq!(active(&modes), "satu");
        assert_eq!(remembered(&modes), NONE);
    }

    #[test]
    fn restore_skips_unknown_modes_and_invalid_names() {
        let mut modes = modes();
        modes.restore([
            ("klinik.exe", "dua"),
            ("lama.exe", "dihapus"),
            ("", "dua"),
            ("notepad.exe", "satu"),
        ]);
        assert_eq!(remembered(&modes), [("klinik.exe", "dua")]);
        modes.follow(&app("klinik.exe"));
        assert_eq!(active(&modes), "dua");
    }
}
