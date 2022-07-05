use crate::keys::*;
use anyhow::{anyhow, Result};
use nix::sys::signal::{self, SaFlags, SigAction, SigHandler, SigSet};
use once_cell::sync::OnceCell;

use std::{
    io::{self, Read},
    slice, str,
    sync::mpsc::{self, Receiver, SyncSender},
    thread,
};

#[derive(Debug, Clone, Copy)]
pub struct KeyEvent(pub char, pub u8, pub bool);

#[derive(Debug, Clone)]
pub enum Event {
    Quit,
    Restart,
    Resize,
    Key(KeyEvent),
}

impl EventLoop {
    pub fn start() -> Self {
        let (tx, rx) = mpsc::sync_channel(32);
        register_signal_handler(tx.clone());

        thread::spawn(move || {
            let mut stdin = io::stdin().lock();
            let mut buf = [0; 64];
            loop {
                let n = stdin.read(&mut buf).unwrap();
                // eprintln!("{n}");
                if let Ok(k) = parse_kitty_key(unsafe { slice::from_raw_parts(buf.as_ptr(), n) }) {
                    tx.send(match k {
                        KeyEvent('q', ..) | KeyEvent('c', CTRL, ..) => Event::Quit,
                        KeyEvent('r', _, true) => Event::Restart,
                        _ => Event::Key(k),
                    })
                    .unwrap();
                }
            }
        });

        Self { events: rx }
    }
}

pub struct EventLoop {
    pub events: Receiver<Event>,
}

static SIGTX: OnceCell<SyncSender<Event>> = OnceCell::new();

// TODO: figure out why i'm not seeing signals on window size change
fn register_signal_handler(tx: SyncSender<Event>) {
    SIGTX.set(tx).unwrap();
    extern "C" fn handler(_n: i32) {
        // eprintln!("signal: {n}");
        SIGTX.get().unwrap().send(Event::Resize).unwrap();
    }
    let mut signals = SigSet::empty();
    signals.add(signal::SIGWINCH);
    let sig_action = SigAction::new(SigHandler::Handler(handler), SaFlags::empty(), signals);
    unsafe {
        signal::sigaction(signal::SIGINT, &sig_action).unwrap();
    }
}

fn parse_kitty_key(buf: &[u8]) -> Result<KeyEvent> {
    assert!(buf.starts_with(b"\x1b["));
    let trailer = *buf.last().unwrap();
    assert!(b"ABCDEFHPQRSu".contains(&trailer));
    let s = str::from_utf8(&buf[2..buf.len() - 1]).unwrap();
    let parts: Vec<Vec<u32>> = s
        .split(';')
        .map(|s| {
            s.split(':')
                .map(|s| s.parse().unwrap_or_default())
                .collect()
        })
        .collect();
    let code = if trailer == b'u' {
        char::from_u32(parts[0][0]).unwrap()
    } else {
        trailer_map(trailer)
    };
    let (mods, release) = if let Some(v) = parts.get(1) {
        match v[..] {
            [a] | [a, 1] => (a - 1, true),
            [a, 3] => (a - 1, false),
            [_, 2] => return Err(anyhow!("ignore repeats")),
            _ => return Err(anyhow!("unrecognized")),
        }
    } else {
        (0, true)
    };
    Ok(KeyEvent(code, mods as u8, release))
}
