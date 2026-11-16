//! Sinyal aktivitas di kolom yang sedang diisi: fokus berpindah, isi berubah, seleksi atau caret
//! bergerak (F-09, ADR 0012).
//!
//! Sinyal tidak pernah membawa teks: hanya sumber, jenis, proses asal, dan waktunya. Ada dua
//! sumber yang dibandingkan di spike:
//!
//! - [`WinEventActivity`]: `SetWinEventHook` di luar proses, murah, tanpa isi. Hanya untuk
//!   `uia-dump`.
//! - [`UiaActivity`]: handler UI Automation pada elemen yang sedang fokus. Event perubahan nilai
//!   UIA membawa nilai barunya; nilai itu langsung dibuang tanpa dibaca. Ini sumber sinyal saran
//!   saat mengetik di aplikasi, dan lewat [`UiaActivity::reader`] juga pembaca kolom saat ada
//!   pemicu.

use std::cell::RefCell;
use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use nuenly_core::{
    Browser, BrowserPage, CaptureKind, ContextError, ContextSnapshot, ContextSource,
    ForegroundWindow, ReadPermit,
};
use uiautomation::UIElement;
use uiautomation::events::{
    CustomEventHandlerFn, CustomFocusChangedEventHandlerFn, CustomPropertyChangedEventHandlerFn,
    UIEventHandler, UIEventType, UIFocusChangedEventHandler, UIPropertyChangedEventHandler,
};
use uiautomation::types::{TreeScope, UIProperty};
use uiautomation::variants::Variant;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows::Win32::UI::WindowsAndMessaging::{
    EVENT_OBJECT_FOCUS, EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_TEXTSELECTIONCHANGED,
    EVENT_OBJECT_VALUECHANGE, GetMessageW, MSG, OBJID_CARET, PostThreadMessageW,
    WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS, WM_QUIT,
};

use crate::source::{Job, run_until};
use crate::uia::{self, Uia};
use crate::window::{self, window_process};

/// Asal sinyal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Source {
    WinEvent,
    Uia,
}

/// Jenis sinyal, tanpa isi.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Signal {
    /// Fokus keyboard pindah ke elemen lain.
    Focus,
    /// Nilai elemen berubah (`EVENT_OBJECT_VALUECHANGE` atau properti UIA `Value`).
    ValueChanged,
    /// Teks elemen berubah (event UIA `Text_TextChanged`).
    TextChanged,
    /// Seleksi teks atau posisi caret berubah di dalam kolom.
    SelectionChanged,
    /// Caret sistem berpindah.
    CaretMoved,
}

/// Satu sinyal aktivitas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Activity {
    pub source: Source,
    pub signal: Signal,
    /// Proses pemilik elemen; 0 bila tidak diketahui.
    pub pid: u32,
    pub at: Instant,
}

/// Event `WinEvent` yang dipasang, masing-masing satu hook.
const WIN_EVENTS: [u32; 4] = [
    EVENT_OBJECT_FOCUS,
    EVENT_OBJECT_LOCATIONCHANGE,
    EVENT_OBJECT_VALUECHANGE,
    EVENT_OBJECT_TEXTSELECTIONCHANGED,
];

struct Receiver {
    on_activity: Box<dyn Fn(Activity)>,
    ignored: Arc<AtomicU64>,
}

thread_local! {
    /// Penerima di thread hook. Callback WinEvent tidak membawa data milik pemasangnya.
    static RECEIVER: RefCell<Option<Receiver>> = const { RefCell::new(None) };
}

/// Sinyal aktivitas dari `WinEvent`, untuk semua proses selain Nuenly sendiri. Berhenti saat
/// di-drop.
#[derive(Debug)]
pub struct WinEventActivity {
    thread_id: u32,
    ignored: Arc<AtomicU64>,
}

