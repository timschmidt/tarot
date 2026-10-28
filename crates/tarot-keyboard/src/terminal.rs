//! US virtual-key strokes for one-shot terminal shortcuts. Text still uses IME.
use anyhow::{Context, Result};
use xkbcommon::xkb;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Stroke {
    pub code: u32,
    pub shift: bool,
}

pub fn stroke(ch: char) -> Option<Stroke> {
    for (plain, shifted, first) in [
        ("1234567890-=", "!@#$%^&*()_+", 2),
        ("qwertyuiop[]", "QWERTYUIOP{}", 16),
        ("asdfghjkl;'", "ASDFGHJKL:\"", 30),
        ("zxcvbnm,./", "ZXCVBNM<>?", 44),
        ("`", "~", 41),
        ("\\", "|", 43),
        (" ", " ", 57),
    ] {
        if let Some(column) = plain.chars().position(|c| c == ch) {
            return Some(Stroke {
                code: first + column as u32,
                shift: false,
            });
        }
        if let Some(column) = shifted.chars().position(|c| c == ch) {
            return Some(Stroke {
                code: first + column as u32,
                shift: true,
            });
        }
    }
    None
}

#[derive(Default)]
pub struct Masks {
    pub shift: u32,
    pub control: u32,
    pub alt: u32,
}

impl Masks {
    pub fn from_keymap(keymap: &xkb::Keymap) -> Result<Self> {
        let mask = |name: &str| {
            1u32.checked_shl(keymap.mod_get_index(name))
                .context("missing virtual-key modifier")
        };
        Ok(Self {
            shift: mask(xkb::MOD_NAME_SHIFT)?,
            control: mask(xkb::MOD_NAME_CTRL)?,
            alt: mask(xkb::MOD_NAME_ALT)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn printable_strokes_match_the_actual_us_xkb_keymap() {
        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let keymap = xkb::Keymap::new_from_names(
            &context,
            "",
            "",
            "us",
            "",
            None,
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        )
        .unwrap();
        let masks = Masks::from_keymap(&keymap).unwrap();
        let mut state = xkb::State::new(&keymap);
        for byte in 32u8..=126 {
            let ch = char::from(byte);
            let key = stroke(ch).unwrap();
            state.update_mask(if key.shift { masks.shift } else { 0 }, 0, 0, 0, 0, 0);
            assert_eq!(state.key_get_utf8((key.code + 8).into()), ch.to_string());
        }
        assert!(stroke('é').is_none() && stroke('🙂').is_none());
        state.update_mask(masks.control, 0, 0, 0, 0, 0);
        assert_eq!(state.key_get_utf8(54u32.into()), "\u{3}");
    }
}
