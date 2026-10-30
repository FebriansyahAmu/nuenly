//! [`OllamaProvider`]: model lokal lewat API Ollama (`/api/chat`, `/api/version`, `/api/tags`).

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::time::Duration;

use bytes::Bytes;
use nuenly_core::SensitiveText;
use serde::de::IgnoredAny;
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::{BoxFuture, ChatRequest, LlmError, LlmProvider, OutputSchema, Role, Usage};

/// Alamat bawaan Ollama. API Ollama tanpa autentikasi, jadi Nuenly hanya memakai loopback.
pub const DEFAULT_ADDRESS: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 11434);

/// Di loopback, sambungan yang sehat langsung terjadi.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(1);
/// Jeda terlama antara dua potongan jawaban; memuat model besar di CPU bisa lama.
const READ_TIMEOUT: Duration = Duration::from_secs(120);
/// Satu baris NDJSON memuat satu potongan jawaban; baris sepanjang ini berarti protokol rusak.
const MAX_LINE: usize = 1024 * 1024;
/// Batas jawaban non-streaming: versi, daftar model, dan jawaban terstruktur.
const MAX_BODY: usize = 4 * 1024 * 1024;

/// Model yang terdaftar di Ollama.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OllamaModel {
    pub name: String,
    /// Ukuran bobot di disk, dalam byte. Model cloud berukuran 0.
    pub size: u64,
    /// Model cloud: Ollama meneruskan permintaannya ke server di luar PC. Nuenly tidak
    /// memakainya.
    pub remote: bool,
}

/// Penyedia model lewat Ollama di loopback.
#[derive(Clone, Debug)]
pub struct OllamaProvider {
    client: reqwest::Client,
    base: String,
}

impl OllamaProvider {
    /// Ollama di [`DEFAULT_ADDRESS`].
    ///
    /// # Errors
    ///
    /// [`LlmError::Unreachable`] bila klien HTTP tidak bisa dibuat.
    pub fn new() -> Result<Self, LlmError> {
        Self::with_address(SocketAddr::V4(DEFAULT_ADDRESS))
    }

    /// # Errors
    ///
    /// [`LlmError::NotLoopback`] bila `address` bukan loopback.
    pub fn with_address(address: SocketAddr) -> Result<Self, LlmError> {
        if !address.ip().is_loopback() {
            return Err(LlmError::NotLoopback);
        }
        let client = reqwest::Client::builder()
            // Proxy dari `HTTP_PROXY` atau setelan sistem bisa membelokkan permintaan keluar PC.
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(CONNECT_TIMEOUT)
            .read_timeout(READ_TIMEOUT)
            .build()
            .map_err(|_| LlmError::Unreachable)?;
        Ok(Self {
            client,
            base: format!("http://{address}"),
        })
    }

    /// Versi Ollama. Sekaligus memastikan yang menjawab memang Ollama.
    ///
    /// # Errors
    ///
    /// [`LlmError::Unreachable`] bila tidak ada yang menjawab, [`LlmError::Protocol`] bila yang
    /// menjawab bukan Ollama.
    pub async fn version(&self) -> Result<String, LlmError> {
        #[derive(Deserialize)]
        struct Version {
            version: String,
        }

        let body = self.get("/api/version").await?;
        serde_json::from_slice::<Version>(&body)
            .map(|found| found.version)
            .map_err(|_| LlmError::Protocol)
    }

