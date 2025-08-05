mod fps;
mod skin;

use std::array;

use log::info;
use tetris::{Config, Game};
use wasm_bindgen::{Clamped, prelude::*};
use wasm_bindgen_futures::JsFuture;
use web_sys::{Blob, ImageData, Response, console, js_sys::Uint8Array};

#[wasm_bindgen]
pub async fn main() -> Result<(), JsValue> {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    wasm_logger::init(wasm_logger::Config::default());
    info!("initializing");
    let window = web_sys::window().unwrap();
    let document = window.document().expect("Could not get document");
    let default_skin = "https://i.imgur.com/zjItrsg.png";
    let skin = skin::load_skin(default_skin);
    let canvas =
        document.get_element_by_id("board").unwrap().dyn_into::<web_sys::HtmlCanvasElement>()?;
    let timer_div =
        document.get_element_by_id("timer").unwrap().dyn_into::<web_sys::HtmlDivElement>()?;
    let fps_div =
        document.get_element_by_id("fps").unwrap().dyn_into::<web_sys::HtmlDivElement>()?;
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
    let game = Game::new(config);
    info!("starting event loop");
    loop {
        raf_loop.next().await;
        let fps = fps.tick();
        fps_div.set_text_content(Some(&format!("fps: {fps}")));
    }
}

pub fn draw(game: Game, skin: &skin::Skin) -> Result<(), JsValue> {
    let window = web_sys::window().unwrap();
    let document = window.document().expect("Could not get document");
    let canvas =
        document.get_element_by_id("board").unwrap().dyn_into::<web_sys::HtmlCanvasElement>()?;
    let context =
        canvas.get_context("2d")?.unwrap().dyn_into::<web_sys::CanvasRenderingContext2d>()?;
    context.draw_image_with_image_bitmap(todo!(), 0.0, 0.0);
    Ok(())
}
