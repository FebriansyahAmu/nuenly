//! Log diagnostik ke stderr, misalnya waktu tampil Tanya Nuenly dan token pertama (ADR 0010).
//!
//! Isinya hanya teks statis, angka, nama model, dan kode error. Event dan span dengan field
//! bernama `value`, `text`, atau `content` ditolak di tingkat callsite, jadi teks pengguna tidak
//! masuk log walau field seperti itu tidak sengaja ditulis.

use std::time::Duration;

use tracing::subscriber::Interest;
use tracing::{Metadata, Subscriber};
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::layer::{Context, Layer, SubscriberExt};

/// Nama field yang biasa memuat teks pengguna.
const FORBIDDEN_FIELDS: [&str; 3] = ["value", "text", "content"];

/// Memasang log ke stderr. Di build rilis tidak ada konsol, jadi log hanya terlihat bila stderr
/// dialihkan, misalnya `nuenly.exe 2> nuenly.log`.
pub fn init() {
    let subscriber = tracing_subscriber::registry()
        .with(NoUserText)
        .with(LevelFilter::INFO)
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(std::io::stderr)
                .with_target(false),
        );
    // Gagal hanya bila subscriber global sudah terpasang.
    let _ = tracing::subscriber::set_global_default(subscriber);
}

/// Lama `duration` dalam milidetik, untuk field log.
pub fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

/// Menolak event dan span yang punya field terlarang, untuk semua layer.
struct NoUserText;

impl<S: Subscriber> Layer<S> for NoUserText {
    fn register_callsite(&self, metadata: &'static Metadata<'static>) -> Interest {
        if allowed(metadata) {
            Interest::always()
        } else {
            Interest::never()
        }
    }

    fn enabled(&self, metadata: &Metadata<'_>, _: Context<'_, S>) -> bool {
        allowed(metadata)
    }
}

fn allowed(metadata: &Metadata<'_>) -> bool {
    FORBIDDEN_FIELDS
        .iter()
        .all(|name| metadata.fields().field(name).is_none())
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use tracing::field::{Field, Visit};
    use tracing::{Event, Subscriber};
    use tracing_subscriber::layer::{Context, Layer, SubscriberExt};

    use super::NoUserText;

    /// Mencatat nama field setiap event yang lolos.
    #[derive(Clone, Default)]
    struct Recorded(Arc<Mutex<Vec<Vec<String>>>>);

    struct Names(Vec<String>);

    impl Visit for Names {
        fn record_debug(&mut self, field: &Field, _: &dyn std::fmt::Debug) {
            self.0.push(field.name().to_owned());
        }
    }

    impl<S: Subscriber> Layer<S> for Recorded {
        fn on_event(&self, event: &Event<'_>, _: Context<'_, S>) {
            let mut names = Names(Vec::new());
            event.record(&mut names);
            self.0.lock().unwrap().push(names.0);
        }
    }

    #[test]
    fn events_that_could_carry_user_text_are_dropped() {
        let recorded = Recorded::default();
        let subscriber = tracing_subscriber::registry()
            .with(NoUserText)
            .with(recorded.clone());

        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(text = "teks sintetis", "dibuang");
            tracing::info!(value = 1, "dibuang");
            tracing::warn!(content = "isi sintetis", "dibuang");
            tracing::info!(shown_ms = 42_u64, "dicatat");
        });

        assert_eq!(*recorded.0.lock().unwrap(), [["message", "shown_ms"]]);
    }
}
