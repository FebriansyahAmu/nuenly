//! Sumber pola dan kamus label PII guard.
//!
//! Label ditulis huruf kecil dalam sintaks regex dan dicocokkan tanpa beda huruf besar. Angka
//! hanya ASCII `[0-9]`; karakter lain yang tampak seperti angka sudah dinormalisasi lebih dulu.
//! Tambah kasus lolos dan tolak di test `detect` bila mengubah file ini.

/// Label yang diikuti nama orang.
pub(crate) const NAME_LABELS: &[&str] = &[
    r"nama(?:[ \t]+(?:lengkap|pasien|peserta|pemohon|pasangan|suami|istri|ayah|ibu(?:[ \t]+kandung)?|anak|keluarga|wali|kontak(?:[ \t]+darurat)?|penanggung[ \t]*jawab|pj|dokter|perawat))?",
    r"pasien",
    r"penanggung[ \t]*jawab",
    r"patient(?:[ \t]+name)?",
    r"full[ \t]+name",
    r"name",
];

/// Label yang diikuti alamat. "Alamat email" sengaja tidak termasuk.
pub(crate) const ADDRESS_LABELS: &[&str] = &[
    r"alamat(?:[ \t]+(?:rumah|domisili|lengkap|ktp|tinggal|pasien|kantor|surat))?",
    r"domisili",
    r"tempat[ \t]+tinggal",
    r"(?:home[ \t]+)?address",
];

/// Label yang diikuti tanggal lahir, kadang dengan tempat lahir di depannya.
pub(crate) const BIRTH_LABELS: &[&str] = &[
    r"tempat[ \t]*[,/]?[ \t]*(?:dan[ \t]+)?(?:tgl\.?|tanggal)[ \t]*lahir",
    r"tgl\.?[ \t]*lahir",
    r"tanggal[ \t]+lahir",
    r"ttl",
    r"lahir",
    r"kelahiran",
    r"date[ \t]+of[ \t]+birth",
    r"birth[ \t]*date",
    r"d\.?o\.?b\.?",
];

/// Label yang diikuti NIK atau nomor KK.
pub(crate) const NIK_LABELS: &[&str] = &[
    r"nik",
    r"(?:no\.?|nomor)[ \t]*(?:ktp|kk|kartu[ \t]+keluarga|induk[ \t]+kependudukan)",
    r"ktp",
];

/// Label yang diikuti nomor kartu BPJS atau nomor peserta.
pub(crate) const BPJS_LABELS: &[&str] = &[
    r"(?:(?:no\.?|nomor)[ \t]*)?(?:kartu[ \t]+)?(?:bpjs(?:[ \t]+kesehatan)?|jkn(?:[ \t]*-[ \t]*kis)?|kis)",
    r"(?:no\.?|nomor)[ \t]*(?:peserta|kartu)",
];

pub(crate) const NPWP_LABELS: &[&str] = &[r"npwp"];

/// Label yang diikuti nomor rekening atau virtual account.
pub(crate) const ACCOUNT_LABELS: &[&str] = &[
    r"(?:(?:no\.?|nomor)[ \t]*)?rek(?:ening)?\.?",
    r"account(?:[ \t]+(?:no\.?|number))?",
    r"acct?\.?[ \t]*no\.?",
    r"virtual[ \t]+account",
    r"iban",
];

/// Label yang diikuti nomor telepon dalam format apa pun.
pub(crate) const PHONE_LABELS: &[&str] = &[
    r"(?:(?:no\.?|nomor)[ \t]*)?(?:telp|telepon|tlp|tel|hp|handphone|ponsel|wa|whatsapp)\.?",
    r"phone",
    r"mobile",
];

/// Label yang diikuti nomor rekam medis atau registrasi.
pub(crate) const RECORD_LABELS: &[&str] = &[
    r"(?:no\.?|nomor)[ \t]*(?:rm|mr|rekam[ \t]+medi[sk]|registrasi|reg\.?|pendaftaran)",
    r"rekam[ \t]+medi[sk]",
    r"rm",
    r"mrn",
    r"medical[ \t]+record(?:[ \t]+(?:no\.?|number))?",
];

