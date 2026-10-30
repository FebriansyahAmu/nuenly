//! Penyusun konteks Nuenly, bebas OS.
//!
//! Berisi [`ContextAssembler`] (F-08), yang menyusun prompt dari perintah pengguna dan teks
//! layar, dan [`ResourceProfiler`] (F-13), yang memilih model, panjang konteks, dan
//! `keep_alive` dari RAM dan VRAM.

#![forbid(unsafe_code)]

mod assembler;
mod resource;

pub use assembler::{AssembleError, ContextAssembler, Prompt};
pub use resource::{Candidate, Hardware, ModelPlan, Placement, ResourceProfiler};
