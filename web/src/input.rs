use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::mpsc::{self};
use std::sync::{Arc, Mutex};

use bimap::BiHashMap;
use log::{info, warn};
use tetris::Event;
use wasm_bindgen::prelude::*;
use web_sys::{AddEventListenerOptions, KeyboardEvent};

use tetris::InputEvent;

pub fn init_input_handlers(events: mpsc::Sender<Event>) -> Result<(), JsValue> {
    info!("initializing input handlers");
    let window = web_sys::window().unwrap();
    let doc = window.document().unwrap();
    let storage = window.local_storage().unwrap().unwrap();
    let keymap = Arc::new(Mutex::new(BiHashMap::<String, String>::new()));
    let keyupmap = Arc::new(Mutex::new(HashMap::new()));
    let keydownmap = Arc::new(Mutex::new(HashMap::new()));
    use InputEvent::*;
    static BINDING_KEY: AtomicBool = AtomicBool::new(false);
    let default = [
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
    ];
    // let bind_all = doc
    //     .get_element_by_id("bind-all")
    //     .unwrap()
    //     .dyn_into::<web_sys::HtmlButtonElement>()
    //     .unwrap();
    let reset = doc
        .get_element_by_id("reset-bindings")
        .unwrap()
        .dyn_into::<web_sys::HtmlButtonElement>()
        .unwrap();
    let reset_handler = {
        let keymap = keymap.clone();
        let keydownmap = keydownmap.clone();
        let keyupmap = keyupmap.clone();
        move |_event: web_sys::Event| {
            let window = web_sys::window().unwrap();
            let doc = window.document().unwrap();
            keymap.lock().unwrap().clear();
            keydownmap.lock().unwrap().clear();
            keyupmap.lock().unwrap().clear();
            for (name, key, press, release) in default {
                let button = doc
                    .get_element_by_id(&format!("{name}-key"))
                    .unwrap()
                    .dyn_into::<web_sys::HtmlButtonElement>()
                    .unwrap();
                let storage = window.local_storage().unwrap().unwrap();
                storage.remove_item(name).unwrap();
                keymap.lock().unwrap().insert(name.to_owned(), key.to_owned());
                keydownmap.lock().unwrap().insert(key.to_owned(), press);
                if let Some(release) = release {
                    keyupmap.lock().unwrap().insert(key.to_owned(), release);
                }
                button.set_text_content(Some(key));
            }
        }
    };
    let closure = Closure::wrap(Box::new(reset_handler) as Box<dyn FnMut(_)>);
    reset.set_onclick(Some(closure.as_ref().unchecked_ref()));
    std::mem::forget(closure);
    for (name, key, press, release) in default {
        let input_id = format!("{name}-key");
        let button = doc
            .get_element_by_id(&input_id)
            .unwrap()
            .dyn_into::<web_sys::HtmlButtonElement>()
            .unwrap();
        let rebind_key = {
            let keymap = keymap.clone();
            let keydownmap = keydownmap.clone();
            let keyupmap = keyupmap.clone();
            move |_event: web_sys::Event| {
                if BINDING_KEY.compare_exchange(false, true, Relaxed, Relaxed) != Ok(false) {
                    info!("already waiting on key");
                    return;
                }
                info!("waiting for keypress for {name}");
                let window = web_sys::window().unwrap();
                let doc = window.document().unwrap();
                let input_id = input_id.clone();
                let keymap = keymap.clone();
                let keydownmap = keydownmap.clone();
                let keyupmap = keyupmap.clone();
                let button = doc
                    .get_element_by_id(&input_id)
                    .unwrap()
                    .dyn_into::<web_sys::HtmlButtonElement>()
                    .unwrap();
                button.set_text_content(Some("<press key>"));
                let next_keypress = move |keydown: KeyboardEvent| {
                    info!("pressed key for {name}!");
                    let window = web_sys::window().unwrap();
                    let doc = window.document().unwrap();
                    let storage = window.local_storage().unwrap().unwrap();
                    let button = doc
                        .get_element_by_id(&input_id)
                        .unwrap()
                        .dyn_into::<web_sys::HtmlButtonElement>()
                        .unwrap();
                    let new = keydown.key();
                    if let Some(old) = keymap.lock().unwrap().get_by_left(name) {
                        keydownmap.lock().unwrap().remove(old.as_str());
                        keyupmap.lock().unwrap().remove(old.as_str());
                    } else {
                        warn!("{name} wasn't in the keymap?");
                    }
                    storage.set(name, &new).unwrap();
                    button.set_text_content(Some(&new));
                    keydownmap.lock().unwrap().insert(new.clone(), press);
                    if let Some(release) = release {
                        keyupmap.lock().unwrap().insert(new, release);
                    }
                    info!("finished handling key binding: {keymap:?}");
                    BINDING_KEY.store(false, Relaxed);
                };
                let closure = Closure::wrap(Box::new(next_keypress) as Box<dyn FnMut(_)>);
                let options = AddEventListenerOptions::new();
                options.set_once(true); // TODO: this is deprecated, migrate to "capture"
                doc.add_event_listener_with_callback_and_add_event_listener_options(
                    "keydown",
                    closure.as_ref().unchecked_ref(),
                    &options,
                )
                .unwrap();
                std::mem::forget(closure);
                button.blur().unwrap();
            }
        };
        let closure = Closure::wrap(Box::new(rebind_key) as Box<dyn FnMut(_)>);
        button.set_onclick(Some(closure.as_ref().unchecked_ref()));
        std::mem::forget(closure);
        let initial = storage.get(name).unwrap().unwrap_or_else(|| key.to_owned());
        let mut keymap = keymap.lock().unwrap();
        keymap.insert(name.to_owned(), initial.to_owned());
        keydownmap.lock().unwrap().insert(initial.to_owned(), press);
        button.set_text_content(Some(&format!("{initial}")));
        if let Some(release) = release {
            keyupmap.lock().unwrap().insert(initial.to_owned(), release);
        }
    }

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
    let doc = window.document().unwrap();
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
