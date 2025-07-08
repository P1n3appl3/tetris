mod graphics;
mod input;

use std::{
    fs::{self},
    num::NonZeroU16,
    path::{Path, PathBuf},
    sync::mpsc,
    time::{Duration, Instant},
};

use clap::Parser;
use graphics::RawMode;
use input::EventLoop;
use log::{debug, error};
use rand::prelude::*;

use tetris::{
    Event, Game, GameState, InputEvent,
    replay::Replay,
    sound::{Player, RodioPlayer},
};

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// set the number of lines
    #[arg(short, long)]
    lines: Option<NonZeroU16>,

    /// enable practice mode (no line count)
    #[arg(short, long)]
    practice: bool,

    // /// cheese race mode
    // #[arg(short, long)]
    // cheese: bool,
    /// path to settings file
    config: Option<PathBuf>,

    /// where to output logs (/tmp/tetris.log by default)
    #[arg(short, long)]
    log_file: Option<PathBuf>,

    /// where to output replays
    replay_dir: Option<PathBuf>,
}

fn main() {
    // TODO: on first run create config file and print help, maybe open config in editor?
    // TODO: add mode to generate bindings for config with input prompts accepting raw mode keypresses
    // TODO: error on failed kitty input mode detection, print list of links (wez/kitty/alacritty/ghost/foot/iterm2/rio)
    // TODO: panic hook to exit raw mode first so backtraces actually print

    let args = Args::parse();
    let log_file = args.log_file.as_deref().unwrap_or(Path::new("/tmp/tetris.log"));
    ftail::Ftail::new().single_file(log_file, true, log::LevelFilter::Debug).init().ok();
    log_panics::init();
    // todo: use dir-rs/dirs/xdg for config dir
    let settings_file = args.config.as_deref().unwrap_or(Path::new("assets/settings.kdl"));
    let settings = fs::read_to_string(settings_file).expect("Couldn't find settings file");
    let mut player = RodioPlayer::new().expect("Failed to initialize audio engine");
    let (config, keys) =
        tetris::settings::load(&settings, &mut player).expect("Invalid settings file");
    let _mode = RawMode::enter();
    let input = EventLoop::start(keys);
    let mut game = Game::new(config);
    game.target_lines =
        if args.practice { None } else { Some(args.lines.map(u16::from).unwrap_or(40)) };
    while run_game(&mut game, &input, &player) {}
}

fn run_game(game: &mut Game, input: &EventLoop, player: &impl Player) -> bool {
    let (width, height) = get_size();
    if width < 40 || height < 22 {
        panic!("screen too small");
    }

    let seed = rand::rng().random();
    let mut replay = Replay::new(game.config, seed);
    game.start(seed, player);
    replay.start();

    graphics::draw(width as i16, height as i16, game).unwrap();

    let done = loop {
        use GameState::*;
        use InputEvent::*;
        use mpsc::RecvTimeoutError::*;
        let now = Instant::now();
        let redraw_timeout =
            Duration::from_millis(if game.state == Done { 10000 } else { 100 });
        let deadline = game
            .timers
            .front()
            .map(|&(t, _)| t)
            .unwrap_or(now + redraw_timeout)
            .min(now + redraw_timeout);
        match input.events.recv_timeout(deadline - now) {
            Ok(Restart) => break true,
            Ok(Quit) => break false,
            Ok(input_event) => {
                if game.state == Running
                    || game.state == Startup
                        && matches!(
                            input_event,
                            PressLeft | PressRight | ReleaseLeft | ReleaseRight
                        )
                {
                    debug!(target: "input", "{input_event:?}");
                    let t = Instant::now();
                    replay.push(input_event, t);
                    game.handle(Event::Input(input_event), t, player);
                }
            }
            Err(Timeout) => {
                if game.state != Done
                    && let Some(&(t, timer_event)) = game.timers.front()
                {
                    let now = Instant::now();
                    if t < now {
                        game.timers.pop_front();
                        debug!(target: "timer","{timer_event:?}");
                        game.handle(Event::Timer(timer_event), Instant::now(), player);
                    }
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

    if game.state == GameState::Done
        && let Some(target) = game.target_lines
        && game.lines >= target
    {
        replay.length =
            (game.end_time.unwrap() - game.start_time.unwrap()).as_millis() as u32;
        replay.save();
    }
    done
}

fn get_size() -> (u16, u16) {
    let mut size = libc::winsize { ws_row: 0, ws_col: 0, ws_xpixel: 0, ws_ypixel: 0 };
    unsafe { libc::ioctl(1, libc::TIOCGWINSZ, &mut size) };
    (size.ws_col, size.ws_row)
}