/// Label kolom lain yang tidak disamarkan, hanya dipakai untuk memotong alamat yang ditulis
/// satu baris dengan kolom berikutnya.
const OTHER_LABELS: &[&str] = &[
    r"umur",
    r"usia",
    r"jenis[ \t]+kelamin",
    r"agama",
    r"pekerjaan",
    r"status",
    r"poli(?:klinik)?",
    r"keluhan",
    r"diagnos[ai]s?",
    r"dokter",
    r"dpjp",
    r"kelas",
    r"ruang(?:an)?",
    r"email",
];

/// Sapaan yang diikuti nama berhuruf besar di level standar. Yang wajib bertitik ada di regex.
pub(crate) const HONORIFICS: &[&str] = &[
    r"tn",
    r"ny",
    r"nn",
    r"sdr",
    r"sdri",
    r"sdra",
    r"bpk",
    r"bp",
    r"yth",
    r"atas[ \t]+nama",
];

/// Sapaan tambahan di level ketat. Lebih sering salah tangkap ("Ibu Kota", "MS Word"), jadi
/// tidak dipakai di level standar.
pub(crate) const STRICT_HONORIFICS: &[&str] = &[
    r"dr", r"drg", r"dokter", r"bapak", r"ibu", r"pak", r"bu", r"saudara", r"saudari", r"mas",
    r"mbak", r"kak", r"prof", r"mr", r"mrs", r"ms", r"miss",
];

/// Sapaan dan gelar di awal nilai label nama, yang dibiarkan terlihat karena bukan identitas.
pub(crate) const TITLES: &[&str] = &[
    "tn", "ny", "nn", "sdr", "sdri", "sdra", "bpk", "bp", "yth", "dr", "drg", "bapak", "ibu",
    "pak", "bu", "prof", "mr", "mrs", "ms", "miss",
];

/// Gelar yang hanya dihitung bila bertitik, karena tanpa titik bisa bagian nama ("An Nisa").
pub(crate) const DOTTED_TITLES: &[&str] = &["an", "h", "hj"];

/// Kata yang mengakhiri nama setelah label atau sapaan: label kolom berikutnya dan kata sambung.
pub(crate) const STOP_WORDS: &[&str] = &[
    "umur",
    "usia",
    "thn",
    "tahun",
    "th",
    "jenis",
    "kelamin",
    "jk",
    "sex",
    "gender",
    "alamat",
    "address",
    "tgl",
    "tanggal",
    "lahir",
    "ttl",
    "dob",
    "nik",
    "ktp",
    "no",
    "nomor",
    "rm",
    "mrn",
    "telp",
    "telepon",
    "tlp",
    "hp",
    "wa",
    "phone",
    "email",
    "agama",
    "pekerjaan",
    "status",
    "gol",
    "golongan",
    "darah",
    "poli",
    "poliklinik",
    "dokter",
    "dpjp",
    "kelas",
    "ruang",
    "ruangan",
    "bangsal",
    "keluhan",
    "diagnosa",
    "diagnosis",
    "anamnesis",
    "tensi",
    "td",
    "bb",
    "tb",
    "suhu",
    "nadi",
    "rr",
    "age",
    "l",
    "p",
    "dan",
    "and",
    "dengan",
    "yang",
    "di",
    "ke",
    "dari",
    "pada",
    "untuk",
    "telah",
    "sudah",
    "sedang",
    "datang",
    "mengeluh",
    "berobat",
    "kontrol",
    "rujukan",
];

pub(crate) const EMAIL: &str = r"(?i)[a-z0-9._%+\-]+@[a-z0-9\-]+(?:\.[a-z0-9\-]+)*\.[a-z]{2,}";

/// Telepon Indonesia: `+62`, `62` untuk ponsel, atau `0`, lalu kelompok angka. Panjangnya
/// diperiksa di `detect`, yang juga melepas kelompok di belakang yang membuatnya terlalu panjang.
pub(crate) const PHONE_ID: &str =
    r"(?:\+62[ .\-]?\(?[1-9]|62[ .\-]?8|\(?0[1-9])[0-9]{1,3}\)?(?:[ .\-]?[0-9]{2,5}){1,4}";

