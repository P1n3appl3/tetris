use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::{Config, InputEvent};

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
    // for full roundtrippable replays
    #[serde(skip)]
    pub last: Option<Instant>,
}

impl Replay {
    pub fn new(config: Config, seed: u64) -> Self {
        Self { seed, config, events: Default::default(), length: 0, last: None }
    }

    pub fn start(&mut self) {
        self.last = Some(Instant::now());
    }

    pub fn push(&mut self, input: InputEvent, t: Instant) {
        let elapsed = (t - self.last.unwrap()).as_millis() as u16;
        self.last = Some(t);
        self.events.push(ReplayEvent { elapsed, input })
    }
}
