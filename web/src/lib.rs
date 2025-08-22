mod fps;
mod graphics;

use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, channel};
use std::sync::{Arc, Mutex};

use bimap::BiHashMap;
use log::info;
use tetris::sound::{NullSink, Sink, SoundPlayer};
use tetris::{Config, Event, Game, GameState, InputEvent};
use wasm_bindgen::prelude::*;
use web_sys::{
    AddEventListenerOptions, HtmlCanvasElement, HtmlDivElement, HtmlInputElement, KeyboardEvent,
    Window,
};
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
        soft_drop: 2,
        lock_delay: (60, 300, 1200),
        ghost: true,
    };

    let (tx, rx) = channel();
    init_input_handlers(window, tx)?;
    let (mut raf_loop, _canceler) = wasm_repeated_animation_frame::RafLoop::new();
    let mut fps = fps::FPSCounter::new();
    let mut game = Game::new(config);
    game.mode = tetris::Mode::Sprint { target_lines: 10 };
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

fn init_input_handlers(window: Window, events: mpsc::Sender<Event>) -> Result<(), JsValue> {
    info!("initializing input handlers");
    let doc = window.document().unwrap();
    let storage = window.local_storage().unwrap().unwrap();
    let keymap = Arc::new(Mutex::new(BiHashMap::new()));
    let keyupmap = Arc::new(Mutex::new(HashMap::new()));
    let keydownmap = Arc::new(Mutex::new(HashMap::new()));
    use InputEvent::*;
    for (name, key, press, release) in [
        ("left", "j", PressLeft, Some(ReleaseLeft)),
        ("right", "l", PressRight, Some(ReleaseRight)),
        ("hard", " ", Hard, None),
        ("soft", "k", PressSoft, Some(ReleaseSoft)),
        ("cw", "f", Cw, None),
        ("ccw", "d", Ccw, None),
        ("flip", "s", Flip, None),
        ("hold", "a", Hold, None),
        ("restart", "r", Restart, None),
        ("undo", "u", Undo, None),
    ] {
        let initial = storage.get(name).unwrap().unwrap_or_else(|| key.to_owned());
        let mut keymap = keymap.lock().unwrap();
        // Node
        let text = doc
            .get_element_by_id(&format!("{name}-key"))
            .unwrap()
            .dyn_into::<web_sys::HtmlInputElement>()
            .unwrap();
        text.set_text_content(Some(key));
        let handler = move |event: web_sys::Event| {
            let input_elem: HtmlInputElement = event.target().unwrap().dyn_into().unwrap();

            // app.store.borrow_mut().msg(&Msg::SetReflectivity(reflectivity));
        };
        let closure = Closure::wrap(Box::new(handler) as Box<dyn FnMut(_)>);
        text.set_onclick(Some(closure.as_ref().unchecked_ref()));
        // text.add_event_listener_with_callback_and_add_event_listener_options(type_, listener, options)
        keymap.insert(name.to_owned(), key.to_owned());
        keydownmap.lock().unwrap().insert(key, press);
        if let Some(release) = release {
            keyupmap.lock().unwrap().insert(key, release);
        }
    }

    // let save_binding = Box::new({
    //     move |keydown: KeyboardEvent| {
    //         let next_keypress = Box::new(move |keydown: KeyboardEvent| {
    //             if keydown.repeat() {
    //                 return;
    //             }
    //             let key = keydown.key();
    //             if let Some(&ev) = keymap.get(key.as_str()) {
    //                 events.send(Event::Input(ev)).unwrap();
    //             }
    //         });
    //         let closure = Closure::wrap(next_keypress);
    //         let mut options = AddEventListenerOptions::new();
    //         options.set_once(true); // TODO: this is deprecated, migrate to "capture"
    //         doc.add_event_listener_with_callback_and_add_event_listener_options(
    //             "keydown",
    //             closure.as_ref().unchecked_ref(),
    //             &options,
    //         );
    //     }
    // });

    // TODO: why doesn't focus work here?
    // let div = doc.get_element_by_id("main").unwrap().dyn_into::<web_sys::HtmlDivElement>()?;

    let closure: Box<dyn FnMut(_)> = Box::new({
        let events = events.clone();
        move |keydown: KeyboardEvent| {
            if keydown.repeat() {
                return;
            }
            let key = keydown.key();
            if let Some(&ev) = keydownmap.lock().unwrap().get(key.as_str()) {
                events.send(Event::Input(ev)).unwrap();
            }
        }
    });
    let closure = Closure::wrap(closure);
    doc.add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref())?;
    closure.forget();

    let closure: Box<dyn FnMut(_)> = Box::new({
        let events = events.clone();
        move |keydown: web_sys::KeyboardEvent| {
            let key = keydown.key();
            if let Some(&ev) = keyupmap.lock().unwrap().get(key.as_str()) {
                events.send(Event::Input(ev)).unwrap();
            }
        }
    });
    let closure = Closure::wrap(closure);
    doc.add_event_listener_with_callback("keyup", closure.as_ref().unchecked_ref())?;
    closure.forget();

    Ok(())
}