/// Telepon dengan kode negara lain.
pub(crate) const PHONE_INTL: &str = r"\+[1-9][0-9]{0,3}(?:[ .\-]?\(?[0-9]{1,5}\)?){2,6}";

/// NIK atau nomor KK: 16 digit, utuh atau dikelompokkan dengan satu jenis pemisah. Pemisah
/// campuran tidak diterima, agar ekor satu nomor telepon dan nomor berikutnya tidak terbaca NIK.
pub(crate) const NIK: &str = r"[0-9]{16}|[0-9]{2}\.[0-9]{2}\.[0-9]{2}\.[0-9]{6}\.[0-9]{4}|[0-9]{2} [0-9]{2} [0-9]{2} [0-9]{6} [0-9]{4}|[0-9]{4}(?: [0-9]{4}){3}|[0-9]{4}(?:-[0-9]{4}){3}|[0-9]{4}(?:\.[0-9]{4}){3}";

/// NPWP 15 digit berformat `99.999.999.9-999.999`. NPWP 16 digit sama dengan NIK.
pub(crate) const NPWP: &str = r"[0-9]{2}\.[0-9]{3}\.[0-9]{3}\.[0-9][.\-][0-9]{3}\.[0-9]{3}";

/// Kartu BPJS Kesehatan: 13 digit yang diawali 000.
pub(crate) const BPJS: &str = r"000[0-9]{10}";

pub(crate) const CARD: &str = r"[0-9]{4}(?:[ \-]?[0-9]{4}){2}[ \-]?[0-9]{1,7}";

/// Alamat jalan dengan nomor rumah.
pub(crate) const STREET: &str = r"\b(?i:jl|jln|jalan|gg|gang|komp|kompleks|perum|perumahan)\.?[ \t]+\p{Lu}[\p{L}\p{M}0-9 .'\-]{0,60}?[ \t,]+(?i:no)\.?[ \t]*[0-9]+[A-Za-z]?(?:[ \t]*,?[ \t]*(?i:rt)\.?[ \t]*[0-9]{1,3}[ \t]*/[ \t]*(?i:rw)\.?[ \t]*[0-9]{1,3})?";

/// Level ketat: nama jalan tanpa nomor rumah.
pub(crate) const STREET_LOOSE: &str = r"\b(?i:jl|jln|jalan|gg|gang)\.?[ \t]+\p{Lu}[\p{L}\p{M}'\-]*(?: \p{Lu}[\p{L}\p{M}'\-]*){0,3}(?:[ \t,]+(?i:no)\.?[ \t]*[0-9]+[A-Za-z]?)?";

/// Level ketat: RT/RW.
pub(crate) const RT_RW: &str =
    r"\b(?i:rt)\.?[ \t]*[0-9]{1,3}[ \t]*/[ \t]*(?i:rw)\.?[ \t]*[0-9]{1,3}";

/// Level ketat: deret angka panjang apa pun.
pub(crate) const NUMBER: &str = r"[0-9]{8,}";

pub(crate) const NUMBER_VALUE: &str = r"[0-9]+(?:[ .\-/][0-9]+)*";
pub(crate) const RECORD_VALUE: &str =
    r"[0-9]+(?:[ .\-/][0-9]+)*|[A-Za-z0-9]+(?:[.\-/][A-Za-z0-9]+)*";
pub(crate) const PHONE_VALUE: &str = r"\+?\(?[0-9][0-9 ().\-]{4,20}[0-9]";
pub(crate) const ADDRESS_VALUE: &str = r"[^\r\n\t|;]+";

const NAME_WORD: &str = r"\p{L}[\p{L}\p{M}'’.\-]*";
const CAP_WORD: &str = r"\p{Lu}[\p{L}\p{M}'’.\-]*";

const MONTHS: &str = r"jan(?:uari|uary)?|feb(?:ruari|ruary)?|mar(?:et|ch)?|apr(?:il)?|mei|may|jun(?:i|e)?|jul(?:i|y)?|agu(?:stus)?|agt|aug(?:ust)?|sep(?:t(?:ember)?)?|okt(?:ober)?|oct(?:ober)?|nov(?:ember)?|des(?:ember)?|dec(?:ember)?";

