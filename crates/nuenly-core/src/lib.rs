//! Tipe inti Nuenly yang bebas OS: `ContextSnapshot`, trait platform, dan `ActivePolicy`.
//!
//! Crate ini tidak memanggil API OS apa pun. Implementasi platform ada di crate
//! `nuenly-platform-*`; test memakai `FakeContextSource` lewat feature `fake`.
//!
//! Aplikasi membaca konteks hanya lewat [`capture`], yang memeriksa [`ActivePolicy`]
//! sebelum isi jendela dibaca.

#![forbid(unsafe_code)]

mod app;
mod browser;
mod error;
#[cfg(any(test, feature = "fake"))]
mod fake;
mod policy;
mod shortcut;
mod snapshot;
mod source;
mod text;

pub use app::{AppIdentity, ForegroundWindow, ProcessName, WindowHandle};
pub use browser::{Browser, BrowserPage, SensitivePage, classify};
pub use error::{ContextError, InvalidName};
#[cfg(any(test, feature = "fake"))]
pub use fake::FakeContextSource;
pub use policy::{
    ALWAYS_BLOCKED, ActivePolicy, AppFilter, Egress, ModeId, ProactivePolicy, ReadPermit,
    RedactionLevel,
};
pub use shortcut::{Key, Shortcut};
pub use snapshot::{CaptureKind, ContextSnapshot, ControlKind, FieldInfo, Rect};
pub use source::{CaptureOutcome, ContextSource, capture};
pub use text::SensitiveText;
