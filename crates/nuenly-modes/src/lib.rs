//! Mode Nuenly (F-14): mode pack TOML dan [`ModeManager`] yang menyusun
//! [`ActivePolicy`](nuenly_core::ActivePolicy) untuk mode aktif.
//!
//! Mode pack adalah folder `modes/<id>/` berisi `mode.toml` (identitas dan kebijakan),
//! `prompts/ask.txt` (prompt sistem Tanya Nuenly), dan untuk saran proaktif
//! `prompts/suggest.txt` serta `schemas/suggest.json`. Modul lain membaca kebijakan, prompt, dan
//! schema dari mode pack aktif, tidak pernah bercabang berdasarkan ID mode.

#![forbid(unsafe_code)]

mod error;
mod manager;
mod official;
mod pack;

pub use error::ModeError;
pub use manager::ModeManager;
pub use pack::{ModePack, PackFiles, SuggestFiles, SuggestSpec};