impl WinEventActivity {
    /// Memanggil `on_activity` di thread hook untuk setiap sinyal.
    ///
    /// # Errors
    ///
    /// [`io::Error`] bila thread tidak bisa dibuat atau Windows menolak memasang hook.
    pub fn start(on_activity: impl Fn(Activity) + Send + 'static) -> io::Result<Self> {
        let ignored = Arc::new(AtomicU64::new(0));
        let counter = Arc::clone(&ignored);
        let (ready, started) = mpsc::sync_channel(1);
        thread::Builder::new()
            .name("nuenly-activity".to_owned())
            .spawn(move || {
                // SAFETY: GetCurrentThreadId tidak punya prasyarat.
                let thread_id = unsafe { GetCurrentThreadId() };
                RECEIVER.with_borrow_mut(|slot| {
                    *slot = Some(Receiver {
                        on_activity: Box::new(on_activity),
                        ignored: counter,
                    });
                });
                let mut hooks = Vec::with_capacity(WIN_EVENTS.len());
                for event in WIN_EVENTS {
                    // SAFETY: `on_win_event` cocok dengan WINEVENTPROC dan hidup selama program.
                    // Dengan WINEVENT_OUTOFCONTEXT tanpa modul, Windows memanggilnya di thread ini
                    // saat antreannya dipompa `listen`.
                    let hook = unsafe {
                        SetWinEventHook(
                            event,
                            event,
                            None,
                            Some(on_win_event),
                            0,
                            0,
                            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
                        )
                    };
                    if hook.is_invalid() {
                        unhook(&hooks);
                        let _ =
                            ready.send(Err(io::Error::other("hook aktivitas tidak bisa dipasang")));
                        return;
                    }
                    hooks.push(hook);
                }
                let _ = ready.send(Ok(thread_id));
                listen();
                unhook(&hooks);
            })?;
        let thread_id = started
            .recv()
            .map_err(|_| io::Error::other("thread aktivitas berhenti"))??;
        Ok(Self { thread_id, ignored })
    }

    /// Jumlah `LOCATIONCHANGE` yang bukan caret (kebanyakan gerakan kursor mouse) dan dibuang.
    /// Ini biaya memasang hook caret.
    #[must_use]
    pub fn ignored(&self) -> u64 {
        self.ignored.load(Ordering::Relaxed)
    }
}

impl Drop for WinEventActivity {
    fn drop(&mut self) {
        // SAFETY: tanpa prasyarat; bila thread-nya sudah berhenti, panggilan ini hanya gagal.
        let _ = unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) };
    }
}

/// Callback semua hook aktivitas, di thread hook.
unsafe extern "system" fn on_win_event(
    _hook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    object: i32,
    _child: i32,
    _thread: u32,
    _time: u32,
) {
    let at = Instant::now();
    RECEIVER.with_borrow(|slot| {
        let Some(receiver) = slot else {
            return;
        };
        let signal = match event {
            EVENT_OBJECT_FOCUS => Signal::Focus,
            EVENT_OBJECT_VALUECHANGE => Signal::ValueChanged,
            EVENT_OBJECT_TEXTSELECTIONCHANGED => Signal::SelectionChanged,
            EVENT_OBJECT_LOCATIONCHANGE if object == OBJID_CARET.0 => Signal::CaretMoved,
            _ => {
                receiver.ignored.fetch_add(1, Ordering::Relaxed);
                return;
            }
        };
        (receiver.on_activity)(Activity {
            source: Source::WinEvent,
            signal,
            pid: window_process(hwnd).unwrap_or(0),
            at,
        });
    });
}

fn unhook(hooks: &[HWINEVENTHOOK]) {
    for hook in hooks {
        // SAFETY: setiap hook dipasang di thread ini dan hanya dilepas sekali di sini.
        let _ = unsafe { UnhookWinEvent(*hook) };
    }
}

/// Memompa antrean pesan thread hook sampai `WM_QUIT`; callback dipanggil di dalam `GetMessageW`.
fn listen() {
    let mut message = MSG::default();
    // GetMessageW mengembalikan 0 untuk WM_QUIT dan -1 bila gagal; keduanya mengakhiri loop.
    // SAFETY: `message` adalah MSG yang bisa ditulis dan hidup selama setiap panggilan.
    while unsafe { GetMessageW(&raw mut message, None, 0, 0) }.0 > 0 {}
}

/// Pesan ke thread UIA aktivitas. Handler UIA berjalan di thread milik UIA, jadi mereka hanya
/// mengirim pesan; semua panggilan UIA, `on_activity`, dan pembacaan terjadi di thread ini.
enum Message {
    FocusChanged(Instant),
    /// Sinyal dari handler elemen pemantauan ke-`watch`.
    Signal {
        signal: Signal,
        pid: u32,
        watch: u64,
        at: Instant,
    },
    /// Pembacaan untuk [`ActivityReader`].
    Job(Job<Session>),
    Stop,
}

