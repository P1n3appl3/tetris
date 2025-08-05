mod fps;
mod skin;

use log::info;
use tetris::{Config, Game};
use ultraviolet::DVec3;
use wasm_bindgen::prelude::*;
use web_time::Instant;

#[wasm_bindgen]
pub async fn main() -> Result<(), JsValue> {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    wasm_logger::init(wasm_logger::Config::default());
    info!("initializing");
    let window = web_sys::window().unwrap();
    let document = window.document().expect("Could not get document");
    let default_skin = "https://i.imgur.com/zjItrsg.png";
    let skin = skin::load_skin(default_skin);
    let board =
        document.get_element_by_id("board").unwrap().dyn_into::<web_sys::HtmlCanvasElement>()?;
    let hold =
        document.get_element_by_id("hold").unwrap().dyn_into::<web_sys::HtmlCanvasElement>()?;
    let ctx = hold.get_context("2d")?.unwrap().dyn_into::<web_sys::CanvasRenderingContext2d>()?;
    let queue =
        document.get_element_by_id("queue").unwrap().dyn_into::<web_sys::HtmlCanvasElement>()?;
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
    let start_time = Instant::now();
    loop {
        raf_loop.next().await;
        let fps = fps.tick();
        let t = (Instant::now() - start_time).as_secs_f64();
        timer_div.set_text_content(Some(&format!("{t:.2}")));
        fps_div.set_text_content(Some(&format!("fps: {fps}")));
        let (r, g, b) = fun_color(t / 10.0).into();
        ctx.set_fill_style_str(&format!("rgb({r}, {g}, {b})"));
        ctx.fill_rect(0.0, 0.0, hold.width() as f64, hold.height() as f64);
    }
}

// ty inigo <3
pub fn fun_color(t: f64) -> DVec3 {
    let a = DVec3::new(0.5, 0.5, 0.5);
    let b = DVec3::new(0.5, 0.5, 0.5);
    let c = DVec3::new(1.0, 1.0, 1.0);
    let d = DVec3::new(0.0, 0.33, 0.67);
    a + b * (6.283185 * (c * t + d)).map(|f| f.cos()) * 256.0
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
