//! [`ResponseCache`] (F-18): jawaban model untuk teks, mode, model, dan prompt yang sama tidak
//! diminta ulang.

use std::collections::VecDeque;
use std::hash::{BuildHasher, RandomState};
use std::num::NonZeroUsize;

use nuenly_core::SensitiveText;

/// Kunci cache: hash SipHash berkunci acak per proses. Teksnya tidak disimpan dan hash-nya tidak
/// bisa dicocokkan di luar proses ini.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CacheKey(u64);

/// Cache LRU berbatas. Isinya hanya di memori dan hilang saat Nuenly ditutup.
#[derive(Debug)]
pub struct ResponseCache<V> {
    capacity: NonZeroUsize,
    /// Yang paling lama tidak dipakai ada di depan.
    entries: VecDeque<(CacheKey, V)>,
    hasher: RandomState,
}

impl<V: Clone> ResponseCache<V> {
    #[must_use]
    pub fn new(capacity: NonZeroUsize) -> Self {
        Self {
            capacity,
            entries: VecDeque::with_capacity(capacity.get()),
            hasher: RandomState::new(),
        }
    }

    /// Kunci untuk `text` dengan `mode`, `model`, dan versi prompt. `prompt` memuat semua bagian
    /// selain teks yang menentukan jawaban, misalnya prompt sistem, schema, dan perintah tetap, jadi
    /// mengubah salah satunya otomatis membuat kunci baru.
    #[must_use]
    pub fn key(&self, text: &SensitiveText, mode: &str, model: &str, prompt: &[&str]) -> CacheKey {
        CacheKey(self.hasher.hash_one((text.expose(), mode, model, prompt)))
    }

    /// Jawaban yang tersimpan untuk `key`; yang dipakai menjadi paling baru.
    pub fn get(&mut self, key: CacheKey) -> Option<V> {
        let index = self.entries.iter().position(|(stored, _)| *stored == key)?;
        let entry = self.entries.remove(index)?;
        let value = entry.1.clone();
        self.entries.push_back(entry);
        Some(value)
    }

    /// Menyimpan jawaban; yang paling lama tidak dipakai dibuang bila cache penuh.
    pub fn insert(&mut self, key: CacheKey, value: V) {
        self.entries.retain(|(stored, _)| *stored != key);
        if self.entries.len() == self.capacity.get() {
            self.entries.pop_front();
        }
        self.entries.push_back((key, value));
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;

    use nuenly_core::SensitiveText;

    use super::ResponseCache;

    const PROMPT: &[&str] = &["prompt sintetis", "{\"type\":\"object\"}"];

    fn cache(capacity: usize) -> ResponseCache<&'static str> {
        ResponseCache::new(NonZeroUsize::new(capacity).unwrap())
    }

    fn text(raw: &str) -> SensitiveText {
        SensitiveText::new(raw)
    }

    #[test]
    fn the_same_text_mode_model_and_prompt_hit_the_cache() {
        let mut cache = cache(4);
        let key = cache.key(&text("teks sintetis"), "all", "gemma3:4b", PROMPT);
        assert_eq!(cache.get(key), None);
        cache.insert(key, "saran");
        assert_eq!(
            cache.get(cache.key(&text("teks sintetis"), "all", "gemma3:4b", PROMPT)),
            Some("saran")
        );
    }

    #[test]
    fn every_part_of_the_key_matters() {
        let cache = cache(4);
        let key = cache.key(&text("teks sintetis"), "all", "gemma3:4b", PROMPT);
        for other in [
            cache.key(&text("teks sintetis."), "all", "gemma3:4b", PROMPT),
            cache.key(&text("teks sintetis"), "medical", "gemma3:4b", PROMPT),
            cache.key(&text("teks sintetis"), "all", "gemma3:1b", PROMPT),
            cache.key(
                &text("teks sintetis"),
                "all",
                "gemma3:4b",
                &["prompt sintetis v2"],
            ),
            // Batas antarbagian ikut dihitung.
            cache.key(&text("teks sintetisall"), "", "gemma3:4b", PROMPT),
        ] {
            assert_ne!(key, other);
        }
    }

    #[test]
    fn the_least_recently_used_answer_is_dropped_first() {
        let mut cache = cache(2);
        let [first, second, third] =
            ["satu", "dua", "tiga"].map(|raw| cache.key(&text(raw), "all", "m", PROMPT));
        cache.insert(first, "1");
        cache.insert(second, "2");
        assert_eq!(cache.get(first), Some("1"));
        cache.insert(third, "3");

        assert_eq!(cache.len(), 2);
        assert_eq!(cache.get(second), None);
        assert_eq!(cache.get(first), Some("1"));
        assert_eq!(cache.get(third), Some("3"));

        cache.insert(third, "3 baru");
        assert_eq!(cache.len(), 2);
        assert_eq!(cache.get(third), Some("3 baru"));

        cache.clear();
        assert!(cache.is_empty());
    }

    #[test]
    fn keys_differ_between_caches() {
        // Kunci acak per cache: hash dari proses atau cache lain tidak bisa dicocokkan.
        let (one, two) = (cache(1), cache(1));
        let raw = text("teks sintetis");
        assert_ne!(
            one.key(&raw, "all", "m", PROMPT),
            two.key(&raw, "all", "m", PROMPT)
        );
    }
}
