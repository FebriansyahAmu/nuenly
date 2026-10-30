//! Jendela Tanya Nuenly (F-22, US-1): overlay redup di layar kerja tempat kursor berada.
//! Hotkey (F-03) membukanya dengan teks terpilih di aplikasi aktif sebagai konteks (F-02).

use std::sync::{Mutex, MutexGuard, PoisonError};

use nuenly_core::{CaptureKind, CaptureOutcome, ContextError, ContextSnapshot, SensitivePage};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::oneshot;

use crate::context::ContextReader;
use crate::onboarding;
use crate::status::AppStatus;

pub const LABEL: &str = "ask";

/// Sesi Tanya Nuenly yang sedang terbuka. Teks konteksnya tetap di Rust; jendela hanya menerima
/// asal dan nama aplikasinya. Menutup jendela membuang teks itu dan menghentikan jawaban.
#[derive(Default)]
pub struct AskState(Mutex<Session>);

#[derive(Default)]
struct Session {
    context: Option<ContextSnapshot>,
    /// Jawaban yang sedang berjalan berhenti saat pengirim ini di-drop.
    answering: Option<oneshot::Sender<()>>,
}

impl AskState {
    fn session(&self) -> MutexGuard<'_, Session> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Sesi baru: konteks diganti dan jawaban dari sesi lama dihentikan.
    fn replace(&self, context: Option<ContextSnapshot>) {
        *self.session() = Session {
            context,
            answering: None,
        };
    }

    /// Teks konteks untuk dijawab. Bila pengguna menghapus chip (`use_context` `false`),
    /// teksnya langsung dibuang.
    pub fn context(&self, use_context: bool) -> Option<ContextSnapshot> {
        let mut session = self.session();
        if !use_context {
            session.context = None;
        }
        session.context.clone()
    }

    /// Memulai jawaban baru dan menghentikan jawaban sebelumnya. Hasilnya selesai saat jawaban
    /// ini harus berhenti: jendela ditutup, sesi baru dibuka, atau jawaban lain dimulai.
    pub fn begin_answer(&self) -> oneshot::Receiver<()> {
        let (stop, stopped) = oneshot::channel();
        self.session().answering = Some(stop);
        stopped
    }
}

/// Isi event `ask:open`; bentuknya sama dengan `AskOpen` di `app/ui/src/windows/ask/backend.ts`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AskOpen {
    pub context: Option<ContextView>,
    /// Alasan teks terpilih tidak dipakai, bila perlu diberitahukan.
    pub notice: Option<Notice>,
}

/// Asal konteks untuk chip "Dari:".
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextView {
    /// `selection` atau `field`.
    pub source: &'static str,
    /// Nama aplikasi dari metadata executable, bukan dari layar.
    pub app_name: String,
    /// Cuplikan satu baris, paling banyak [`PREVIEW_CHARS`] karakter, bila kebijakan mode
    /// mengizinkan (`ActivePolicy::context_preview`). Teks utuhnya tetap di Rust.
    pub preview: Option<String>,
}

/// Panjang cuplikan teks terpilih yang dikirim ke jendela, dalam karakter.
const PREVIEW_CHARS: usize = 200;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Notice {
    /// Nuenly sedang dijeda, jadi tidak membaca apa pun.
    Paused,
    /// Filter aplikasi menolak aplikasi ini.
    NotAllowed,
    /// Jendela privat di browser.
    PrivateWindow,
    /// Situs perbankan.
    Banking,
    /// Field kata sandi.
    ProtectedField,
    /// Aplikasi tidak menjawab atau tidak mengekspos teksnya.
    Unreadable,
}

/// Dipanggil setiap kali hotkey ditekan, di thread hotkey. Menekannya lagi saat Tanya Nuenly
/// terbuka akan menutupnya.
pub fn on_hotkey(app: &AppHandle) {
    let Some(window) = app.get_webview_window(LABEL) else {
        return;
    };
    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
        closed(app);
        return;
    }
    // Teks terpilih hanya dibaca setelah "Saya mengerti" di onboarding.
    if !onboarding::ensure_accepted(app) {
        return;
    }
    // Dibaca sebelum jendela tampil, karena setelah itu fokus sudah pindah ke Nuenly.
    let (context, open) = read_selection(app);
    let _ = show(app, context, &open);
}

/// Dari menu tray. Tidak membaca apa pun, karena fokus sudah pindah ke menu.
pub fn open(app: &AppHandle) -> tauri::Result<()> {
    if !onboarding::ensure_accepted(app) {
        return Ok(());
    }
    show(app, None, &AskOpen::default())
}