/// Sinyal aktivitas dari UI Automation: fokus (global), lalu perubahan nilai dan teks pada
/// elemen yang sedang fokus saja. Berhenti saat di-drop.
pub struct UiaActivity {
    messages: mpsc::Sender<Message>,
    thread: Option<JoinHandle<()>>,
}

impl UiaActivity {
    /// Memanggil `on_activity` di thread UIA untuk setiap sinyal.
    ///
    /// Untuk sinyal fokus, hasil `on_activity` menentukan apakah perubahan isi elemen itu
    /// dipantau; `false` misalnya untuk aplikasi yang ditolak filter. Hasil untuk sinyal lain
    /// diabaikan. Elemen milik Nuenly sendiri, field kata sandi, dan field baca-saja tidak
    /// pernah dipantau, dan fokus ke jendela Nuenly tidak dilaporkan.
    ///
    /// # Errors
    ///
    /// [`io::Error`] bila thread tidak bisa dibuat atau UIA menolak handler fokus.
    pub fn start(on_activity: impl Fn(Activity) -> bool + Send + 'static) -> io::Result<Self> {
        let (messages, inbox) = mpsc::channel();
        let (ready, started) = mpsc::sync_channel(1);
        let sender = messages.clone();
        let thread = thread::Builder::new()
            .name("nuenly-uia-activity".to_owned())
            .spawn(move || {
                let uia = match Uia::new() {
                    Ok(uia) => uia,
                    Err(err) => {
                        let _ = ready.send(Err(io::Error::other(err.to_string())));
                        return;
                    }
                };
                let focus_sender = sender.clone();
                let focus: Box<CustomFocusChangedEventHandlerFn> = Box::new(move |_| {
                    let _ = focus_sender.send(Message::FocusChanged(Instant::now()));
                    Ok(())
                });
                let focus = UIFocusChangedEventHandler::from(focus);
                if let Err(err) = uia
                    .automation()
                    .add_focus_changed_event_handler(None, &focus)
                {
                    let _ = ready.send(Err(io::Error::other(err.to_string())));
                    return;
                }
                let _ = ready.send(Ok(()));
                let mut session = Session {
                    uia,
                    own: std::process::id(),
                    watched: None,
                    left: None,
                    watches: 0,
                };
                session.run(&sender, &inbox, &on_activity);
                let _ = session.uia.automation().remove_all_event_handlers();
            })?;
        started
            .recv()
            .map_err(|_| io::Error::other("thread UIA berhenti"))??;
        Ok(Self {
            messages,
            thread: Some(thread),
        })
    }

    /// Sumber konteks untuk memeriksa pemicu dari proses `pid`, dengan batas tunggu `timeout`
    /// per bacaan. Pembacaannya tetap lewat `nuenly_core::capture`, jadi filter aplikasi dan
    /// halaman browser diperiksa lebih dulu.
    ///
    /// Bacaan yang didukung: [`CaptureKind::FocusedField`] (elemen fokus saat ini) dan
    /// [`CaptureKind::LeftField`] (field yang terakhir diisi lalu ditinggalkan, sekali pakai).
    /// Bila jendela di latar depan bukan milik `pid`, tidak ada yang dibaca.
    #[must_use]
    pub fn reader(&self, pid: u32, timeout: Duration) -> ActivityReader {
        ActivityReader {
            messages: self.messages.clone(),
            pid,
            timeout,
        }
    }
}

impl Drop for UiaActivity {
    fn drop(&mut self) {
        let _ = self.messages.send(Message::Stop);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// [`ContextSource`] dari [`UiaActivity::reader`].
pub struct ActivityReader {
    messages: mpsc::Sender<Message>,
    pid: u32,
    timeout: Duration,
}

impl ActivityReader {
    fn run<T: Send + 'static>(
        &self,
        job: impl FnOnce(&mut Session) -> Result<T, ContextError> + Send + 'static,
    ) -> Result<T, ContextError> {
        run_until(
            |work| self.messages.send(Message::Job(work)).is_ok(),
            self.timeout,
            job,
        )
    }
}

impl ContextSource for ActivityReader {
    fn foreground_window(&self) -> Result<Option<ForegroundWindow>, ContextError> {
        // Pemicu dari aplikasi yang sudah ditinggalkan tidak membaca aplikasi yang kini di depan.
        let hwnd = window::foreground();
        if window_process(hwnd) != Some(self.pid) {
            return Ok(None);
        }
        window::window_info(hwnd)
    }

