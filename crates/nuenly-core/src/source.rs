//! Trait platform untuk membaca konteks, dan satu-satunya jalur baca yang dipakai aplikasi.

use crate::app::{AppIdentity, ForegroundWindow};
use crate::browser::{self, Browser, BrowserPage, SensitivePage};
use crate::error::ContextError;
use crate::policy::{ActivePolicy, ReadPermit};
use crate::snapshot::{CaptureKind, ContextSnapshot};

/// Sumber konteks dari OS: UI Automation di Windows, kelak AX di macOS dan AT-SPI di Linux.
///
/// Method di trait ini blocking. Dari kode async, panggil lewat `spawn_blocking` atau thread
/// worker; implementasi Windows meneruskannya ke thread COM khusus.
pub trait ContextSource: Send + Sync {
    /// Jendela di latar depan, atau `None` bila tidak ada (misalnya desktop atau layar kunci).
    /// Hanya membaca identitas proses, tidak membaca isi jendela.
    ///
    /// # Errors
    ///
    /// [`ContextError`] bila platform gagal menjawab.
    fn foreground_window(&self) -> Result<Option<ForegroundWindow>, ContextError>;

    /// Judul jendela dan isi address bar browser yang diizinkan `permit`, hanya untuk
    /// memeriksa jendela privat dan situs perbankan sebelum isinya dibaca.
    ///
    /// # Errors
    ///
    /// [`ContextError::ForegroundChanged`] bila jendela itu sudah tidak di latar depan, atau
    /// [`ContextError`] lain dari platform, termasuk bila address bar ada tetapi tidak bisa
    /// dibaca.
    fn browser_page(
        &self,
        permit: &ReadPermit<'_>,
        browser: Browser,
    ) -> Result<BrowserPage, ContextError>;

    /// Membaca konteks dari jendela yang diizinkan `permit`. `None` bila tidak ada yang bisa
    /// dibaca, misalnya tidak ada teks terpilih.
    ///
    /// # Errors
    ///
    /// [`ContextError::ForegroundChanged`] bila jendela itu sudah tidak di latar depan, atau
    /// [`ContextError`] lain dari platform.
    fn read(
        &self,
        permit: &ReadPermit<'_>,
        kind: CaptureKind,
    ) -> Result<Option<ContextSnapshot>, ContextError>;
}

/// Hasil [`capture`].
#[derive(Debug, PartialEq, Eq)]
pub enum CaptureOutcome {
    Captured(ContextSnapshot),
    /// Tidak ada jendela di latar depan.
    NoWindow,
    /// Filter aplikasi menolak aplikasi ini; isi jendelanya tidak dibaca.
    NotAllowed(AppIdentity),
    /// Jendela privat atau situs perbankan di browser; isi halamannya tidak dibaca.
    Sensitive(SensitivePage),
    /// Field kata sandi; hasil bacanya dibuang.
    ProtectedField,
    /// Tidak ada teks yang bisa dibaca.
    Empty,
}

/// Membaca konteks dari jendela di latar depan setelah memeriksa kebijakan.
///
/// Urutannya: ambil identitas jendela, periksa filter aplikasi, periksa halaman bila jendelanya
/// browser (jendela privat dan situs perbankan tidak pernah dibaca), baru baca isinya.
/// Hasil dari field kata sandi dibuang, dan hasil dari aplikasi lain (jendela berganti
/// saat dibaca) ditolak.
///
/// # Errors
///
/// Meneruskan [`ContextError`] dari platform, atau [`ContextError::ForegroundChanged`] bila
/// hasil baca berasal dari aplikasi selain yang diizinkan.
pub fn capture<S>(
    source: &S,
    policy: &ActivePolicy,
    kind: CaptureKind,
) -> Result<CaptureOutcome, ContextError>
where
    S: ContextSource + ?Sized,
{
    let Some(window) = source.foreground_window()? else {
        return Ok(CaptureOutcome::NoWindow);
    };
    let Some(permit) = policy.permit(&window) else {
        return Ok(CaptureOutcome::NotAllowed(window.app));
    };
    if let Some(browser) = Browser::from_process(&window.app.process) {
        let page = source.browser_page(&permit, browser)?;
        if let Some(reason) = browser::classify(&page) {
            return Ok(CaptureOutcome::Sensitive(reason));
        }
    }
    let Some(snapshot) = source.read(&permit, kind)? else {
        return Ok(CaptureOutcome::Empty);
    };

    if snapshot.app.process != permit.window().app.process {
        return Err(ContextError::ForegroundChanged);
    }
    if snapshot
        .field
        .as_ref()
        .is_some_and(|field| field.is_password)
    {
        return Ok(CaptureOutcome::ProtectedField);
    }
    if snapshot.text.is_empty() {
        return Ok(CaptureOutcome::Empty);
    }
    Ok(CaptureOutcome::Captured(snapshot))
}

