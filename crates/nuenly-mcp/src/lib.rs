//! MCP client Nuenly (F-16, F-17).
//!
//! - [`ServerSpec`]: server yang ditambahkan pengguna, terikat ke mode tertentu. Server stdio
//!   dijalankan tanpa jendela konsol, dengan lingkungan minimal ([`INHERITED_ENV`]) dan job
//!   object agar proses turunannya ikut berhenti.
//! - [`Connection`]: koneksi `rmcp` dengan batas waktu dan ukuran ([`Limits`]). Nuenly tidak
//!   menawarkan sampling, roots, atau elicitation ke server.
//! - [`Gate`]: izin per tool (allow, ask, deny) dan [`ToolPin`]. Semua tool bawaannya `Ask`,
//!   karena anotasi "baca saja" tidak dipercaya. Definisi yang berubah setelah disetujui
//!   menonaktifkan tool sampai pengguna meninjaunya.
//!
//! Tool hanya bisa dipanggil dengan [`CallPermit`], dan permit hanya dibuat oleh [`Gate`].
//! Hasil tool adalah data dari luar: bungkus dengan tag ber-ID acak sebelum masuk prompt.

#![forbid(unsafe_code)]

mod client;
mod gate;
mod server;
mod tool;

pub use client::{Connection, Limits, McpError, ToolOutput};
pub use gate::{Approval, CallPermit, Change, Decision, Gate, Permission, Refusal, ToolRule};
pub use server::{INHERITED_ENV, InvalidServerId, ServerId, ServerSpec, Transport};
pub use tool::{MAX_DEFINITION_BYTES, ToolDefinition, ToolPin};
