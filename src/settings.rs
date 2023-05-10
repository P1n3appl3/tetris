use anyhow::{Context, Result};
use kdl::{KdlDocument, KdlNode, KdlValue};
use std::fs;

use crate::{game::Config, keys, sound::Player, Bindings};

pub fn load() -> Result<(Config, Bindings, Player)> {
    // todo: use dir-rs/dirs/xdg for config dir
    let doc: KdlDocument = fs::read_to_string("settings.kdl")?.parse()?;
    let get_node = |name| {
        doc.get(name).and_then(KdlNode::children).context(format!("missing {name} node"))
    };
    let config_node = get_node("config")?;
    let sound_node = get_node("sound")?;
    let bindings_node = get_node("bindings")?;

    let config = Config {
        das: get_config("das", config_node)? as u16,
        arr: get_config("arr", config_node)? as u16,
        gravity: get_config("gravity", config_node)? as u16,
        soft_drop: get_config("soft_drop", config_node)? as u16,
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
    let mut player = Player::new()?;
    if let Some(f) = sound_node.get_arg("volume").and_then(KdlValue::as_f64) {
        player.volume = f as f32;
    }
    for sound in ["ready", "go", "move", "rotate", "lock", "line", "hold", "lose", "win"] {
        if let Some(s) = sound_node.get_arg(sound).and_then(KdlValue::as_string) {
            if let Err(e) = player.add_sound(sound, s) {
                log::error!("Failed to load sound {sound} from {s}");
            }
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
                keys::get_key(s)
                    .context(format!("invalid key name '{s}' for '{name}' binding"))
            }
        })
}
