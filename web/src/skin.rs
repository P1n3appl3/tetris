use std::array;

use image::{DynamicImage, ImageFormat, imageops::FilterType};
use log::info;
use tetris::{Cell, Config, Game};
use wasm_bindgen::{Clamped, prelude::*};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    Blob, ImageBitmap, ImageData, Response, console,
    js_sys::{Uint8Array, Uint8ClampedArray},
};

/// permanent, garbage, z, l, o, s, i, j, t
pub type Skin = [ImageBitmap; 9];

// TODO: rewrite skin as a newtype with a getter method that takes a piece (and one for cell)
pub fn skindex(c: Cell) -> Option<usize> {
    match (c) {
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
        let img = image.crop(i as u32 * h, 0, h, h).resize(24, 24, FilterType::Triangle);
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
