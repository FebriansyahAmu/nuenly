//! Pintasan keyboard global (F-03), bebas OS. Platform menerjemahkannya ke kode tombolnya.

use std::fmt;

use crate::error::InvalidName;

/// Tombol utama pintasan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Space,
    /// Huruf `A`–`Z`, sebagai byte ASCII huruf besar.
    Letter(u8),
    /// Angka `0`–`9` di baris atas, sebagai byte ASCII.
    Digit(u8),
    /// `F1`–`F24`.
    Function(u8),
}

/// Kombinasi tombol untuk pintasan global, misalnya `Ctrl+Space` atau `Alt+Shift+K`.
///
/// Ctrl atau Alt wajib ada, agar ketikan biasa (huruf, atau Shift + huruf) tidak pernah
/// tertangkap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)] // Setiap bool adalah tombol pengubah yang berdiri sendiri.
pub struct Shortcut {
    ctrl: bool,
    alt: bool,
    shift: bool,
    win: bool,
    key: Key,
}

impl Shortcut {
    /// Pintasan bawaan Tanya Nuenly.
    pub const CTRL_SPACE: Self = Self {
        ctrl: true,
        alt: false,
        shift: false,
        win: false,
        key: Key::Space,
    };

    /// Membaca pintasan seperti `Ctrl+Space` atau `alt + shift + f5`, tanpa membedakan huruf
    /// besar dan kecil.
    ///
    /// # Errors
    ///
    /// [`InvalidName`] bila ada tombol yang tidak dikenal atau berulang, tombol utamanya bukan
    /// satu yang terakhir, atau tanpa Ctrl maupun Alt.
    pub fn parse(text: &str) -> Result<Self, InvalidName> {
        let invalid = || InvalidName::new("pintasan");
        let mut parts: Vec<String> = text
            .split('+')
            .map(|part| part.trim().to_lowercase())
            .collect();
        let key = parts
            .pop()
            .and_then(|key| parse_key(&key))
            .ok_or_else(invalid)?;

        let mut shortcut = Self {
            ctrl: false,
            alt: false,
            shift: false,
            win: false,
            key,
        };
        for part in &parts {
            let flag = match part.as_str() {
                "ctrl" | "control" => &mut shortcut.ctrl,
                "alt" => &mut shortcut.alt,
                "shift" => &mut shortcut.shift,
                "win" | "windows" | "meta" | "super" => &mut shortcut.win,
                _ => return Err(invalid()),
            };
            if *flag {
                return Err(invalid());
            }
            *flag = true;
        }
        if shortcut.ctrl || shortcut.alt {
            Ok(shortcut)
        } else {
            Err(invalid())
        }
    }

    #[must_use]
    pub fn ctrl(self) -> bool {
        self.ctrl
    }

    #[must_use]
    pub fn alt(self) -> bool {
        self.alt
    }

    #[must_use]
    pub fn shift(self) -> bool {
        self.shift
    }

    #[must_use]
    pub fn win(self) -> bool {
        self.win
    }

    #[must_use]
    pub fn key(self) -> Key {
        self.key
    }
}

fn parse_key(key: &str) -> Option<Key> {
    match key.as_bytes() {
        b"space" | b"spasi" => Some(Key::Space),
        [letter] if letter.is_ascii_lowercase() => Some(Key::Letter(letter.to_ascii_uppercase())),
        [digit] if digit.is_ascii_digit() => Some(Key::Digit(*digit)),
        [b'f', number @ ..] if !number.is_empty() && number[0] != b'0' => {
            let number: u8 = std::str::from_utf8(number).ok()?.parse().ok()?;
            (1..=24).contains(&number).then_some(Key::Function(number))
        }
        _ => None,
    }
}

impl fmt::Display for Shortcut {
    /// Urutan tetap `Ctrl+Alt+Shift+Win+Tombol`, sama dengan yang dibaca [`Shortcut::parse`].
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (on, name) in [
            (self.ctrl, "Ctrl+"),
            (self.alt, "Alt+"),
            (self.shift, "Shift+"),
            (self.win, "Win+"),
        ] {
            if on {
                f.write_str(name)?;
            }
        }
        match self.key {
            Key::Space => f.write_str("Space"),
            Key::Letter(letter) | Key::Digit(letter) => write!(f, "{}", char::from(letter)),
            Key::Function(number) => write!(f, "F{number}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Key, Shortcut};

    #[test]
    fn reads_and_writes_the_same_form() {
        for text in [
            "Ctrl+Space",
            "Alt+Space",
            "Ctrl+Shift+K",
            "Ctrl+Alt+Shift+Win+F24",
            "Alt+7",
        ] {
            assert_eq!(Shortcut::parse(text).unwrap().to_string(), text);
        }
        assert_eq!(Shortcut::parse("Ctrl+Space").unwrap(), Shortcut::CTRL_SPACE);
    }

    #[test]
    fn accepts_loose_spelling() {
        let shortcut = Shortcut::parse(" shift + alt + f5 ").unwrap();
        assert_eq!(shortcut.to_string(), "Alt+Shift+F5");
        assert_eq!(shortcut.key(), Key::Function(5));
        assert_eq!(
            Shortcut::parse("control+spasi").unwrap(),
            Shortcut::CTRL_SPACE
        );
    }

    #[test]
    fn refuses_shortcuts_that_would_catch_typing() {
        for text in [
            "",
            "Space",
            "K",
            "Shift+K",
            "Win+K",
            "Ctrl",
            "Ctrl+",
            "Ctrl+Ctrl+K",
            "Ctrl+Enter",
            "Ctrl+F0",
            "Ctrl+F25",
            "Ctrl+F01",
            "Ctrl+KK",
            "Ctrl+Space+K",
            "Hyper+K",
        ] {
            assert!(Shortcut::parse(text).is_err(), "{text:?}");
        }
    }
}