    /// Model yang terdaftar di Ollama, termasuk model cloud, urut abjad.
    ///
    /// # Errors
    ///
    /// Sama seperti [`OllamaProvider::version`].
    pub async fn models(&self) -> Result<Vec<OllamaModel>, LlmError> {
        #[derive(Deserialize)]
        struct Tags {
            models: Vec<Tag>,
        }
        #[derive(Deserialize)]
        struct Tag {
            name: String,
            #[serde(default)]
            size: u64,
            remote_host: Option<IgnoredAny>,
            remote_model: Option<IgnoredAny>,
        }

        let body = self.get("/api/tags").await?;
        let tags = serde_json::from_slice::<Tags>(&body).map_err(|_| LlmError::Protocol)?;
        let mut models: Vec<OllamaModel> = tags
            .models
            .into_iter()
            .map(|tag| OllamaModel {
                remote: tag.remote_host.is_some()
                    || tag.remote_model.is_some()
                    || cloud_tag(&tag.name),
                name: tag.name,
                size: tag.size,
            })
            .collect();
        models.sort_by(|a, b| a.name.cmp(&b.name));
        models.dedup_by(|a, b| a.name == b.name);
        Ok(models)
    }

    /// Memastikan `model` ada di Ollama dan berjalan di PC ini, sebelum teks apa pun dikirim.
    async fn ensure_local(&self, model: &str) -> Result<(), LlmError> {
        match self
            .models()
            .await?
            .iter()
            .find(|found| found.name == model)
        {
            Some(found) if found.remote => Err(LlmError::RemoteModel),
            Some(_) => Ok(()),
            None => Err(LlmError::ModelNotFound),
        }
    }

    async fn stream_chat(
        &self,
        request: &ChatRequest,
        on_token: &mut (dyn FnMut(&str) + Send),
    ) -> Result<Usage, LlmError> {
        let mut response = self.post_chat(&ChatBody::new(request, true, None)).await?;
        let mut lines = Lines::default();
        while let Some(chunk) = response.chunk().await? {
            lines.push(&chunk);
            while let Some(line) = lines.next_line() {
                if let Some(usage) = handle_line(&line, on_token)? {
                    return Ok(usage);
                }
            }
            if lines.pending() > MAX_LINE {
                return Err(LlmError::Protocol);
            }
        }
        // Jawaban berhenti sebelum baris `done`.
        Err(LlmError::Protocol)
    }

    async fn complete(
        &self,
        request: &ChatRequest,
        schema: &OutputSchema,
    ) -> Result<SensitiveText, LlmError> {
        #[derive(Deserialize)]
        struct Complete {
            message: Option<LineMessage>,
            error: Option<IgnoredAny>,
        }

        let body = ChatBody::new(request, false, Some(schema.raw()));
        let body = read_body(self.post_chat(&body).await?).await?;
        let parsed = serde_json::from_slice::<Complete>(&body).map_err(|_| LlmError::Protocol)?;
        if parsed.error.is_some() {
            return Err(LlmError::Server);
        }
        let message = parsed.message.ok_or(LlmError::Protocol)?;
        Ok(SensitiveText::new(message.content))
    }

    async fn get(&self, path: &str) -> Result<Zeroizing<Vec<u8>>, LlmError> {
        let response = self.client.get(self.url(path)).send().await?;
        read_body(check(response)?).await
    }

    async fn post_chat(&self, body: &ChatBody<'_>) -> Result<reqwest::Response, LlmError> {
        // Model cloud akan meneruskan teks ke luar PC; ditolak sebelum body dibuat.
        self.ensure_local(body.model).await?;
        // Body memuat teks pengguna; `from_owner` menghapusnya dari memori setelah terkirim.
        let json = Zeroizing::new(serde_json::to_vec(body).map_err(|_| LlmError::Protocol)?);
        let response = self
            .client
            .post(self.url("/api/chat"))
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(Bytes::from_owner(json))
            .send()
            .await?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(LlmError::ModelNotFound);
        }
        check(response)
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }
}

impl LlmProvider for OllamaProvider {
    fn chat<'a>(
        &'a self,
        request: &'a ChatRequest,
        on_token: &'a mut (dyn FnMut(&str) + Send),
    ) -> BoxFuture<'a, Result<Usage, LlmError>> {
        Box::pin(self.stream_chat(request, on_token))
    }

    fn complete_json<'a>(
        &'a self,
        request: &'a ChatRequest,
        schema: &'a OutputSchema,
    ) -> BoxFuture<'a, Result<SensitiveText, LlmError>> {
        Box::pin(self.complete(request, schema))
    }
}

