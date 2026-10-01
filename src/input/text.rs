//! Text to key steps, for a US keyboard layout.

use super::binding::MacroStep;
use super::keys::InputKey;
use crate::error::{Error, Result};

/// The key and whether Shift is needed to type `c` on a US layout.
fn us_key(c: char) -> Option<(u16, bool)> {
    let plain = |code| Some((code, false));
    let shifted = |code| Some((code, true));
    match c {
        'a'..='z' => plain(letter_code(c)),
        'A'..='Z' => shifted(letter_code(c.to_ascii_lowercase())),
        '1'..='9' => plain(c as u16 - '1' as u16 + 2),
        '0' => plain(11),
        '!' => shifted(2),
        '@' => shifted(3),
        '#' => shifted(4),
        '$' => shifted(5),
        '%' => shifted(6),
        '^' => shifted(7),
        '&' => shifted(8),
        '*' => shifted(9),
        '(' => shifted(10),
        ')' => shifted(11),
        '-' => plain(12),
        '_' => shifted(12),
        '=' => plain(13),
        '+' => shifted(13),
        '[' => plain(26),
        '{' => shifted(26),
        ']' => plain(27),
        '}' => shifted(27),
        ';' => plain(39),
        ':' => shifted(39),
        '\'' => plain(40),
        '"' => shifted(40),
        '`' => plain(41),
        '~' => shifted(41),
        '\\' => plain(43),
        '|' => shifted(43),
        ',' => plain(51),
        '<' => shifted(51),
        '.' => plain(52),
        '>' => shifted(52),
        '/' => plain(53),
        '?' => shifted(53),
        ' ' => plain(57),
        '\n' | '\r' => plain(28),
        '\t' => plain(15),
        _ => None,
    }
}

/// evdev codes of a..z follow the QWERTY rows, not the alphabet.
fn letter_code(c: char) -> u16 {
    const CODES: [u16; 26] = [
        30, 48, 46, 32, 18, 33, 34, 35, 23, 36, 37, 38, 50, 49, 24, 25, 16, 19, 31, 20, 22, 47, 17, 45, 21, 44,
    ];
    CODES[(c as u8 - b'a') as usize]
}

/// Convert text into macro steps that type it on a US keyboard layout.
///
/// Shifted characters are wrapped in a Left Shift press/release (consecutive shifted characters
/// share one). `\n`, `\r\n` and `\r` become Enter, `\t` becomes Tab. Any other character that a
/// US layout cannot type without a compose sequence is an error naming the character and its
/// position.
pub fn text_to_steps(text: &str) -> Result<Vec<MacroStep>> {
    let mut steps = Vec::with_capacity(text.len());
    let mut shift_down = false;
    let mut chars = text.chars().enumerate().peekable();
    while let Some((index, c)) = chars.next() {
        if c == '\r' && matches!(chars.peek(), Some((_, '\n'))) {
            continue;
        }
        let (code, shifted) = us_key(c).ok_or_else(|| {
            Error::InvalidConfig(format!(
                "character {c:?} at position {index} cannot be typed with a US keyboard layout"
            ))
        })?;
        if shifted != shift_down {
            steps.push(if shifted {
                MacroStep::Press(InputKey::KEY_LEFTSHIFT)
            } else {
                MacroStep::Release(InputKey::KEY_LEFTSHIFT)
            });
            shift_down = shifted;
        }
        steps.push(MacroStep::Tap(InputKey::from_code(code)));
    }
    if shift_down {
        steps.push(MacroStep::Release(InputKey::KEY_LEFTSHIFT));
    }
    Ok(steps)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tap(name: &str) -> MacroStep {
        MacroStep::Tap(InputKey::parse(name).unwrap())
    }

    const SHIFT_DOWN: MacroStep = MacroStep::Press(InputKey::KEY_LEFTSHIFT);
    const SHIFT_UP: MacroStep = MacroStep::Release(InputKey::KEY_LEFTSHIFT);

    #[test]
    fn lowercase_and_digits() {
        assert_eq!(
            text_to_steps("az09").unwrap(),
            vec![tap("a"), tap("z"), tap("0"), tap("9")]
        );
    }

    #[test]
    fn every_letter_maps_to_its_named_key() {
        for c in 'a'..='z' {
            let expected = InputKey::parse(&c.to_string()).unwrap();
            assert_eq!(
                text_to_steps(&c.to_string()).unwrap(),
                vec![MacroStep::Tap(expected)],
                "{c}"
            );
        }
    }

    #[test]
    fn shifted_characters_share_one_shift() {
        assert_eq!(
            text_to_steps("Hi!\n").unwrap(),
            vec![
                SHIFT_DOWN,
                tap("h"),
                SHIFT_UP,
                tap("i"),
                SHIFT_DOWN,
                tap("1"),
                SHIFT_UP,
                tap("enter")
            ]
        );
        assert_eq!(
            text_to_steps("AB").unwrap(),
            vec![SHIFT_DOWN, tap("a"), tap("b"), SHIFT_UP]
        );
    }

    #[test]
    fn punctuation_whitespace_and_newlines() {
        assert_eq!(
            text_to_steps("a b\tc").unwrap(),
            vec![tap("a"), tap("space"), tap("b"), tap("tab"), tap("c")]
        );
        assert_eq!(text_to_steps("\r\n").unwrap(), vec![tap("enter")]);
        assert_eq!(text_to_steps("\r").unwrap(), vec![tap("enter")]);
        assert_eq!(
            text_to_steps("{:?}").unwrap(),
            vec![
                SHIFT_DOWN,
                tap("leftbrace"),
                tap("semicolon"),
                tap("slash"),
                tap("rightbrace"),
                SHIFT_UP
            ]
        );
        assert_eq!(text_to_steps("-=[]\\;',./`").unwrap().len(), 11);
    }

    #[test]
    fn unsupported_characters_are_errors() {
        let err = text_to_steps("ok \u{e9}").unwrap_err().to_string();
        assert!(err.contains("position 3"), "{err}");
        assert!(err.contains('\u{e9}'), "{err}");
        assert!(text_to_steps("\u{1F600}").is_err());
        assert!(text_to_steps("\u{0}").is_err());
    }
}
