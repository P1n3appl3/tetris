use std::{
    collections::HashMap,
    io::{self, Read},
    str,
    sync::mpsc::{self, Receiver},
    thread,
};

use anyhow::{anyhow, Result};

use tetris::{settings::keys::*, InputEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyEvent {
    pub key: char,
    pub mods: u8,
    pub press: bool,
}

impl From<(char, u8, bool)> for KeyEvent {
    fn from((key, mods, press): (char, u8, bool)) -> Self {
        KeyEvent { key, mods, press }
    }
}

pub struct EventLoop {
    pub events: Receiver<InputEvent>,
}

impl EventLoop {
    pub fn start(bindings: tetris::Bindings) -> Self {
        let (tx, rx) = mpsc::channel();

        thread::spawn(move || {
            let mut stdin = io::stdin().lock();
            use InputEvent::*;
            let keymap = [
                ((bindings.left, 0, true), PressLeft),
                ((bindings.left, 0, false), ReleaseLeft),
                ((bindings.right, 0, true), PressRight),
                ((bindings.right, 0, false), ReleaseRight),
                ((bindings.soft, 0, true), PressSoft),
                ((bindings.soft, 0, false), ReleaseSoft),
                ((bindings.hard, 0, true), Hard),
                ((bindings.cw, 0, true), Cw),
                ((bindings.ccw, 0, true), Ccw),
                ((bindings.flip, 0, true), Flip),
                ((bindings.hold, 0, true), Hold),
                (('r', 0, true), Restart),
                (('q', 0, true), Quit),
                (('c', CTRL, true), Quit),
                // (('u', 0, true), Undo),
                // (('r', CTRL, true), Redo),
            ]
            .into_iter()
            .map(|(k, i)| (k.into(), i))
            .collect::<HashMap<KeyEvent, InputEvent>>();
            let mut buf = [0; 64];
            loop {
                let n = stdin.read(&mut buf).unwrap();
                if let Ok(k) = crate::input::parse_kitty_key(&buf[..n]) {
                    if let Some(&ev) = keymap.get(&k) {
                        tx.send(ev).unwrap();
                    }
                }
            }
        });
        Self { events: rx }
    }
}

fn trailer_map(c: u8) -> char {
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

// https://sw.kovidgoyal.net/kitty/keyboard-protocol#detection-of-support-for-this-protocol
fn parse_kitty_key(buf: &[u8]) -> Result<KeyEvent> {
    assert!(buf.starts_with(b"\x1b["));
    let trailer = *buf.last().unwrap();
    assert!(b"ABCDEFHPQRSu".contains(&trailer));
    let s = str::from_utf8(&buf[2..buf.len() - 1]).unwrap();
    let parts: Vec<Vec<u32>> = s
        .split(';')
        .map(|s| s.split(':').map(|s| s.parse().unwrap_or_default()).collect())
        .collect();
    let code = if trailer == b'u' {
        char::from_u32(parts[0][0]).unwrap()
    } else {
        trailer_map(trailer)
    };
    let (mods, press) = if let Some(v) = parts.get(1) {
        match v[..] {
            [a] | [a, 1] => (a - 1, true),
            [a, 3] => (a - 1, false),
            [_, 2] => return Err(anyhow!("ignore repeats")),
            _ => return Err(anyhow!("unrecognized")),
        }
    } else {
        (0, true)
    };
    Ok(KeyEvent { key: code, mods: mods as u8, press })
}
