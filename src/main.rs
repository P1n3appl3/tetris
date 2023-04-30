mod game;
mod graphics;
mod input;
mod keys;
mod replay;
mod settings;
mod sound;

use std::{
    fs::File,
    time::{SystemTime, UNIX_EPOCH},
};

use game::{Direction, Game, GameState, Inputs, Spin};
use graphics::RawMode;
use input::{EventLoop, KeyEvent};
use keys::*;
use rand::prelude::*;
use replay::{Input, Replay};
use sound::Player;

use spin_sleep::LoopHelper;

fn main() {
    // on first run create config file and pring help, maybe open config in editor?
    // error on failed kitty input mode, foot/wezterm/kitty only
    let _mode = RawMode::enter();
    let input = EventLoop::start();
    let (config, keys, player) = settings::load().unwrap();
    let mut game = Game::new(config);
    while run_game(&mut game, &input, &keys, &player) {}
}

fn run_game(game: &mut Game, input: &EventLoop, keys: &Bindings, player: &Player) -> bool {
    let mut inputs = Inputs::default();
    let mut main_loop = LoopHelper::builder().build_with_target_rate(60.0);
    let (width, height) = get_size();
    if width < 40 || height < 22 {
        panic!("screen too small");
    }

    let seed = thread_rng().gen();
    game.start(seed);
    let mut replay = Replay::new(game.config, seed);

    let done = 'outer: loop {
        let _delta = main_loop.loop_start();
        // preserve held inputs
        inputs = Inputs {
            left: inputs.left,
            right: inputs.right,
            soft: inputs.soft,
            ..Default::default()
        };

        while let Ok(event) = input.events.try_recv() {
            match event {
                KeyEvent('q', ..) | KeyEvent('c', CTRL, ..) => break 'outer false,
                KeyEvent('r', _, true) => break 'outer true,
                KeyEvent(c, _, true) => {
                    if c == keys.left {
                        inputs.dir = Some(Direction::Left);
                        inputs.left = true;
                        replay.push(game, Input::Left, true);
                    } else if c == keys.right {
                        inputs.dir = Some(Direction::Right);
                        inputs.right = true;
                        replay.push(game, Input::Right, true);
                    } else if c == keys.soft {
                        inputs.soft = true;
                        replay.push(game, Input::Soft, true);
                    } else if c == keys.hard {
                        inputs.hard = true;
                        replay.push(game, Input::Hard, true);
                    } else if c == keys.hold {
                        inputs.hold = true;
                        replay.push(game, Input::Hold, true);
                    } else if c == keys.cw {
                        inputs.rotate = Some(Spin::Cw);
                        replay.push(game, Input::Cw, true);
                    } else if c == keys.ccw {
                        inputs.rotate = Some(Spin::Ccw);
                        replay.push(game, Input::Ccw, true);
                    } else if c == keys.flip {
                        inputs.rotate = Some(Spin::Flip);
                    }
                }
                KeyEvent(c, _, false) => {
                    if c == keys.left {
                        inputs.left = false;
                        replay.push(game, Input::Left, false);
                    } else if c == keys.right {
                        inputs.right = false;
                        replay.push(game, Input::Right, false);
                    } else if c == keys.soft {
                        inputs.soft = false;
                        replay.push(game, Input::Soft, false);
                    }
                }
            }
        }

        game.step(&inputs, player);

        let (ox, oy) = (width as i8 / 2 - 19, height as i8 / 2 - 11);
        graphics::draw_board(game, (ox + 10, oy)).unwrap();
        if let Some(hold) = game.hold {
            graphics::draw_piece(hold, (ox, oy + 2)).unwrap();
        }
        for i in 0..5 {
            graphics::draw_piece(
                *game.upcomming.get(i).unwrap(),
                (ox + 32, oy + 2 + 3 * i as i8),
            )
            .unwrap();
        }
        let text_color = (255, 255, 255);
        graphics::draw_text(
            (ox + 34, oy + 20),
            text_color,
            &(40 - game.lines as i32).max(0).to_string(),
        )
        .unwrap();
        let frames = game.current_frame.saturating_sub(120);
        let mins = frames / 3600;
        let secs = frames % 3600 / 60;
        let millis = frames % 60 * 10 / 6;
        let time = if mins != 0 {
            format!("{mins}:{secs:02}.{millis:02} ")
        } else {
            format!("{secs}.{millis:02} ")
        };
        graphics::draw_text((ox + 1, oy + 20), text_color, &time).unwrap();
        main_loop.loop_sleep();
    };

    if game.state == GameState::Done {
        let time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_secs();
        replay.total_frames = game.current_frame - 120;
        {
            let replayfile = File::create(format!("replays/{time}.bin")).unwrap();
            replay.save(replayfile).unwrap();
        }
        // does it round-trip?
        replay.current_frame = 0;
        let replayfile = File::open(format!("replays/{time}.bin")).unwrap();
        assert_eq!(replay, Replay::load(replayfile).unwrap());
    }
    done
}

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