    fn browser_page(
        &self,
        permit: &ReadPermit<'_>,
        browser: Browser,
    ) -> Result<BrowserPage, ContextError> {
        let window = permit.window().clone();
        self.run(move |session| session.uia.browser_page(&window, browser))
    }

    fn read(
        &self,
        permit: &ReadPermit<'_>,
        kind: CaptureKind,
    ) -> Result<Option<ContextSnapshot>, ContextError> {
        let window = permit.window().clone();
        match kind {
            CaptureKind::FocusedField => self.run(move |session| {
                let element = session.uia.focused_in(&window)?;
                if uia::read_only(&element) {
                    return Ok(None);
                }
                Ok(uia::field_snapshot(&window, &element, kind))
            }),
            CaptureKind::LeftField => self.run(move |session| {
                let Some(element) = session.left.take() else {
                    return Ok(None);
                };
                uia::read_left(&window, &element)
            }),
            _ => Err(ContextError::Unsupported),
        }
    }
}

/// State thread UIA aktivitas.
struct Session {
    uia: Uia,
    /// Proses Nuenly sendiri.
    own: u32,
    /// Elemen yang sedang fokus.
    watched: Option<Watched>,
    /// Field terakhir yang diisi lalu ditinggalkan, untuk pemicu blur. Dibaca sekali.
    left: Option<UIElement>,
    /// Jumlah pemantauan sejauh ini, untuk mengenali sinyal dari handler yang sudah dilepas.
    watches: u64,
}

/// Elemen yang sedang fokus dan handler yang terpasang padanya.
struct Watched {
    element: UIElement,
    id: u64,
    /// `None` bila perubahan isinya tidak dipantau.
    handlers: Option<Handlers>,
    /// Isinya berubah sejak menerima fokus.
    edited: bool,
}

struct Handlers {
    text: Option<UIEventHandler>,
    value: Option<UIPropertyChangedEventHandler>,
}

impl Session {
    fn run(
        &mut self,
        sender: &mpsc::Sender<Message>,
        inbox: &mpsc::Receiver<Message>,
        on_activity: &impl Fn(Activity) -> bool,
    ) {
        while let Ok(message) = inbox.recv() {
            match message {
                Message::Stop => break,
                Message::Job(job) => job(self),
                Message::Signal {
                    signal,
                    pid,
                    watch,
                    at,
                } => {
                    // Sinyal yang masih antre dari elemen sebelumnya dibuang.
                    let Some(watched) = self.watched.as_mut().filter(|watched| watched.id == watch)
                    else {
                        continue;
                    };
                    watched.edited = true;
                    on_activity(Activity {
                        source: Source::Uia,
                        signal,
                        pid,
                        at,
                    });
                }
                Message::FocusChanged(at) => self.focus_changed(at, sender, on_activity),
            }
        }
        self.leave();
    }

    fn focus_changed(
        &mut self,
        at: Instant,
        sender: &mpsc::Sender<Message>,
        on_activity: &impl Fn(Activity) -> bool,
    ) {
        let Ok(element) = self.uia.automation().get_focused_element() else {
            return;
        };
        let pid = element.get_process_id().unwrap_or(0);
        // Jendela Nuenly sendiri tidak dilaporkan, dan pemantauan sebelumnya tetap berjalan.
        if pid == self.own {
            return;
        }
        // Fokus kembali ke elemen yang sama, misalnya setelah Tanya Nuenly tertutup, bukan
        // perpindahan kolom.
        let same = self.watched.as_ref().is_some_and(|watched| {
            self.uia
                .automation()
                .compare_elements(&watched.element, &element)
                .unwrap_or(false)
        });
        if same {
            return;
        }
        self.leave();
        let watch = on_activity(Activity {
            source: Source::Uia,
            signal: Signal::Focus,
            pid,
            at,
        });
        self.watches += 1;
        let id = self.watches;
        let handlers =
            (watch && uia::typeable(&element)).then(|| self.watch(&element, pid, id, sender));
        self.watched = Some(Watched {
            element,
            id,
            handlers,
            edited: false,
        });
    }

