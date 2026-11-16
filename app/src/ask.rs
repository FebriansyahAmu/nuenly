//! Jendela Tanya Nuenly (F-22, US-1): overlay redup di layar kerja tempat kursor berada.
//! Hotkey (F-03) membukanya dengan teks terpilih di aplikasi aktif sebagai konteks, atau isi
//! field fokus bila tidak ada yang dipilih (F-02).

use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Instant;

use nuenly_core::{
    CaptureKind, CaptureOutcome, ContextError, ContextSnapshot, SensitivePage, SensitiveText,
};
use nuenly_store::ReplaceSource;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::oneshot;

use crate::context::ContextReader;
use crate::replace::{ReplaceFailure, Replacement};
use crate::status::AppStatus;
use crate::{foreground, logging, onboarding};

pub const LABEL: &str = "ask";

/// Sesi Tanya Nuenly yang sedang terbuka. Teks konteksnya tetap di Rust; jendela hanya menerima
/// asal dan nama aplikasinya. Menutup jendela membuang teks itu dan menghentikan jawaban.
#[derive(Default)]
pub struct AskState(Mutex<Session>);

#[derive(Default)]
struct Session {
    context: Option<ContextSnapshot>,
    /// Target Ganti teks untuk konteks ini; `None` bila teksnya tidak bisa diganti.
    target: Option<u64>,
    /// Jawaban yang sedang berjalan berhenti saat pengirim ini di-drop.
    answering: Option<oneshot::Sender<()>>,
    /// Nomor jawaban terakhir yang dimulai. Terus naik antar-sesi, jadi jawaban dari sesi lama
    /// tidak pernah tersimpan di sesi baru.
    answers: u64,
    /// Jawaban terakhir yang selesai, dipakai paling banyak sekali oleh Ganti teks.
    answer: Option<SensitiveText>,
}

impl AskState {
    fn session(&self) -> MutexGuard<'_, Session> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Sesi baru: konteks diganti dan jawaban dari sesi lama dihentikan dan dibuang.
    fn start(&self, context: Option<ContextSnapshot>, target: Option<u64>) {
        let mut session = self.session();
        let answers = session.answers + 1;
        *session = Session {
            context,
            target,
            answers,
            ..Session::default()
        };
    }

    /// Teks konteks untuk dijawab. Bila pengguna menghapus chip (`use_context` `false`),
    /// teksnya langsung dibuang, begitu juga target Ganti teks-nya.
    pub fn context(&self, use_context: bool) -> Option<ContextSnapshot> {
        let mut session = self.session();
        if !use_context {
            session.context = None;
            session.target = None;
        }
        session.context.clone()
    }

    /// Memulai jawaban baru dan menghentikan jawaban sebelumnya. Hasilnya selesai saat jawaban
    /// ini harus berhenti (jendela ditutup, sesi baru dibuka, atau jawaban lain dimulai), beserta
    /// nomor jawaban untuk [`finish_answer`](Self::finish_answer).
    pub fn begin_answer(&self) -> (oneshot::Receiver<()>, u64) {
        let (stop, stopped) = oneshot::channel();
        let mut session = self.session();
        session.answers += 1;
        session.answer = None;
        session.answering = Some(stop);
        (stopped, session.answers)
    }

    /// Jawaban ke-`id` selesai utuh. Disimpan hanya bila belum ada jawaban atau sesi yang lebih
    /// baru.
    pub fn finish_answer(&self, id: u64, answer: SensitiveText) {
        let mut session = self.session();
        if session.answers == id {
            session.answer = Some(answer);
        }
    }

    /// Bahan Ganti teks: target, teks yang dibaca saat hotkey, dan jawaban terakhir tanpa spasi
    /// di tepi. Jawabannya diambil, jadi satu jawaban hanya bisa dipakai sekali.
    ///
    /// # Errors
    ///
    /// [`ReplaceFailure::Unavailable`] bila tidak ada konteks yang bisa diganti atau belum ada
    /// jawaban yang selesai.
    pub fn replacement(&self) -> Result<Replacement, ReplaceFailure> {
        let mut session = self.session();
        let (Some(context), Some(target)) = (&session.context, session.target) else {
            return Err(ReplaceFailure::Unavailable);
        };
        let source = match context.kind {
            CaptureKind::SelectedText => ReplaceSource::Selection,
            CaptureKind::FocusedField => ReplaceSource::Field,
            _ => return Err(ReplaceFailure::Unavailable),
        };
        let expected = context.text.clone();
        let answer = session.answer.take().ok_or(ReplaceFailure::Unavailable)?;
        let text = SensitiveText::new(answer.expose().trim());
        if text.is_empty() {
            return Err(ReplaceFailure::Unavailable);
        }
        Ok(Replacement {
            target,
            source,
            expected,
            text,
        })
    }
}

