//! Platform Windows untuk Nuenly: UI Automation dan Win32.
//!
//! Satu-satunya crate di workspace yang boleh memakai `unsafe`; setiap blok diberi komentar
//! `// SAFETY:`. Di OS lain crate ini kosong, jadi `cargo test --workspace` tetap jalan.

#![cfg(windows)]

pub mod clipboard;
mod hardening;
pub mod hardware;
pub mod hotkey;
mod process;
pub mod shell;
mod source;
pub mod uia;
mod window;

pub use hardening::harden_dll_search;
pub use process::process_name;
pub use source::UiaContextSource;
