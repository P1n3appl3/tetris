use image::{imageops::FilterType, ImageFormat};
use ringbuffer::RingBuffer;
use tetris::{Cell, Game, GameState, Piece, PieceLocation, Rotation};
use ultraviolet::DVec3;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    js_sys::{Uint8Array, Uint8ClampedArray},
    Blob, CanvasRenderingContext2d, HtmlCanvasElement, HtmlDivElement, ImageBitmap, ImageData,
    Response,
};

const SIZE: usize = 24;

/// permanent, garbage, z, l, o, s, i, j, t
pub type Skin = [ImageBitmap; 9];

// TODO: rewrite skin as a newtype with a getter method that takes a piece (and one for cell)
pub fn skindex(c: Cell) -> Option<usize> {
    match c {
        Cell::Piece(piece) => Some(match piece {
            tetris::Piece::Z => 2,
            tetris::Piece::L => 3,
            tetris::Piece::O => 4,
            tetris::Piece::S => 5,
            tetris::Piece::I => 6,
            tetris::Piece::J => 7,
            tetris::Piece::T => 8,
        }),
        Cell::Garbage => Some(1),
        Cell::Empty => None,
    }
}

pub fn draw_board(
    game: &Game,
    canvas: &HtmlCanvasElement,
    skin: &Skin,
    t: f64,
) -> Result<(), JsValue> {
    let cx = canvas.get_context("2d")?.unwrap().dyn_into::<CanvasRenderingContext2d>()?;
    cx.clear_rect(0.0, 0.0, canvas.width() as f64, canvas.height() as f64);
    if !game.should_draw_board() {
        return Ok(());
    }
    let border_width = 1.0;
    for y in 0..20 {
        for x in 0..10 {
            if let Some(mut sprite) = skindex(game.board[y][x]).map(|i| &skin[i]) {
                if game.state != GameState::Running {
                    sprite = &skin[0];
                }
                cx.draw_image_with_image_bitmap(
                    sprite,
                    (x * SIZE) as f64 + border_width,
                    ((19 - y) * SIZE) as f64 + border_width,
                )?;
            }
        }
    }
    // only draw ghost and current piece while game is running
    if game.state != GameState::Running {
        return Ok(());
    }

    let PieceLocation { piece, pos: (x, y), rot } = game.current;
    cx.set_global_alpha(0.25); // TODO: slider
    let ghost = game.ghost_pos();
    let origin = (
        (ghost.pos.0 as f64 * SIZE as f64 + border_width),
        ((19 - ghost.pos.1) as f64 * SIZE as f64 + border_width),
    );
    draw_piece(canvas, skin, piece, rot, origin, game.draw_only_mino())?;
    cx.set_global_alpha(1.0);

    let origin =
        (x as f64 * SIZE as f64 + border_width, (19 - y) as f64 * SIZE as f64 + border_width);
    draw_piece(canvas, skin, piece, rot, origin, game.draw_only_mino())?;

    // rainbow border cuz why not :3
    let (r, g, b) = fun_color(t / 10.0).into();
    cx.set_stroke_style_str(&format!("rgb({r}, {g}, {b})"));
    cx.set_line_width(2.0);
    cx.stroke_rect(1.0, 1.0, (canvas.width()) as f64 - 2.0, (canvas.height()) as f64 - 2.0);

    Ok(())
}

fn draw_piece(
    canvas: &HtmlCanvasElement,
    skin: &Skin,
    piece: Piece,
    rot: Rotation,
    origin: (f64, f64),
    mino_mode: bool,
) -> Result<(), JsValue> {
    let cx = canvas.get_context("2d")?.unwrap().dyn_into::<CanvasRenderingContext2d>()?;
    let loc = PieceLocation { piece, rot, pos: (0, 0) };
    let blocks: &[(i8, i8)] = if mino_mode { &[(0, 0)] } else { &loc.blocks() };
    for &(x, y) in blocks.iter() {
        let sprite = skindex(Cell::Piece(piece)).map(|i| &skin[i]).unwrap();
        cx.draw_image_with_image_bitmap(
            sprite,
            x as f64 * SIZE as f64 + origin.0,
            -y as f64 * SIZE as f64 + origin.1,
        )?;
    }
    Ok(())
}

