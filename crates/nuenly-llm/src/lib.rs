//! Penyedia model bahasa untuk Nuenly (F-10).
//!
//! [`LlmProvider`] adalah antarmuka yang dipakai aplikasi; [`OllamaProvider`] menjalankannya
//! lewat Ollama di loopback. Output terstruktur selalu melewati [`structured`], yang
//! memvalidasi JSON schema, mengulang sekali bila gagal, lalu menyerah.
//!
//! Isi pesan adalah [`SensitiveText`](nuenly_core::SensitiveText) dan tidak pernah masuk ke
//! [`LlmError`].

#![forbid(unsafe_code)]

mod error;
mod message;
pub mod ollama;
mod provider;
mod schema;

pub use error::LlmError;
pub use message::{ChatRequest, Message, Role, Usage};
pub use ollama::OllamaProvider;
pub use provider::{BoxFuture, LlmProvider, structured};
pub use schema::OutputSchema;
