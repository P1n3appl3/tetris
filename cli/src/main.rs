mod graphics;
mod input;
mod settings;
mod sound;

use std::{
    fs,
    num::NonZeroU16,
    path::{Path, PathBuf},
    sync::mpsc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use clap::{
    Parser,
    builder::{Styles, styling::AnsiColor::*},
};
use directories::ProjectDirs;
use graphics::RawMode;
use input::EventLoop;
use log::{LevelFilter, debug, error};
use rand::prelude::*;
use tetris::{Event, Game, GameState, InputEvent, replay::Replay, sound::Sink};
use web_time::Instant;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about, long_about = None, styles = STYLES)]
struct Args {
    /// Set the number of lines
    #[arg(short, long)]
    lines: Option<NonZeroU16>,

    /// Enable practice mode (no line count)
    #[arg(short, long)]
    practice: bool,

    /// Path to settings file
    config: Option<PathBuf>,

    /// Where to output logs
    #[arg(short = 'o', long)]
    log_file: Option<PathBuf>,

    /// Where to output replays
    #[arg(short, long)]
    replay_dir: Option<PathBuf>,

    /// Include more log output
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
}

fn main() {
    // TODO: on first run create config file and print help, maybe open config in editor?
    // TODO: add mode to generate bindings for config with input prompts accepting raw mode keypresses
    // TODO: error on failed kitty input mode detection, print list of links (wez/kitty/alacritty/ghost/foot/iterm2/rio)
    // TODO: panic hook to exit raw mode first so backtraces actually print

    let args = Args::parse();
    let dirs = ProjectDirs::from("", "", "tetris").unwrap();
    let log_file = args.log_file.unwrap_or_else(|| {
        let d = dirs.data_dir();
        fs::create_dir_all(d).ok();
        d.join("log.txt")
    });
    let level = LevelFilter::iter().nth(1 + args.verbose as usize).unwrap_or(LevelFilter::max());
    ftail::Ftail::new().single_file(&log_file, true, level).init().ok();
    log_panics::init();
    let settings = if let Some(path) = args.config {
        fs::read_to_string(path).expect("Couldn't read settings file")
    } else {
        let default_settings_content = include_str!("../settings.kdl");
        fs::create_dir_all(dirs.config_dir()).ok();
        let settings_path = dirs.config_dir().join("settings.kdl");
        match fs::read_to_string(&settings_path) {
            Ok(s) => s,
            Err(_) => {
                fs::write(settings_path, default_settings_content).ok();
                default_settings_content.to_owned()
            }
        }
    };
    let mut player = sound::Rodio::new(&dirs).expect("Failed to initialize audio engine");
    let (config, keys) = settings::load(&settings, &mut player).expect("Invalid settings file");
    return;
    let _mode = RawMode::enter();
    let input = EventLoop::start(keys);
    let mut game = Game::new(config);
    game.target_lines =
        if args.practice { None } else { Some(args.lines.map(u16::from).unwrap_or(40)) };
    let replay_dir = args.replay_dir.unwrap_or_else(|| {
        let d = dirs.data_dir().join("replays");
        fs::create_dir_all(&d).ok();
        d
    });
    while run_game(&mut game, &input, &player, &replay_dir) {}
}

fn run_game(game: &mut Game, input: &EventLoop, player: &impl Sink, replay_dir: &Path) -> bool {
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
        let redraw_timeout = Duration::from_millis(if game.state == Done { 10000 } else { 100 });
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
                        game.handle(Event::Timer(timer_event), now, player);
                    }
                }
            }
            Err(Disconnected) => {
                error!("input thread died unexpectedly");
                break false;
            }
        }

        // TODO: draw timers every tenth of a second in a separate loop, checking a
        // "paused" atomic bool that's set by this thread based on gamestate
        graphics::draw(width as i16, height as i16, game).unwrap();
    };

    if game.state == GameState::Done
        && let Some(target) = game.target_lines
        && game.lines >= target
    {
        replay.length = (game.end_time.unwrap() - game.start_time.unwrap()).as_millis() as u32;
        save_replay(&mut replay, replay_dir);
    }
    done
}

// TODO: update on sigwinch
fn get_size() -> (u16, u16) {
    let mut size = libc::winsize { ws_row: 0, ws_col: 0, ws_xpixel: 0, ws_ypixel: 0 };
    unsafe { libc::ioctl(1, libc::TIOCGWINSZ, &mut size) };
    (size.ws_col, size.ws_row)
}

fn save_replay(replay: &mut Replay, dir: &Path) {
    let time = SystemTime::now().duration_since(UNIX_EPOCH).expect("time went backwards").as_secs();
    let path = dir.join(format!("{time}.json"));
    let raw_replay = serde_json::to_string_pretty(replay).expect("Failed to serialize replay");
    fs::write(&path, &raw_replay).expect("Failed to write replay");
    log::info!("Replay saved to {path:?}");

    // does it round-trip?
    let raw_replay = fs::read_to_string(&path).expect("Failed to read replay");
    let round_trip: Replay =
        serde_json::from_str(&raw_replay).expect("Failed to deserialize replay");
    replay.last = None;
    debug_assert_eq!(*replay, round_trip);
}

const STYLES: Styles =
    Styles::styled().literal(Cyan.on_default().bold()).placeholder(Blue.on_default());
