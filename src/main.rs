mod game;
mod graphics;
mod input;
mod keys;
mod settings;
mod sound;

use game::{Direction, Game, Inputs};
use graphics::RawMode;
use input::{Event, EventLoop, KeyEvent};
use keys::*;
use sound::Player;

use spin_sleep::LoopHelper;

fn main() {
    // on first run create config file and pring help, maybe open config in editor?
    // error on failed kitty input mode, foot/wezterm/kitty only
    let _mode = RawMode::enter();
    let input = EventLoop::start();
    let (config, keys, player) = settings::load().unwrap();
    let mut game = Game::new();
    game.config = config;
    while run_game(&mut game, &input, &keys, &player) {}
}

fn run_game(game: &mut Game, input: &EventLoop, keys: &Bindings, player: &Player) -> bool {
    let mut inputs = Inputs::default();
    let mut main_loop = LoopHelper::builder().build_with_target_rate(60.0);
    let (mut width, mut height) = get_size();
    if width < 40 || height < 22 {
        panic!("screen too small");
    }

    game.start();

    loop {
        let _delta = main_loop.loop_start();
        // preserve held inputs
        inputs = Inputs {
            left: inputs.left,
            right: inputs.right,
            soft: inputs.soft,
            ..Default::default()
        };

        while let Ok(event) = input.events.try_recv() {
            use Event::*;
            match event {
                Quit => return false,
                Restart => return true,
                Resize => {
                    (width, height) = get_size();
                    // eprintln!("resized {width} {height}");
                    if width < 40 || height < 22 {
                        panic!("screen too small");
                    }
                }
                Key(KeyEvent(c, _, true)) => {
                    if c == keys.left {
                        inputs.dir = Some(Direction::Left);
                        inputs.left = true;
                    } else if c == keys.right {
                        inputs.dir = Some(Direction::Right);
                        inputs.right = true;
                    } else if c == keys.soft {
                        inputs.soft = true;
                    } else if c == keys.hard {
                        inputs.hard = true;
                    } else if c == keys.hold {
                        inputs.hold = true
                    } else if c == keys.cw {
                        inputs.rotate = Some(Direction::Right);
                    } else if c == keys.ccw {
                        inputs.rotate = Some(Direction::Left);
                    }
                }
                Key(KeyEvent(c, _, false)) => {
                    if c == keys.left {
                        inputs.left = false;
                    } else if c == keys.right {
                        inputs.right = false;
                    } else if c == keys.soft {
                        inputs.soft = false;
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
        // graphics::draw_text((ox + 32, oy + 19), text_color, "lines").unwrap();
        // graphics::draw_text((ox + 32, oy + 20), text_color, "left").unwrap();

        main_loop.loop_sleep();
    }
}

pub struct Bindings {
    pub left: char,
    pub right: char,
    pub soft: char,
    pub hard: char,
    pub cw: char,
    pub ccw: char,
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
    (size.ws_col as u16, size.ws_row as u16)
}