/// Awal kolom: awal baris, setelah tanda baca, atau setelah dua spasi atau tab. Label nama dan
/// alamat di tengah kalimat ("keluhan pasien: demam") tidak dihitung.
const FIELD_START: &str = r"(?:^|[^\p{L}\p{N} \t]|[ \t]{2,}|\t)[ \t]*";

/// Pemisah label dan nilai: titik dua atau sama dengan (nilai boleh di baris berikutnya), atau
/// label sendirian dengan nilai di baris berikutnya atau setelah tab.
const WORD_SEPARATOR: &str = r"(?:[ \t]*[:=][ \t]*(?:\r?\n[ \t]*)?|(?:[ \t]*\r?\n|[ ]*\t)[ \t]*)";

/// Nama setelah label: sampai enam kata dipisah satu spasi, huruf besar atau kecil.
pub(crate) fn name_value() -> String {
    format!("{NAME_WORD}(?: {NAME_WORD}){{0,5}}")
}

/// Tanggal dengan angka atau nama bulan (Indonesia dan Inggris), tanpa flag.
fn date() -> String {
    format!(
        r"[0-9]{{1,2}}[\-/.][0-9]{{1,2}}[\-/.](?:[0-9]{{4}}|[0-9]{{2}})|[0-9]{{4}}-[0-9]{{1,2}}-[0-9]{{1,2}}|[0-9]{{1,2}}[ \-]?(?:{MONTHS})\.?[ \-,]*[0-9]{{4}}|(?:{MONTHS})\.?[ ]+[0-9]{{1,2}},?[ ]+[0-9]{{4}}"
    )
}

/// Level ketat: tanggal lengkap di mana pun.
pub(crate) fn date_alone() -> String {
    format!("(?i){}", date())
}

/// Tanggal lahir setelah label, boleh didahului tempat lahir dan koma atau garis miring.
pub(crate) fn birth_value() -> String {
    format!(
        r"(?:\p{{L}}[\p{{L}}\p{{M}} .'\-]{{0,40}}?[ \t]*[,/][ \t]*)?(?:{})",
        date()
    )
}

/// Kolom berlabel dengan nilai berupa kata (nama, alamat).
pub(crate) fn word_field(labels: &[&str], value: &str) -> String {
    format!(
        "(?im){FIELD_START}(?:{})(?:{WORD_SEPARATOR})(?P<value>{value})",
        labels.join("|")
    )
}

/// Kolom berlabel dengan nilai berupa nomor atau tanggal. Titik dua boleh tidak ada karena
/// bentuk nilainya sudah cukup ketat.
pub(crate) fn number_field(labels: &[&str], value: &str) -> String {
    format!(
        r"(?i)\b(?:{})[ \t]*[:=]?[ \t]*(?:\r?\n[ \t]*)?(?P<value>{value})",
        labels.join("|")
    )
}

/// Nama berhuruf besar setelah sapaan. `an.` (anak) dan `a.n.` (atas nama) wajib bertitik.
pub(crate) fn honorific() -> String {
    format!(
        r"(?i:\b(?:(?:an|a\.[ \t]?n)\.[ \t]*|(?:{})(?:\.[ \t]*|[ \t]+)))(?P<value>{})",
        HONORIFICS.join("|"),
        cap_value()
    )
}

/// Level ketat: nama berhuruf besar setelah sapaan tambahan.
pub(crate) fn strict_honorific() -> String {
    format!(
        r"(?i:\b(?:{})(?:\.[ \t]*|[ \t]+))(?P<value>{})",
        STRICT_HONORIFICS.join("|"),
        cap_value()
    )
}

fn cap_value() -> String {
    format!("{CAP_WORD}(?: {CAP_WORD}){{0,3}}")
}

/// Label kolom mana pun diikuti titik dua, untuk memotong alamat sebelum kolom berikutnya.
pub(crate) fn next_label() -> String {
    let labels = [
        NAME_LABELS,
        ADDRESS_LABELS,
        BIRTH_LABELS,
        NIK_LABELS,
        BPJS_LABELS,
        NPWP_LABELS,
        ACCOUNT_LABELS,
        PHONE_LABELS,
        RECORD_LABELS,
        OTHER_LABELS,
    ]
    .concat();
    format!(r"(?i)\b(?:{})[ \t]*[:=]", labels.join("|"))
}
