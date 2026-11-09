//! Pendeteksi data pribadi: pola regex, kamus label, dan validasi struktur nomor.

use std::cmp::Reverse;
use std::ops::Range;

use nuenly_core::RedactionLevel;
use regex::Regex;

use crate::kind::Kind;
use crate::patterns as p;
use crate::validate;

/// Rentang byte data pribadi di teks yang sudah dinormalisasi.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Span {
    pub start: usize,
    pub end: usize,
    pub kind: Kind,
}

/// Pemeriksaan lanjutan untuk satu kecocokan regex.
#[derive(Clone, Copy, Debug)]
enum Check {
    /// Diterima apa adanya.
    Any,
    /// Tidak menempel pada angka lain.
    Isolated,
    /// Telepon Indonesia 9–13 digit (bentuk lokal). Kelompok angka di belakang yang membuatnya
    /// terlalu panjang dilepas, karena mungkin nomor lain yang hanya dipisah spasi.
    IndonesianPhone,
    /// Telepon internasional 8–15 digit.
    InternationalPhone,
    Nik,
    Card,
    Date,
    /// Nilai label berupa angka, paling sedikit sekian digit.
    Digits(usize),
    /// Nomor rekam medis: paling sedikit tiga karakter dan memuat angka.
    Record,
    /// Nama: berhenti di label kolom berikutnya atau kata sambung.
    Name,
    /// Alamat: berhenti di dua spasi atau label kolom berikutnya.
    Address,
}

#[derive(Debug)]
struct Rule {
    kind: Kind,
    regex: Regex,
    check: Check,
    /// Level terendah yang memakai aturan ini: `Standard` berarti selalu.
    level: RedactionLevel,
}

impl Rule {
    fn new(
        kind: Kind,
        source: &str,
        check: Check,
        level: RedactionLevel,
    ) -> Result<Self, regex::Error> {
        Ok(Self {
            kind,
            regex: Regex::new(source)?,
            check,
            level,
        })
    }

    fn applies(&self, level: RedactionLevel) -> bool {
        self.level == RedactionLevel::Standard || level == RedactionLevel::Strict
    }
}

#[derive(Debug)]
pub(crate) struct Detector {
    rules: Vec<Rule>,
    next_label: Regex,
}

impl Detector {
    pub(crate) fn new() -> Result<Self, regex::Error> {
        use Kind as K;
        use RedactionLevel::{Standard, Strict};
        let rules = vec![
            Rule::new(K::Email, p::EMAIL, Check::Any, Standard)?,
            Rule::new(K::Phone, p::PHONE_ID, Check::IndonesianPhone, Standard)?,
            Rule::new(K::Phone, p::PHONE_INTL, Check::InternationalPhone, Standard)?,
            Rule::new(K::Nik, p::NIK, Check::Nik, Standard)?,
            Rule::new(K::Npwp, p::NPWP, Check::Isolated, Standard)?,
            Rule::new(K::Bpjs, p::BPJS, Check::Isolated, Standard)?,
            Rule::new(K::Card, p::CARD, Check::Card, Standard)?,
            Rule::new(K::Address, p::STREET, Check::Any, Standard)?,
            Rule::new(K::Name, &p::honorific(), Check::Name, Standard)?,
            Rule::new(
                K::Name,
                &p::word_field(p::NAME_LABELS, &p::name_value()),
                Check::Name,
                Standard,
            )?,
            Rule::new(
                K::Address,
                &p::word_field(p::ADDRESS_LABELS, p::ADDRESS_VALUE),
                Check::Address,
                Standard,
            )?,
            Rule::new(
                K::BirthDate,
                &p::number_field(p::BIRTH_LABELS, &p::birth_value()),
                Check::Any,
                Standard,
            )?,
            Rule::new(
                K::Nik,
                &p::number_field(p::NIK_LABELS, p::NUMBER_VALUE),
                Check::Digits(8),
                Standard,
            )?,
            Rule::new(
                K::Bpjs,
                &p::number_field(p::BPJS_LABELS, p::NUMBER_VALUE),
                Check::Digits(8),
                Standard,
            )?,
            Rule::new(
                K::Npwp,
                &p::number_field(p::NPWP_LABELS, p::NUMBER_VALUE),
                Check::Digits(9),
                Standard,
            )?,
            Rule::new(
                K::Account,
                &p::number_field(p::ACCOUNT_LABELS, p::NUMBER_VALUE),
                Check::Digits(6),
                Standard,
            )?,
            Rule::new(
                K::Phone,
                &p::number_field(p::PHONE_LABELS, p::PHONE_VALUE),
                Check::Digits(6),
                Standard,
            )?,
            Rule::new(
                K::MedicalRecord,
                &p::number_field(p::RECORD_LABELS, p::RECORD_VALUE),
                Check::Record,
                Standard,
            )?,
            Rule::new(K::Date, &p::date_alone(), Check::Date, Strict)?,
            Rule::new(K::Number, p::NUMBER, Check::Any, Strict)?,
            Rule::new(K::Name, &p::strict_honorific(), Check::Name, Strict)?,
            Rule::new(K::Address, p::STREET_LOOSE, Check::Any, Strict)?,
            Rule::new(K::Address, p::RT_RW, Check::Any, Strict)?,
        ];
        Ok(Self {
            rules,
            next_label: Regex::new(&p::next_label())?,
        })
    }

