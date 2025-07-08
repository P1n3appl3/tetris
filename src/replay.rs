use std::{
    fs,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

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

    pub fn save(&mut self) {
        let time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_secs();
        let path = format!("replays/{time}.json");
        let raw_replay =
            serde_json::to_string_pretty(&self).expect("Failed to serialize replay");
        fs::write(&path, &raw_replay).expect("Failed to write replay");
        log::info!("Replay saved to {path:?}");

        // does it round-trip?
        let raw_replay = fs::read_to_string(&path).expect("Failed to read replay");
        let round_trip: Replay =
            serde_json::from_str(&raw_replay).expect("Failed to deserialize replay");
        self.last = None;
        debug_assert_eq!(*self, round_trip);
    }
}