pub fn draw_hold(game: &Game, canvas: &HtmlCanvasElement, skin: &Skin) -> Result<(), JsValue> {
    let cx = canvas.get_context("2d")?.unwrap().dyn_into::<CanvasRenderingContext2d>()?;
    // cx.set_fill_style_str("rgb(17, 17, 17)");
    // cx.fill_rect(0.0, 0.0, canvas.width() as f64, canvas.height() as f64);
    cx.clear_rect(0.0, 0.0, canvas.width() as f64, canvas.height() as f64);
    if !game.should_draw_hold() {
        return Ok(());
    }
    if let Some(piece) = game.hold {
        draw_piece(
            canvas,
            skin,
            piece,
            Rotation::North,
            (SIZE as _, SIZE as _),
            game.draw_only_mino(),
        )?;
    }
    Ok(())
}

pub fn draw_queue(
    game: &Game,
    canvas: &HtmlCanvasElement,
    skin: &Skin,
    depth: usize,
) -> Result<(), JsValue> {
    let cx = canvas.get_context("2d")?.unwrap().dyn_into::<CanvasRenderingContext2d>()?;
    // cx.set_fill_style_str("rgb(17, 17, 17)");
    // cx.fill_rect(0.0, 0.0, canvas.width() as f64, canvas.height() as f64);
    cx.clear_rect(0.0, 0.0, canvas.width() as f64, canvas.height() as f64);
    if !game.should_draw_queue() {
        return Ok(());
    }
    for (i, &piece) in game.upcomming.iter().take(depth).enumerate() {
        draw_piece(
            canvas,
            skin,
            piece,
            Rotation::North,
            (SIZE as f64, ((1 + 3 * i) * SIZE) as f64),
            game.draw_only_mino(),
        )?;
    }
    Ok(())
}

// ty inigo <3
fn fun_color(t: f64) -> DVec3 {
    let a = DVec3::new(0.5, 0.5, 0.5);
    let b = DVec3::new(0.5, 0.5, 0.5);
    let c = DVec3::new(1.0, 1.0, 1.0);
    let d = DVec3::new(0.0, 0.33, 0.67);
    a + b * (std::f64::consts::TAU * (c * t + d)).map(|f| f.cos()) * 256.0
}

#[wasm_bindgen]
pub async fn fetch_url(url: &str) -> Result<Blob, JsValue> {
    let window = web_sys::window().unwrap();
    let result = JsFuture::from(window.fetch_with_str(url)).await?;
    let response: Response = result.dyn_into().unwrap();
    JsFuture::from(response.blob()?).await?.dyn_into()
}

// https://konsola5.github.io/jstris-customization-database/
// https://i.imgur.com/HkJWOEQ.png
// TODO: maybe remove `image` dep and use js apis to reduce bundle size? check bloaty
pub async fn load_skin(url: &str) -> Result<Skin, JsValue> {
    let blob = fetch_url(url).await?;
    let mime = blob.type_();
    let image_data = Uint8Array::new(&JsFuture::from(blob.array_buffer()).await?);
    use ImageFormat::*;
    let formats = [("png", Png), ("webp", WebP), ("qoi", Qoi), ("gif", Gif)];
    let format = formats.iter().find_map(|(s, f)| mime.contains(s).then_some(*f)).unwrap_or(Png);

    // TODO: don't do image decoding in wasm, use the browser api, maybe for slicing too?
    let mut image = image::load_from_memory_with_format(&image_data.to_vec(), format).unwrap();
    assert_eq!(
        image.width(),
        image.height() * 9,
        "Skin had wrong dimensions: {}x{}, should be a 9:1 ratio",
        image.width(),
        image.height()
    );
    let h = image.height();
    let mut sprites = Vec::new();
    for i in 0..9 {
        let img = image.crop(i as u32 * h, 0, h, h).resize(
            SIZE as u32,
            SIZE as u32,
            FilterType::Triangle,
        );
        let rgba_img = img.to_rgba8();
        let (width, height) = rgba_img.dimensions();
        let pixels = rgba_img.into_raw();
        let pixel_array = Uint8ClampedArray::new_with_length(pixels.len() as u32);
        pixel_array.copy_from(&pixels);
        let image_data =
            ImageData::new_with_js_u8_clamped_array_and_sh(&pixel_array, width, height)?;
        let window = web_sys::window().unwrap();
        let promise = window.create_image_bitmap_with_image_data(&image_data)?;
        let js_future = JsFuture::from(promise);
        let result = js_future.await?;

        sprites.push(result.dyn_into::<ImageBitmap>()?);
    }
    Ok(sprites.try_into()?)
}
