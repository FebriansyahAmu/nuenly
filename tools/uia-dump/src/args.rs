//! Argumen baris perintah. Diparse manual agar tool spike ini tidak butuh dependensi tambahan.

use std::str::FromStr;
use std::time::Duration;

pub const HELP: &str = "\
uia-dump: mencetak tree UI Automation jendela di latar depan (spike Fase 0)

Pemakaian: uia-dump [opsi]

Setelah dijalankan, pindah ke aplikasi target sebelum hitungan mundur selesai.

Opsi:
  --focused          Hanya elemen yang fokus: label, teks terpilih, saudara sebelumnya,
                     dan waktu baca
  --delay <detik>    Jeda sebelum membaca (bawaan: 3)
  --depth <n>        Kedalaman tree maksimum (bawaan: 30)
  --max-nodes <n>    Jumlah elemen maksimum (bawaan: 3000)
  --show-names       Tampilkan nama dan label elemen (bisa memuat teks layar)
  --show-values      Tampilkan isi field dan teks terpilih (memuat teks layar)
  -h, --help         Tampilkan bantuan ini

Nama dan nilai disamarkan secara default. Pakai --show-names dan --show-values
hanya dengan data sintetis, dan jangan bagikan output yang memuat data asli.
";

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Help,
    Run(Args),
}

#[derive(Debug, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)] // Setiap bool adalah flag CLI yang berdiri sendiri.
pub struct Args {
    pub focused: bool,
    pub delay: Duration,
    pub max_depth: usize,
    pub max_nodes: usize,
    pub show_names: bool,
    pub show_values: bool,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            focused: false,
            delay: Duration::from_secs(3),
            max_depth: 30,
            max_nodes: 3000,
            show_names: false,
            show_values: false,
        }
    }
}

impl Args {
    /// Mem-parse argumen tanpa nama program.
    pub fn parse(mut args: impl Iterator<Item = String>) -> Result<Command, String> {
        let mut parsed = Self::default();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "-h" | "--help" => return Ok(Command::Help),
                "--focused" => parsed.focused = true,
                "--show-names" => parsed.show_names = true,
                "--show-values" => parsed.show_values = true,
                "--delay" => parsed.delay = Duration::from_secs(number(&mut args, "--delay")?),
                "--depth" => parsed.max_depth = number(&mut args, "--depth")?,
                "--max-nodes" => parsed.max_nodes = number(&mut args, "--max-nodes")?,
                other => return Err(format!("opsi tidak dikenal: {other}")),
            }
        }
        Ok(Command::Run(parsed))
    }
}

fn number<T: FromStr>(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<T, String> {
    let value = args.next().ok_or_else(|| format!("{flag} butuh nilai"))?;
    value
        .parse()
        .map_err(|_| format!("{flag}: '{value}' bukan angka"))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{Args, Command};

    fn parse(args: &[&str]) -> Result<Command, String> {
        Args::parse(args.iter().map(|arg| (*arg).to_owned()))
    }

    #[test]
    fn defaults_hide_screen_text() {
        let Command::Run(args) = parse(&[]).unwrap() else {
            panic!("seharusnya Run");
        };
        assert_eq!(args, Args::default());
        assert!(!args.show_names && !args.show_values);
    }

    #[test]
    fn parses_all_flags() {
        let parsed = parse(&[
            "--focused",
            "--delay",
            "5",
            "--depth",
            "8",
            "--max-nodes",
            "100",
            "--show-names",
            "--show-values",
        ])
        .unwrap();
        assert_eq!(
            parsed,
            Command::Run(Args {
                focused: true,
                delay: Duration::from_secs(5),
                max_depth: 8,
                max_nodes: 100,
                show_names: true,
                show_values: true,
            })
        );
    }

    #[test]
    fn help_wins() {
        assert_eq!(parse(&["--focused", "-h"]).unwrap(), Command::Help);
    }

    #[test]
    fn rejects_bad_input() {
        assert!(parse(&["--verbose"]).is_err());
        assert!(parse(&["--delay"]).is_err());
        assert!(parse(&["--depth", "dalam"]).is_err());
        assert!(parse(&["--max-nodes", "-1"]).is_err());
    }
}
