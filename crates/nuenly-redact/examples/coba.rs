//! Mencoba PII guard pada teks sintetis, tanpa menjalankan aplikasi:
//!
//! ```text
//! cargo run -p nuenly-redact --example coba -- contoh.txt
//! cargo run -p nuenly-redact --example coba -- --ketat contoh.txt
//! ```
//!
//! Tanpa nama file, teks dibaca dari stdin. Yang dicetak hanya teks yang sudah disamarkan dan
//! jumlah temuan per jenis, bukan nilai aslinya. Pakai hanya data sintetis.

use std::error::Error;
use std::io::Read as _;

use nuenly_core::{RedactionLevel, SensitiveText};
use nuenly_redact::Guard;

fn main() -> Result<(), Box<dyn Error>> {
    let mut strict = false;
    let mut path = None;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--ketat" => strict = true,
            "-h" | "--help" => {
                println!("Pemakaian: coba [--ketat] [file]");
                return Ok(());
            }
            _ => path = Some(arg),
        }
    }
    let text = if let Some(path) = path {
        std::fs::read_to_string(path)?
    } else {
        let mut text = String::new();
        std::io::stdin().read_to_string(&mut text)?;
        text
    };

    let level = if strict {
        RedactionLevel::Strict
    } else {
        RedactionLevel::Standard
    };
    let redacted = Guard::new()?.redact(&SensitiveText::new(text), level);
    println!("{}", redacted.text().expose());

    let found: Vec<String> = redacted
        .found()
        .iter()
        .map(|(kind, count)| format!("{} {count}", kind.label()))
        .collect();
    eprintln!(
        "\nLevel {}. Disamarkan: {}",
        if strict { "ketat" } else { "standar" },
        if found.is_empty() {
            "-".to_owned()
        } else {
            found.join(", ")
        }
    );
    Ok(())
}
