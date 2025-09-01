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
    builder::{styling::AnsiColor::*, Styles},
    Parser,
};
use directories::ProjectDirs;
use graphics::RawMode;
use input::EventLoop;
use log::{debug, error, LevelFilter};
use rand::prelude::*;
use tetris::{
    replay::Replay,
    sound::{Sink, SoundPlayer},
    Event, Game, GameState, InputEvent, Mode,
};
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
    let mut player = sound::Rodio::new().expect("Failed to initialize audio engine").into();
    let (config, keys) =
        settings::load(args.config.as_deref(), &dirs, &mut player).expect("Invalid settings file");
    let _mode = RawMode::enter();
    let input = EventLoop::start(keys);
    let mut game = Game::new(config);
    game.mode = if args.practice {
        Mode::TrainingLab { search: true, lookahead: None }
    } else {
        Mode::Sprint { target_lines: args.lines.map(u16::from).unwrap_or(40) }
    };
    let replay_dir = args.replay_dir.unwrap_or_else(|| {
        let d = dirs.data_dir().join("replays");
        fs::create_dir_all(&d).ok();
        d
    });
    while run_game(&mut game, &input, &player, &replay_dir) {}
}

// plan for how to integrate tetrizz search algorithm
//
// 1. call the algorithm once per placement
//      a. for now i'll probably just do this in the same thread and ignore the delays that it
//      introduces, but long term i'll want to background this and redraw the current state of the
//      search results as they update
// 2. apply some manual rule to filter through the options (e.g. only show z spins)
//      a. ideally this should be applied in the search algorithm itself to focus the beam search
//      b. or we might move away from beam search as an approach
// 3. display the options, either rendered in the UI or printed to the log

fn run_game(
    game: &mut Game,
    input: &EventLoop,
    player: &SoundPlayer<impl Sink>,
    replay_dir: &Path,
) -> bool {
    let eval = &tetrizz::eval::Eval::new(
        -79.400375,
        -55.564907,
        -125.680145,
        -170.41902,
        10.167948,
        -172.78625,
        -478.7291,
        86.84883,
        368.89203,
        272.57874,
        28.938646,
        -104.59018,
        -496.8832,
        458.29822,
    );
    let (width, height) = get_size();
    if width < 40 || height < 22 {
        panic!("screen too small");
    }

    let seed = rand::rng().random();
    let mut replay = Replay::new(game.config, seed);
    game.start(Some(seed), player);
    replay.start();

    graphics::draw(width as i16, height as i16, game).unwrap();
    let mut new_piece = false;

    let done = loop {
        if game.mode.search_enabled() && new_piece {
            // call search algorithm
            log::info!("upcoming: {:?}", game.upcomming);
            log::info!("hold: {:?}", game.hold);
            log::info!("current: {:?}", game.current);
            let (tetrizz_game, queue) = game.as_tetrizz_game_and_queue();
            log::info!("queue: {queue:?}");
            log::info!("hold: {:?}", tetrizz_game.hold);
            let search_loc = tetrizz::movegen::movegen(&tetrizz_game, queue[0]);
            let heap = tetrizz::beam_search::search_results(
                &tetrizz_game,
                &search_loc,
                queue,
                eval,
                7,
                3000,
            );
            let mut spins = vec![];
            for node in heap.iter() {
                for (m, placement_info) in node.moves.iter() {
                    if placement_info.lines_cleared > 0 {
                        if m.spun {
                            spins.push(node.clone());
                        }
                        break;
                    }
                }
            }
            spins.sort_by_key(|s| s.score);
            spins.sort_by_key(|s| s.moves.iter().take_while(|m| !m.1.spin).count());
            game.spins = spins;
            new_piece = false;
        }
        use mpsc::RecvTimeoutError::*;
        use GameState::*;
        use InputEvent::*;
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
                    new_piece |= game.handle(Event::Input(input_event), t, player)
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
                        new_piece |= game.handle(Event::Timer(timer_event), now, player);
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

    if game.state == GameState::Done && game.mode.is_complete(game.lines) {
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
