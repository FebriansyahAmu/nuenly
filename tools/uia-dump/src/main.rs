//! `uia-dump`: mencetak tree UI Automation jendela di latar depan untuk spike Fase 0.
//!
//! Nama dan nilai elemen disamarkan secara default. `--show-names` dan `--show-values` hanya
//! untuk data sintetis.

#![forbid(unsafe_code)]

mod args;
#[cfg(windows)]
mod dump;
#[cfg(any(windows, test))]
mod render;

use std::process::ExitCode;

use args::{Args, Command, HELP};

fn main() -> ExitCode {
    match Args::parse(std::env::args().skip(1)) {
        Ok(Command::Help) => {
            print!("{HELP}");
            ExitCode::SUCCESS
        }
        Ok(Command::Run(args)) => run(&args),
        Err(message) => {
            eprintln!("uia-dump: {message}\n\n{HELP}");
            ExitCode::from(2)
        }
    }
}

#[cfg(windows)]
fn run(args: &Args) -> ExitCode {
    match dump::run(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("uia-dump: {err:#}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(not(windows))]
fn run(_args: &Args) -> ExitCode {
    eprintln!("uia-dump hanya berjalan di Windows.");
    ExitCode::FAILURE
}
