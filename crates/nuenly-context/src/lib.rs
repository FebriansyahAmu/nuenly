//! Penyusun konteks Nuenly, bebas OS.
//!
//! Berisi:
//! - [`ContextAssembler`] (F-08), yang menyusun prompt dari perintah pengguna dan teks layar;
//! - [`ResourceProfiler`] (F-13), yang memilih model, panjang konteks, dan `keep_alive` dari RAM
//!   dan VRAM;
//! - [`ResponseCache`] (F-18), agar teks yang sama tidak diperiksa ulang ke model.

#![forbid(unsafe_code)]

mod assembler;
mod cache;
mod resource;

pub use assembler::{AssembleError, ContextAssembler, Prompt};
pub use cache::{CacheKey, ResponseCache};
pub use nuenly_core::TAG_PREFIX;
pub use resource::{Candidate, Hardware, ModelPlan, Placement, ResourceProfiler};
