#![allow(unused)]
#![cfg_attr(rustfmt, rustfmt_skip)]
pub const SHIFT:     u8 = 0b1;
pub const ALT:       u8 = 0b10;
pub const CTRL:      u8 = 0b100;
pub const SUPER:     u8 = 0b1000;
pub const HYPER:     u8 = 0b10000;
pub const META:      u8 = 0b100000;
pub const CAPS_LOCK: u8 = 0b1000000;
pub const NUM_LOCK:  u8 = 0b10000000;

pub const LEFT:       char = '\u{e006}';
pub const RIGHT:      char = '\u{e007}';
pub const UP:         char = '\u{e008}';
pub const DOWN:       char = '\u{e009}';
pub const LEFT_SHIFT: char = '\u{e061}';

pub fn get_key(name: &str) -> Option<char> {
    Some(match name.to_lowercase().as_str() {
        "left" => LEFT,
        "right" => RIGHT,
        "up" => UP,
        "down" => DOWN,
        "shift" => LEFT_SHIFT,
        "space" => ' ',
        _ => return None,
    })
}

pub fn trailer_map(c: u8) -> char {
    match c {
        b'A' => UP,
        b'B' => DOWN,
        b'C' => RIGHT,
        b'D' => LEFT,
        b'E' => '\u{e053}',
        b'F' => '\u{e00d}',
        b'H' => '\u{e00c}',
        b'P' => '\u{e014}',
        b'Q' => '\u{e015}',
        b'R' => '\u{e001}',
        b'S' => '\u{e003}',
        _ => unreachable!(),
    }
}
