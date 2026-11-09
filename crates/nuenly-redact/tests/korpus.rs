//! Korpus sintetis: tidak ada data pribadi yang lolos, dan teks biasa tetap utuh. Bagian
//! "nol PII lolos" dari suite red-team (F-15).
//!
//! Semua data di sini sintetis. NIK memakai kode kabupaten 99 yang tidak ada, email memakai
//! domain `.invalid`, dan kartu memakai nomor uji penerbit.

use nuenly_core::RedactionLevel::{self, Standard, Strict};
use nuenly_core::SensitiveText;
use nuenly_redact::{Guard, GuardError, Kind, Redacted};

const REGISTRASI: &str = "\
REGISTRASI PASIEN RAWAT JALAN
No. RM: 00-12-34
Nama Pasien: Tn. Budi Santoso
NIK: 3299994501900001
Tgl. Lahir: 12-03-1985
TTL: Kota Contoh, 12 Maret 1985
Alamat: Jl. Mawar Indah No. 12, RT 03/RW 05, Kel. Sukamaju
No. HP: 0812-0000-1234
No. BPJS: 0009999999999
Penanggung jawab: Ny. Siti Aminah
Keluhan: demam 3 hari, batuk berdahak.
Trombosit 250000, TD 120/80 mmHg.
DPJP: dr. Andi Wijaya, Sp.PD
Kontrol 20/11/2026
";

const SURAT: &str = "\
Halo Pak Rudi,
Mohon transfer ke rekening BCA a.n. Dewi Lestari, No. Rek: 1234567890.
Kartu 4111 1111 1111 1111 jangan dipakai lagi.
NPWP kantor 99.888.777.6-555.444.
Hubungi saya di +62 812 0000 5678 atau dewi.lestari@contoh.invalid.
Rapat tanggal 12 November 2026 pukul 10.00.
";

fn redact(text: &str, level: RedactionLevel) -> Result<Redacted, GuardError> {
    Ok(Guard::new()?.redact(&SensitiveText::new(text), level))
}

fn assert_hidden(redacted: &Redacted, secrets: &[&str]) {
    let sent = redacted.text().expose();
    for secret in secrets {
        assert!(!sent.contains(secret), "lolos: {secret}\n{sent}");
    }
}

fn assert_kept(redacted: &Redacted, phrases: &[&str]) {
    let sent = redacted.text().expose();
    for phrase in phrases {
        assert!(sent.contains(phrase), "hilang: {phrase}\n{sent}");
    }
}

const PATIENT: &[&str] = &[
    "00-12-34",
    "Budi",
    "Santoso",
    "3299994501900001",
    "12-03-1985",
    "Kota Contoh",
    "12 Maret 1985",
    "Mawar",
    "Sukamaju",
    "0812-0000-1234",
    "0009999999999",
    "Siti",
    "Aminah",
];

const CLINICAL: &[&str] = &[
    "REGISTRASI PASIEN RAWAT JALAN",
    "Keluhan: demam 3 hari, batuk berdahak.",
    "Trombosit 250000, TD 120/80 mmHg.",
    "Nama Pasien: Tn. ",
    "No. HP: ",
];

#[test]
fn registration_form_at_the_standard_level() {
    let redacted = redact(REGISTRASI, Standard).unwrap();
    assert_hidden(&redacted, PATIENT);
    assert_kept(&redacted, CLINICAL);
    // Nama dokter dan tanggal kontrol baru disamarkan di level ketat.
    assert_kept(&redacted, &["dr. Andi Wijaya", "Kontrol 20/11/2026"]);
}

#[test]
fn registration_form_at_the_strict_level() {
    let redacted = redact(REGISTRASI, Strict).unwrap();
    assert_hidden(&redacted, PATIENT);
    assert_hidden(&redacted, &["Andi", "Wijaya", "20/11/2026"]);
    assert_kept(&redacted, CLINICAL);
    assert_eq!(redacted.found().count(Kind::Date), 1);
}

#[test]
fn office_letter() {
    let secrets = [
        "Dewi Lestari",
        "1234567890",
        "4111 1111 1111 1111",
        "99.888.777.6-555.444",
        "812 0000 5678",
        "dewi.lestari@contoh.invalid",
    ];
    let standard = redact(SURAT, Standard).unwrap();
    assert_hidden(&standard, &secrets);
    assert_kept(
        &standard,
        &[
            "Halo Pak Rudi,",
            "Mohon transfer ke rekening BCA a.n. [NAMA-1]",
            "Rapat tanggal 12 November 2026 pukul 10.00.",
        ],
    );

    let strict = redact(SURAT, Strict).unwrap();
    assert_hidden(&strict, &secrets);
    assert_hidden(&strict, &["Rudi", "12 November 2026"]);
    assert_kept(&strict, &["pukul 10.00."]);
}

#[test]
fn ordinary_text_is_sent_unchanged() {
    let text = "Laporan triwulan: penjualan naik 12% menjadi Rp 1.250.000.000, \
                target 2027 adalah 15.000 unit. Versi aplikasi 3.2.1 dirilis \
                pukul 09.30 di ruang rapat 4.";
    for level in [Standard, Strict] {
        let redacted = redact(text, level).unwrap();
        assert_eq!(redacted.text().expose(), text);
        assert!(redacted.found().is_empty());
    }
}

#[test]
fn answers_get_the_original_values_back() {
    let redacted = redact(REGISTRASI, Standard).unwrap();
    let sent = redacted.text().expose();
    // Model menjawab memakai nama samaran yang ia terima.
    let answer = format!(
        "Pasien {} perlu kontrol. Hubungi {}.",
        placeholder(sent, "NAMA").unwrap(),
        placeholder(sent, "TELEPON").unwrap(),
    );
    assert_eq!(
        redacted.restore(&answer).expose(),
        "Pasien Budi Santoso perlu kontrol. Hubungi 0812-0000-1234."
    );
}

#[test]
fn hidden_characters_do_not_let_numbers_through() {
    let text = "NIK 3299\u{200B}9945\u{200B}0190\u{200B}0001, HP ０８１２-0000-1234";
    let redacted = redact(text, Standard).unwrap();
    assert_hidden(&redacted, &["3299", "0812", "０８１２"]);
}

#[test]
fn findings_and_debug_output_never_carry_values() {
    let redacted = redact(REGISTRASI, Strict).unwrap();
    let debug = format!("{redacted:?} {:?}", redacted.found());
    for secret in PATIENT {
        assert!(!debug.contains(secret), "lolos lewat Debug: {secret}");
    }
}

/// Nama samaran pertama berlabel `label` di teks yang dikirim.
fn placeholder(sent: &str, label: &str) -> Option<String> {
    let start = sent.find(&format!("[{label}-"))?;
    let end = start + sent[start..].find(']')? + 1;
    Some(sent[start..end].to_owned())
}
