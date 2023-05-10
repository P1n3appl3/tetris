mod game;
mod graphics;
mod input;
mod keys;
// mod replay;
mod settings;
mod sound;

use std::{
    fs::File,
    path::Path,
    sync::mpsc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use game::{Event, Game, GameState, InputEvent};
use graphics::RawMode;
use input::EventLoop;
use keys::*;
use log::{error, info};
use rand::prelude::*;
use replay::Replay;
use sound::Player;

fn main() {
    // TODO: on first run create config file and print help, maybe open config in editor?
    // TODO: error on failed kitty input mode detection, print list of links (wez/kitty/alacritty/ghost/foot/iterm2/rio)
    ftail::Ftail::new()
        .single_file(Path::new("log.txt"), true, log::LevelFilter::Debug)
        .init()
        .ok();
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
    let replay = Replay::new(game.config, seed);

    graphics::draw(width as i16, height as i16, game).unwrap();

    let done = loop {
        use mpsc::RecvTimeoutError::*;
        let now = Instant::now();
        let deadline = game
            .timers
            .front()
            .map(|&(t, _)| t)
            .unwrap_or(now + Duration::from_millis(100));
        match input.events.recv_timeout(deadline - now) {
            Ok(InputEvent::Restart) => break true,
            Ok(InputEvent::Quit) => break false,
            Ok(input_event) => {
                game.handle(Event::Input(input_event), Instant::now(), player)
            }
            Err(Timeout) => {
                if let Some((t, timer_event)) = game.timers.pop_front() {
                    info!("{:?}, {timer_event:?}", deadline - Instant::now());
                    game.handle(Event::Timer(timer_event), Instant::now(), player);
                    debug_assert!(t < Instant::now());
                }
            }
            Err(Disconnected) => {
                error!("input thread died unexpectedly");
                break false;
            }
        }

        // TODO: draw every tenth of a second in a separate loop, checking a "paused" atomic bool
        // that's set by this thread based on gamestate
        graphics::draw(width as i16, height as i16, game).unwrap();
    };

    if game.state == GameState::Done {
        let time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_secs();
        {
            let replayfile = File::create(format!("replays/{time}.bin")).unwrap();
            replay.save(replayfile).unwrap();
        }
        // does it round-trip?
        let replayfile = File::open(format!("replays/{time}.bin")).unwrap();
        debug_assert_eq!(replay, Replay::load(replayfile).unwrap());
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
    let mut size = libc::winsize { ws_row: 0, ws_col: 0, ws_xpixel: 0, ws_ypixel: 0 };
    unsafe { libc::ioctl(1, libc::TIOCGWINSZ, &mut size) };
    (size.ws_col, size.ws_row)
}
