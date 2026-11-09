//! Koneksi ke satu server MCP lewat `rmcp`.

use std::collections::BTreeMap;
use std::process::Stdio;
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use nuenly_core::SensitiveText;
use process_wrap::tokio::{CommandWrap, JobObject, KillOnDrop};
use rmcp::ServiceExt as _;
use rmcp::model::{
    CallToolRequestParams, ClientCapabilities, ClientConfig, ContentBlock, Implementation,
    ResourceContents,
};
use rmcp::service::{RoleClient, RunningService, ServiceError};
use rmcp::transport::TokioChildProcess;
use serde_json::{Map, Value};
use tokio::process::Command;

use crate::gate::CallPermit;
use crate::server::{ServerId, ServerSpec, Transport, environment};
use crate::tool::{ToolDefinition, fingerprint};

/// Batas waktu dan ukuran untuk satu server.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Menjalankan server sampai jabat tangan MCP selesai. `npx` yang mengunduh paket untuk
    /// pertama kali butuh waktu lebih lama.
    pub connect: Duration,
    /// Satu permintaan daftar tool atau panggilan tool.
    pub request: Duration,
    /// Teks hasil tool paling panjang (karakter); sisanya dipotong.
    pub output_chars: usize,
    /// Tool paling banyak per server; sisanya diabaikan.
    pub tools: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            connect: Duration::from_secs(60),
            request: Duration::from_secs(30),
            output_chars: 16_000,
            tools: 64,
        }
    }
}

/// Kegagalan berbicara dengan server. Pesan dari server tidak ikut, karena bisa memuat data
/// pengguna.
#[derive(Debug, thiserror::Error)]
pub enum McpError {
    #[error("server MCP tidak bisa dijalankan")]
    Spawn(#[source] std::io::Error),
    #[error("server MCP tidak menyelesaikan jabat tangan")]
    Handshake,
    #[error("server MCP terlalu lama menjawab")]
    Timeout,
    #[error("server MCP berhenti")]
    Closed,
    #[error("server MCP menolak permintaan (kode {0})")]
    Rejected(i32),
    #[error("jawaban server MCP tidak sesuai protokol")]
    Protocol,
    /// Permit untuk server lain, atau definisi tool berubah sejak daftar terakhir.
    #[error("izin tidak cocok dengan tool di server ini")]
    PermitMismatch,
}

impl From<ServiceError> for McpError {
    fn from(error: ServiceError) -> Self {
        match error {
            ServiceError::McpError(error) => Self::Rejected(error.code.0),
            ServiceError::TransportClosed
            | ServiceError::TransportSend(_)
            | ServiceError::Cancelled { .. } => Self::Closed,
            ServiceError::Timeout { .. } => Self::Timeout,
            _ => Self::Protocol,
        }
    }
}

/// Hasil satu panggilan tool. Teksnya data dari luar: bungkus dengan tag ber-ID acak sebelum
/// masuk prompt, jangan pernah digabung ke instruksi sistem.
#[derive(Debug)]
pub struct ToolOutput {
    pub text: SensitiveText,
    /// Server menandai hasil ini sebagai kegagalan tool.
    pub is_error: bool,
    /// Teks dipotong di [`Limits::output_chars`].
    pub truncated: bool,
    /// Isi selain teks (gambar, audio, tautan) yang tidak diteruskan.
    pub skipped: usize,
}

/// Koneksi yang sedang berjalan ke satu server. Prosesnya berhenti saat nilai ini di-drop.
pub struct Connection {
    server: ServerId,
    limits: Limits,
    service: RunningService<RoleClient, ClientConfig>,
    /// Sidik definisi tool dari daftar terakhir, untuk memeriksa permit.
    listed: Mutex<BTreeMap<String, String>>,
}

impl std::fmt::Debug for Connection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Connection")
            .field("server", &self.server)
            .finish_non_exhaustive()
    }
}

impl Connection {
    /// Menjalankan server dan menyelesaikan jabat tangan MCP. Nuenly tidak menawarkan sampling,
    /// roots, atau elicitation ke server.
    ///
    /// # Errors
    ///
    /// [`McpError`] bila proses gagal dijalankan, jabat tangan gagal, atau melewati
    /// [`Limits::connect`].
    pub async fn connect(spec: &ServerSpec, limits: Limits) -> Result<Self, McpError> {
        let Transport::Stdio {
            command,
            args,
            env,
            cwd,
        } = &spec.transport;
        let mut process = Command::new(command);
        process
            .args(args)
            .env_clear()
            .envs(environment(std::env::vars_os(), env));
        if let Some(cwd) = cwd {
            process.current_dir(cwd);
        }
        let mut wrapped = CommandWrap::from(process);
        hide_console(&mut wrapped);
        // Job object: proses turunan server ikut berhenti, juga bila Nuenly berhenti mendadak.
        wrapped.wrap(JobObject).wrap(KillOnDrop);
        // Stderr server bisa memuat data pengguna, jadi dibuang dan tidak masuk log.
        let (transport, _) = TokioChildProcess::builder(wrapped)
            .stderr(Stdio::null())
            .spawn()
            .map_err(McpError::Spawn)?;

        let info = ClientConfig::new(
            ClientCapabilities::default(),
            Implementation::new("nuenly", env!("CARGO_PKG_VERSION")),
        );
        let service = tokio::time::timeout(limits.connect, info.serve(transport))
            .await
            .map_err(|_| McpError::Timeout)?
            .map_err(|_| McpError::Handshake)?;
        Ok(Self {
            server: spec.id.clone(),
            limits,
            service,
            listed: Mutex::new(BTreeMap::new()),
        })
    }

