//! Daftar mode pack dan mode yang sedang aktif.

use crate::error::ModeError;
use crate::{ModePack, official};

/// Memegang mode pack yang tersedia dan memilih yang aktif. Modul lain membaca
/// [`ModePack::policy`] dan prompt mode aktif, bukan ID-nya.
#[derive(Clone, Debug)]
pub struct ModeManager {
    packs: Vec<ModePack>,
    /// Selalu indeks yang valid di `packs`.
    active: usize,
}

impl ModeManager {
    /// Mode pack pertama menjadi mode awal.
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
        Ok(Self { packs, active: 0 })
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

    /// Mengganti mode aktif; `false` bila `id` tidak dikenal.
    pub fn activate(&mut self, id: &str) -> bool {
        match self.packs.iter().position(|pack| pack.id().as_str() == id) {
            Some(index) => {
                self.active = index;
                true
            }
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ModeManager;
    use crate::{ModeError, ModePack};

    fn pack(id: &str) -> ModePack {
        ModePack::parse(
            &format!("id = \"{id}\"\nlabel = \"Uji\"\naccent = \"#000000\"\n"),
            "Kamu asisten uji.",
        )
        .unwrap()
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
        let mut modes = ModeManager::new(vec![pack("satu"), pack("dua")]).unwrap();
        assert_eq!(modes.active().id().as_str(), "satu");
        assert!(modes.activate("dua"));
        assert!(!modes.activate("tiga"));
        assert_eq!(modes.active().id().as_str(), "dua");
    }
}
