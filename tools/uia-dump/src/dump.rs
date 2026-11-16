//! Menjalankan dump UIA dan mencetak hasilnya.

use std::fmt::Write as _;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, bail};
use nuenly_core::{
    ActivePolicy, AppFilter, Browser, ContextSource, ModeId, SensitivePage, classify,
};
use nuenly_platform_windows::UiaContextSource;
use nuenly_platform_windows::uia::{Uia, UiaNode, UiaWindow, WalkLimits};

use crate::args::Args;
use crate::render;

pub fn run(args: &Args) -> Result<()> {
    if args.show_names || args.show_values {
        eprintln!(
            "Peringatan: output memuat teks dari layar. Pakai hanya dengan data sintetis, \
             dan jangan bagikan output yang memuat data asli."
        );
    }
    eprintln!(
        "Pindah ke aplikasi target. Membaca dalam {} detik...",
        args.delay.as_secs()
    );
    thread::sleep(args.delay);
    if args.browser {
        return print_browser_page(args);
    }

    let uia = Uia::new().context("gagal menyiapkan UI Automation")?;
    let Some(window) = uia
        .foreground_window()
        .context("gagal membaca jendela di latar depan")?
    else {
        bail!("tidak ada jendela di latar depan");
    };
    if !AppFilter::default().permits(&window.process) {
        bail!(
            "{} termasuk aplikasi yang tidak pernah dibaca Nuenly",
            window.process
        );
    }
    println!("Aplikasi: {} (pid {})", window.process, window.pid);

    if args.focused {
        print_focused(&uia, &window, args)
    } else {
        print_tree(&uia, &window, args);
        Ok(())
    }
}

fn print_tree(uia: &Uia, window: &UiaWindow, args: &Args) {
    let limits = WalkLimits {
        max_depth: args.max_depth,
        max_nodes: args.max_nodes,
    };
    let stats = uia.walk(window, limits, |depth, node| {
        println!("{}{}", "  ".repeat(depth), line(node, args));
    });

    let mut summary = format!(
        "\n{} elemen, kedalaman maks {}, {}",
        stats.nodes,
        stats.max_depth,
        millis(stats.elapsed)
    );
    if stats.truncated {
        summary.push_str(", terpotong oleh batas --depth atau --max-nodes");
    }
    if stats.errors > 0 {
        let _ = write!(summary, ", {} elemen gagal dibaca anaknya", stats.errors);
    }
    println!("{summary}");
}

fn print_focused(uia: &Uia, window: &UiaWindow, args: &Args) -> Result<()> {
    let Some(report) = uia
        .focused(window)
        .context("gagal membaca elemen yang fokus")?
    else {
        bail!("elemen yang fokus tidak berada di aplikasi ini");
    };

    println!("Fokus: {}", line(&report.node, args));
    println!(
        "LabeledBy: {}",
        report.labeled_by.as_ref().map_or_else(
            || "-".to_owned(),
            |label| render::text(label, args.show_names)
        )
    );
    println!(
        "Teks terpilih: {}",
        report.selection.as_ref().map_or_else(
            || "-".to_owned(),
            |text| render::text(text, args.show_values)
        )
    );
    println!("Saudara sebelumnya (terdekat lebih dulu):");
    if report.previous_siblings.is_empty() {
        println!("  -");
    }
    for sibling in &report.previous_siblings {
        println!("  {}", line(sibling, args));
    }
    println!(
        "Waktu baca elemen fokus: {} (target PRD ≤ 50 ms)",
        millis(report.elapsed)
    );
    Ok(())
}

