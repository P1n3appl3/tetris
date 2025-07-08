mod graphics;
mod input;

use std::{
    fs::{self},
    path::Path,
    sync::mpsc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use graphics::RawMode;
use input::EventLoop;
use log::{error, info};
use rand::prelude::*;
use tetris::{
    replay::Replay,
    sound::{Player, RodioPlayer},
    Event, Game, GameState, InputEvent,
};

fn main() {
    // TODO: on first run create config file and print help, maybe open config in editor?
    // TODO: add mode to generate bindings for config with input prompts accepting raw mode keypresses
    // TODO: error on failed kitty input mode detection, print list of links (wez/kitty/alacritty/ghost/foot/iterm2/rio)
    ftail::Ftail::new()
        .single_file(Path::new("log.txt"), true, log::LevelFilter::Debug)
        .init()
        .ok();
    log_panics::init();
    // todo: use dir-rs/dirs/xdg for config dir
    let settings =
        fs::read_to_string("assets/settings.kdl").expect("Couldn't find settings file");
    let mut player = RodioPlayer::new().expect("Failed to initialize audio engine");
    let (config, keys) =
        tetris::settings::load(&settings, &mut player).expect("Invalid settings file");
    let _mode = RawMode::enter();
    let input = EventLoop::start(keys);
    let mut game = Game::new(config);
    while run_game(&mut game, &input, &player) {}
}

fn run_game(game: &mut Game, input: &EventLoop, player: &impl Player) -> bool {
    let (width, height) = get_size();
    if width < 40 || height < 22 {
        panic!("screen too small");
    }

    let seed = rand::rng().random();
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
                info!("{input_event:?}");
                game.handle(Event::Input(input_event), Instant::now(), player)
            }
            Err(Timeout) => {
                if let Some((t, timer_event)) = game.timers.pop_front() {
                    info!("{timer_event:?}");
                    game.handle(Event::Timer(timer_event), Instant::now(), player);
                    debug_assert!(t < Instant::now());
                }
            }
            Err(Disconnected) => {
                error!("input thread died unexpectedly");
                break false;
            }
        }

        // TODO: draw timers every tenth of a second in a separate loop, checking a "paused"
        // atomic bool that's set by this thread based on gamestate
        graphics::draw(width as i16, height as i16, game).unwrap();
    };

    if game.state == GameState::Done {
        let time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_secs();
        let path = format!("replays/{time}.bin");
        let raw_replay =
            serde_json::to_string_pretty(&replay).expect("Failed to serialize replay");
        fs::write(&path, &raw_replay).expect("Failed to write replay");
        // does it round-trip?
        let raw_replay = fs::read_to_string(&path).expect("Failed to read replay");
        let round_trip =
            serde_json::from_str(&raw_replay).expect("Failed to deserialize replay");
        debug_assert_eq!(replay, round_trip);
    }
    done
}

fn get_size() -> (u16, u16) {
    let mut size = libc::winsize { ws_row: 0, ws_col: 0, ws_xpixel: 0, ws_ypixel: 0 };
    unsafe { libc::ioctl(1, libc::TIOCGWINSZ, &mut size) };
    (size.ws_col, size.ws_row)
}
