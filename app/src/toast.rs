//! Jendela toast (F-21, US-2): satu toast di pojok kanan bawah yang tidak mengambil fokus.
//!
//! Jendelanya `focusable: false` (`WS_EX_NOACTIVATE`), jadi muncul maupun diklik tidak
//! memindahkan fokus keyboard dari aplikasi pengguna.

use serde::Serialize;
use tauri::{AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, WebviewWindow};

pub const LABEL: &str = "toast";

/// Lebar jendela dalam piksel logis: kartu 376 ditambah jarak 12 di kiri dan kanan.
const WIDTH: f64 = 400.0;
/// Tinggi sementara sebelum frontend melaporkan tinggi toast sebenarnya.
const INITIAL_HEIGHT: f64 = 160.0;
const MAX_HEIGHT: f64 = 640.0;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Toast {
    pub id: String,
    /// `None`: tetap tampil sampai ditutup pengguna, misalnya saran penting di Medical.
    pub duration_ms: Option<u64>,
    #[serde(flatten)]
    pub content: ToastContent,
}

/// Isi toast; bentuknya sama dengan `ToastPayload` di `app/ui/src/windows/toast/types.ts`.
// Produsen saran dan izin (trigger engine, policy gate) baru ada di Fase 2; sampai itu varian
// selain `Success` hanya dipakai submenu uji di build debug.
#[cfg_attr(not(debug_assertions), allow(dead_code))]
#[derive(Clone, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ToastContent {
    Suggestion {
        title: String,
        body: String,
        warning: Option<String>,
        /// Catatan dari mode pack, misalnya "Saran, bukan diagnosis".
        note: Option<String>,
        points: Vec<String>,
        source: Option<String>,
    },
    Success {
        message: String,
    },
    Thinking {
        message: String,
    },
    Permission {
        title: String,
        body: String,
    },
}

/// Menampilkan toast. Jendela langsung ditampilkan (tanpa diaktifkan) dengan tinggi sementara
/// agar `WebView2` tetap merender, lalu frontend menyesuaikan tingginya lewat [`toast_layout`].
pub fn show(app: &AppHandle, toast: &Toast) -> tauri::Result<()> {
    let Some(window) = app.get_webview_window(LABEL) else {
        return Ok(());
    };
    if !window.is_visible()? {
        place(&window, INITIAL_HEIGHT)?;
    }
    app.emit_to(LABEL, "toast:show", toast)
}

/// Dipanggil frontend setiap tinggi toast terdepan berubah. `height` dalam piksel logis;
/// 0 berarti tidak ada toast dan jendela disembunyikan.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri menyuntikkan jendela pemanggil sebagai nilai.
pub fn toast_layout(window: WebviewWindow, height: f64) -> tauri::Result<()> {
    if window.label() != LABEL {
        return Ok(());
    }
    if height.is_nan() || height <= 0.0 {
        return window.hide();
    }
    place(&window, height.min(MAX_HEIGHT))
}

/// Menempatkan jendela di pojok kanan bawah work area monitor utama, lalu menampilkannya.
fn place(window: &WebviewWindow, height: f64) -> tauri::Result<()> {
    let Some(monitor) = window.primary_monitor()? else {
        return Ok(());
    };
    let area = monitor.work_area();
    let size = LogicalSize::new(WIDTH, height).to_physical::<u32>(monitor.scale_factor());
    let x = far_edge(area.position.x, area.size.width, size.width);
    let y = far_edge(area.position.y, area.size.height, size.height);
    window.set_size(size)?;
    window.set_position(PhysicalPosition::new(x, y))?;
    window.show()
}

/// Koordinat awal agar benda sepanjang `inner` menempel di ujung jauh area `[start, start + outer)`.
fn far_edge(start: i32, outer: u32, inner: u32) -> i32 {
    start.saturating_add(i32::try_from(outer.saturating_sub(inner)).unwrap_or(i32::MAX))
}

/// Toast contoh untuk submenu tray "Uji toast" di build debug: memeriksa tampilan dan bahwa toast
/// tidak mencuri fokus di Word, Chrome, dan `WinForms` (spike Fase 0). Teksnya rekaan.
#[cfg(debug_assertions)]
pub mod demo {
    use super::{Toast, ToastContent};

    pub const PREFIX: &str = "uji-toast:";
    pub const ALL: [(&str, &str); 5] = [
        ("saran", "Saran"),
        ("penting", "Saran penting"),
        ("berhasil", "Berhasil"),
        ("berpikir", "Sedang berpikir"),
        ("izin", "Butuh izin"),
    ];

    pub fn toast(name: &str) -> Option<Toast> {
        let text = str::to_owned;
        let (duration_ms, content) = match name {
            "saran" => (
                Some(8000),
                ToastContent::Suggestion {
                    title: text("Email ini bisa lebih singkat"),
                    body: text("Dua kalimat terakhir bisa digabung."),
                    warning: None,
                    note: None,
                    points: vec![text("Gabungkan dua kalimat terakhir menjadi satu ajakan.")],
                    source: None,
                },
            ),
            "penting" => (
                None,
                ToastContent::Suggestion {
                    title: text("3 hal belum tercatat"),
                    body: text("Durasi nyeri, alergi obat, dan faktor risiko."),
                    warning: Some(text("Pertimbangkan EKG segera.")),
                    note: Some(text("Saran, bukan diagnosis")),
                    points: vec![
                        text("Sejak kapan nyeri dimulai, berapa lama"),
                        text("Riwayat alergi obat"),
                        text("Merokok, diabetes, hipertensi"),
                    ],
                    source: Some(text("Sumber: [PANDUAN KLINIS] · diproses di PC ini")),
                },
            ),
            "berhasil" => (
                Some(3000),
                ToastContent::Success {
                    message: text("Poin disalin. Tempel dengan Ctrl V."),
                },
            ),
            "berpikir" => (
                None,
                ToastContent::Thinking {
                    message: text("Membaca field anamnesis…"),
                },
            ),
            "izin" => (
                None,
                ToastContent::Permission {
                    title: text("Boleh cari kode ICD-10?"),
                    body: text("Hanya keluhan yang dikirim, tanpa data pasien."),
                },
            ),
            _ => return None,
        };
        Some(Toast {
            id: format!("uji-{name}"),
            duration_ms,
            content,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::far_edge;

    #[test]
    fn far_edge_sticks_to_the_end_of_the_area() {
        assert_eq!(far_edge(0, 1920, 400), 1520);
        assert_eq!(far_edge(-1920, 1920, 400), -400);
        assert_eq!(far_edge(100, 300, 400), 100);
    }

    #[cfg(debug_assertions)]
    #[test]
    fn every_demo_toast_serializes_with_its_kind() {
        for (name, _) in super::demo::ALL {
            let toast = super::demo::toast(name).unwrap();
            let json = serde_json::to_value(&toast).unwrap();
            assert!(json["kind"].is_string());
            assert!(json.get("durationMs").is_some());
        }
    }
}
