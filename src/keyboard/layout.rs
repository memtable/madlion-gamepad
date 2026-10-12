use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KeyId {
    // row 0
    Esc,
    Key1,
    Key2,
    Key3,
    Key4,
    Key5,
    Key6,
    Key7,
    Key8,
    Key9,
    Key0,
    Minus,
    Equal,
    Backspace,

    // row 1
    Tab,
    Q,
    W,
    E,
    R,
    T,
    Y,
    U,
    I,
    O,
    P,
    LeftBracket,
    RightBracket,
    Backslash,

    // row 2
    CapsLock,
    A,
    S,
    D,
    F,
    G,
    H,
    J,
    K,
    L,
    Semicolon,
    Quote,
    Enter,

    // row 3
    LeftShift,
    Z,
    X,
    C,
    V,
    B,
    N,
    M,
    Comma,
    Dot,
    Slash,
    RightShift,

    // row 4
    LeftControl,
    LeftMeta,
    LeftAlt,
    Space,
    RightMeta,
    RightAlt,
    Menu,
    RightControl,
    Fn,
}

pub const MATRIX_KEY_COUNT: usize = 70;

pub const MAD60_LAYOUT: [Option<KeyId>; MATRIX_KEY_COUNT] = [
    // row 0 (0..13)
    Some(KeyId::Esc),       // 0
    Some(KeyId::Key1),      // 1
    Some(KeyId::Key2),      // 2
    Some(KeyId::Key3),      // 3
    Some(KeyId::Key4),      // 4
    Some(KeyId::Key5),      // 5
    Some(KeyId::Key6),      // 6
    Some(KeyId::Key7),      // 7
    Some(KeyId::Key8),      // 8
    Some(KeyId::Key9),      // 9
    Some(KeyId::Key0),      // 10
    Some(KeyId::Minus),     // 11
    Some(KeyId::Equal),     // 12
    Some(KeyId::Backspace), // 13
    // row 1 (14..27)
    Some(KeyId::Tab),          // 14
    Some(KeyId::Q),            // 15
    Some(KeyId::W),            // 16
    Some(KeyId::E),            // 17
    Some(KeyId::R),            // 18
    Some(KeyId::T),            // 19
    Some(KeyId::Y),            // 20
    Some(KeyId::U),            // 21
    Some(KeyId::I),            // 22
    Some(KeyId::O),            // 23
    Some(KeyId::P),            // 24
    Some(KeyId::LeftBracket),  // 25
    Some(KeyId::RightBracket), // 26
    Some(KeyId::Backslash),    // 27
    // row 2 (28..41)
    Some(KeyId::CapsLock),  // 28
    Some(KeyId::A),         // 29
    Some(KeyId::S),         // 30
    Some(KeyId::D),         // 31
    Some(KeyId::F),         // 32
    Some(KeyId::G),         // 33
    Some(KeyId::H),         // 34
    Some(KeyId::J),         // 35
    Some(KeyId::K),         // 36
    Some(KeyId::L),         // 37
    Some(KeyId::Semicolon), // 38
    Some(KeyId::Quote),     // 39
    None,                   // 40
    Some(KeyId::Enter),     // 41
    // row 3 (42..55)
    Some(KeyId::LeftShift),  // 42
    None,                    // 43
    Some(KeyId::Z),          // 44
    Some(KeyId::X),          // 45
    Some(KeyId::C),          // 46
    Some(KeyId::V),          // 47
    Some(KeyId::B),          // 48
    Some(KeyId::N),          // 49
    Some(KeyId::M),          // 50
    Some(KeyId::Comma),      // 51
    Some(KeyId::Dot),        // 52
    Some(KeyId::Slash),      // 53
    None,                    // 54
    Some(KeyId::RightShift), // 55
    // row 4 (56..69)
    Some(KeyId::LeftControl),  // 56
    Some(KeyId::LeftMeta),     // 57
    Some(KeyId::LeftAlt),      // 58
    None,                      // 59
    None,                      // 60
    None,                      // 61
    Some(KeyId::Space),        // 62
    None,                      // 63
    None,                      // 64
    Some(KeyId::RightMeta),    // 65
    Some(KeyId::RightAlt),     // 66
    Some(KeyId::Menu),         // 67
    Some(KeyId::RightControl), // 68
    Some(KeyId::Fn),           // 69
];

pub fn find_key_matrix_index(key: KeyId) -> Option<u8> {
    for (idx, slot) in MAD60_LAYOUT.iter().enumerate() {
        if *slot == Some(key) {
            return Some(idx as u8);
        }
    }
    None
}

#[cfg(test)]
pub fn find_key_by_matrix_index(index: u8) -> Option<KeyId> {
    if (index as usize) < MAD60_LAYOUT.len() {
        MAD60_LAYOUT[index as usize]
    } else {
        None
    }
}

