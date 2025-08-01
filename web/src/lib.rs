use wasm_bindgen::prelude::*;

// TODO: maybe install a panic hook

#[wasm_bindgen]
extern "C" {
    fn alert(s: &str);
}

#[wasm_bindgen]
pub fn hiiii() {
    alert("Hello, my-project!");
}
