use std::{
    io::{BufReader, BufWriter, Read, Write},
    mem::transmute,
};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::game::{Config, Event, Game, GameState, InputEvent};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayEvent {
    elapsed: u16, // millis
    input: InputEvent,
}

// TODO: write a PRNG compatible with the one jstris uses so that seeds can be
// shared harddrop.com/forums/index.php%3Fs=&showtopic=7087&view=findpost&
// p=92057 also rework soft-drop config to match jstris's
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Replay {
    pub length: u32, // millis
    pub seed: u64,
    pub config: Config,
    pub events: Vec<ReplayEvent>,
    // TODO: include optional sounds and skin (by link or directly encoded?)
    // for portable 1:1 viewing
}

impl Replay {
    pub fn new(config: Config, seed: u64) -> Self {
        Self { seed, config, events: Default::default(), length: 0 }
    }

    pub fn push(&mut self, input: InputEvent, elapsed: u16) {
        self.events.push(ReplayEvent { elapsed, input })
    }
}