/// Pemeriksaan halaman browser yang sama dengan `nuenly_core::capture`, dua kali: yang kedua
/// memakai address bar yang sudah ditemukan.
fn print_browser_page(args: &Args) -> Result<()> {
    let source = UiaContextSource::spawn(Duration::from_secs(5))
        .context("gagal menyiapkan UI Automation")?;
    let Some(window) = source
        .foreground_window()
        .context("gagal membaca jendela di latar depan")?
    else {
        bail!("tidak ada jendela di latar depan");
    };
    let Some(browser) = Browser::from_process(&window.app.process) else {
        bail!("{} bukan browser yang dikenali Nuenly", window.app.process);
    };
    let policy = ActivePolicy::new(ModeId::new("all").context("ID mode")?);
    let Some(permit) = policy.permit(&window) else {
        bail!(
            "{} termasuk aplikasi yang tidak pernah dibaca Nuenly",
            window.app.process
        );
    };
    println!("Aplikasi: {} ({browser:?})", window.app.process);

    let started = Instant::now();
    let page = source
        .browser_page(&permit, browser)
        .context("gagal memeriksa halaman")?;
    let first = started.elapsed();
    let started = Instant::now();
    source
        .browser_page(&permit, browser)
        .context("gagal memeriksa halaman")?;
    let again = started.elapsed();

    for (index, title) in page.titles.iter().enumerate() {
        println!(
            "Judul {}: {}",
            index + 1,
            render::text(title, args.show_names)
        );
    }
    println!(
        "Address bar: {}",
        page.address.as_ref().map_or_else(
            || "tidak ditemukan (yang diperiksa hanya judul)".to_owned(),
            |address| render::text(address, args.show_values)
        )
    );
    println!(
        "Hasil: {}",
        match classify(&page) {
            Some(SensitivePage::PrivateWindow) => "jendela privat, tidak dibaca",
            Some(SensitivePage::Banking) => "situs perbankan, tidak dibaca",
            None => "boleh dibaca",
        }
    );
    println!(
        "Waktu periksa: {} pertama, {} berikutnya",
        millis(first),
        millis(again)
    );
    Ok(())
}

fn line(node: &UiaNode, args: &Args) -> String {
    let mut out = if node.control_type.is_empty() {
        "?".to_owned()
    } else {
        node.control_type.clone()
    };
    let _ = write!(out, " name={}", render::text(&node.name, args.show_names));
    if let Some(value) = &node.value {
        let _ = write!(out, " value={}", render::text(value, args.show_values));
    }
    if node.is_password {
        out.push_str(" [sandi]");
    }
    if node.has_focus {
        out.push_str(" [fokus]");
    }
    for (key, field) in [
        ("id", &node.automation_id),
        ("class", &node.class_name),
        ("fw", &node.framework_id),
    ] {
        if !field.is_empty() {
            let _ = write!(out, " {key}={field:?}");
        }
    }
    if !node.patterns.is_empty() {
        let _ = write!(out, " pola={}", node.patterns.join(","));
    }
    if let Some(rect) = node.bounds {
        let _ = write!(
            out,
            " @{},{} {}x{}",
            rect.x, rect.y, rect.width, rect.height
        );
    }
    out
}

fn millis(elapsed: Duration) -> String {
    format!("{:.1} ms", elapsed.as_secs_f64() * 1000.0)
}

#[cfg(test)]
mod tests {
    use nuenly_core::SensitiveText;
    use nuenly_platform_windows::uia::UiaNode;

    use super::line;
    use crate::args::Args;

    fn node() -> UiaNode {
        UiaNode {
            control_type: "Edit".to_owned(),
            class_name: "WindowsForms10.EDIT".to_owned(),
            automation_id: "txtAnamnesis".to_owned(),
            framework_id: "WinForm".to_owned(),
            name: SensitiveText::new("Anamnesis"),
            value: Some(SensitiveText::new("Pasien sintetis mengeluh pusing")),
            has_focus: true,
            patterns: vec!["Value", "Text"],
            ..UiaNode::default()
        }
    }

    #[test]
    fn line_hides_names_and_values_by_default() {
        let out = line(&node(), &Args::default());
        assert!(!out.contains("name=\""));
        assert!(!out.contains("Pasien"));
        assert!(out.contains("name=<9 karakter>"));
        assert!(out.contains("value=<31 karakter>"));
        assert!(out.contains("id=\"txtAnamnesis\""));
        assert!(out.contains("[fokus]"));
    }

    #[test]
    fn line_reveals_only_what_is_asked() {
        let args = Args {
            show_names: true,
            ..Args::default()
        };
        let out = line(&node(), &args);
        assert!(out.contains("name=\"Anamnesis\""));
        assert!(!out.contains("Pasien"));
    }
}
