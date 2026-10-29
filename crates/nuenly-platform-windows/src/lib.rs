//! Platform Windows untuk Nuenly: UI Automation dan Win32.
//!
//! Satu-satunya crate di workspace yang boleh memakai `unsafe`; setiap blok diberi komentar
//! `// SAFETY:`. Di OS lain crate ini kosong, jadi `cargo test --workspace` tetap jalan.

#![cfg(windows)]

mod process;
pub mod uia;

pub use process::process_name;
