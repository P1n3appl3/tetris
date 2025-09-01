mod fps;
mod graphics;
mod input;

use std::sync::mpsc::{Receiver, channel};

use log::info;
use tetris::sound::{NullSink, Sink, SoundPlayer};
use tetris::{Config, Event, Game, GameState};
use tetrizz::eval::Eval;
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
    let spins_div = doc.get_element_by_id("spins").unwrap().dyn_into::<HtmlDivElement>()?;
    let right_info_div =
        doc.get_element_by_id("right-info").unwrap().dyn_into::<HtmlDivElement>()?;
    let config = Config {
        das: 6,
        arr: 0,
        gravity: Some(60),
        soft_drop: 1,
        lock_delay: (60, 300, 1200),
        ghost: true,
    };

    let (tx, rx) = channel();
    input::init_input_handlers(tx)?;
    let (mut raf_loop, _canceler) = wasm_repeated_animation_frame::RafLoop::new();
    let mut fps = fps::FPSCounter::new();
    let mut game = Game::new(config);
    game.mode = tetris::Mode::Sprint { target_lines: 40 };
    // game.mode = tetris::Mode::TrainingLab {
    //     search: false,
    //     // lookahead: Some(Lookahead::new(3, 30)),
    //     lookahead: None,
    //     mino_mode: true,
    // };
    info!("starting event loop, why won't you work!?");
    info!("mode: {:?}", game.mode);
    let sound = SoundPlayer::<NullSink>::default();
    game.start(None, &sound);
    let mut new_piece = false;
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
                &spins_div,
                &rx,
                &sound,
                eval,
                &mut new_piece,
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
    spins: &HtmlDivElement,
    rx: &Receiver<Event>,
    sound: &SoundPlayer<impl Sink>,
    eval: &Eval,
    new_piece: &mut bool,
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
        info!("search enabled: {}, new_piece: {}", game.mode.search_enabled(), new_piece);
        if game.mode.search_enabled() && *new_piece {
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
            *new_piece = false;
        }
        if game.state == Running
            || game.state == Startup
                && matches!(e, Input(PressLeft | PressRight | ReleaseLeft | ReleaseRight))
        {
            *new_piece |= game.handle(e, now, sound);
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

    let spin_text = game.display_spins().to_string();
    info!("spins: {spin_text}");
    spins.set_text_content(Some(&spin_text));
}
