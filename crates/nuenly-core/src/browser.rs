//! Halaman browser yang tidak pernah dibaca, apa pun modenya: jendela privat dan situs perbankan
//! (F-01). Platform hanya melaporkan judul jendela dan isi address bar; keputusannya di sini,
//! sebelum isi halaman dibaca.

use zeroize::Zeroizing;

use crate::app::ProcessName;
use crate::text::SensitiveText;

const CHROMIUM: &[&str] = &[
    "chrome.exe",
    "msedge.exe",
    "brave.exe",
    "opera.exe",
    "vivaldi.exe",
    "chromium.exe",
];
const FIREFOX: &[&str] = &["firefox.exe", "librewolf.exe", "waterfox.exe", "floorp.exe"];

/// Penanda jendela privat di judul jendela, huruf kecil. Chrome menambahkan `(Incognito)` ke
/// nama aksesibilitas jendela, Edge memakai `InPrivate`, Firefox `Private Browsing`; versi
/// bahasa Indonesia ikut didaftar.
const PRIVATE_TITLE: &[&str] = &[
    "inprivate",
    "(incognito)",
    "(samaran)",
    "(penyamaran)",
    "(private)",
    "(pribadi)",
    "private browsing",
    "penjelajahan pribadi",
    "jelajah pribadi",
];

/// Domain bank yang namanya tidak memuat kata "bank". Subdomainnya ikut, misalnya
/// `ibank.klikbca.com`.
const BANK_DOMAINS: &[&str] = &[
    "klikbca.com",
    "bca.co.id",
    "blubybcadigital.id",
    "bni.co.id",
    "bri.co.id",
    "btn.co.id",
    "cimbniaga.co.id",
    "octoclicks.co.id",
    "danamon.co.id",
    "permatanet.com",
    "ocbc.id",
    "ocbcnisp.com",
    "panin.co.id",
    "maybank.co.id",
    "jenius.com",
    "btpn.com",
    "jago.com",
    "uob.co.id",
    "hsbc.co.id",
    "dbs.id",
    "chase.com",
    "wellsfargo.com",
    "citi.com",
    "hsbc.com",
    "barclays.co.uk",
];

/// Penanda halaman perbankan di judul, untuk jendela tanpa address bar dan sebagai lapisan
/// kedua. Hanya frasa yang khas, agar artikel tentang perbankan tetap bisa dibaca.
const BANK_TITLE: &[&str] = &[
    "internet banking",
    "online banking",
    "mobile banking",
    "net banking",
    "netbanking",
    "e-banking",
    "ebanking",
    "i-banking",
    "klikbca",
    "brimo",
    "livin' by mandiri",
    "wondr by bni",
    "octo clicks",
    "permatanet",
];

/// Keluarga browser; menentukan cara platform menemukan address bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Browser {
    /// Chrome, Edge, Brave, Opera, Vivaldi, dan browser lain berbasis Chromium.
    Chromium,
    /// Firefox dan turunannya.
    Firefox,
}

impl Browser {
    /// Browser dari nama prosesnya, atau `None` untuk aplikasi lain.
    #[must_use]
    pub fn from_process(process: &ProcessName) -> Option<Self> {
        let name = process.as_str();
        if CHROMIUM.contains(&name) {
            Some(Self::Chromium)
        } else if FIREFOX.contains(&name) {
            Some(Self::Firefox)
        } else {
            None
        }
    }
}

/// Yang dibaca platform dari jendela browser, hanya untuk [`classify`]. Tidak disimpan.
#[derive(Clone, Debug, Default)]
pub struct BrowserPage {
    /// Judul jendela dari setiap sumber yang ada, misalnya judul native dan nama aksesibilitas.
    pub titles: Vec<SensitiveText>,
    /// Isi address bar. `None` bila jendela tidak punya address bar, misalnya aplikasi web
    /// terpasang atau mode layar penuh; yang diperiksa tinggal judulnya.
    pub address: Option<SensitiveText>,
}

/// Kenapa isi halaman tidak dibaca.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SensitivePage {
    PrivateWindow,
    Banking,
}

/// Memeriksa apakah halaman ini termasuk yang tidak pernah dibaca.
#[must_use]
pub fn classify(page: &BrowserPage) -> Option<SensitivePage> {
    let titles: Vec<Zeroizing<String>> = page
        .titles
        .iter()
        .map(|title| Zeroizing::new(title.expose().to_lowercase()))
        .collect();
    if titles
        .iter()
        .any(|title| contains_any(title, PRIVATE_TITLE))
    {
        return Some(SensitivePage::PrivateWindow);
    }
    let bank_address = page
        .address
        .as_ref()
        .and_then(|address| host(address.expose()))
        .is_some_and(|host| bank_host(&host));
    if bank_address || titles.iter().any(|title| contains_any(title, BANK_TITLE)) {
        return Some(SensitivePage::Banking);
    }
    None
}

