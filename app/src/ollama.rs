//! Pemeriksaan cepat untuk onboarding (F-26): apakah Ollama berjalan di PC ini, atau setidaknya
//! sudah terpasang. Percakapan dengan model menyusul lewat `OllamaProvider` (langkah 6).

use std::ffi::OsStr;
use std::io::{Read as _, Write as _};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpStream};
use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Alamat bawaan Ollama. Hanya loopback: API Ollama tidak memakai autentikasi.
const ADDRESS: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 11434);
const TIMEOUT: Duration = Duration::from_millis(800);
/// Jawaban `/api/version` hanya puluhan byte; selebihnya tidak dibaca.
const MAX_RESPONSE: u64 = 4096;
const EXE: &str = if cfg!(windows) {
    "ollama.exe"
} else {
    "ollama"
};

/// Hasil pemeriksaan; bentuknya sama dengan `OllamaStatus` di
/// `app/ui/src/windows/onboarding/backend.ts`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OllamaStatus {
    /// Ollama menjawab di 127.0.0.1:11434.
    Running,
    /// Program Ollama ada, tetapi belum dijalankan.
    Installed,
    Missing,
}

/// Memeriksa Ollama. Blocking hingga sekitar satu detik, jadi jalankan di luar thread UI.
pub fn detect() -> OllamaStatus {
    if version(SocketAddr::V4(ADDRESS)).is_some() {
        OllamaStatus::Running
    } else if installed(
        std::env::var_os("LOCALAPPDATA").as_deref(),
        std::env::var_os("PATH").as_deref(),
    ) {
        OllamaStatus::Installed
    } else {
        OllamaStatus::Missing
    }
}

/// Versi Ollama dari `GET /api/version`, atau `None` bila tidak ada yang menjawab atau yang
/// menjawab bukan Ollama.
fn version(address: SocketAddr) -> Option<String> {
    let mut stream = TcpStream::connect_timeout(&address, TIMEOUT).ok()?;
    stream.set_read_timeout(Some(TIMEOUT)).ok()?;
    stream.set_write_timeout(Some(TIMEOUT)).ok()?;
    // HTTP/1.0: server menutup koneksi setelah menjawab dan tidak memakai chunked encoding.
    let request = format!("GET /api/version HTTP/1.0\r\nHost: {address}\r\n\r\n");
    stream.write_all(request.as_bytes()).ok()?;
    let mut response = Vec::new();
    stream.take(MAX_RESPONSE).read_to_end(&mut response).ok()?;
    parse_version(&response)
}

fn parse_version(response: &[u8]) -> Option<String> {
    #[derive(Deserialize)]
    struct Version {
        version: String,
    }

    let response = std::str::from_utf8(response).ok()?;
    let (head, body) = response.split_once("\r\n\r\n")?;
    let status = head.lines().next()?.split_whitespace().nth(1)?;
    if status != "200" {
        return None;
    }
    serde_json::from_str::<Version>(body)
        .ok()
        .map(|found| found.version)
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

    use super::{EXE, installed, version};

    /// Server satu kali di port acak yang membalas `response` apa adanya.
    fn serve_once(response: &'static str) -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            // Baca permintaan dulu agar menutup koneksi tidak berubah menjadi reset.
            let mut request = [0; 1024];
            let read = stream.read(&mut request).unwrap();
            assert!(request[..read].starts_with(b"GET /api/version HTTP/1.0\r\n"));
            stream.write_all(response.as_bytes()).unwrap();
        });
        address
    }

    #[test]
    fn recognises_ollama_by_its_version_endpoint() {
        let address = serve_once(
            "HTTP/1.0 200 OK\r\nContent-Type: application/json; charset=utf-8\r\n\r\n{\"version\":\"0.12.6\"}",
        );
        assert_eq!(version(address).as_deref(), Some("0.12.6"));
    }

    #[test]
    fn another_server_on_the_port_is_not_ollama() {
        let not_found = serve_once("HTTP/1.0 404 Not Found\r\n\r\n404 page not found");
        assert_eq!(version(not_found), None);

        let web_page = serve_once("HTTP/1.0 200 OK\r\n\r\n<!doctype html><title>x</title>");
        assert_eq!(version(web_page), None);
    }

    #[test]
    fn nothing_listening_is_not_ollama() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        assert_eq!(version(address), None);
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
