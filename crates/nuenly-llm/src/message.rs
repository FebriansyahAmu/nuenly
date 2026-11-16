//! Permintaan percakapan ke model.

use std::num::NonZeroU32;
use std::time::Duration;

use nuenly_core::SensitiveText;
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

/// Satu pesan. Isinya selalu [`SensitiveText`], termasuk prompt sistem, agar aturan log dan
/// zeroize berlaku sama untuk semua pesan.
#[derive(Clone, Debug)]
pub struct Message {
    pub role: Role,
    pub content: SensitiveText,
}

impl Message {
    #[must_use]
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: SensitiveText::new(content),
        }
    }

    #[must_use]
    pub fn user(content: SensitiveText) -> Self {
        Self {
            role: Role::User,
            content,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
    /// Panjang konteks dalam token. Wajib diisi: bawaan server terlalu kecil dan bisa berubah
    /// antarversi, dan nilai yang tetap menjaga prefix cache.
    pub num_ctx: NonZeroU32,
    pub temperature: Option<f32>,
    /// Lama model tetap dimuat setelah permintaan; `None` memakai bawaan server.
    pub keep_alive: Option<Duration>,
}

impl ChatRequest {
    #[must_use]
    pub fn new(model: impl Into<String>, num_ctx: NonZeroU32, messages: Vec<Message>) -> Self {
        Self {
            model: model.into(),
            messages,
            num_ctx,
            temperature: None,
            keep_alive: None,
        }
    }
}

/// Jumlah token yang dipakai satu permintaan, untuk metrik latensi dan anggaran konteks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub output_tokens: u32,
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU32;

    use nuenly_core::SensitiveText;

    use super::{ChatRequest, Message};

    #[test]
    fn debug_never_shows_message_content() {
        let request = ChatRequest::new(
            "gemma3:4b",
            NonZeroU32::MIN,
            vec![
                Message::system("prompt sistem sintetis"),
                Message::user(SensitiveText::new("keluhan pasien sintetis")),
            ],
        );
        let debug = format!("{request:?}");
        assert!(!debug.contains("sintetis"));
        assert!(debug.contains("gemma3:4b"));
    }
}
