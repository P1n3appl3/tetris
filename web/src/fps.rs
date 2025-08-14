use std::{collections::VecDeque, time::Duration};

use web_time::Instant;

#[derive(Debug)]
pub struct FPSCounter {
    past_frames: VecDeque<Instant>,
}

impl FPSCounter {
    pub fn new() -> FPSCounter {
        FPSCounter { past_frames: VecDeque::with_capacity(128) }
    }

    pub fn tick(&mut self, now: Instant) -> usize {
        while self.past_frames.front().is_some_and(|t| *t + Duration::from_secs(1) < now) {
            self.past_frames.pop_front();
        }

        self.past_frames.push_back(now);
        self.past_frames.len()
    }
}