/// Model cloud Ollama memakai tag `cloud` atau berakhiran `-cloud`, misalnya `gpt-oss:120b-cloud`.
/// Cadangan bila Ollama versi lama belum mengirim `remote_host`.
fn cloud_tag(name: &str) -> bool {
    name.rsplit_once(':')
        .is_some_and(|(_, tag)| tag == "cloud" || tag.ends_with("-cloud"))
}

fn check(response: reqwest::Response) -> Result<reqwest::Response, LlmError> {
    if response.status().is_success() {
        Ok(response)
    } else {
        Err(LlmError::Status(response.status().as_u16()))
    }
}

async fn read_body(mut response: reqwest::Response) -> Result<Zeroizing<Vec<u8>>, LlmError> {
    let mut body = Zeroizing::new(Vec::new());
    while let Some(chunk) = response.chunk().await? {
        if body.len() + chunk.len() > MAX_BODY {
            return Err(LlmError::Protocol);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

/// Satu baris jawaban streaming `/api/chat`.
#[derive(Deserialize)]
struct ChatLine {
    message: Option<LineMessage>,
    #[serde(default)]
    done: bool,
    error: Option<IgnoredAny>,
    #[serde(default)]
    prompt_eval_count: u32,
    #[serde(default)]
    eval_count: u32,
}

/// Bagian pesan; field lain seperti `thinking` diabaikan.
#[derive(Deserialize)]
struct LineMessage {
    #[serde(default)]
    content: String,
}

/// Meneruskan potongan jawaban di satu baris; `Some` saat jawaban selesai.
fn handle_line(
    line: &[u8],
    on_token: &mut (dyn FnMut(&str) + Send),
) -> Result<Option<Usage>, LlmError> {
    if line.iter().all(u8::is_ascii_whitespace) {
        return Ok(None);
    }
    let parsed = serde_json::from_slice::<ChatLine>(line).map_err(|_| LlmError::Protocol)?;
    if parsed.error.is_some() {
        return Err(LlmError::Server);
    }
    if let Some(message) = parsed.message {
        let content = Zeroizing::new(message.content);
        if !content.is_empty() {
            on_token(&content);
        }
    }
    Ok(parsed.done.then_some(Usage {
        prompt_tokens: parsed.prompt_eval_count,
        output_tokens: parsed.eval_count,
    }))
}

/// Penampung potongan jawaban sampai satu baris NDJSON lengkap.
#[derive(Default)]
struct Lines {
    buffer: Zeroizing<Vec<u8>>,
}

impl Lines {
    fn push(&mut self, chunk: &[u8]) {
        self.buffer.extend_from_slice(chunk);
    }

    fn next_line(&mut self) -> Option<Zeroizing<Vec<u8>>> {
        let end = self.buffer.iter().position(|&byte| byte == b'\n')?;
        Some(Zeroizing::new(self.buffer.drain(..=end).collect()))
    }

    /// Panjang baris yang belum lengkap.
    fn pending(&self) -> usize {
        self.buffer.len()
    }
}

#[derive(Serialize)]
struct ChatBody<'a> {
    model: &'a str,
    messages: Vec<WireMessage<'a>>,
    stream: bool,
    options: Options,
    /// Dalam detik.
    #[serde(skip_serializing_if = "Option::is_none")]
    keep_alive: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    format: Option<&'a serde_json::Value>,
}

impl<'a> ChatBody<'a> {
    fn new(request: &'a ChatRequest, stream: bool, format: Option<&'a serde_json::Value>) -> Self {
        Self {
            model: &request.model,
            messages: request
                .messages
                .iter()
                .map(|message| WireMessage {
                    role: message.role,
                    content: message.content.expose(),
                })
                .collect(),
            stream,
            options: Options {
                num_ctx: request.num_ctx.get(),
                temperature: request.temperature,
            },
            keep_alive: request.keep_alive.map(|duration| duration.as_secs()),
            format,
        }
    }
}

#[derive(Serialize)]
struct WireMessage<'a> {
    role: Role,
    content: &'a str,
}

#[derive(Serialize)]
struct Options {
    num_ctx: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead as _, BufReader, Read as _, Write as _};
    use std::net::{SocketAddr, TcpListener};
    use std::num::NonZeroU32;
    use std::sync::mpsc;
    use std::time::Duration;

    use nuenly_core::SensitiveText;
    use serde_json::{Value, json};

    use super::{OllamaModel, OllamaProvider};
    use crate::{ChatRequest, LlmError, LlmProvider, Message, OutputSchema, Usage};

    /// Permintaan yang diterima server palsu: baris pertama dan body JSON.
    struct Received {
        line: String,
        body: Value,
    }

    /// Satu jawaban server palsu. Potongan body ditulis satu per satu dengan jeda, agar satu
    /// baris JSON bisa terbelah di antara dua potongan.
    struct Reply {
        status: &'static str,
        chunks: Vec<&'static str>,
    }

    fn ok(chunks: Vec<&'static str>) -> Reply {
        Reply {
            status: "200 OK",
            chunks,
        }
    }

    /// Daftar model yang memuat `gemma3:4b`; dijawab sebelum setiap percakapan.
    fn tags() -> Reply {
        ok(vec![
            r#"{"models":[{"name":"gemma3:4b","size":3338801804}]}"#,
        ])
    }

    /// Ollama palsu yang menjawab satu koneksi per `Reply`, berurutan.
    fn serve(replies: Vec<Reply>) -> (SocketAddr, mpsc::Receiver<Received>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            for reply in replies {
                let (stream, _) = listener.accept().unwrap();
                let mut reader = BufReader::new(stream);
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let mut length = 0;
                loop {
                    let mut header = String::new();
                    reader.read_line(&mut header).unwrap();
                    if header == "\r\n" {
                        break;
                    }
                    if let Some((name, value)) = header.split_once(':')
                        && name.eq_ignore_ascii_case("content-length")
                    {
                        length = value.trim().parse().unwrap();
                    }
                }
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                let body = if body.is_empty() {
                    Value::Null
                } else {
                    serde_json::from_slice(&body).unwrap()
                };
                sender
                    .send(Received {
                        line: line.trim_end().to_owned(),
                        body,
                    })
                    .unwrap();

                let mut stream = reader.into_inner();
                write!(
                    stream,
                    "HTTP/1.1 {}\r\nContent-Type: application/x-ndjson\r\nConnection: close\r\n\r\n",
                    reply.status
                )
                .unwrap();
                for chunk in reply.chunks {
                    stream.write_all(chunk.as_bytes()).unwrap();
                    stream.flush().unwrap();
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
        });
        (address, receiver)
    }

    fn provider(address: SocketAddr) -> OllamaProvider {
        OllamaProvider::with_address(address).unwrap()
    }

    fn request() -> ChatRequest {
        let mut request = ChatRequest::new(
            "gemma3:4b",
            NonZeroU32::new(4096).unwrap(),
            vec![
                Message::system("Jawab singkat."),
                Message::user(SensitiveText::new("Halo, teks sintetis")),
            ],
        );
        request.keep_alive = Some(Duration::from_mins(5));
        request
    }

    async fn chat(provider: &OllamaProvider) -> (Result<Usage, LlmError>, Vec<String>) {
        let mut tokens = Vec::new();
        let mut on_token = |token: &str| tokens.push(token.to_owned());
        let result = provider.chat(&request(), &mut on_token).await;
        (result, tokens)
    }

    /// Permintaan percakapan, setelah permintaan daftar model.
    fn chat_request(received: &mpsc::Receiver<Received>) -> Received {
        assert_eq!(received.recv().unwrap().line, "GET /api/tags HTTP/1.1");
        received.recv().unwrap()
    }

    #[tokio::test]
    async fn streams_tokens_and_reports_usage() {
        let (address, received) = serve(vec![
            tags(),
            ok(vec![
                "{\"message\":{\"role\":\"assistant\",\"content\":\"Halo\"},\"done\":false}\n{\"message\":{\"con",
                "tent\":\" dunia\"},\"done\":false}\n",
                "{\"message\":{\"content\":\"\"},\"done\":true,\"prompt_eval_count\":12,\"eval_count\":2}\n",
            ]),
        ]);

        let (result, tokens) = chat(&provider(address)).await;

        assert_eq!(
            result.unwrap(),
            Usage {
                prompt_tokens: 12,
                output_tokens: 2
            }
        );
        assert_eq!(tokens, ["Halo", " dunia"]);
        let received = chat_request(&received);
        assert_eq!(received.line, "POST /api/chat HTTP/1.1");
        assert_eq!(
            received.body,
            json!({
                "model": "gemma3:4b",
                "messages": [
                    { "role": "system", "content": "Jawab singkat." },
                    { "role": "user", "content": "Halo, teks sintetis" }
                ],
                "stream": true,
                "options": { "num_ctx": 4096 },
                "keep_alive": 300
            })
        );
    }

    #[tokio::test]
    async fn ignores_thinking_and_blank_lines() {
        let (address, _received) = serve(vec![
            tags(),
            ok(vec![
                "{\"message\":{\"thinking\":\"menimbang\",\"content\":\"\"},\"done\":false}\n\n",
                "{\"message\":{\"content\":\"Ya\"},\"done\":false}\n{\"done\":true}\n",
            ]),
        ]);
        let (result, tokens) = chat(&provider(address)).await;
        assert!(result.is_ok());
        assert_eq!(tokens, ["Ya"]);
    }

    #[tokio::test]
    async fn an_error_line_fails_the_request() {
        let (address, _received) = serve(vec![
            tags(),
            ok(vec![
                "{\"message\":{\"content\":\"Ha\"},\"done\":false}\n{\"error\":\"model runner crashed\"}\n",
            ]),
        ]);
        let (result, tokens) = chat(&provider(address)).await;
        assert_eq!(result.unwrap_err(), LlmError::Server);
        assert_eq!(tokens, ["Ha"]);
    }

    #[tokio::test]
    async fn a_cut_off_answer_is_an_error() {
        let (address, _received) = serve(vec![
            tags(),
            ok(vec!["{\"message\":{\"content\":\"Ha\"},\"done\":false}\n"]),
        ]);
        let (result, _tokens) = chat(&provider(address)).await;
        assert_eq!(result.unwrap_err(), LlmError::Protocol);
    }

    #[tokio::test]
    async fn refuses_cloud_models_before_sending_any_text() {
        let (address, received) = serve(vec![ok(vec![
            r#"{"models":[{"name":"gemma3:4b","size":0,"remote_model":"gemma3:4b","remote_host":"https://ollama.com:443"}]}"#,
        ])]);

        let (result, tokens) = chat(&provider(address)).await;

        assert_eq!(result.unwrap_err(), LlmError::RemoteModel);
        assert_eq!(tokens, Vec::<String>::new());
        assert_eq!(received.recv().unwrap().line, "GET /api/tags HTTP/1.1");
        assert!(received.recv_timeout(Duration::from_millis(200)).is_err());
    }

    #[tokio::test]
    async fn refuses_unknown_models_before_sending_any_text() {
        let (address, received) = serve(vec![ok(vec![
            r#"{"models":[{"name":"gpt-oss:20b","size":13780173839}]}"#,
        ])]);

        let (result, _tokens) = chat(&provider(address)).await;

        assert_eq!(result.unwrap_err(), LlmError::ModelNotFound);
        assert_eq!(received.recv().unwrap().line, "GET /api/tags HTTP/1.1");
        assert!(received.recv_timeout(Duration::from_millis(200)).is_err());
    }

    #[tokio::test]
    async fn reports_a_model_removed_while_asking() {
        let (address, _received) = serve(vec![
            tags(),
            Reply {
                status: "404 Not Found",
                chunks: vec![
                    "{\"error\":\"model \\\"gemma3:4b\\\" not found, try pulling it first\"}",
                ],
            },
        ]);
        let (result, _tokens) = chat(&provider(address)).await;
        assert_eq!(result.unwrap_err(), LlmError::ModelNotFound);
    }

    #[tokio::test]
    async fn nobody_listening_is_unreachable() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        let (result, _tokens) = chat(&provider(address)).await;
        assert_eq!(result.unwrap_err(), LlmError::Unreachable);
    }

    #[test]
    fn only_loopback_addresses_are_allowed() {
        for address in ["192.168.1.10:11434", "0.0.0.0:11434", "[2001:db8::1]:11434"] {
            let address: SocketAddr = address.parse().unwrap();
            assert_eq!(
                OllamaProvider::with_address(address).unwrap_err(),
                LlmError::NotLoopback
            );
        }
        assert!(OllamaProvider::with_address("[::1]:11434".parse().unwrap()).is_ok());
    }

    #[tokio::test]
    async fn structured_output_sends_the_schema() {
        let (address, received) = serve(vec![
            tags(),
            ok(vec![
                "{\"message\":{\"role\":\"assistant\",\"content\":\"{\\\"title\\\":\\\"Ringkas\\\"}\"},\"done\":true}",
            ]),
        ]);
        let schema = OutputSchema::new(json!({ "type": "object" })).unwrap();

        let answer = provider(address)
            .complete_json(&request(), &schema)
            .await
            .unwrap();

        assert_eq!(answer.expose(), r#"{"title":"Ringkas"}"#);
        let received = chat_request(&received);
        assert_eq!(received.body["stream"], json!(false));
        assert_eq!(received.body["format"], json!({ "type": "object" }));
    }

    #[tokio::test]
    async fn reads_version_and_models() {
        let (address, received) = serve(vec![ok(vec!["{\"version\":\"0.12.6\"}"])]);
        assert_eq!(provider(address).version().await.unwrap(), "0.12.6");
        assert_eq!(received.recv().unwrap().line, "GET /api/version HTTP/1.1");

        let (address, _received) = serve(vec![ok(vec![
            r#"{"models":[
                {"name":"gpt-oss:20b","size":13780173839},
                {"name":"gemma3:4b","size":3338801804},
                {"name":"gpt-oss:120b-cloud","size":0},
                {"name":"glm-4.6:cloud","size":0},
                {"name":"kimi-k2:1t","size":0,"remote_model":"kimi-k2:1t","remote_host":"https://ollama.com:443"}
            ]}"#,
        ])]);
        let model = |name: &str, size: u64, remote: bool| OllamaModel {
            name: name.to_owned(),
            size,
            remote,
        };
        assert_eq!(
            provider(address).models().await.unwrap(),
            [
                model("gemma3:4b", 3_338_801_804, false),
                model("glm-4.6:cloud", 0, true),
                model("gpt-oss:120b-cloud", 0, true),
                model("gpt-oss:20b", 13_780_173_839, false),
                model("kimi-k2:1t", 0, true),
            ]
        );
    }

    #[tokio::test]
    async fn another_server_on_the_port_is_not_ollama() {
        let (address, _received) = serve(vec![ok(vec!["<!doctype html><title>x</title>"])]);
        assert_eq!(
            provider(address).version().await.unwrap_err(),
            LlmError::Protocol
        );

        let (address, _received) = serve(vec![Reply {
            status: "404 Not Found",
            chunks: vec!["404 page not found"],
        }]);
        assert_eq!(
            provider(address).version().await.unwrap_err(),
            LlmError::Status(404)
        );
    }
}