#[cfg(test)]
mod tests {
    use super::{CaptureOutcome, capture};
    use crate::app::{AppIdentity, ProcessName};
    use crate::browser::SensitivePage;
    use crate::error::ContextError;
    use crate::fake::FakeContextSource;
    use crate::policy::{ActivePolicy, AppFilter, ModeId};
    use crate::snapshot::{CaptureKind, ControlKind, FieldInfo};

    fn app(process: &str, display_name: &str) -> AppIdentity {
        AppIdentity {
            process: ProcessName::new(process).unwrap(),
            display_name: display_name.to_owned(),
        }
    }

    fn policy() -> ActivePolicy {
        ActivePolicy::new(ModeId::new("all").unwrap())
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
    fn captures_selected_text() {
        let source = FakeContextSource::new()
            .with_foreground(app("outlook.exe", "Mail"), "Balas: Jadwal rapat")
            .with_selection("jadi gini, rapat minggu depan digeser ke kamis aja");

        let outcome = capture(&source, &policy(), CaptureKind::SelectedText).unwrap();

        let CaptureOutcome::Captured(snapshot) = outcome else {
            panic!("seharusnya Captured, dapat {outcome:?}");
        };
        assert_eq!(
            snapshot.text.expose(),
            "jadi gini, rapat minggu depan digeser ke kamis aja"
        );
        assert_eq!(snapshot.app.display_name, "Mail");
        assert_eq!(snapshot.kind, CaptureKind::SelectedText);
    }

    #[test]
    fn captures_focused_field() {
        let source = FakeContextSource::new()
            .with_foreground(app("notepad.exe", "Notepad"), "catatan.txt")
            .with_focused_field(field(false), "daftar belanja sintetis");

        let outcome = capture(&source, &policy(), CaptureKind::FocusedField).unwrap();

        assert!(matches!(outcome, CaptureOutcome::Captured(s) if s.field.is_some()));
    }

    #[test]
    fn blocked_app_is_never_read() {
        let mut policy = policy();
        policy.apps = AppFilter {
            allow: None,
            block: vec![ProcessName::new("outlook.exe").unwrap()],
        };
        let source = FakeContextSource::new()
            .with_foreground(app("outlook.exe", "Mail"), "Kotak masuk")
            .with_selection("teks sintetis");

        let outcome = capture(&source, &policy, CaptureKind::SelectedText).unwrap();

        assert_eq!(
            outcome,
            CaptureOutcome::NotAllowed(app("outlook.exe", "Mail"))
        );
        assert_eq!(source.read_count(), 0);
    }

    #[test]
    fn always_blocked_app_is_never_read() {
        let source = FakeContextSource::new()
            .with_foreground(app("KeePassXC.exe", "KeePassXC"), "Brankas")
            .with_focused_field(field(false), "teks sintetis");

        let outcome = capture(&source, &policy(), CaptureKind::FocusedField).unwrap();

        assert!(matches!(outcome, CaptureOutcome::NotAllowed(_)));
        assert_eq!(source.read_count(), 0);
    }

    #[test]
    fn no_foreground_window_reads_nothing() {
        let source = FakeContextSource::new().with_selection("teks sintetis");

        let outcome = capture(&source, &policy(), CaptureKind::SelectedText).unwrap();

        assert_eq!(outcome, CaptureOutcome::NoWindow);
        assert_eq!(source.read_count(), 0);
    }

    #[test]
    fn password_field_is_discarded() {
        let source = FakeContextSource::new()
            .with_foreground(app("chrome.exe", "Chrome"), "Masuk")
            .with_focused_field(field(true), "sandi-sintetis-123");

        let outcome = capture(&source, &policy(), CaptureKind::FocusedField).unwrap();

        assert_eq!(outcome, CaptureOutcome::ProtectedField);
    }

    #[test]
    fn switch_during_read_is_rejected() {
        let source = FakeContextSource::new()
            .with_foreground(app("outlook.exe", "Mail"), "Kotak masuk")
            .with_selection("teks sintetis")
            .with_switch_during_read(app("medapp.exe", "MedApp"));

        let result = capture(&source, &policy(), CaptureKind::SelectedText);

        assert_eq!(result, Err(ContextError::ForegroundChanged));
    }

    #[test]
    fn private_window_and_bank_site_are_never_read() {
        let private = FakeContextSource::new()
            .with_foreground(app("chrome.exe", "Google Chrome"), "")
            .with_browser_page(&["Tab Baru - Google Chrome (Incognito)"], Some("contoh.id"))
            .with_selection("teks sintetis");
        let bank = FakeContextSource::new()
            .with_foreground(app("msedge.exe", "Microsoft Edge"), "")
            .with_browser_page(&["Masuk"], Some("https://ibank.klikbca.com/"))
            .with_selection("teks sintetis");

        for (source, reason) in [
            (private, SensitivePage::PrivateWindow),
            (bank, SensitivePage::Banking),
        ] {
            let outcome = capture(&source, &policy(), CaptureKind::SelectedText).unwrap();
            assert_eq!(outcome, CaptureOutcome::Sensitive(reason));
            assert_eq!(source.read_count(), 0);
        }
    }

    #[test]
    fn ordinary_browser_page_is_read_after_the_check() {
        let source = FakeContextSource::new()
            .with_foreground(app("firefox.exe", "Firefox"), "")
            .with_browser_page(
                &["Kotak masuk sintetis — Mozilla Firefox"],
                Some("mail.contoh.id"),
            )
            .with_selection("teks sintetis");

        let outcome = capture(&source, &policy(), CaptureKind::SelectedText).unwrap();

        assert!(matches!(outcome, CaptureOutcome::Captured(_)));
        assert_eq!(source.page_read_count(), 1);
    }

    #[test]
    fn unreadable_browser_page_stops_the_read() {
        let source = FakeContextSource::new()
            .with_foreground(app("chrome.exe", "Google Chrome"), "")
            .with_browser_page_error(ContextError::Timeout)
            .with_selection("teks sintetis");

        let result = capture(&source, &policy(), CaptureKind::SelectedText);

        assert_eq!(result, Err(ContextError::Timeout));
        assert_eq!(source.read_count(), 0);
    }

    #[test]
    fn only_allowed_browsers_are_checked() {
        let notepad = FakeContextSource::new()
            .with_foreground(app("notepad.exe", "Notepad"), "")
            .with_selection("teks sintetis");
        let _ = capture(&notepad, &policy(), CaptureKind::SelectedText).unwrap();
        assert_eq!(notepad.page_read_count(), 0);

        let mut blocking = policy();
        blocking.apps = AppFilter {
            allow: None,
            block: vec![ProcessName::new("chrome.exe").unwrap()],
        };
        let chrome = FakeContextSource::new()
            .with_foreground(app("chrome.exe", "Google Chrome"), "")
            .with_selection("teks sintetis");
        let outcome = capture(&chrome, &blocking, CaptureKind::SelectedText).unwrap();
        assert!(matches!(outcome, CaptureOutcome::NotAllowed(_)));
        assert_eq!(chrome.page_read_count(), 0);
    }

    #[test]
    fn missing_or_blank_text_is_empty() {
        let nothing = FakeContextSource::new().with_foreground(app("notepad.exe", "Notepad"), "");
        let blank = FakeContextSource::new()
            .with_foreground(app("notepad.exe", "Notepad"), "")
            .with_selection("");

        for source in [nothing, blank] {
            let outcome = capture(&source, &policy(), CaptureKind::SelectedText).unwrap();
            assert_eq!(outcome, CaptureOutcome::Empty);
        }
    }
}