fn contains_any(text: &str, markers: &[&str]) -> bool {
    markers.iter().any(|marker| text.contains(marker))
}

/// Nama host dari isi address bar, huruf kecil. `None` bila isinya bukan alamat, misalnya kata
/// pencarian yang sedang diketik.
fn host(address: &str) -> Option<Zeroizing<String>> {
    let address = address.trim();
    let rest = address.split_once("://").map_or(address, |(_, rest)| rest);
    let authority = rest.split(['/', '?', '#']).next()?;
    let host_and_port = authority.rsplit('@').next()?;
    let host = host_and_port.split(':').next()?.trim_end_matches('.');
    let looks_like_host =
        host.contains('.') && !host.starts_with('.') && !host.contains(char::is_whitespace);
    looks_like_host.then(|| Zeroizing::new(host.to_lowercase()))
}

/// Host bank: ada label yang memuat "bank" (misalnya `bankmandiri.co.id`, `ibank.bni.co.id`),
/// atau termasuk [`BANK_DOMAINS`].
fn bank_host(host: &str) -> bool {
    host.split('.').any(|label| label.contains("bank"))
        || BANK_DOMAINS.iter().any(|domain| {
            host.strip_suffix(domain)
                .is_some_and(|rest| rest.is_empty() || rest.ends_with('.'))
        })
}

#[cfg(test)]
mod tests {
    use super::{Browser, BrowserPage, SensitivePage, classify, host};
    use crate::app::ProcessName;
    use crate::text::SensitiveText;

    fn page(titles: &[&str], address: Option<&str>) -> BrowserPage {
        BrowserPage {
            titles: titles
                .iter()
                .map(|title| SensitiveText::new(*title))
                .collect(),
            address: address.map(SensitiveText::new),
        }
    }

    #[test]
    fn recognizes_browsers_by_process() {
        let browser = |name: &str| Browser::from_process(&ProcessName::new(name).unwrap());
        assert_eq!(browser("MSEDGE.EXE"), Some(Browser::Chromium));
        assert_eq!(browser("chrome.exe"), Some(Browser::Chromium));
        assert_eq!(browser("firefox.exe"), Some(Browser::Firefox));
        assert_eq!(browser("notepad.exe"), None);
    }

    #[test]
    fn private_windows_are_recognized_from_the_title() {
        for title in [
            "Tab Baru - Google Chrome (Incognito)",
            "Tab Baru - Google Chrome (Samaran)",
            "Halaman sintetis - [InPrivate] - Microsoft Edge",
            "Halaman sintetis — Mozilla Firefox Private Browsing",
            "Halaman sintetis - Brave (Private)",
        ] {
            assert_eq!(
                classify(&page(&["Halaman sintetis", title], Some("contoh.id"))),
                Some(SensitivePage::PrivateWindow),
                "{title}"
            );
        }
    }

    #[test]
    fn bank_sites_are_recognized_from_the_address() {
        for address in [
            "https://ibank.klikbca.com/login",
            "klikbca.com",
            "ib.bri.co.id",
            "https://www.bankmandiri.co.id/",
            "ibank.bni.co.id/MBAWeb",
            "https://user@netbank.contoh.co.id:443/x",
        ] {
            assert_eq!(
                classify(&page(&["Masuk"], Some(address))),
                Some(SensitivePage::Banking),
                "{address}"
            );
        }
    }

    #[test]
    fn similar_looking_hosts_are_not_banks() {
        for address in [
            "https://notbca.co.id.contoh.com",
            "mybca.co.idx",
            "https://contoh.id/klikbca.com",
            "cara buka rekening bca",
        ] {
            assert_eq!(
                classify(&page(&["Artikel sintetis"], Some(address))),
                None,
                "{address}"
            );
        }
    }

    #[test]
    fn titles_cover_windows_without_an_address_bar() {
        assert_eq!(
            classify(&page(&["KlikBCA Individual"], None)),
            Some(SensitivePage::Banking)
        );
        assert_eq!(
            classify(&page(&["Berita perbankan sintetis - Google Chrome"], None)),
            None
        );
    }

    #[test]
    fn ordinary_pages_are_readable() {
        assert_eq!(
            classify(&page(
                &["Kotak masuk - Gmail - Google Chrome"],
                Some("mail.google.com/mail/u/0/")
            )),
            None
        );
        assert_eq!(classify(&BrowserPage::default()), None);
    }

    #[test]
    fn host_ignores_scheme_credentials_port_and_path() {
        let parsed = |address: &str| host(address).map(|host| host.to_string());
        assert_eq!(
            parsed("HTTPS://Ib.Bri.Co.Id:8443/path?q=1#x").as_deref(),
            Some("ib.bri.co.id")
        );
        assert_eq!(parsed("localhost"), None);
        assert_eq!(parsed(""), None);
    }
}
