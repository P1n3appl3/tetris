#![feature(deadline_api)]
mod game;
mod graphics;
mod input;
mod keys;
mod replay;
mod settings;
mod sound;

use std::{sync::mpsc, time::Instant};

use game::{Event, Game, GameState, InputEvent};
use graphics::RawMode;
use input::EventLoop;
use keys::*;
use rand::prelude::*;
// use replay::{Input, Replay};
use sound::Player;

fn main() {
    // on first run create config file and pring help, maybe open config in editor?
    // error on failed kitty input mode, foot/wezterm/kitty only
    let _mode = RawMode::enter();
    let (config, keys, player) = settings::load().unwrap();
    let input = EventLoop::start(keys);
    let mut game = Game::new(config);
    while run_game(&mut game, &input, &player) {}
}

fn run_game(game: &mut Game, input: &EventLoop, player: &Player) -> bool {
    let (width, height) = get_size();
    if width < 40 || height < 22 {
        panic!("screen too small");
    }

    let seed = thread_rng().gen();
    game.start(seed, player);
    // let mut replay = Replay::new(game.config, seed);

    let done = loop {
        use mpsc::RecvTimeoutError::*;
        let &(deadline, timer_event) = game.timers.front().expect("no timers");
        match input.events.recv_deadline(deadline) {
            Ok(InputEvent::Restart) => break true,
            Ok(InputEvent::Quit) => break false,
            // TODO: busy wait with try_recv until the iteration where the current instant is
            // actually past the deadline (or within some bound of it) to increase accuracy of
            // timer events
            Ok(input_event) => game.handle(Event::Input(input_event), Instant::now(), player),
            Err(Timeout) => game.handle(Event::Timer(timer_event), Instant::now(), player),
            Err(Disconnected) => panic!("input thread died unexpectedly"),
        }

        graphics::draw(width as i16, height as i16, game).unwrap();
    };

    if game.state == GameState::Done {
        // let time = SystemTime::now()
        //     .duration_since(UNIX_EPOCH)
        //     .expect("time went backwards")
        //     .as_secs();
        // replay.total_frames = game.current_frame - 120;
        // {
        //     let replayfile =
        // File::create(format!("replays/{time}.bin")).unwrap();
        //     replay.save(replayfile).unwrap();
        // }
        // // does it round-trip?
        // replay.current_frame = 0;
        // let replayfile = File::open(format!("replays/{time}.bin")).unwrap();
        // assert_eq!(replay, Replay::load(replayfile).unwrap());
    }
    done
}

#[derive(Copy, Clone, Debug)]
pub struct Bindings {
    pub left: char,
    pub right: char,
    pub soft: char,
    pub hard: char,
    pub cw: char,
    pub ccw: char,
    pub flip: char,
    pub hold: char,
}

impl Default for Bindings {
    fn default() -> Self {
        Self {
            left: LEFT,
            right: RIGHT,
            soft: DOWN,
            hard: UP,
            cw: 'x',
            ccw: 'z',
            flip: 'a',
            hold: LEFT_SHIFT,
        }
    }
}

fn get_size() -> (u16, u16) {
    let mut size = libc::winsize {
        ws_row: 0,
        ws_col: 0,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    unsafe { libc::ioctl(1, libc::TIOCGWINSZ, &mut size) };
    (size.ws_col, size.ws_row)
}
