use std::{collections::VecDeque, time::Duration};

use web_time::Instant;

#[derive(Debug)]
pub struct FPSCounter {
    last_second_frames: VecDeque<Instant>,
}

impl FPSCounter {
    pub fn new() -> FPSCounter {
        FPSCounter { last_second_frames: VecDeque::with_capacity(128) }
    }

    pub fn tick(&mut self) -> usize {
        let now = Instant::now();
        while self.last_second_frames.front().is_some_and(|t| *t + Duration::from_secs(1) < now) {
            self.last_second_frames.pop_front();
        }

        self.last_second_frames.push_back(now);
        self.last_second_frames.len()
    }
}