/// Isi event `ask:open`; bentuknya sama dengan `AskOpen` di `app/ui/src/windows/ask/backend.ts`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AskOpen {
    pub context: Option<ContextView>,
    /// Alasan teks dari aplikasi tidak dipakai, bila perlu diberitahukan.
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
    /// Ganti teks bisa dipakai untuk teks ini: teks terpilih, atau seluruh isi kolom.
    pub replaceable: bool,
}

/// Panjang cuplikan teks konteks yang dikirim ke jendela, dalam karakter.
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
    let pressed = Instant::now();
    let Some(window) = app.get_webview_window(LABEL) else {
        return;
    };
    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
        closed(app);
        return;
    }
    // Teks dari aplikasi hanya dibaca setelah "Saya mengerti" di onboarding.
    if !onboarding::ensure_accepted(app) {
        return;
    }
    // Dibaca sebelum jendela tampil, karena setelah itu fokus sudah pindah ke Nuenly.
    let (context, target, open) = read_context(app);
    let read = pressed.elapsed();
    let source = open
        .context
        .as_ref()
        .map_or("none", |context| context.source);
    let _ = show(app, context, target, &open);
    // Target PRD: tampil ≤ 150 ms setelah hotkey. Diukur sampai Windows menampilkan jendela.
    tracing::info!(
        read_ms = logging::millis(read),
        shown_ms = logging::millis(pressed.elapsed()),
        source,
        "Tanya Nuenly tampil setelah hotkey"
    );
}

/// Dari menu tray. Tidak membaca apa pun, karena fokus sudah pindah ke menu.
pub fn open(app: &AppHandle) -> tauri::Result<()> {
    if !onboarding::ensure_accepted(app) {
        return Ok(());
    }
    show(app, None, None, &AskOpen::default())
}

/// Dipanggil saat jendela disembunyikan: sesi selesai, teks konteks dan jawabannya dibuang,
/// target Ganti teks dilupakan, dan jawaban yang masih berjalan dihentikan.
pub fn closed(app: &AppHandle) {
    app.state::<AskState>().start(None, None);
    if let Some(reader) = app.try_state::<ContextReader>() {
        reader.forget_target();
    }
}

fn read_context(app: &AppHandle) -> (Option<ContextSnapshot>, Option<u64>, AskOpen) {
    // Jendela Nuenly sendiri tidak pernah dibaca; dari sana Tanya Nuenly dibuka tanpa konteks.
    let own_window_focused = app
        .webview_windows()
        .values()
        .any(|window| window.is_focused().unwrap_or(false));
    if own_window_focused {
        return (None, None, AskOpen::default());
    }
    // Mode dipastikan milik aplikasi ini sebelum kebijakannya dipakai, tanpa menunggu event
    // latar depan yang mungkin belum tiba.
    foreground::sync(app);
    let Some(policy) = app.state::<AppStatus>().reading_policy() else {
        let open = AskOpen {
            context: None,
            notice: Some(Notice::Paused),
        };
        return (None, None, open);
    };
    let reader = app.state::<ContextReader>();
    let result = reader.for_ask(&policy);
    let target = matches!(result, Ok(CaptureOutcome::Captured(_)))
        .then(|| reader.replace_target())
        .flatten();
    let (context, mut open) = prepare(result, policy.context_preview);
    if let Some(view) = &mut open.context {
        view.replaceable = target.is_some();
    }
    (context, target, open)
}

/// Memisahkan hasil baca menjadi teks konteks (tetap di Rust) dan isi event untuk jendela,
/// dengan cuplikan teks hanya bila `show_preview`. `replaceable` diisi pemanggil.
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
                replaceable: false,
            };
            let open = AskOpen {
                context: Some(context),
                notice: None,
            };
            return (Some(snapshot), open);
        }
        // Tidak ada teks terpilih maupun isi field: bertanya tanpa konteks adalah hal biasa.
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
fn show(
    app: &AppHandle,
    context: Option<ContextSnapshot>,
    target: Option<u64>,
    open: &AskOpen,
) -> tauri::Result<()> {
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

    app.state::<AskState>().start(context, target);
    app.emit_to(LABEL, "ask:open", open)?;
    window.show()?;
    window.set_focus()
}