impl fmt::Display for KeyId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            KeyId::Esc => "Esc",
            KeyId::Key1 => "1",
            KeyId::Key2 => "2",
            KeyId::Key3 => "3",
            KeyId::Key4 => "4",
            KeyId::Key5 => "5",
            KeyId::Key6 => "6",
            KeyId::Key7 => "7",
            KeyId::Key8 => "8",
            KeyId::Key9 => "9",
            KeyId::Key0 => "0",
            KeyId::Minus => "-",
            KeyId::Equal => "=",
            KeyId::Backspace => "Backspace",
            KeyId::Tab => "Tab",
            KeyId::Q => "Q",
            KeyId::W => "W",
            KeyId::E => "E",
            KeyId::R => "R",
            KeyId::T => "T",
            KeyId::Y => "Y",
            KeyId::U => "U",
            KeyId::I => "I",
            KeyId::O => "O",
            KeyId::P => "P",
            KeyId::LeftBracket => "[",
            KeyId::RightBracket => "]",
            KeyId::Backslash => "\\",
            KeyId::CapsLock => "CapsLock",
            KeyId::A => "A",
            KeyId::S => "S",
            KeyId::D => "D",
            KeyId::F => "F",
            KeyId::G => "G",
            KeyId::H => "H",
            KeyId::J => "J",
            KeyId::K => "K",
            KeyId::L => "L",
            KeyId::Semicolon => ";",
            KeyId::Quote => "'",
            KeyId::Enter => "Enter",
            KeyId::LeftShift => "LShift",
            KeyId::Z => "Z",
            KeyId::X => "X",
            KeyId::C => "C",
            KeyId::V => "V",
            KeyId::B => "B",
            KeyId::N => "N",
            KeyId::M => "M",
            KeyId::Comma => ",",
            KeyId::Dot => ".",
            KeyId::Slash => "/",
            KeyId::RightShift => "RShift",
            KeyId::LeftControl => "LCtrl",
            KeyId::LeftMeta => "LMeta",
            KeyId::LeftAlt => "LAlt",
            KeyId::Space => "Space",
            KeyId::RightMeta => "RMeta",
            KeyId::RightAlt => "RAlt",
            KeyId::Menu => "Menu",
            KeyId::RightControl => "RCtrl",
            KeyId::Fn => "Fn",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ParseKeyError(pub String);

impl fmt::Display for ParseKeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown key '{}'", self.0)
    }
}

impl std::error::Error for ParseKeyError {}

