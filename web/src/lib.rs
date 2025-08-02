use wasm_bindgen::{Clamped, prelude::*};
use wasm_bindgen_futures::JsFuture;
use web_sys::{ImageData, js_sys::Uint8Array};

// TODO: maybe install a panic hook

#[wasm_bindgen]
extern "C" {
    fn alert(s: &str);
}

#[wasm_bindgen]
pub fn hiiii() {
    alert("Hello, my-project!");
}

#[wasm_bindgen]
pub async fn fetch_url_binary(url: String) -> Result<Uint8Array, JsValue> {
    let window = web_sys::window().unwrap();
    let promise = JsFuture::from(window.fetch_with_str(&url));
    let result = promise.await?;
    let response: web_sys::Response = result.dyn_into().unwrap();
    let image_data = JsFuture::from(response.array_buffer()?).await?;
    Ok(Uint8Array::new(&image_data))
}

#[wasm_bindgen]
pub async fn show_image(url: String, canvas: String) -> Result<(), JsValue> {
    let binary = fetch_url_binary(url).await?;
    let altbuf = binary.to_vec();

    let image = image::load_from_memory_with_format(&altbuf, image::ImageFormat::Png).unwrap();
    let rgba_image = image.to_rgba8();
    let clamped_buf: Clamped<&[u8]> = Clamped(rgba_image.as_raw());

    // for (_, _, pixel) in rgba_image.enumerate_pixels_mut() {
    //     if pixel[0] > 0 {
    //         *pixel = image::Rgba([0, pixel[1], pixel[2], pixel[3]]);
    //     }
    // }

    let image_data =
        ImageData::new_with_u8_clamped_array_and_sh(clamped_buf, image.width(), image.height())?;

    let window = web_sys::window().unwrap();
    let document = window.document().expect("Could not get document");
    let canvas = document
        .get_element_by_id(&canvas)
        .unwrap()
        .dyn_into::<web_sys::HtmlCanvasElement>()?;
    let context = canvas
        .get_context("2d")?
        .unwrap()
        .dyn_into::<web_sys::CanvasRenderingContext2d>()?;

    context.put_image_data(&image_data, 0.0, 0.0)?;

    Ok(())
}
