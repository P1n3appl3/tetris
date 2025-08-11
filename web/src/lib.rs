#![allow(unused)]
mod fps;
mod skin;

use std::cell::RefCell;
use std::future;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, Sender, channel};
use std::{array, collections::HashMap};

use futures::prelude::*;
use log::{error, info};
use tetris::sound::{NullSink, SoundPlayer};
use tetris::{Cell, Config, Event, Game, GameState, InputEvent};
use ultraviolet::DVec3;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, HtmlDivElement, KeyboardEvent};
use web_time::Instant;

use crate::skin::Skin;

#[wasm_bindgen]
pub async fn main() -> Result<(), JsValue> {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    wasm_logger::init(wasm_logger::Config::default());
    info!("wasm blob initialized, running main...");
    let (tx, mut rx) = channel();
    init_input_handlers(tx)?;
    let window = web_sys::window().unwrap();
    let doc = window.document().expect("Could not get document");
    let default_skin = "https://i.imgur.com/zjItrsg.png";
    let skin = skin::load_skin(default_skin).await?;
    let board = doc.get_element_by_id("board").unwrap().dyn_into::<web_sys::HtmlCanvasElement>()?;
    let hold = doc.get_element_by_id("hold").unwrap().dyn_into::<web_sys::HtmlCanvasElement>()?;
    let hold_cx: CanvasRenderingContext2d =
        hold.get_context("2d")?.unwrap().dyn_into::<CanvasRenderingContext2d>()?;
    let queue = doc.get_element_by_id("queue").unwrap().dyn_into::<HtmlCanvasElement>()?;
    let timer_div = doc.get_element_by_id("timer").unwrap().dyn_into::<HtmlDivElement>()?;
    let fps_div = doc.get_element_by_id("fps").unwrap().dyn_into::<HtmlDivElement>()?;
    let right_info_div =
        doc.get_element_by_id("right-info").unwrap().dyn_into::<HtmlDivElement>()?;
    let config = Config {
        das: 6,
        arr: 0,
        gravity: 60,
        soft_drop: 2,
        lock_delay: (60, 300, 1200),
        ghost: true,
    };

    let (mut raf_loop, canceler) = wasm_repeated_animation_frame::RafLoop::new();
    let mut fps = fps::FPSCounter::new();
    let mut game = Game::new(config);
    info!("starting event loop");
    let start_time = Instant::now();
    // TODO: timers
    let sound = SoundPlayer::<NullSink>::default();
    game.start(0xabad1d3a, &sound);
    game.state = GameState::Running;

    // TODO: eventually we wanna go back to separate event loops for inputs/drawing/timers,
    // but for now this makes it easy to share game state between those
    let raf_fut = async {
        loop {
            raf_loop.next().await;
            let fps = fps.tick();
            let t = (Instant::now() - start_time).as_secs_f64();
            timer_div.set_text_content(Some(&format!("{t:.2}")));
            if let Some(target) = game.target_lines {
                right_info_div
                    .set_text_content(Some(&format!("{}", target.saturating_sub(game.lines))));
            }
            fps_div.set_text_content(Some(&format!("fps: {fps}")));
            let (r, g, b) = fun_color(t / 10.0).into();
            hold_cx.set_fill_style_str(&format!("rgb({r}, {g}, {b})"));
            hold_cx.fill_rect(0.0, 0.0, hold.width() as f64, hold.height() as f64);
            // info!("iterating");
            while let Ok(e) = rx.try_recv() {
                use tetris::{Event::*, InputEvent::*};
                match e {
                    Input(Restart) => {
                        game.start((t * 1000.0) as u64, &sound);
                        break;
                    }
                    _ => {}
                }
                info!("handling: {e:?}");
                let t = Instant::now();
                game.handle(e, t, &sound);
            }
            draw_board(&game, &board, &skin);
            draw_queue(&game, &queue, &skin, 8);
        }
    };
    raf_fut.await;
    Ok(())
}

