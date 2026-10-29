//! Pembacaan tree UI Automation untuk diagnosis, dipakai `tools/uia-dump` di spike Fase 0.
//!
//! Semua teks dari layar dikembalikan sebagai [`SensitiveText`]. `UIElement` dari crate
//! `uiautomation` tidak keluar dari modul ini karena `Debug` dan `Display`-nya mencetak nama
//! elemen.

use std::time::{Duration, Instant};

use nuenly_core::{ContextError, ProcessName, Rect, SensitiveText};
use uiautomation::patterns::{UILegacyIAccessiblePattern, UITextPattern, UIValuePattern};
use uiautomation::types::Handle;
use uiautomation::{UIAutomation, UIElement, UITreeWalker};
use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

use crate::process::process_name;

/// Batas panjang teks terpilih yang dibaca per rentang, dalam unit UTF-16.
const MAX_SELECTION_LEN: i32 = 10_000;

/// Klien UI Automation. COM diinisialisasi (MTA) di thread pembuat, dan klien dipakai di
/// thread yang sama.
pub struct Uia {
    automation: UIAutomation,
    walker: UITreeWalker,
}

/// Jendela di latar depan beserta prosesnya.
pub struct UiaWindow {
    element: UIElement,
    pub process: ProcessName,
    pub pid: u32,
}

/// Properti satu elemen UIA.
#[derive(Clone, Debug, Default)]
pub struct UiaNode {
    /// Jenis kontrol UIA, misalnya `Edit` atau `Document`.
    pub control_type: String,
    pub class_name: String,
    pub automation_id: String,
    /// Teknologi UI, misalnya `Win32`, `WinForm`, `WPF`, atau `Chrome`.
    pub framework_id: String,
    pub name: SensitiveText,
    /// Isi dari Value pattern. Tidak pernah dibaca untuk field kata sandi.
    pub value: Option<SensitiveText>,
    pub is_password: bool,
    pub has_focus: bool,
    /// Pattern yang didukung, dari yang relevan untuk Nuenly.
    pub patterns: Vec<&'static str>,
    pub bounds: Option<Rect>,
}

/// Batas penelusuran tree.
#[derive(Clone, Copy, Debug)]
pub struct WalkLimits {
    pub max_depth: usize,
    pub max_nodes: usize,
}

/// Ringkasan penelusuran tree.
#[derive(Clone, Copy, Debug, Default)]
pub struct WalkStats {
    pub nodes: usize,
    pub max_depth: usize,
    /// Ada elemen yang tidak ditelusuri karena batas kedalaman atau jumlah.
    pub truncated: bool,
    /// Berapa kali anak elemen gagal dibaca, misalnya karena elemennya hilang saat ditelusuri.
    pub errors: usize,
    pub elapsed: Duration,
}

/// Hasil membaca elemen yang sedang fokus.
#[derive(Debug)]
pub struct FocusReport {
    pub node: UiaNode,
    /// Nama elemen yang ditunjuk properti `LabeledBy`.
    pub labeled_by: Option<SensitiveText>,
    /// Teks terpilih lewat Text pattern.
    pub selection: Option<SensitiveText>,
    /// Hingga tiga saudara sebelumnya, terdekat lebih dulu. Di banyak aplikasi `WinForms`,
    /// label field ada di sini, bukan di `LabeledBy`.
    pub previous_siblings: Vec<UiaNode>,
    /// Waktu membaca elemen fokus: properti, nilai, `LabeledBy`, dan seleksi.
    pub elapsed: Duration,
}

impl Uia {
    /// # Errors
    ///
    /// [`ContextError::Platform`] bila COM atau UI Automation gagal diinisialisasi.
    pub fn new() -> Result<Self, ContextError> {
        let automation = UIAutomation::new().map_err(uia_error("CoCreateInstance"))?;
        let walker = automation
            .get_control_view_walker()
            .map_err(uia_error("ControlViewWalker"))?;
        Ok(Self { automation, walker })
    }

    /// Jendela di latar depan, atau `None` bila tidak ada.
    ///
    /// # Errors
    ///
    /// [`ContextError::Platform`] bila elemen jendela atau nama prosesnya tidak bisa dibaca.
    pub fn foreground_window(&self) -> Result<Option<UiaWindow>, ContextError> {
        // SAFETY: GetForegroundWindow tidak punya prasyarat dan hanya membaca state sistem.
        let hwnd = unsafe { GetForegroundWindow() };
        if hwnd.is_invalid() {
            return Ok(None);
        }
        let element = self
            .automation
            .element_from_handle(Handle::from(hwnd))
            .map_err(uia_error("ElementFromHandle"))?;
        let pid = element.get_process_id().map_err(uia_error("ProcessId"))?;
        let process = process_name(pid)?;
        Ok(Some(UiaWindow {
            element,
            process,
            pid,
        }))
    }

