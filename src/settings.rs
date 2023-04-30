use anyhow::{Context, Result};
use kdl::{KdlDocument, KdlNode, KdlValue};
use std::fs;

use crate::{game::Config, keys, sound::Player, Bindings};

pub fn load() -> Result<(Config, Bindings, Player)> {
    // todo: use dir-rs/dirs/xdg for config dir
    let doc: KdlDocument = fs::read_to_string("settings.kdl")?.parse()?;
    let config = doc
        .get("config")
        .and_then(KdlNode::children)
        .context("missing config node")?;
    let sound_node = doc
        .get("sound")
        .and_then(KdlNode::children)
        .context("missing sound node")?;
    let bindings = doc
        .get("bindings")
        .and_then(KdlNode::children)
        .context("missing bindings node")?;

    let config = Config {
        das: get_config("das", config)? as u8,
        arr: get_config("arr", config)? as u8,
        gravity: get_config("gravity", config)? as u16,
        soft_drop: get_config("soft_drop", config)? as u8,
        lock_delay: (
            get_config("lock", config)? as u8,
            get_config("extended", config)? as u16,
            get_config("timeout", config)? as u16,
        ),
        ghost: config
            .get_arg("ghost")
            .and_then(KdlValue::as_bool)
            .unwrap_or(true),
    };
    let bindings = Bindings {
        left: get_binding("left", bindings)?,
        right: get_binding("right", bindings)?,
        soft: get_binding("soft", bindings)?,
        hard: get_binding("hard", bindings)?,
        cw: get_binding("cw", bindings)?,
        ccw: get_binding("ccw", bindings)?,
        flip: get_binding("flip", bindings)?,
        hold: get_binding("hold", bindings)?,
    };
    let mut player = Player::new()?;
    if let Some(f) = sound_node.get_arg("volume").and_then(KdlValue::as_f64) {
        player.volume = f as f32;
    }
    for sound in [
        "ready", "go", "move", "rotate", "lock", "line", "hold", "lose", "win",
    ] {
        if let Some(s) = sound_node.get_arg(sound).and_then(KdlValue::as_string) {
            player.add_sound(sound, s)?
        }
    }
    Ok((config, bindings, player))
}

fn get_config(name: &str, config: &KdlDocument) -> Result<i64> {
    config
        .get_arg(name)
        .and_then(KdlValue::as_i64)
        .context(format!("need a setting for '{name}' in the config block"))
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
                keys::get_key(s).context(format!("invalid key name '{s}' for '{name}' binding"))
            }
        })
}
