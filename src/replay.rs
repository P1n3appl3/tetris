use anyhow::Result;
use byteorder::{ReadBytesExt, WriteBytesExt, LE};

use std::{
    intrinsics::transmute,
    io::{BufReader, BufWriter, Read, Write},
};

use crate::game::{Config, Game, GameState};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Input {
    Left,
    Right,
    Soft,
    Cw,
    Ccw,
    Hard,
    Hold,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Event {
    elapsed: u16,
    press: bool,
    input: Input,
}

// TODO: write a PRNG compatible with the one jstris uses so that seeds can be shared
// harddrop.com/forums/index.php%3Fs=&showtopic=7087&view=findpost&p=92057
// also rework soft-drop config to match jstris's
#[derive(Debug, PartialEq, Eq)]
pub struct Replay {
    pub total_frames: u16,
    pub seed: u64,
    pub config: Config,
    pub events: Vec<Event>,
    pub current_frame: u16,
}

impl Replay {
    pub fn new(config: Config, seed: u64) -> Self {
        Self {
            total_frames: 0,
            current_frame: 0,
            seed,
            config,
            events: Default::default(),
        }
    }

    pub fn push(&mut self, game: &Game, input: Input, press: bool) {
        if matches!(game.state, GameState::Done | GameState::Lost) {
            return;
        }
        let elapsed = game.current_frame - self.current_frame;
        self.current_frame = game.current_frame;
        self.events.push(Event {
            elapsed,
            press,
            input,
        })
    }

    pub fn save<W: Write>(&self, w: W) -> Result<()> {
        let mut w = BufWriter::new(w);
        w.write_u16::<LE>(self.total_frames)?;
        w.write_u64::<LE>(self.seed)?;
        w.write_u16::<LE>(self.config.gravity)?;
        w.write_u8(self.config.soft_drop)?;
        w.write_u8(self.config.das)?;
        w.write_u8(self.config.arr | if self.config.ghost { 0x80 } else { 0 })?;
        w.write_u8(self.config.lock_delay.0)?;
        w.write_u16::<LE>(self.config.lock_delay.1)?;
        w.write_u16::<LE>(self.config.lock_delay.2)?;
        w.write_u16::<LE>(self.events.len() as u16)?;
        for event in self.events.iter() {
            let input = (event.input as u8) << 4 | (event.press as u8) << 7;
            if event.elapsed < 15 {
                w.write_u8(input | event.elapsed as u8)?;
            } else {
                w.write_u8(input | 0b1111)?;
                if event.elapsed < 128 {
                    w.write_u8(event.elapsed as u8)?;
                } else {
                    let first = (event.elapsed | 0x80) as u8;
                    let second = (event.elapsed >> 7) as u8;
                    w.write_all(&[first, second])?;
                }
            }
        }
        Ok(w.flush()?)
    }

    pub fn load<R: Read>(r: R) -> Result<Self> {
        let mut r = BufReader::new(r);
        let total_frames = r.read_u16::<LE>()?;
        let seed = r.read_u64::<LE>()?;
        let gravity = r.read_u16::<LE>()?;
        let soft_drop = r.read_u8()?;
        let das = r.read_u8()?;
        let arr = r.read_u8()?;
        let ghost = arr & 0x80 != 0;
        let lock = r.read_u8()?;
        let extended = r.read_u16::<LE>()?;
        let timeout = r.read_u16::<LE>()?;
        let config = Config {
            das,
            gravity,
            soft_drop,
            ghost,
            arr: arr & 0x7F,
            lock_delay: (lock, extended, timeout),
        };
        let num_events = r.read_u16::<LE>()?;
        let mut events = Vec::with_capacity(num_events as usize);
        for _ in 0..num_events {
            let byte = r.read_u8()?;
            let input = unsafe { transmute(byte >> 4 & 0b111) };
            let press = byte & 0x80 != 0;
            let time = byte & 0b1111;
            let elapsed = if time != 15 {
                time as u16
            } else {
                let time = r.read_u8()?;
                if time < 128 {
                    time as u16
                } else {
                    let extra = r.read_u8()?;
                    (extra as u16) << 7 | (time as u16 & 0x7f)
                }
            };
            events.push(Event {
                elapsed,
                press,
                input,
            })
        }
        assert!(r.bytes().next().is_none());

        Ok(Self {
            total_frames,
            seed,
            config,
            events,
            current_frame: 0,
        })
    }
}
