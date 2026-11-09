//! Teks yang sudah disamarkan, daftar nama samarannya, dan pengembaliannya di jawaban.

use std::collections::BTreeMap;

use nuenly_core::SensitiveText;
use zeroize::Zeroizing;

use crate::detect::Span;
use crate::kind::Kind;

/// Banyaknya data pribadi yang disamarkan per jenis, tanpa nilainya. Aman untuk audit dan log.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Findings(BTreeMap<Kind, u32>);

impl Findings {
    /// Jumlah kemunculan yang disamarkan untuk `kind`.
    #[must_use]
    pub fn count(&self, kind: Kind) -> u32 {
        self.0.get(&kind).copied().unwrap_or(0)
    }

    /// Jumlah semua kemunculan yang disamarkan.
    #[must_use]
    pub fn total(&self) -> u32 {
        self.0.values().sum()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Jenis dan jumlahnya, terurut menurut [`Kind`].
    pub fn iter(&self) -> impl Iterator<Item = (Kind, u32)> + '_ {
        self.0.iter().map(|(&kind, &count)| (kind, count))
    }

    fn add(&mut self, kind: Kind) {
        *self.0.entry(kind).or_default() += 1;
    }
}

/// Satu nama samaran dan nilai aslinya.
#[derive(Debug)]
struct Pseudonym {
    kind: Kind,
    /// Bentuk baku nilai, agar penulisan lain dari data yang sama memakai samaran yang sama.
    key: SensitiveText,
    /// Misalnya `[NAMA-1]`.
    placeholder: String,
    original: SensitiveText,
}

/// Teks yang aman dikirim keluar perangkat, beserta nama samarannya untuk dikembalikan di
/// jawaban. Daftar nama samaran hanya ada di memori dan dihapus saat nilai ini di-drop.
#[derive(Debug)]
pub struct Redacted {
    text: SensitiveText,
    pseudonyms: Vec<Pseudonym>,
    found: Findings,
}

impl Redacted {
    /// Mengganti setiap rentang di `spans` (terurut, tanpa tumpang tindih) dengan nama
    /// samarannya. Nilai yang sama mendapat nama samaran yang sama.
    pub(crate) fn new(normalized: &str, spans: &[Span]) -> Self {
        let mut text = Zeroizing::new(String::with_capacity(normalized.len()));
        let mut pseudonyms: Vec<Pseudonym> = Vec::new();
        let mut found = Findings::default();
        let mut at = 0;
        for span in spans {
            text.push_str(&normalized[at..span.start]);
            let original = &normalized[span.start..span.end];
            let key = SensitiveText::new(span.kind.canonical(original));
            let known = pseudonyms
                .iter()
                .find(|pseudonym| pseudonym.kind == span.kind && pseudonym.key == key);
            if let Some(known) = known {
                text.push_str(&known.placeholder);
            } else {
                let placeholder = placeholder(span.kind, &pseudonyms, normalized);
                text.push_str(&placeholder);
                pseudonyms.push(Pseudonym {
                    kind: span.kind,
                    key,
                    placeholder,
                    original: SensitiveText::new(original),
                });
            }
            found.add(span.kind);
            at = span.end;
        }
        text.push_str(&normalized[at..]);
        Self {
            text: SensitiveText::new(std::mem::take(&mut *text)),
            pseudonyms,
            found,
        }
    }

    /// Teks yang dikirim keluar perangkat.
    #[must_use]
    pub fn text(&self) -> &SensitiveText {
        &self.text
    }

    /// Apa saja yang disamarkan.
    #[must_use]
    pub fn found(&self) -> &Findings {
        &self.found
    }

    /// Jawaban utuh dengan nama samaran dikembalikan ke nilai aslinya. Nama samaran yang tidak
    /// dikenal dibiarkan.
    #[must_use]
    pub fn restore(&self, answer: &str) -> SensitiveText {
        let mut restored = Zeroizing::new(String::with_capacity(answer.len()));
        self.restore_into(answer, &mut restored);
        SensitiveText::new(std::mem::take(&mut *restored))
    }

    /// Pengembali untuk jawaban yang datang potong demi potong.
    #[must_use]
    pub fn restorer(&self) -> Restorer<'_> {
        Restorer {
            redacted: self,
            pending: Zeroizing::new(String::new()),
        }
    }

    fn restore_into(&self, answer: &str, restored: &mut String) {
        let mut rest = answer;
        while let Some(open) = rest.find('[') {
            restored.push_str(&rest[..open]);
            let tail = &rest[open..];
            let known = self
                .pseudonyms
                .iter()
                .find(|pseudonym| tail.starts_with(&pseudonym.placeholder));
            if let Some(known) = known {
                restored.push_str(known.original.expose());
                rest = &tail[known.placeholder.len()..];
            } else {
                restored.push('[');
                rest = &tail[1..];
            }
        }
        restored.push_str(rest);
    }

    /// `true` bila `tail` (diawali `[`) masih bisa menjadi awal salah satu nama samaran.
    fn may_continue(&self, tail: &str) -> bool {
        self.pseudonyms
            .iter()
            .any(|pseudonym| pseudonym.placeholder.starts_with(tail))
    }
}

