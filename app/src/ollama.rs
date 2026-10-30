//! Pemeriksaan cepat untuk onboarding (F-26) dan Pengaturan › Model AI (F-27): apakah Ollama
//! berjalan di PC ini, versinya, dan model yang sudah diunduh. Percakapan dengan model menyusul
//! lewat `OllamaProvider` (langkah 6).

use std::ffi::OsStr;
use std::io::{Read as _, Write as _};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpStream};
use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Alamat bawaan Ollama. Hanya loopback: API Ollama tidak memakai autentikasi.
pub const ADDRESS: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 11434);
const TIMEOUT: Duration = Duration::from_millis(800);
/// Jawaban `/api/version` hanya puluhan byte.
const MAX_VERSION_RESPONSE: u64 = 4 * 1024;
/// Jawaban `/api/tags` sekitar 500 byte per model; jawaban yang lebih besar dianggap gagal.
const MAX_TAGS_RESPONSE: u64 = 256 * 1024;
const EXE: &str = if cfg!(windows) {
    "ollama.exe"
} else {
    "ollama"
};
const DOWNLOAD_PAGE: &str = "https://ollama.com/download/windows";

/// Bentuknya sama dengan `OllamaStatus` di `app/ui/src/lib/ollama.ts`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OllamaStatus {
    /// Ollama menjawab di 127.0.0.1:11434.
    Running,
    /// Program Ollama ada, tetapi belum dijalankan.
    Installed,
    #[default]
    Missing,
}

/// Bentuknya sama dengan `OllamaInfo` di `app/ui/src/lib/ollama.ts`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OllamaInfo {
    pub status: OllamaStatus,
    pub version: Option<String>,
    /// Nama model yang sudah diunduh, urut abjad. Kosong bila Ollama tidak berjalan.
    pub models: Vec<String>,
}

/// Memeriksa Ollama. Blocking hingga sekitar dua detik, jadi jalankan di luar thread UI.
pub fn detect() -> OllamaInfo {
    let address = SocketAddr::V4(ADDRESS);
    let version = get(address, "/api/version", MAX_VERSION_RESPONSE)
        .as_deref()
        .and_then(parse_version);
    if version.is_some() {
        let models = get(address, "/api/tags", MAX_TAGS_RESPONSE)
            .as_deref()
            .and_then(parse_models)
            .unwrap_or_default();
        return OllamaInfo {
            status: OllamaStatus::Running,
            version,
            models,
        };
    }
    let status = if installed(
        std::env::var_os("LOCALAPPDATA").as_deref(),
        std::env::var_os("PATH").as_deref(),
    ) {
        OllamaStatus::Installed
    } else {
        OllamaStatus::Missing
    };
    OllamaInfo {
        status,
        ..OllamaInfo::default()
    }
}

#[tauri::command]
pub async fn ollama_status() -> OllamaInfo {
    tauri::async_runtime::spawn_blocking(detect)
        .await
        .unwrap_or_default()
}

/// Membuka halaman unduhan Ollama. Tautannya tetap; frontend tidak bisa memilih alamat lain.
#[tauri::command]
pub fn ollama_open_download() -> Result<(), String> {
    open_https(DOWNLOAD_PAGE).map_err(|_| "Halaman unduhan tidak bisa dibuka.".to_owned())
}

#[cfg(windows)]
fn open_https(url: &str) -> std::io::Result<()> {
    nuenly_platform_windows::shell::open_https(url)
}

#[cfg(not(windows))]
fn open_https(_url: &str) -> std::io::Result<()> {
    Err(std::io::Error::other(
        "membuka tautan belum didukung di OS ini",
    ))
}

/// Isi jawaban `GET path` bila server menjawab 200 dalam `limit` byte, selain itu `None`.
fn get(address: SocketAddr, path: &str, limit: u64) -> Option<String> {
    let mut stream = TcpStream::connect_timeout(&address, TIMEOUT).ok()?;
    stream.set_read_timeout(Some(TIMEOUT)).ok()?;
    stream.set_write_timeout(Some(TIMEOUT)).ok()?;
    // HTTP/1.0: server menutup koneksi setelah menjawab dan tidak memakai chunked encoding.
    let request = format!("GET {path} HTTP/1.0\r\nHost: {address}\r\n\r\n");
    stream.write_all(request.as_bytes()).ok()?;
    let mut response = Vec::new();
    stream.take(limit).read_to_end(&mut response).ok()?;
    body(&response)
}

fn body(response: &[u8]) -> Option<String> {
    let response = std::str::from_utf8(response).ok()?;
    let (head, body) = response.split_once("\r\n\r\n")?;
    let status = head.lines().next()?.split_whitespace().nth(1)?;
    (status == "200").then(|| body.to_owned())
}

/// Versi dari jawaban `/api/version`; `None` bila yang menjawab bukan Ollama.
fn parse_version(body: &str) -> Option<String> {
    #[derive(Deserialize)]
    struct Version {
        version: String,
    }

    serde_json::from_str::<Version>(body)
        .ok()
        .map(|found| found.version)
}