    #[must_use]
    pub fn server(&self) -> &ServerId {
        &self.server
    }

    /// Daftar tool terbaru dari server, paling banyak [`Limits::tools`]. Teruskan ke
    /// [`Gate::observe`](crate::Gate::observe) sebelum ditawarkan ke model.
    ///
    /// # Errors
    ///
    /// [`McpError`] bila server gagal menjawab.
    pub async fn tools(&self) -> Result<Vec<ToolDefinition>, McpError> {
        let tools = tokio::time::timeout(self.limits.request, self.service.list_all_tools())
            .await
            .map_err(|_| McpError::Timeout)??;
        let definitions: Vec<ToolDefinition> = tools
            .iter()
            .take(self.limits.tools)
            .map(ToolDefinition::from_rmcp)
            .collect();
        let listed = definitions
            .iter()
            .map(|definition| (definition.name.clone(), fingerprint(definition)))
            .collect();
        *self.listed.lock().unwrap_or_else(PoisonError::into_inner) = listed;
        Ok(definitions)
    }

    /// Memanggil tool yang sudah diizinkan gate. Permit harus untuk server ini dan untuk
    /// definisi tool yang sama dengan daftar terakhir dari [`tools`](Self::tools).
    ///
    /// # Errors
    ///
    /// [`McpError::PermitMismatch`] sebelum apa pun dikirim bila permit tidak cocok; selain itu
    /// [`McpError`] bila server gagal menjawab dalam [`Limits::request`].
    pub async fn call(
        &self,
        permit: CallPermit,
        arguments: Map<String, Value>,
    ) -> Result<ToolOutput, McpError> {
        let listed = self
            .listed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(permit.tool())
            .cloned();
        if permit.server() != &self.server || listed.as_deref() != Some(permit.hash()) {
            return Err(McpError::PermitMismatch);
        }
        let request =
            CallToolRequestParams::new(permit.tool().to_owned()).with_arguments(arguments);
        let result = tokio::time::timeout(self.limits.request, self.service.call_tool(request))
            .await
            .map_err(|_| McpError::Timeout)??;
        Ok(output(
            &result.content,
            result.structured_content.as_ref(),
            result.is_error.unwrap_or(false),
            self.limits.output_chars,
        ))
    }

    /// Menutup koneksi dan menghentikan proses server.
    pub async fn close(self) {
        let _ = self.service.cancel().await;
    }
}

#[cfg(windows)]
fn hide_console(command: &mut CommandWrap) {
    use process_wrap::tokio::CreationFlags;
    use windows::Win32::System::Threading::CREATE_NO_WINDOW;
    // Tanpa ini, server berbasis konsol (npx, python) membuka jendela hitam di desktop.
    command.wrap(CreationFlags(CREATE_NO_WINDOW));
}

#[cfg(not(windows))]
fn hide_console(_: &mut CommandWrap) {}

/// Teks hasil tool: blok teks dan isi teks dari resource tersemat, dipisah baris kosong. Bila
/// tidak ada teks, JSON `structuredContent` dipakai.
fn output(
    content: &[ContentBlock],
    structured: Option<&Value>,
    is_error: bool,
    limit: usize,
) -> ToolOutput {
    let mut parts: Vec<&str> = Vec::new();
    let mut skipped = 0;
    for block in content {
        match block {
            ContentBlock::Text(text) => parts.push(&text.text),
            ContentBlock::Resource(embedded) => match &embedded.resource {
                ResourceContents::TextResourceContents { text, .. } => parts.push(text),
                _ => skipped += 1,
            },
            _ => skipped += 1,
        }
    }
    let structured_text;
    if parts.is_empty()
        && let Some(structured) = structured
    {
        structured_text = structured.to_string();
        parts.push(&structured_text);
    }
    let joined = parts.join("\n\n");
    let truncated = joined.chars().count() > limit;
    let text = if truncated {
        joined.chars().take(limit).collect()
    } else {
        joined
    };
    ToolOutput {
        text: SensitiveText::new(text),
        is_error,
        truncated,
        skipped,
    }
}

#[cfg(test)]
mod tests {
    use rmcp::model::ContentBlock;
    use serde_json::json;

    use super::output;

    #[test]
    fn text_blocks_are_joined_and_other_content_is_counted() {
        let content: Vec<ContentBlock> = serde_json::from_value(json!([
            { "type": "text", "text": "Baris satu." },
            { "type": "image", "data": "AAAA", "mimeType": "image/png" },
            { "type": "resource", "resource": { "uri": "file:///contoh.txt", "text": "Isi berkas." } }
        ]))
        .unwrap();
        let result = output(&content, None, false, 100);
        assert_eq!(result.text.expose(), "Baris satu.\n\nIsi berkas.");
        assert_eq!(result.skipped, 1);
        assert!(!result.truncated);
    }

    #[test]
    fn long_output_is_cut_at_a_character_boundary() {
        let content: Vec<ContentBlock> =
            serde_json::from_value(json!([{ "type": "text", "text": "é".repeat(50) }])).unwrap();
        let result = output(&content, None, true, 10);
        assert_eq!(result.text.expose(), "é".repeat(10));
        assert!(result.truncated);
        assert!(result.is_error);
    }

    #[test]
    fn structured_content_is_used_when_there_is_no_text() {
        let result = output(&[], Some(&json!({ "jumlah": 3 })), false, 100);
        assert_eq!(result.text.expose(), r#"{"jumlah":3}"#);
    }
}