impl FromStr for KeyId {
    type Err = ParseKeyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let clean = s.trim().to_uppercase();
        match clean.as_str() {
            "ESC" | "ESCAPE" => Ok(KeyId::Esc),
            "1" | "KEY1" | "NUM1" => Ok(KeyId::Key1),
            "2" | "KEY2" | "NUM2" => Ok(KeyId::Key2),
            "3" | "KEY3" | "NUM3" => Ok(KeyId::Key3),
            "4" | "KEY4" | "NUM4" => Ok(KeyId::Key4),
            "5" | "KEY5" | "NUM5" => Ok(KeyId::Key5),
            "6" | "KEY6" | "NUM6" => Ok(KeyId::Key6),
            "7" | "KEY7" | "NUM7" => Ok(KeyId::Key7),
            "8" | "KEY8" | "NUM8" => Ok(KeyId::Key8),
            "9" | "KEY9" | "NUM9" => Ok(KeyId::Key9),
            "0" | "KEY0" | "NUM0" => Ok(KeyId::Key0),
            "-" | "MINUS" => Ok(KeyId::Minus),
            "=" | "EQUAL" | "EQUALS" => Ok(KeyId::Equal),
            "BACKSPACE" | "BKSP" => Ok(KeyId::Backspace),
            "TAB" => Ok(KeyId::Tab),
            "Q" => Ok(KeyId::Q),
            "W" => Ok(KeyId::W),
            "E" => Ok(KeyId::E),
            "R" => Ok(KeyId::R),
            "T" => Ok(KeyId::T),
            "Y" => Ok(KeyId::Y),
            "U" => Ok(KeyId::U),
            "I" => Ok(KeyId::I),
            "O" => Ok(KeyId::O),
            "P" => Ok(KeyId::P),
            "[" | "LEFTBRACKET" | "LBRACKET" => Ok(KeyId::LeftBracket),
            "]" | "RIGHTBRACKET" | "RBRACKET" => Ok(KeyId::RightBracket),
            "\\" | "BACKSLASH" | "BSLASH" => Ok(KeyId::Backslash),
            "CAPSLOCK" | "CAPS" => Ok(KeyId::CapsLock),
            "A" => Ok(KeyId::A),
            "S" => Ok(KeyId::S),
            "D" => Ok(KeyId::D),
            "F" => Ok(KeyId::F),
            "G" => Ok(KeyId::G),
            "H" => Ok(KeyId::H),
            "J" => Ok(KeyId::J),
            "K" => Ok(KeyId::K),
            "L" => Ok(KeyId::L),
            ";" | "SEMICOLON" | "SEMI" => Ok(KeyId::Semicolon),
            "'" | "QUOTE" | "APOSTROPHE" => Ok(KeyId::Quote),
            "ENTER" | "RETURN" => Ok(KeyId::Enter),
            "LSHIFT" | "LEFTSHIFT" | "SHIFT" => Ok(KeyId::LeftShift),
            "Z" => Ok(KeyId::Z),
            "X" => Ok(KeyId::X),
            "C" => Ok(KeyId::C),
            "V" => Ok(KeyId::V),
            "B" => Ok(KeyId::B),
            "N" => Ok(KeyId::N),
            "M" => Ok(KeyId::M),
            "," | "COMMA" => Ok(KeyId::Comma),
            "." | "DOT" | "PERIOD" => Ok(KeyId::Dot),
            "/" | "SLASH" => Ok(KeyId::Slash),
            "RSHIFT" | "RIGHTSHIFT" => Ok(KeyId::RightShift),
            "LCTRL" | "LEFTCONTROL" | "LEFTCTRL" | "CTRL" => Ok(KeyId::LeftControl),
            "LMETA" | "LEFTMETA" | "SUPER" | "WIN" | "LEFTSUPER" => Ok(KeyId::LeftMeta),
            "LALT" | "LEFTALT" | "ALT" => Ok(KeyId::LeftAlt),
            "SPACE" | "SPACEBAR" => Ok(KeyId::Space),
            "RMETA" | "RIGHTMETA" | "RIGHTSUPER" => Ok(KeyId::RightMeta),
            "RALT" | "RIGHTALT" => Ok(KeyId::RightAlt),
            "MENU" | "APP" => Ok(KeyId::Menu),
            "RCTRL" | "RIGHTCONTROL" | "RIGHTCTRL" => Ok(KeyId::RightControl),
            "FN" => Ok(KeyId::Fn),
            _ => Err(ParseKeyError(s.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mad60_layout_indices() {
        assert_eq!(find_key_matrix_index(KeyId::Esc), Some(0));
        assert_eq!(find_key_matrix_index(KeyId::Backspace), Some(13));
        assert_eq!(find_key_matrix_index(KeyId::Tab), Some(14));
        assert_eq!(find_key_matrix_index(KeyId::Q), Some(15));
        assert_eq!(find_key_matrix_index(KeyId::W), Some(16));
        assert_eq!(find_key_matrix_index(KeyId::E), Some(17));
        assert_eq!(find_key_matrix_index(KeyId::A), Some(29));
        assert_eq!(find_key_matrix_index(KeyId::S), Some(30));
        assert_eq!(find_key_matrix_index(KeyId::D), Some(31));
        assert_eq!(find_key_matrix_index(KeyId::Space), Some(62));
        assert_eq!(find_key_matrix_index(KeyId::Fn), Some(69));
    }

    #[test]
    fn test_matrix_length() {
        assert_eq!(MAD60_LAYOUT.len(), 70);
    }

    #[test]
    fn test_parse_keys() {
        assert_eq!("w".parse::<KeyId>().unwrap(), KeyId::W);
        assert_eq!("W".parse::<KeyId>().unwrap(), KeyId::W);
        assert_eq!("a".parse::<KeyId>().unwrap(), KeyId::A);
        assert_eq!("space".parse::<KeyId>().unwrap(), KeyId::Space);
        assert_eq!("lshift".parse::<KeyId>().unwrap(), KeyId::LeftShift);
        assert_eq!("Enter".parse::<KeyId>().unwrap(), KeyId::Enter);
        assert!("invalid_key".parse::<KeyId>().is_err());
    }

    #[test]
    fn test_find_by_index() {
        assert_eq!(find_key_by_matrix_index(0), Some(KeyId::Esc));
        assert_eq!(find_key_by_matrix_index(40), None);
        assert_eq!(find_key_by_matrix_index(43), None);
        assert_eq!(find_key_by_matrix_index(100), None);
    }

    #[test]
    fn test_every_key_has_index() {
        for (idx, slot) in MAD60_LAYOUT.iter().enumerate() {
            if let Some(key) = slot {
                assert_eq!(find_key_matrix_index(*key), Some(idx as u8));
            }
        }
    }

    #[test]
    fn test_parse_display_roundtrip() {
        let keys = [
            KeyId::Esc,
            KeyId::Key1,
            KeyId::Backspace,
            KeyId::Tab,
            KeyId::Q,
            KeyId::Enter,
            KeyId::LeftShift,
            KeyId::Space,
            KeyId::Fn,
        ];
        for k in keys {
            let s = format!("{k}");
            let parsed: KeyId = s.parse().unwrap();
            assert_eq!(parsed, k);
        }
    }
}
