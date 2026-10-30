//! Sumber konteks palsu untuk test yang jalan tanpa Windows.

use std::sync::atomic::{AtomicUsize, Ordering};

use crate::app::{AppIdentity, ForegroundWindow, WindowHandle};
use crate::browser::{Browser, BrowserPage};
use crate::error::ContextError;
use crate::policy::ReadPermit;
use crate::snapshot::{CaptureKind, ContextSnapshot, FieldInfo};
use crate::source::ContextSource;
use crate::text::SensitiveText;

/// [`ContextSource`] yang isinya ditentukan oleh test. Pakai hanya data sintetis.
#[derive(Debug)]
pub struct FakeContextSource {
    window: Option<ForegroundWindow>,
    window_title: SensitiveText,
    page: Result<BrowserPage, ContextError>,
    selection: Option<SensitiveText>,
    focused: Option<(FieldInfo, SensitiveText)>,
    app_after_read: Option<AppIdentity>,
    page_reads: AtomicUsize,
    reads: AtomicUsize,
}

impl Default for FakeContextSource {
    fn default() -> Self {
        Self {
            window: None,
            window_title: SensitiveText::default(),
            page: Ok(BrowserPage::default()),
            selection: None,
            focused: None,
            app_after_read: None,
            page_reads: AtomicUsize::new(0),
            reads: AtomicUsize::new(0),
        }
    }
}

impl FakeContextSource {
    /// Sumber tanpa jendela di latar depan.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Menetapkan jendela di latar depan.
    #[must_use]
    pub fn with_foreground(mut self, app: AppIdentity, window_title: &str) -> Self {
        self.window = Some(ForegroundWindow {
            handle: WindowHandle(1),
            app,
        });
        self.window_title = SensitiveText::new(window_title);
        self
    }

    /// Menetapkan judul dan isi address bar bila jendelanya browser.
    #[must_use]
    pub fn with_browser_page(mut self, titles: &[&str], address: Option<&str>) -> Self {
        self.page = Ok(BrowserPage {
            titles: titles
                .iter()
                .map(|title| SensitiveText::new(*title))
                .collect(),
            address: address.map(SensitiveText::new),
        });
        self
    }

    /// Mensimulasikan halaman browser yang gagal diperiksa.
    #[must_use]
    pub fn with_browser_page_error(mut self, err: ContextError) -> Self {
        self.page = Err(err);
        self
    }

    /// Menetapkan teks yang sedang dipilih.
    #[must_use]
    pub fn with_selection(mut self, text: &str) -> Self {
        self.selection = Some(SensitiveText::new(text));
        self
    }

    /// Menetapkan field yang sedang fokus beserta isinya.
    #[must_use]
    pub fn with_focused_field(mut self, field: FieldInfo, text: &str) -> Self {
        self.focused = Some((field, SensitiveText::new(text)));
        self
    }

    /// Mensimulasikan aplikasi lain yang naik ke latar depan selagi jendela dibaca.
    #[must_use]
    pub fn with_switch_during_read(mut self, app: AppIdentity) -> Self {
        self.app_after_read = Some(app);
        self
    }

    /// Berapa kali isi jendela dibaca lewat [`ContextSource::read`].
    #[must_use]
    pub fn read_count(&self) -> usize {
        self.reads.load(Ordering::Relaxed)
    }

    /// Berapa kali halaman browser diperiksa lewat [`ContextSource::browser_page`].
    #[must_use]
    pub fn page_read_count(&self) -> usize {
        self.page_reads.load(Ordering::Relaxed)
    }
}

impl ContextSource for FakeContextSource {
    fn foreground_window(&self) -> Result<Option<ForegroundWindow>, ContextError> {
        Ok(self.window.clone())
    }

    fn browser_page(
        &self,
        _permit: &ReadPermit<'_>,
        _browser: Browser,
    ) -> Result<BrowserPage, ContextError> {
        self.page_reads.fetch_add(1, Ordering::Relaxed);
        self.page.clone()
    }

    fn read(
        &self,
        permit: &ReadPermit<'_>,
        kind: CaptureKind,
    ) -> Result<Option<ContextSnapshot>, ContextError> {
        self.reads.fetch_add(1, Ordering::Relaxed);

        let (field, text) = match kind {
            CaptureKind::SelectedText => match &self.selection {
                Some(text) => (None, text.clone()),
                None => return Ok(None),
            },
            CaptureKind::FocusedField => match &self.focused {
                Some((field, text)) => (Some(field.clone()), text.clone()),
                None => return Ok(None),
            },
        };
        let app = self
            .app_after_read
            .clone()
            .unwrap_or_else(|| permit.window().app.clone());

        Ok(Some(ContextSnapshot {
            app,
            window_title: self.window_title.clone(),
            kind,
            field,
            text,
        }))
    }
}