    /// Menelusuri control view dari jendela secara depth-first dan memanggil `visit` untuk
    /// setiap elemen beserta kedalamannya (jendela = 0). Elemen yang gagal dibaca dilewati.
    pub fn walk(
        &self,
        window: &UiaWindow,
        limits: WalkLimits,
        mut visit: impl FnMut(usize, &UiaNode),
    ) -> WalkStats {
        let started = Instant::now();
        let mut stats = WalkStats::default();
        // Stack eksplisit agar tree yang sangat dalam tidak menghabiskan stack thread.
        let mut stack = vec![(window.element.clone(), 0usize)];

        while let Some((element, depth)) = stack.pop() {
            if stats.nodes >= limits.max_nodes {
                stats.truncated = true;
                break;
            }
            visit(depth, &describe(&element));
            stats.nodes += 1;
            stats.max_depth = stats.max_depth.max(depth);

            match self.children(&element) {
                Ok(children) if depth >= limits.max_depth => {
                    stats.truncated |= !children.is_empty();
                }
                Ok(children) => {
                    stack.extend(children.into_iter().rev().map(|child| (child, depth + 1)));
                }
                Err(_) => stats.errors += 1,
            }
        }

        stats.elapsed = started.elapsed();
        stats
    }

    /// Membaca elemen yang sedang fokus, atau `None` bila fokus berada di proses lain.
    ///
    /// # Errors
    ///
    /// [`ContextError::Platform`] bila elemen fokus tidak bisa diambil.
    pub fn focused(&self, window: &UiaWindow) -> Result<Option<FocusReport>, ContextError> {
        let started = Instant::now();
        let element = self
            .automation
            .get_focused_element()
            .map_err(uia_error("GetFocusedElement"))?;
        if element.get_process_id().map_err(uia_error("ProcessId"))? != window.pid {
            return Ok(None);
        }

        let node = describe(&element);
        let labeled_by = element
            .get_labeled_by()
            .ok()
            .map(|label| SensitiveText::new(label.get_name().unwrap_or_default()));
        let selection = if node.is_password {
            None
        } else {
            selection(&element)
        };
        let elapsed = started.elapsed();

        Ok(Some(FocusReport {
            node,
            labeled_by,
            selection,
            previous_siblings: self.previous_siblings(&element, 3),
            elapsed,
        }))
    }

    fn children(&self, element: &UIElement) -> Result<Vec<UIElement>, ContextError> {
        let mut children = Vec::new();
        let mut next = optional(self.walker.get_first_child(element), "GetFirstChildElement")?;
        while let Some(child) = next {
            next = optional(
                self.walker.get_next_sibling(&child),
                "GetNextSiblingElement",
            )?;
            children.push(child);
        }
        Ok(children)
    }

    fn previous_siblings(&self, element: &UIElement, limit: usize) -> Vec<UiaNode> {
        let mut nodes = Vec::new();
        let mut current = self.walker.get_previous_sibling(element).ok();
        while let Some(sibling) = current {
            if nodes.len() >= limit {
                break;
            }
            nodes.push(describe(&sibling));
            current = self.walker.get_previous_sibling(&sibling).ok();
        }
        nodes
    }
}

fn describe(element: &UIElement) -> UiaNode {
    // Bila tidak bisa dipastikan, anggap field kata sandi agar nilainya tidak dibaca.
    let is_password = element.is_password().unwrap_or(true);
    let value_pattern = element.get_pattern::<UIValuePattern>().ok();

    let mut patterns = Vec::new();
    if value_pattern.is_some() {
        patterns.push("Value");
    }
    if element.get_pattern::<UITextPattern>().is_ok() {
        patterns.push("Text");
    }
    if element.get_pattern::<UILegacyIAccessiblePattern>().is_ok() {
        patterns.push("LegacyIAccessible");
    }

    let value = if is_password {
        None
    } else {
        value_pattern
            .and_then(|pattern| pattern.get_value().ok())
            .map(SensitiveText::new)
    };

    UiaNode {
        control_type: element
            .get_control_type()
            .map(|kind| format!("{kind:?}"))
            .unwrap_or_default(),
        class_name: element.get_classname().unwrap_or_default(),
        automation_id: element.get_automation_id().unwrap_or_default(),
        framework_id: element.get_framework_id().unwrap_or_default(),
        name: SensitiveText::new(element.get_name().unwrap_or_default()),
        value,
        is_password,
        has_focus: element.has_keyboard_focus().unwrap_or(false),
        patterns,
        bounds: element.get_bounding_rectangle().ok().and_then(|rect| {
            Some(Rect {
                x: rect.get_left(),
                y: rect.get_top(),
                width: u32::try_from(rect.get_width()).ok()?,
                height: u32::try_from(rect.get_height()).ok()?,
            })
        }),
    }
}

fn selection(element: &UIElement) -> Option<SensitiveText> {
    let ranges = element
        .get_pattern::<UITextPattern>()
        .ok()?
        .get_selection()
        .ok()?;
    let text: String = ranges
        .iter()
        .filter_map(|range| range.get_text(MAX_SELECTION_LEN).ok())
        .collect();
    Some(SensitiveText::new(text))
}

/// UIA mengembalikan null (kode 0) bila elemen tidak ada; itu menjadi `Ok(None)`.
fn optional(
    result: uiautomation::Result<UIElement>,
    operation: &'static str,
) -> Result<Option<UIElement>, ContextError> {
    match result {
        Ok(element) => Ok(Some(element)),
        Err(err) if err.code() == 0 => Ok(None),
        Err(err) => Err(ContextError::Platform {
            operation,
            code: err.code(),
        }),
    }
}

fn uia_error(operation: &'static str) -> impl FnOnce(uiautomation::Error) -> ContextError {
    move |err| ContextError::Platform {
        operation,
        code: err.code(),
    }
}
