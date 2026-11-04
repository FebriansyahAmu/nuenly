//! Penyimpanan Nuenly (F-28): satu file SQLite terenkripsi SQLCipher berisi setelan dan audit.
//!
//! Kuncinya 256 bit acak dari pemanggil ([`StoreKey`]); aplikasi menyimpannya terkunci DPAPI
//! (ADR 0013). Store tidak pernah memuat teks pengguna: audit hanya mencatat apa yang terjadi,
//! di aplikasi dan mode apa, ke model mana, dan seberapa banyak.

#![forbid(unsafe_code)]

mod audit;
mod error;
mod key;
mod store;

pub use audit::{AskOutcome, AuditEntry, AuditEvent, RETENTION};
pub use error::StoreError;
pub use key::{KEY_LEN, StoreKey};
pub use store::Store;