    /// Data pribadi di `text` untuk `level`, terurut dan tanpa tumpang tindih.
    pub(crate) fn spans(&self, text: &str, level: RedactionLevel) -> Vec<Span> {
        let mut spans = Vec::new();
        for rule in self.rules.iter().filter(|rule| rule.applies(level)) {
            self.scan(rule, text, &mut spans);
        }
        merge(spans)
    }

    /// Semua kecocokan `rule` yang lolos pemeriksaannya. Bila ditolak, pencarian diulang satu
    /// karakter setelah awal kecocokan itu, agar kecocokan sah di dalamnya tidak terlewat.
    fn scan(&self, rule: &Rule, text: &str, spans: &mut Vec<Span>) {
        let mut at = 0;
        while at < text.len() {
            let Some(captures) = rule.regex.captures_at(text, at) else {
                break;
            };
            let Some(whole) = captures.get(0) else {
                break;
            };
            let value = captures.name("value").unwrap_or(whole);
            let next = next_char(text, whole.start());
            match self.check(rule.check, text, value.range()) {
                Some(range) => {
                    at = range.end.max(next);
                    spans.push(Span {
                        start: range.start,
                        end: range.end,
                        kind: rule.kind,
                    });
                }
                None => at = next,
            }
        }
    }

    fn check(&self, check: Check, text: &str, range: Range<usize>) -> Option<Range<usize>> {
        let written = &text[range.clone()];
        let accepted = match check {
            Check::Any => true,
            Check::Isolated => isolated(text, &range),
            Check::IndonesianPhone => return indonesian_phone(text, range),
            Check::InternationalPhone => {
                isolated(text, &range) && (8..=15).contains(&validate::digits(written).len())
            }
            Check::Nik => isolated(text, &range) && validate::nik(&validate::digits(written)),
            Check::Card => isolated(text, &range) && validate::card(&validate::digits(written)),
            Check::Date => isolated(text, &range) && validate::date(written),
            Check::Digits(min) => validate::digits(written).len() >= min,
            Check::Record => written.len() >= 3 && written.bytes().any(|b| b.is_ascii_digit()),
            Check::Name => return name(range, written),
            Check::Address => return self.address(range, written),
        };
        accepted.then_some(range)
    }

    fn address(&self, range: Range<usize>, written: &str) -> Option<Range<usize>> {
        let mut end = written.find("  ").unwrap_or(written.len());
        if let Some(label) = self.next_label.find(&written[..end]) {
            end = label.start();
        }
        let kept = written[..end].trim_end_matches(|c: char| {
            c.is_whitespace() || matches!(c, ',' | '.' | ';' | ':' | '-')
        });
        (kept.chars().count() >= 3).then(|| range.start..range.start + kept.len())
    }
}

/// Indeks byte karakter setelah karakter di `at`.
fn next_char(text: &str, at: usize) -> usize {
    text[at..]
        .chars()
        .next()
        .map_or(text.len(), |c| at + c.len_utf8())
}

