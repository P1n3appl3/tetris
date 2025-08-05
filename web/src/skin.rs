use std::array;

use image::{DynamicImage, ImageFormat};
use log::info;
use tetris::{Config, Game};
use wasm_bindgen::{Clamped, prelude::*};
use wasm_bindgen_futures::JsFuture;
use web_sys::{Blob, ImageData, Response, console, js_sys::Uint8Array};

/// permanent, garbage, z, l, o, s, i, j, t
pub type Skin = [DynamicImage; 9];

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
    Ok(array::from_fn(|i| image.crop(i as u32 * h, 0, (i as u32 + 1) * h, h)))
}