/// Dipanggil saat jendela disembunyikan: sesi selesai, teks konteksnya dibuang, dan jawaban
/// yang masih berjalan dihentikan.
pub fn closed(app: &AppHandle) {
    app.state::<AskState>().replace(None);
}

fn read_selection(app: &AppHandle) -> (Option<ContextSnapshot>, AskOpen) {
    // Jendela Nuenly sendiri tidak pernah dibaca; dari sana Tanya Nuenly dibuka tanpa konteks.
    let own_window_focused = app
        .webview_windows()
        .values()
        .any(|window| window.is_focused().unwrap_or(false));
    if own_window_focused {
        return (None, AskOpen::default());
    }
    let Some(policy) = app.state::<AppStatus>().reading_policy() else {
        let open = AskOpen {
            context: None,
            notice: Some(Notice::Paused),
        };
        return (None, open);
    };
    prepare(
        app.state::<ContextReader>().selection(&policy),
        policy.context_preview,
    )
}

/// Memisahkan hasil baca menjadi teks konteks (tetap di Rust) dan isi event untuk jendela,
/// dengan cuplikan teks hanya bila `show_preview`.
fn prepare(
    result: Result<CaptureOutcome, ContextError>,
    show_preview: bool,
) -> (Option<ContextSnapshot>, AskOpen) {
    let notice = match result {
        Ok(CaptureOutcome::Captured(snapshot)) => {
            let context = ContextView {
                source: match snapshot.kind {
                    CaptureKind::SelectedText => "selection",
                    _ => "field",
                },
                app_name: snapshot.app.display_name.clone(),
                preview: show_preview.then(|| preview(snapshot.text.expose())),
            };
            let open = AskOpen {
                context: Some(context),
                notice: None,
            };
            return (Some(snapshot), open);
        }
        // Tidak ada teks terpilih: bertanya tanpa konteks adalah hal biasa.
        Ok(CaptureOutcome::NoWindow | CaptureOutcome::Empty) => None,
        Ok(CaptureOutcome::NotAllowed(_)) => Some(Notice::NotAllowed),
        Ok(CaptureOutcome::Sensitive(SensitivePage::PrivateWindow)) => Some(Notice::PrivateWindow),
        Ok(CaptureOutcome::Sensitive(SensitivePage::Banking)) => Some(Notice::Banking),
        Ok(CaptureOutcome::ProtectedField) => Some(Notice::ProtectedField),
        Err(_) => Some(Notice::Unreadable),
    };
    (
        None,
        AskOpen {
            context: None,
            notice,
        },
    )
}

/// Awal `text` dalam satu baris: spasi dan baris baru diringkas menjadi satu spasi, lalu
/// dipotong di [`PREVIEW_CHARS`] karakter dengan "…". Teks sisanya tidak pernah disalin.
fn preview(text: &str) -> String {
    let mut preview = String::with_capacity(PREVIEW_CHARS * 4 + '…'.len_utf8());
    let mut chars = 0;
    let mut words = text.split_whitespace().peekable();
    while let Some(word) = words.next() {
        for c in word.chars() {
            if chars == PREVIEW_CHARS {
                preview.push('…');
                return preview;
            }
            preview.push(c);
            chars += 1;
        }
        if words.peek().is_some() {
            if chars == PREVIEW_CHARS {
                preview.push('…');
                return preview;
            }
            preview.push(' ');
            chars += 1;
        }
    }
    preview
}

/// Membuka Tanya Nuenly di monitor tempat kursor berada dan memulai sesi baru.
///
/// Jendelanya sudah dibuat (tersembunyi) saat start, jadi membuka hanya memindahkan,
/// memberi tahu frontend, lalu menampilkan; target PRD ≤ 150 ms setelah hotkey.
fn show(app: &AppHandle, context: Option<ContextSnapshot>, open: &AskOpen) -> tauri::Result<()> {
    let Some(window) = app.get_webview_window(LABEL) else {
        return Ok(());
    };

    let cursor = app.cursor_position()?;
    let monitor = match app.monitor_from_point(cursor.x, cursor.y)? {
        Some(monitor) => Some(monitor),
        None => app.primary_monitor()?,
    };
    if let Some(monitor) = monitor {
        let area = monitor.work_area();
        window.set_position(area.position)?;
        window.set_size(area.size)?;
    }

    app.state::<AskState>().replace(context);
    app.emit_to(LABEL, "ask:open", open)?;
    window.show()?;
    window.set_focus()
}

#[cfg(test)]
mod tests {
    use nuenly_core::{
        ActivePolicy, AppFilter, AppIdentity, CaptureKind, ContextError, ControlKind,
        FakeContextSource, FieldInfo, ModeId, ProcessName, capture,
    };

    use super::{ContextView, Notice, PREVIEW_CHARS, prepare, preview};