/// Nama model dari jawaban `/api/tags`.
fn parse_models(body: &str) -> Option<Vec<String>> {
    #[derive(Deserialize)]
    struct Tags {
        models: Vec<Tag>,
    }
    #[derive(Deserialize)]
    struct Tag {
        name: String,
    }

    let tags = serde_json::from_str::<Tags>(body).ok()?;
    let mut names: Vec<String> = tags.models.into_iter().map(|tag| tag.name).collect();
    names.sort();
    names.dedup();
    Some(names)
}

/// Mencari program Ollama di lokasi pemasangan bawaan (`%LOCALAPPDATA%\Programs\Ollama`) dan
/// di `PATH`.
fn installed(local_app_data: Option<&OsStr>, path: Option<&OsStr>) -> bool {
    let default = local_app_data.map(|dir| Path::new(dir).join("Programs").join("Ollama"));
    let on_path = path.into_iter().flat_map(std::env::split_paths);
    default
        .into_iter()
        .chain(on_path)
        .any(|dir| dir.join(EXE).is_file())
}

#[cfg(test)]
mod tests {
    use std::io::{Read as _, Write as _};
    use std::net::{SocketAddr, TcpListener};
    use std::path::PathBuf;

    use super::{EXE, get, installed, parse_models, parse_version};

    /// Server satu kali di port acak yang membalas `response` apa adanya untuk `GET path`.
    fn serve_once(path: &'static str, response: &'static str) -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            // Baca permintaan dulu agar menutup koneksi tidak berubah menjadi reset.
            let mut request = [0; 1024];
            let read = stream.read(&mut request).unwrap();
            let expected = format!("GET {path} HTTP/1.0\r\n");
            assert!(request[..read].starts_with(expected.as_bytes()));
            stream.write_all(response.as_bytes()).unwrap();
        });
        address
    }

    fn version_of(address: SocketAddr) -> Option<String> {
        get(address, "/api/version", 4096)
            .as_deref()
            .and_then(parse_version)
    }

    #[test]
    fn recognises_ollama_by_its_version_endpoint() {
        let address = serve_once(
            "/api/version",
            "HTTP/1.0 200 OK\r\nContent-Type: application/json; charset=utf-8\r\n\r\n{\"version\":\"0.12.6\"}",
        );
        assert_eq!(version_of(address).as_deref(), Some("0.12.6"));
    }

    #[test]
    fn another_server_on_the_port_is_not_ollama() {
        let not_found = serve_once(
            "/api/version",
            "HTTP/1.0 404 Not Found\r\n\r\n404 page not found",
        );
        assert_eq!(version_of(not_found), None);

        let web_page = serve_once(
            "/api/version",
            "HTTP/1.0 200 OK\r\n\r\n<!doctype html><title>x</title>",
        );
        assert_eq!(version_of(web_page), None);
    }

    #[test]
    fn nothing_listening_is_not_ollama() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        assert_eq!(version_of(address), None);
    }

    #[test]
    fn a_response_over_the_limit_is_ignored() {
        let address = serve_once(
            "/api/tags",
            "HTTP/1.0 200 OK\r\n\r\n{\"models\":[{\"name\":\"gpt-oss:20b\"}]}",
        );
        let body = get(address, "/api/tags", 24);
        assert_eq!(body.as_deref().and_then(parse_models), None);
    }

    #[test]
    fn lists_downloaded_models_sorted() {
        let body = r#"{"models":[
            {"name":"gpt-oss:20b","model":"gpt-oss:20b","size":13780173839,"details":{"family":"gptoss"}},
            {"name":"gemma3:4b","model":"gemma3:4b","size":3338801804,"details":{"family":"gemma3"}}
        ]}"#;
        assert_eq!(
            parse_models(body),
            Some(vec!["gemma3:4b".to_owned(), "gpt-oss:20b".to_owned()])
        );
        assert_eq!(parse_models(r#"{"models":[]}"#), Some(Vec::new()));
        assert_eq!(parse_models("bukan json"), None);
    }

    #[test]
    fn finds_ollama_in_the_default_location_or_on_path() {
        let root: PathBuf =
            std::env::temp_dir().join(format!("nuenly-ollama-{}", std::process::id()));
        let local = root.join("local");
        let default = local.join("Programs").join("Ollama");
        let bin = root.join("bin");
        std::fs::create_dir_all(&default).unwrap();
        std::fs::create_dir_all(&bin).unwrap();

        let found = installed(Some(local.as_os_str()), Some(bin.as_os_str()));
        assert!(!found);

        std::fs::write(bin.join(EXE), b"").unwrap();
        assert!(installed(None, Some(bin.as_os_str())));

        std::fs::write(default.join(EXE), b"").unwrap();
        assert!(installed(Some(local.as_os_str()), None));

        std::fs::remove_dir_all(&root).unwrap();
    }
}