/// `true` bila rentang tidak menempel pada angka di kiri atau kanannya.
fn isolated(text: &str, range: &Range<usize>) -> bool {
    let before = text[..range.start].chars().next_back();
    let after = text[range.end..].chars().next();
    !before.is_some_and(|c| c.is_ascii_digit()) && !after.is_some_and(|c| c.is_ascii_digit())
}

/// Awalan terpanjang yang berakhir di ujung kelompok angka dan panjangnya 9–13 digit dalam bentuk
/// lokal (`62` di depan dihitung sebagai `0`).
fn indonesian_phone(text: &str, range: Range<usize>) -> Option<Range<usize>> {
    let written = &text[range.clone()];
    let country = written.trim_start_matches(['+', '(']).starts_with("62");
    let mut digits = 0usize;
    let mut longest = None;
    let mut chars = written.char_indices().peekable();
    while let Some((index, c)) = chars.next() {
        if !c.is_ascii_digit() {
            continue;
        }
        digits += 1;
        let group_ends = chars.peek().is_none_or(|&(_, next)| !next.is_ascii_digit());
        let local = if country { digits - 1 } else { digits };
        if group_ends && (9..=13).contains(&local) {
            longest = Some(index + c.len_utf8());
        }
    }
    let found = range.start..range.start + longest?;
    isolated(text, &found).then_some(found)
}

/// Nama sampai sebelum kata henti pertama, tanpa sapaan di depan dan tanda baca di ujungnya.
fn name(range: Range<usize>, written: &str) -> Option<Range<usize>> {
    let listed =
        |list: &[&str], word: &str| list.iter().any(|item| word.eq_ignore_ascii_case(item));
    let mut start = 0;
    let mut end = 0;
    let mut offset = 0;
    for word in written.split(' ') {
        let bare = word.trim_end_matches('.');
        let title =
            listed(p::TITLES, bare) || (word.ends_with('.') && listed(p::DOTTED_TITLES, bare));
        if end == 0 && title {
            start = offset + word.len() + 1;
        } else if listed(p::STOP_WORDS, bare) {
            break;
        } else {
            end = offset + word.len();
        }
        offset += word.len() + 1;
    }
    if end <= start {
        return None;
    }
    let kept = written[start..end].trim_end_matches(['.', '-', '\'', '’']);
    let start = range.start + start;
    (!kept.is_empty()).then(|| start..start + kept.len())
}