/// Nama samaran berikutnya untuk `kind`. Nomor yang tertulis apa adanya di teks asli dilewati,
/// agar pengembalian tidak mengganti teks yang memang berbunyi seperti nama samaran.
fn placeholder(kind: Kind, pseudonyms: &[Pseudonym], normalized: &str) -> String {
    let mut number = pseudonyms
        .iter()
        .filter(|pseudonym| pseudonym.kind == kind)
        .count();
    loop {
        number += 1;
        let candidate = format!("[{}-{number}]", kind.label());
        let taken = normalized.contains(candidate.as_str())
            || pseudonyms
                .iter()
                .any(|pseudonym| pseudonym.placeholder == candidate);
        if !taken {
            return candidate;
        }
    }
}

/// Mengembalikan nama samaran di jawaban streaming. Potongan yang mungkin masih awal sebuah nama
/// samaran ditahan sampai potongan berikutnya datang.
#[derive(Debug)]
pub struct Restorer<'a> {
    redacted: &'a Redacted,
    pending: Zeroizing<String>,
}

impl Restorer<'_> {
    /// Bagian jawaban yang sudah bisa ditampilkan setelah `chunk` datang; bisa kosong.
    pub fn push(&mut self, chunk: &str) -> SensitiveText {
        self.pending.push_str(chunk);
        // Hanya `[` terakhir yang bisa menjadi nama samaran yang belum selesai, karena nama
        // samaran tidak memuat `[`.
        let hold = self
            .pending
            .rfind('[')
            .filter(|&open| self.redacted.may_continue(&self.pending[open..]))
            .unwrap_or(self.pending.len());
        let ready = Zeroizing::new(self.pending.drain(..hold).collect::<String>());
        self.redacted.restore(&ready)
    }

    /// Sisa jawaban setelah potongan terakhir.
    #[must_use]
    pub fn finish(mut self) -> SensitiveText {
        let rest = Zeroizing::new(std::mem::take(&mut *self.pending));
        self.redacted.restore(&rest)
    }
}

#[cfg(test)]
mod tests {
    use super::{Kind, Redacted, Span};

    fn redact(text: &str, spans: &[(usize, usize, Kind)]) -> Redacted {
        let spans: Vec<Span> = spans
            .iter()
            .map(|&(start, end, kind)| Span { start, end, kind })
            .collect();
        Redacted::new(text, &spans)
    }

    #[test]
    fn the_same_value_gets_the_same_placeholder() {
        let text = "Budi dan Siti; BUDI lagi";
        let redacted = redact(
            text,
            &[
                (0, 4, Kind::Name),
                (9, 13, Kind::Name),
                (15, 19, Kind::Name),
            ],
        );
        assert_eq!(
            redacted.text().expose(),
            "[NAMA-1] dan [NAMA-2]; [NAMA-1] lagi"
        );
        assert_eq!(redacted.found().count(Kind::Name), 3);
        assert_eq!(redacted.found().total(), 3);
    }

    #[test]
    fn placeholders_already_in_the_text_are_skipped() {
        let text = "[NAMA-1] adalah contoh, Budi bukan";
        let redacted = redact(text, &[(24, 28, Kind::Name)]);
        assert_eq!(
            redacted.text().expose(),
            "[NAMA-1] adalah contoh, [NAMA-2] bukan"
        );
        // Teks yang memang berbunyi seperti nama samaran tidak ikut diganti.
        assert_eq!(
            redacted.restore("[NAMA-1] dan [NAMA-2]").expose(),
            "[NAMA-1] dan Budi"
        );
    }

    #[test]
    fn restoring_puts_the_original_values_back() {
        let text = "Telp 0812-0000-1234 a.n. Budi";
        let redacted = redact(text, &[(5, 19, Kind::Phone), (25, 29, Kind::Name)]);
        assert_eq!(redacted.text().expose(), "Telp [TELEPON-1] a.n. [NAMA-1]");
        assert_eq!(
            redacted
                .restore("Hubungi [NAMA-1] di [TELEPON-1] atau [NAMA-9] [ x")
                .expose(),
            "Hubungi Budi di 0812-0000-1234 atau [NAMA-9] [ x"
        );
    }

    #[test]
    fn streaming_restores_placeholders_split_across_chunks() {
        let text = "Budi, 0812-0000-1234";
        let redacted = redact(text, &[(0, 4, Kind::Name), (6, 20, Kind::Phone)]);
        let answer = "Halo [NAMA-1], nomor [TELEPON-1] tercatat [x] [NAMA-";
        let expected = redacted.restore(answer);
        for size in 1..=answer.len() {
            let mut restorer = redacted.restorer();
            let mut streamed = String::new();
            let mut rest = answer;
            while !rest.is_empty() {
                let cut = rest
                    .char_indices()
                    .map(|(index, _)| index)
                    .find(|&index| index >= size)
                    .unwrap_or(rest.len());
                let (chunk, tail) = rest.split_at(cut);
                streamed.push_str(restorer.push(chunk).expose());
                rest = tail;
            }
            streamed.push_str(restorer.finish().expose());
            assert_eq!(streamed, expected.expose(), "potongan {size}");
        }
    }

    #[test]
    fn debug_output_hides_the_values() {
        let text = "Budi Santoso";
        let redacted = redact(text, &[(0, 12, Kind::Name)]);
        let debug = format!("{redacted:?}");
        assert!(!debug.contains("Budi"), "{debug}");
        assert!(debug.contains("[NAMA-1]"));
    }
}
