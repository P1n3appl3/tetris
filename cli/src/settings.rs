use anyhow::{Context, Result};
use kdl::{KdlDocument, KdlNode, KdlValue};
use strum::EnumString;
use tetris::{Bindings, Config, sound::Sink};

use crate::sound::Rodio;

// TODO: make this declarative
pub fn load(raw: &str, player: &mut Rodio) -> Result<(Config, Bindings)> {
    let doc: KdlDocument = raw.parse()?;
    let get_node =
        |name| doc.get(name).and_then(KdlNode::children).context(format!("missing {name} node"));
    let config_node = get_node("config")?;
    let bindings_node = get_node("bindings")?;

    let config = Config {
        das: get_config("das", config_node)? as u16,
        arr: get_config("arr", config_node)? as u16,
        gravity: get_config("gravity", config_node)? as u16,
        soft_drop: get_config("soft-drop", config_node)? as u16,
        lock_delay: (
            get_config("lock", config_node)? as u16,
            get_config("extended", config_node)? as u16,
            get_config("timeout", config_node)? as u16,
        ),
        ghost: config_node.get_arg("ghost").and_then(KdlValue::as_bool).unwrap_or(true),
    };
    let bindings = Bindings {
        left: get_binding("left", bindings_node)?,
        right: get_binding("right", bindings_node)?,
        soft: get_binding("soft", bindings_node)?,
        hard: get_binding("hard", bindings_node)?,
        cw: get_binding("cw", bindings_node)?,
        ccw: get_binding("ccw", bindings_node)?,
        flip: get_binding("flip", bindings_node)?,
        hold: get_binding("hold", bindings_node)?,
    };

    if let Ok(sound_node) = get_node("sound") {

        if let Some(f) = sound_node.get_arg("volume").and_then(KdlValue::as_float) {
            player.set_volume(f as f32);
        }
        if let Some(meta) = sound_node.get("meta") {
            for entry in meta.iter_children() {
                println!("{entry:?}")
                // entry.name()
                    // entry.get(1);
                // entry.entries()
            }
            // meta.into_iter().map(|n| n.); n.)

        }
    }
    // for sound in SOUNDS {
    //     if let Some(s) = sound_node.get_arg(sound).and_then(KdlValue::as_string) {
    //         if let Err(e) = player.add_sound(sound, s) {
    //             log::error!("Failed to load sound {sound} from {s}: {e}");
    //         } else {
    //             log::info!("Loaded sound {sound} from {s}");
    //         }
    //     }
    // }
    Ok((config, bindings))
}

fn get_config(name: &str, config: &KdlDocument) -> Result<i128> {
    config
        .get_arg(name)
        .and_then(KdlValue::as_integer)
        .context(format!("need a setting for '{name}' in the config block"))
}

pub mod keys {
    #![allow(unused)]
    #![cfg_attr(rustfmt, rustfmt_skip)]

    pub const SHIFT:     u8 = 0b1;
    pub const ALT:       u8 = 0b10;
    pub const CTRL:      u8 = 0b100;
    pub const SUPER:     u8 = 0b1000;
    pub const HYPER:     u8 = 0b10000;
    pub const META:      u8 = 0b100000;
    pub const CAPS_LOCK: u8 = 0b1000000;
    pub const NUM_LOCK:  u8 = 0b10000000;

    pub const LEFT:       char = '\u{e006}';
    pub const RIGHT:      char = '\u{e007}';
    pub const UP:         char = '\u{e008}';
    pub const DOWN:       char = '\u{e009}';
    pub const LEFT_SHIFT: char = '\u{e061}';
}

fn get_key(name: &str) -> Option<char> {
    use keys::*;
    Some(match name.to_lowercase().as_str() {
        "left" => LEFT,
        "right" => RIGHT,
        "up" => UP,
        "down" => DOWN,
        "shift" => LEFT_SHIFT,
        "space" => ' ',
        _ => return None,
    })
}

fn get_binding(name: &str, bindings: &KdlDocument) -> Result<char> {
    bindings
        .get_arg(name)
        .and_then(KdlValue::as_string)
        .context(format!("need a binding for '{name}'"))
        .and_then(|s| {
            if s.chars().count() == 1 {
                Ok(s.chars().next().unwrap())
            } else {
                get_key(s).context(format!("invalid key name '{s}' for '{name}' binding"))
            }
        })
}