    /// Melepas handler elemen fokus. Bila isinya sempat berubah, elemen itu diingat untuk
    /// pemicu blur.
    fn leave(&mut self) {
        let Some(watched) = self.watched.take() else {
            return;
        };
        if let Some(handlers) = &watched.handlers {
            self.unwatch(&watched.element, handlers);
        }
        if watched.edited {
            self.left = Some(watched.element);
        }
    }

    fn watch(
        &self,
        element: &UIElement,
        pid: u32,
        watch: u64,
        sender: &mpsc::Sender<Message>,
    ) -> Handlers {
        let automation = self.uia.automation();
        let text_sender = sender.clone();
        let text: Box<CustomEventHandlerFn> = Box::new(move |_, _| {
            let _ = text_sender.send(Message::Signal {
                signal: Signal::TextChanged,
                pid,
                watch,
                at: Instant::now(),
            });
            Ok(())
        });
        let text = UIEventHandler::from(text);
        let text = automation
            .add_automation_event_handler(
                UIEventType::Text_TextChanged,
                element,
                TreeScope::Element,
                None,
                &text,
            )
            .is_ok()
            .then_some(text);

        let value_sender = sender.clone();
        let value: Box<CustomPropertyChangedEventHandlerFn> =
            Box::new(move |_, _, new_value: Variant| {
                // Nilai baru bisa memuat isi kolom; dibuang tanpa dibaca.
                drop(new_value);
                let _ = value_sender.send(Message::Signal {
                    signal: Signal::ValueChanged,
                    pid,
                    watch,
                    at: Instant::now(),
                });
                Ok(())
            });
        let value = UIPropertyChangedEventHandler::from(value);
        let value = automation
            .add_property_changed_event_handler(
                element,
                TreeScope::Element,
                None,
                &value,
                &[UIProperty::ValueValue],
            )
            .is_ok()
            .then_some(value);

        Handlers { text, value }
    }

    fn unwatch(&self, element: &UIElement, handlers: &Handlers) {
        let automation = self.uia.automation();
        if let Some(text) = &handlers.text {
            let _ = automation.remove_automation_event_handler(
                UIEventType::Text_TextChanged,
                element,
                text,
            );
        }
        if let Some(value) = &handlers.value {
            let _ = automation.remove_property_changed_event_handler(element, value);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use nuenly_core::{ActivePolicy, CaptureKind, CaptureOutcome, ModeId, capture};

    use super::{UiaActivity, WinEventActivity};

    #[test]
    fn the_win_event_watch_starts_and_stops() {
        let watch = WinEventActivity::start(|_| {}).unwrap();
        drop(watch);
    }

    #[test]
    fn the_uia_watch_starts_and_stops() {
        let _guard = crate::source::tests::uia_turn();
        let watch = UiaActivity::start(|_| true).unwrap();
        drop(watch);
    }

    #[test]
    fn a_trigger_from_another_process_reads_nothing() {
        let _guard = crate::source::tests::uia_turn();
        let watch = UiaActivity::start(|_| true).unwrap();
        // Tidak ada jendela milik proses 0, jadi apa pun yang di depan tidak dibaca.
        let reader = watch.reader(0, Duration::from_secs(1));
        let policy = ActivePolicy::new(ModeId::new("all").unwrap());
        for kind in [CaptureKind::FocusedField, CaptureKind::LeftField] {
            assert_eq!(
                capture(&reader, &policy, kind),
                Ok(CaptureOutcome::NoWindow)
            );
        }
    }

    #[test]
    fn readers_fail_after_the_watch_stops() {
        let _guard = crate::source::tests::uia_turn();
        let watch = UiaActivity::start(|_| true).unwrap();
        let reader = watch.reader(std::process::id(), Duration::from_secs(1));
        drop(watch);
        let result = reader.run(|_| Ok(()));
        assert_eq!(result, Err(crate::source::THREAD_ERROR));
    }
}