/// Mengurutkan temuan dan menggabungkan yang tumpang tindih, agar setiap karakter yang terdeteksi
/// ikut disamarkan. Jenisnya dari temuan yang mulai paling awal (lalu yang terpanjang).
fn merge(mut spans: Vec<Span>) -> Vec<Span> {
    spans.sort_by_key(|span| (span.start, Reverse(span.end), span.kind));
    let mut merged: Vec<Span> = Vec::with_capacity(spans.len());
    for span in spans {
        match merged.last_mut() {
            Some(last) if span.start < last.end => last.end = last.end.max(span.end),
            _ => merged.push(span),
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use nuenly_core::RedactionLevel::{self, Standard, Strict};

    use super::{Detector, Kind, Span, merge};

    fn found(text: &str, level: RedactionLevel) -> Vec<(Kind, &str)> {
        Detector::new()
            .unwrap()
            .spans(text, level)
            .iter()
            .map(|span| (span.kind, &text[span.start..span.end]))
            .collect()
    }

    fn assert_found(text: &str, expected: &[(Kind, &str)]) {
        assert_eq!(found(text, Standard), expected, "{text}");
    }

    fn assert_nothing(text: &str, level: RedactionLevel) {
        assert_eq!(found(text, level), [], "{text}");
    }

    #[test]
    fn emails() {
        assert_found(
            "kirim ke budi.s+klinik@contoh.invalid ya",
            &[(Kind::Email, "budi.s+klinik@contoh.invalid")],
        );
    }

    #[test]
    fn indonesian_phones() {
        for phone in [
            "0812-0000-1234",
            "081200001234",
            "+62 812 0000 1234",
            "+62-812-0000-1234",
            "6281200001234",
            "(021) 555-0123",
            "021 5550123",
        ] {
            assert_found(&format!("hubungi {phone}."), &[(Kind::Phone, phone)]);
        }
        assert_found("WA:081200001234", &[(Kind::Phone, "081200001234")]);
    }

    #[test]
    fn phones_separated_by_one_space_stay_two_numbers() {
        assert_found(
            "0812-0000-1234 0813-1111-2222",
            &[
                (Kind::Phone, "0812-0000-1234"),
                (Kind::Phone, "0813-1111-2222"),
            ],
        );
    }

    #[test]
    fn numbers_that_only_look_like_phones() {
        for text in [
            "harga Rp 62.500.000.000",
            "versi 0.12.3",
            "kode 0812",
            "nomor 08120000123456789",
            "tahun 2026 ada 1500 pengguna",
        ] {
            assert_nothing(text, Standard);
        }
    }

    #[test]
    fn international_phones() {
        assert_found(
            "call +1 415 555 0100 now",
            &[(Kind::Phone, "+1 415 555 0100")],
        );
    }

    #[test]
    fn population_numbers() {
        // Kode kabupaten 99 tidak ada, jadi bukan NIK sungguhan.
        assert_found("3299994501900001", &[(Kind::Nik, "3299994501900001")]);
        assert_found(
            "nomor 3299 9945 0190 0001 terdaftar",
            &[(Kind::Nik, "3299 9945 0190 0001")],
        );
        assert_found(
            "32.99.99.450190.0001",
            &[(Kind::Nik, "32.99.99.450190.0001")],
        );
        // Provinsi 99 tidak ada, dan tanpa label tidak disamarkan di level standar.
        assert_nothing("kode 9999994501900001", Standard);
        assert_found("NIK: 9999994501900001", &[(Kind::Nik, "9999994501900001")]);
        assert_found("No. KTP 1234 5678", &[(Kind::Nik, "1234 5678")]);
    }

    #[test]
    fn tax_insurance_card_and_account_numbers() {
        assert_found(
            "NPWP 99.888.777.6-555.444",
            &[(Kind::Npwp, "99.888.777.6-555.444")],
        );
        assert_found("kartu 0009999999999", &[(Kind::Bpjs, "0009999999999")]);
        assert_found("No. BPJS: 1234567890123", &[(Kind::Bpjs, "1234567890123")]);
        assert_found(
            "kartu 4111 1111 1111 1111 hilang",
            &[(Kind::Card, "4111 1111 1111 1111")],
        );
        assert_nothing("kode 4111 1111 1111 1112", Standard);
        assert_found("No. Rek: 1234567890", &[(Kind::Account, "1234567890")]);
        assert_found("rekening 123-456-7890", &[(Kind::Account, "123-456-7890")]);
    }

    #[test]
    fn medical_record_numbers() {
        assert_found("No. RM: 00-12-34", &[(Kind::MedicalRecord, "00-12-34")]);
        assert_found("No RM 123456", &[(Kind::MedicalRecord, "123456")]);
        assert_found("MRN: RM2026-0001", &[(Kind::MedicalRecord, "RM2026-0001")]);
        assert_nothing("rm -rf build", Standard);
    }

    #[test]
    fn birth_dates_with_their_place() {
        assert_found("Tgl. Lahir: 12-03-1985", &[(Kind::BirthDate, "12-03-1985")]);
        assert_found(
            "TTL: Bandung, 12 Maret 1985",
            &[(Kind::BirthDate, "Bandung, 12 Maret 1985")],
        );
        assert_found(
            "Tempat/Tgl Lahir : Kota Contoh / 1985-03-12",
            &[(Kind::BirthDate, "Kota Contoh / 1985-03-12")],
        );
    }

    #[test]
    fn names_after_labels() {
        assert_found("Nama Pasien: Budi Santoso", &[(Kind::Name, "Budi Santoso")]);
        assert_found(
            "NAMA : SITI AMINAH   Umur: 45",
            &[(Kind::Name, "SITI AMINAH")],
        );
        assert_found(
            "Nama: budi santoso datang kontrol",
            &[(Kind::Name, "budi santoso")],
        );
        assert_found("Nama\nSiti Aminah\n", &[(Kind::Name, "Siti Aminah")]);
        assert_found(
            "Umur: 45 | Nama ibu kandung: Siti Aminah binti Ahmad.",
            &[(Kind::Name, "Siti Aminah binti Ahmad")],
        );
    }

    #[test]
    fn titles_before_a_labelled_name_stay_visible() {
        assert_found(
            "Nama Pasien: Tn. Budi Santoso",
            &[(Kind::Name, "Budi Santoso")],
        );
        assert_found("Nama: Hj. Siti Aminah", &[(Kind::Name, "Siti Aminah")]);
        assert_found("Nama: dr. Andi", &[(Kind::Name, "Andi")]);
        // Tanpa titik, "An" bisa bagian nama.
        assert_found("Nama: An Nisa", &[(Kind::Name, "An Nisa")]);
        assert_nothing("Nama: Tn.", Standard);
    }

    #[test]
    fn labels_inside_sentences_are_not_names() {
        for text in [
            "Keluhan pasien: demam 3 hari",
            "Nama obat: Paracetamol",
            "Data pasien\nTekanan darah normal",
            "dengan nama Budi",
        ] {
            assert_nothing(text, Standard);
        }
    }

    #[test]
    fn names_after_honorifics() {
        assert_found(
            "Ny. Siti Aminah datang kontrol",
            &[(Kind::Name, "Siti Aminah")],
        );
        assert_found("TN. BUDI SANTOSO UMUR 45", &[(Kind::Name, "BUDI SANTOSO")]);
        assert_found(
            "transfer a.n. Dewi Lestari, terima kasih",
            &[(Kind::Name, "Dewi Lestari")],
        );
        assert_found("An.Rizki demam", &[(Kind::Name, "Rizki")]);
        // Tanpa nama berhuruf besar, sapaan tidak berarti apa-apa.
        assert_nothing("ny. dan tn. lain", Standard);
        assert_nothing("TD 120/80, BP 120/80", Standard);
    }

    #[test]
    fn addresses() {
        assert_found(
            "Alamat: Jl. Mawar No. 12, RT 03/RW 05   Telp: 0812-0000-1234",
            &[
                (Kind::Address, "Jl. Mawar No. 12, RT 03/RW 05"),
                (Kind::Phone, "0812-0000-1234"),
            ],
        );
        assert_found(
            "Alamat: Kp. Contoh, Kel. Sukamaju Umur: 45",
            &[(Kind::Address, "Kp. Contoh, Kel. Sukamaju")],
        );
        assert_found(
            "rumah di Jl. Mawar Indah No. 12.",
            &[(Kind::Address, "Jl. Mawar Indah No. 12")],
        );
        assert_nothing("jalan kaki ke Jl. Sudirman", Standard);
    }

    #[test]
    fn the_strict_level_also_hides_dates_numbers_and_more_names() {
        let text = "dr. Andi Wijaya, Sp.PD · kontrol 20/11/2026 · kode 12345678 · RT 03/RW 05 · Ibu Siti · Jl. Sudirman";
        assert_nothing(text, Standard);
        assert_eq!(
            found(text, Strict),
            [
                (Kind::Name, "Andi Wijaya"),
                (Kind::Date, "20/11/2026"),
                (Kind::Number, "12345678"),
                (Kind::Address, "RT 03/RW 05"),
                (Kind::Name, "Siti"),
                (Kind::Address, "Jl. Sudirman"),
            ]
        );
    }

    #[test]
    fn ordinary_text_stays_as_it_is() {
        let text = "Versi 2.4.1 dirilis tahun 2026 untuk 1500 pengguna; harga Rp 25.000. \
                    Keluhan demam 3 hari, batuk berdahak. Trombosit 250000, TD 120/80 mmHg. \
                    Rapat pukul 10.00 di ruang 3.";
        assert_nothing(text, Standard);
        assert_nothing(text, Strict);
    }

    #[test]
    fn overlapping_findings_are_merged() {
        let span = |start, end, kind| Span { start, end, kind };
        assert_eq!(
            merge(vec![
                span(10, 20, Kind::Phone),
                span(0, 12, Kind::Address),
                span(30, 35, Kind::Email),
                span(30, 33, Kind::Name),
            ]),
            [span(0, 20, Kind::Address), span(30, 35, Kind::Email)]
        );
    }
}