    fn app(process: &str, display_name: &str) -> AppIdentity {
        AppIdentity {
            process: ProcessName::new(process).unwrap(),
            display_name: display_name.to_owned(),
        }
    }

    fn policy() -> ActivePolicy {
        ActivePolicy::new(ModeId::new("all").unwrap())
    }

    fn read(source: &FakeContextSource, policy: &ActivePolicy) -> super::AskOpen {
        let (context, open) = prepare(
            capture(source, policy, CaptureKind::SelectedText),
            policy.context_preview,
        );
        // Teks hanya disimpan bersama konteks yang ditampilkan di chip.
        assert_eq!(context.is_some(), open.context.is_some());
        open
    }

    fn notepad() -> FakeContextSource {
        FakeContextSource::new()
            .with_foreground(app("notepad.exe", "Notepad"), "catatan.txt")
            .with_selection("rapat sintetis\n\ndigeser ke   kamis")
    }

    #[test]
    fn without_preview_only_the_origin_is_sent() {
        let (context, open) = prepare(
            capture(&notepad(), &policy(), CaptureKind::SelectedText),
            false,
        );

        assert_eq!(
            context.unwrap().text.expose(),
            "rapat sintetis\n\ndigeser ke   kamis"
        );
        assert_eq!(
            open.context,
            Some(ContextView {
                source: "selection",
                app_name: "Notepad".to_owned(),
                preview: None,
            })
        );
        let json = serde_json::to_string(&open).unwrap();
        assert!(!json.contains("sintetis"));
        assert!(!json.contains("catatan"));
    }

    #[test]
    fn with_preview_a_one_line_excerpt_is_sent() {
        let mut umum = policy();
        umum.context_preview = true;

        let open = read(&notepad(), &umum);

        assert_eq!(
            open.context.unwrap().preview.as_deref(),
            Some("rapat sintetis digeser ke kamis")
        );
    }

    #[test]
    fn preview_is_cut_without_copying_the_rest() {
        let long = "kata ".repeat(100);
        let cut = preview(&long);
        assert_eq!(cut.chars().count(), PREVIEW_CHARS + 1);
        assert!(cut.ends_with('…'));
        assert_eq!(preview("  satu\tdua\r\n"), "satu dua");
        assert_eq!(preview(""), "");
    }

    #[test]
    fn nothing_selected_opens_without_a_notice() {
        let source = FakeContextSource::new().with_foreground(app("notepad.exe", "Notepad"), "");
        assert_eq!(read(&source, &policy()), super::AskOpen::default());
        assert_eq!(
            read(&FakeContextSource::new(), &policy()),
            super::AskOpen::default()
        );
    }

    #[test]
    fn blocked_app_and_password_field_are_explained() {
        let mut blocking = policy();
        blocking.apps = AppFilter {
            allow: None,
            block: vec![ProcessName::new("outlook.exe").unwrap()],
        };
        let outlook = FakeContextSource::new()
            .with_foreground(app("outlook.exe", "Mail"), "")
            .with_selection("teks sintetis");
        assert_eq!(read(&outlook, &blocking).notice, Some(Notice::NotAllowed));

        let password = FakeContextSource::new()
            .with_foreground(app("chrome.exe", "Chrome"), "Masuk")
            .with_focused_field(
                FieldInfo {
                    label: None,
                    control: ControlKind::Edit,
                    is_password: true,
                    bounds: None,
                },
                "sandi-sintetis",
            );
        let (context, open) = prepare(
            capture(&password, &policy(), CaptureKind::FocusedField),
            false,
        );
        assert!(context.is_none());
        assert_eq!(open.notice, Some(Notice::ProtectedField));
    }

    #[test]
    fn private_window_and_bank_site_are_explained() {
        let private = FakeContextSource::new()
            .with_foreground(app("msedge.exe", "Microsoft Edge"), "")
            .with_browser_page(&["Tab baru - [InPrivate] - Microsoft Edge"], None)
            .with_selection("teks sintetis");
        assert_eq!(
            read(&private, &policy()).notice,
            Some(Notice::PrivateWindow)
        );

        let bank = FakeContextSource::new()
            .with_foreground(app("chrome.exe", "Google Chrome"), "")
            .with_browser_page(&["Masuk"], Some("ib.bri.co.id"))
            .with_selection("teks sintetis");
        assert_eq!(read(&bank, &policy()).notice, Some(Notice::Banking));
    }

    #[test]
    fn platform_errors_open_without_context() {
        for err in [ContextError::Timeout, ContextError::ForegroundChanged] {
            let (context, open) = prepare(Err(err), true);
            assert!(context.is_none());
            assert_eq!(open.notice, Some(Notice::Unreadable));
        }
    }
}