#[cfg(test)]
mod tests {
    use nuenly_core::{
        ActivePolicy, AppFilter, AppIdentity, CaptureKind, CaptureOutcome, ContextError,
        ContextSnapshot, ControlKind, FakeContextSource, FieldInfo, ModeId, ProcessName,
        SensitiveText, capture, capture_first,
    };
    use nuenly_store::ReplaceSource;

    use super::{AskState, ContextView, Notice, PREVIEW_CHARS, prepare, preview};
    use crate::context::ASK_KINDS;
    use crate::replace::ReplaceFailure;

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
            capture_first(source, policy, ASK_KINDS),
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
                replaceable: false,
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

    fn field(is_password: bool) -> FieldInfo {
        FieldInfo {
            label: None,
            control: ControlKind::Edit,
            is_password,
            bounds: None,
        }
    }

    #[test]
    fn without_a_selection_the_focused_field_is_the_context() {
        let mut umum = policy();
        umum.context_preview = true;
        let source = FakeContextSource::new()
            .with_foreground(app("notepad.exe", "Notepad"), "catatan.txt")
            .with_focused_field(field(false), "isi field sintetis");
        let (context, open) = prepare(capture_first(&source, &umum, ASK_KINDS), true);

        assert_eq!(context.unwrap().kind, CaptureKind::FocusedField);
        assert_eq!(
            open.context,
            Some(ContextView {
                source: "field",
                app_name: "Notepad".to_owned(),
                preview: Some("isi field sintetis".to_owned()),
                replaceable: false,
            })
        );
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

        // Tanpa teks terpilih, Nuenly beralih ke field fokus, dan field kata sandi tetap ditolak.
        let password = FakeContextSource::new()
            .with_foreground(app("notepad.exe", "Notepad"), "Masuk")
            .with_focused_field(field(true), "sandi-sintetis");
        let (context, open) = prepare(capture_first(&password, &policy(), ASK_KINDS), false);
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

    fn captured(source: &FakeContextSource) -> ContextSnapshot {
        match capture_first(source, &policy(), ASK_KINDS).unwrap() {
            CaptureOutcome::Captured(snapshot) => snapshot,
            other => panic!("seharusnya Captured, dapat {other:?}"),
        }
    }

    #[test]
    fn a_finished_answer_can_replace_the_text_once() {
        let state = AskState::default();
        state.start(Some(captured(&notepad())), Some(7));
        assert_eq!(state.replacement().err(), Some(ReplaceFailure::Unavailable));

        let (_stopped, id) = state.begin_answer();
        state.finish_answer(id, SensitiveText::new("\n  Rapat digeser ke Kamis.  \n"));
        let replacement = state.replacement().unwrap();
        assert_eq!(replacement.target, 7);
        assert_eq!(replacement.source, ReplaceSource::Selection);
        assert_eq!(
            replacement.expected.expose(),
            "rapat sintetis\n\ndigeser ke   kamis"
        );
        assert_eq!(replacement.text.expose(), "Rapat digeser ke Kamis.");
        // Satu jawaban, satu Ganti teks.
        assert_eq!(state.replacement().err(), Some(ReplaceFailure::Unavailable));
    }

    #[test]
    fn only_the_latest_answer_of_the_open_session_is_kept() {
        let state = AskState::default();
        state.start(Some(captured(&notepad())), Some(7));
        let (_first, old) = state.begin_answer();
        let (_second, latest) = state.begin_answer();
        state.finish_answer(old, SensitiveText::new("jawaban lama"));
        assert!(state.replacement().is_err());

        // Jawaban dari sesi sebelumnya tidak masuk ke sesi baru.
        state.start(Some(captured(&notepad())), Some(8));
        state.finish_answer(latest, SensitiveText::new("jawaban sesi lama"));
        assert!(state.replacement().is_err());
    }

    #[test]
    fn nothing_is_replaceable_without_a_target_or_after_removing_the_context() {
        let answered = |state: &AskState| {
            let (_stopped, id) = state.begin_answer();
            state.finish_answer(id, SensitiveText::new("Jawaban sintetis."));
        };

        let state = AskState::default();
        state.start(Some(captured(&notepad())), None);
        answered(&state);
        assert!(state.replacement().is_err());

        state.start(Some(captured(&notepad())), Some(3));
        assert!(state.context(false).is_none());
        answered(&state);
        assert!(state.replacement().is_err());

        state.start(Some(captured(&notepad())), Some(4));
        let (_stopped, id) = state.begin_answer();
        state.finish_answer(id, SensitiveText::new("   \n"));
        assert!(state.replacement().is_err());
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
