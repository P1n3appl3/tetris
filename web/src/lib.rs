mod fps;
mod graphics;
mod input;

use std::sync::mpsc::{Receiver, channel};

use log::info;
use tetris::sound::{NullSink, Sink, SoundPlayer};
use tetris::{Config, Event, Game, GameState};
use wasm_bindgen::prelude::*;
use web_sys::{HtmlCanvasElement, HtmlDivElement};
use web_time::Instant;

use crate::fps::FPSCounter;
use crate::graphics::Skin;

#[wasm_bindgen]
pub async fn main() -> Result<(), JsValue> {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    wasm_logger::init(wasm_logger::Config::new(log::Level::Debug));
    info!("wasm blob initialized, running main...");
    let window = web_sys::window().unwrap();
    let doc = window.document().unwrap();
    let default_skin = "https://i.imgur.com/zjItrsg.png";
    let skin = graphics::load_skin(default_skin).await?;
    let board = doc.get_element_by_id("board").unwrap().dyn_into::<web_sys::HtmlCanvasElement>()?;
    let hold = doc.get_element_by_id("hold").unwrap().dyn_into::<web_sys::HtmlCanvasElement>()?;
    let queue = doc.get_element_by_id("queue").unwrap().dyn_into::<HtmlCanvasElement>()?;
    let timer_div = doc.get_element_by_id("timer").unwrap().dyn_into::<HtmlDivElement>()?;
    let fps_div = doc.get_element_by_id("fps").unwrap().dyn_into::<HtmlDivElement>()?;
    let right_info_div =
        doc.get_element_by_id("right-info").unwrap().dyn_into::<HtmlDivElement>()?;
    let config = Config {
        das: 6,
        arr: 0,
        gravity: 60,
        soft_drop: 1,
        lock_delay: (60, 300, 1200),
        ghost: true,
    };

    let (tx, rx) = channel();
    input::init_input_handlers(tx)?;
    let (mut raf_loop, _canceler) = wasm_repeated_animation_frame::RafLoop::new();
    let mut fps = fps::FPSCounter::new();
    let mut game = Game::new(config);
    // game.mode = tetris::Mode::Sprint { target_lines: 10 };
    game.mode = tetris::Mode::Practice;
    info!("starting event loop");
    let sound = SoundPlayer::<NullSink>::default();
    game.start(None, &sound);

    // TODO: eventually we wanna go back to separate event loops for inputs/drawing/timers,
    // but for now this makes it easy to share game state between those
    let raf_fut = async {
        loop {
            raf_loop.next().await;
            run_loop(
                &mut game,
                &board,
                &queue,
                &hold,
                &skin,
                &mut fps,
                &timer_div,
                &fps_div,
                &right_info_div,
                &rx,
                &sound,
            )
        }
    };
    raf_fut.await;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_loop(
    game: &mut Game,
    board: &HtmlCanvasElement,
    queue: &HtmlCanvasElement,
    hold: &HtmlCanvasElement,
    skin: &Skin,
    fps_counter: &mut FPSCounter,
    timer: &HtmlDivElement,
    fps: &HtmlDivElement,
    line_count: &HtmlDivElement,
    rx: &Receiver<Event>,
    sound: &SoundPlayer<impl Sink>,
) {
    let now = Instant::now();
    fps.set_text_content(Some(&format!("fps: {}", fps_counter.tick(now))));

    let t = if let Some(start_time) = game.start_time {
        game.end_time.unwrap_or(now).duration_since(start_time).as_secs_f64()
    } else {
        0.0
    };
    timer.set_text_content(Some(&format!("{t:.2}")));

    if let tetris::Mode::Sprint { target_lines: target } = game.mode {
        line_count.set_text_content(Some(&format!("{}", target.saturating_sub(game.lines))));
    }

    while let Ok(e) = rx.try_recv() {
        use tetris::{Event::*, GameState::*, InputEvent::*};
        if let Input(Restart) = e {
            game.start(None, sound);
            break;
        }
        if game.state == Running
            || game.state == Startup
                && matches!(e, Input(PressLeft | PressRight | ReleaseLeft | ReleaseRight))
        {
            game.handle(e, now, sound);
        }
    }
    if game.state == GameState::Done {
        game.timers.clear();
    }
    while let Some(&(t, timer_event)) = game.timers.front() {
        if t < now {
            game.timers.pop_front();
            game.handle(Event::Timer(timer_event), now, sound);
        } else {
            break;
        }
    }

    graphics::draw_board(game, board, skin, t).unwrap();
    // could do these only when needed instead of every frame if we wanted
    graphics::draw_queue(game, queue, skin, 5).unwrap();
    graphics::draw_hold(game, hold, skin).unwrap();
}