// ty inigo <3
pub fn fun_color(t: f64) -> DVec3 {
    let a = DVec3::new(0.5, 0.5, 0.5);
    let b = DVec3::new(0.5, 0.5, 0.5);
    let c = DVec3::new(1.0, 1.0, 1.0);
    let d = DVec3::new(0.0, 0.33, 0.67);
    a + b * (std::f64::consts::TAU * (c * t + d)).map(|f| f.cos()) * 256.0
}

pub fn draw_board(
    game: &Game,
    canvas: &HtmlCanvasElement,
    skin: &skin::Skin,
) -> Result<(), JsValue> {
    let window = web_sys::window().unwrap();
    let document = window.document().expect("Could not get document");
    let cx = canvas.get_context("2d")?.unwrap().dyn_into::<CanvasRenderingContext2d>()?;
    cx.clear_rect(0.0, 0.0, canvas.width() as f64, canvas.height() as f64);
    let border_width = 1.0;
    let mino_size = 24.0;
    let ghost_alpha = 0.5; //TODO: slider
    for y in 0..20 {
        for x in 0..10 {
            if let Some(mut sprite) = skin::skindex(game.board[y][x]).map(|i| &skin[i]) {
                if game.state != GameState::Running {
                    sprite = &skin[0];
                }
                cx.draw_image_with_image_bitmap(
                    sprite,
                    x as f64 * mino_size + border_width,
                    (20 - y) as f64 * mino_size + border_width,
                )?;
            }
        }
    }
    // only draw ghost while game is running
    if game.state != GameState::Running {
        return Ok(());
    }
    cx.set_global_alpha(ghost_alpha);
    let (piece, pos, rot) = game.current;
    for (x, y) in piece.get_pos(rot, pos) {
        let mut sprite = skin::skindex(Cell::Piece(piece)).map(|i| &skin[i]).unwrap();
        if game.state != GameState::Running {
            sprite = &skin[0];
        }
        cx.draw_image_with_image_bitmap(
            sprite,
            x as f64 * mino_size + border_width,
            (20 - y) as f64 * mino_size + border_width,
        )?;
    }
    Ok(())
}
pub fn draw_queue(
    game: &Game,
    canvas: &HtmlCanvasElement,
    skin: &skin::Skin,
    depth: usize,
) -> Result<(), JsValue> {
    let window = web_sys::window().unwrap();
    let document = window.document().expect("Could not get document");
    let cx = canvas.get_context("2d")?.unwrap().dyn_into::<CanvasRenderingContext2d>()?;
    // cx.clear_rect(0.0, 0.0, canvas.width() as f64, canvas.height() as f64);
    cx.set_fill_style_str("rgb(1, 240, 3)");
    cx.fill_rect(0.0, 0.0, canvas.width() as f64, canvas.height() as f64);
    let border_width = 1.0;
    let mino_size = 24.0;
    for i in 0..9 {
        cx.draw_image_with_image_bitmap(&skin[i], 20.0, i as f64 * 24.0 + 20.0)?;
    }
    Ok(())
}

fn init_input_handlers(events: mpsc::Sender<Event>) -> Result<(), JsValue> {
    info!("initializing input handlers");
    let window = web_sys::window().expect("could not get window handle");

    use tetris::InputEvent::*;
    let keymap = [
        ("j", PressLeft),
        ("l", PressRight),
        ("k", PressSoft),
        (" ", Hard),
        ("f", Cw),
        ("d", Ccw),
        ("a", Hold),
        ("s", Flip),
        ("r", Restart),
        ("q", Quit),
    ]
    .into_iter()
    .collect::<HashMap<&'static str, InputEvent>>();

    let closure: Box<dyn FnMut(_)> = Box::new({
        let mut events = events.clone();
        move |keydown: KeyboardEvent| {
            if keydown.repeat() {
                return;
            }
            let key = keydown.key();
            if let Some(&ev) = keymap.get(key.as_str()) {
                info!("sent an event: {key} -> {ev:?}");
                events.clone().send(Event::Input(ev));
            } else {
                info!("'{key}' didn't match");
            }
        }
    });

    let closure = Closure::wrap(closure);

    window.add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref())?;
    closure.forget();

    Ok(())
}
