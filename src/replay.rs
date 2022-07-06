use anyhow::Result;

use std::{
    intrinsics::transmute,
    io::{BufWriter, Read, Write},
};

use crate::game::{Config, Game, GameState};

#[derive(Copy, Clone, Debug)]
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

#[derive(Copy, Clone, Debug)]
pub struct Event {
    timestamp: u16,
    press: bool,
    input: Input,
}

#[derive(Debug)]
pub struct Replay {
    pub total_frames: u16,
    pub seed: u64,
    pub config: Config,
    pub events: Vec<Event>,
    current_frame: u16,
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
        let timestamp = (game.current_frame - self.current_frame) as u16;
        self.current_frame = game.current_frame;
        self.events.push(Event {
            timestamp,
            press,
            input,
        })
    }

    pub fn save<W: Write>(&self, w: W) -> Result<()> {
        let mut w = BufWriter::new(w);
        w.write_all(&self.total_frames.to_le_bytes())?;
        w.write_all(&self.seed.to_le_bytes())?;
        w.write_all(&self.config.gravity.to_le_bytes())?;
        w.write_all(&[self.config.soft_drop])?;
        w.write_all(&[self.config.das])?;
        w.write_all(&[self.config.arr | if self.config.ghost { 0x80 } else { 0 }])?;
        w.write_all(&[self.config.lock_delay.0])?;
        w.write_all(&self.config.lock_delay.1.to_le_bytes())?;
        w.write_all(&self.config.lock_delay.2.to_le_bytes())?;
        w.write_all(&self.events.len().to_le_bytes())?;
        for event in self.events.iter() {
            let input = (event.input as u8) << 4;
            if event.timestamp < 15 {
                w.write_all(&[input | event.timestamp as u8])?;
            } else {
                w.write_all(&[input | 0b1111])?;
                if event.timestamp < 128 {
                    w.write_all(&[event.timestamp as u8])?;
                } else {
                    let first = event.timestamp as u8 & 0x7F;
                    let second = (event.timestamp >> 7) as u8;
                    w.write_all(&[first, second])?;
                }
            }
        }
        Ok(w.flush()?)
    }

    // pub fn load<R: Read>(r: R) -> Result<Self> {
    //     let mut buf = [0u8; 8];
    //     let total_frames = r.read_exact(&mut buf);
    //     todo!()
    // }
}
