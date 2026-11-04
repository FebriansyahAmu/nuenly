//! Sinyal aktivitas di kolom yang sedang diisi: fokus berpindah, isi berubah, seleksi atau caret
//! bergerak (spike Fase 0, dasar trigger F-09).
//!
//! Sinyal tidak pernah membawa teks: hanya sumber, jenis, proses asal, dan waktunya. Ada dua
//! sumber yang dibandingkan di spike:
//!
//! - [`WinEventActivity`]: `SetWinEventHook` di luar proses, murah, tanpa isi.
//! - [`UiaActivity`]: handler UI Automation pada elemen yang sedang fokus. Event perubahan nilai
//!   UIA membawa nilai barunya; nilai itu langsung dibuang tanpa dibaca.

use std::cell::RefCell;
use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::time::Instant;

use uiautomation::events::{
    CustomEventHandlerFn, CustomFocusChangedEventHandlerFn, CustomPropertyChangedEventHandlerFn,
    UIEventHandler, UIEventType, UIFocusChangedEventHandler, UIPropertyChangedEventHandler,
};
use uiautomation::types::{TreeScope, UIProperty};
use uiautomation::variants::Variant;
use uiautomation::{UIAutomation, UIElement};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows::Win32::UI::WindowsAndMessaging::{
    EVENT_OBJECT_FOCUS, EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_TEXTSELECTIONCHANGED,
    EVENT_OBJECT_VALUECHANGE, GetMessageW, MSG, OBJID_CARET, PostThreadMessageW,
    WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS, WM_QUIT,
};

use crate::window::window_process;

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

/// Pesan ke thread UIA. Handler UIA berjalan di thread milik UIA, jadi mereka hanya mengirim
/// pesan; semua panggilan UIA dan `on_activity` terjadi di thread ini.
enum Message {
    FocusChanged(Instant),
    Signal(Signal, u32, Instant),
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
    /// # Errors
    ///
    /// [`io::Error`] bila thread tidak bisa dibuat atau UIA menolak handler fokus.
    pub fn start(on_activity: impl Fn(Activity) + Send + 'static) -> io::Result<Self> {
        let (messages, inbox) = mpsc::channel();
        let (ready, started) = mpsc::sync_channel(1);
        let sender = messages.clone();
        let thread = thread::Builder::new()
            .name("nuenly-uia-activity".to_owned())
            .spawn(move || {
                let automation = match UIAutomation::new() {
                    Ok(automation) => automation,
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
                if let Err(err) = automation.add_focus_changed_event_handler(None, &focus) {
                    let _ = ready.send(Err(io::Error::other(err.to_string())));
                    return;
                }
                let _ = ready.send(Ok(()));
                run_uia(&automation, &sender, &inbox, &on_activity);
                let _ = automation.remove_all_event_handlers();
            })?;
        started
            .recv()
            .map_err(|_| io::Error::other("thread UIA berhenti"))??;
        Ok(Self {
            messages,
            thread: Some(thread),
        })
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

/// Handler yang terpasang pada elemen fokus saat ini.
struct Watched {
    element: UIElement,
    text: Option<UIEventHandler>,
    value: Option<UIPropertyChangedEventHandler>,
}

fn run_uia(
    automation: &UIAutomation,
    sender: &mpsc::Sender<Message>,
    inbox: &mpsc::Receiver<Message>,
    on_activity: &impl Fn(Activity),
) {
    let own = std::process::id();
    let mut watched: Option<Watched> = None;
    while let Ok(message) = inbox.recv() {
        match message {
            Message::Stop => break,
            Message::Signal(signal, pid, at) => on_activity(Activity {
                source: Source::Uia,
                signal,
                pid,
                at,
            }),
            Message::FocusChanged(at) => {
                if let Some(old) = watched.take() {
                    unwatch(automation, &old);
                }
                let Ok(element) = automation.get_focused_element() else {
                    continue;
                };
                let pid = element.get_process_id().unwrap_or(0);
                if pid == own {
                    continue;
                }
                on_activity(Activity {
                    source: Source::Uia,
                    signal: Signal::Focus,
                    pid,
                    at,
                });
                watched = Some(watch(automation, element, pid, sender));
            }
        }
    }
    if let Some(old) = watched.take() {
        unwatch(automation, &old);
    }
}

fn watch(
    automation: &UIAutomation,
    element: UIElement,
    pid: u32,
    sender: &mpsc::Sender<Message>,
) -> Watched {
    let text_sender = sender.clone();
    let text: Box<CustomEventHandlerFn> = Box::new(move |_, _| {
        let _ = text_sender.send(Message::Signal(Signal::TextChanged, pid, Instant::now()));
        Ok(())
    });
    let text = UIEventHandler::from(text);
    let text = automation
        .add_automation_event_handler(
            UIEventType::Text_TextChanged,
            &element,
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
            let _ = value_sender.send(Message::Signal(Signal::ValueChanged, pid, Instant::now()));
            Ok(())
        });
    let value = UIPropertyChangedEventHandler::from(value);
    let value = automation
        .add_property_changed_event_handler(
            &element,
            TreeScope::Element,
            None,
            &value,
            &[UIProperty::ValueValue],
        )
        .is_ok()
        .then_some(value);

    Watched {
        element,
        text,
        value,
    }
}

fn unwatch(automation: &UIAutomation, watched: &Watched) {
    if let Some(text) = &watched.text {
        let _ = automation.remove_automation_event_handler(
            UIEventType::Text_TextChanged,
            &watched.element,
            text,
        );
    }
    if let Some(value) = &watched.value {
        let _ = automation.remove_property_changed_event_handler(&watched.element, value);
    }
}

#[cfg(test)]
mod tests {
    use super::{UiaActivity, WinEventActivity};

    #[test]
    fn the_win_event_watch_starts_and_stops() {
        let watch = WinEventActivity::start(|_| {}).unwrap();
        drop(watch);
    }

    #[test]
    fn the_uia_watch_starts_and_stops() {
        let _guard = crate::source::tests::uia_turn();
        let watch = UiaActivity::start(|_| {}).unwrap();
        drop(watch);
    }
}
