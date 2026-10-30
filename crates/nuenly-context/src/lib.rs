//! Penyusun konteks Nuenly, bebas OS.
//!
//! Sekarang berisi [`ResourceProfiler`] (F-13), yang memilih model, panjang konteks, dan
//! `keep_alive` dari RAM dan VRAM. `ContextAssembler` (F-08) menyusul bersama alur hotkey.

#![forbid(unsafe_code)]

mod resource;

pub use resource::{Candidate, Hardware, ModelPlan, Placement, ResourceProfiler};
