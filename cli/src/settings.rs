use std::{fs, path::Path, str::FromStr};

use anyhow::{Context, Result};
use directories::ProjectDirs;
use kdl::{KdlDocument, KdlEntry, KdlNode, KdlValue};
use tetris::{
    Bindings, Config,
    sound::{Action, Clear, Meta, Sink, SoundPlayer},
};

use crate::sound::Rodio;

pub fn load(
    path: Option<&Path>,
    dirs: &ProjectDirs,
    sound: &mut SoundPlayer<Rodio>,
) -> Result<(Config, Bindings)> {
    let raw = if let Some(path) = path {
        fs::read_to_string(path).expect("Couldn't read settings file")
    } else {
        let default_settings_content = include_str!("../settings.kdl");
        fs::create_dir_all(dirs.config_dir()).ok();
        let settings_path = dirs.config_dir().join("settings.kdl");
        match fs::read_to_string(&settings_path) {
            Ok(s) => s,
            Err(_) => {
                fs::write(settings_path, default_settings_content).ok();
                default_settings_content.to_owned()
            }
        }
    };

    let doc: KdlDocument = raw.parse()?;
    let get_node =
        |name| doc.get(name).and_then(KdlNode::children).context(format!("missing {name} node"));
    let config_node = get_node("config")?;
    let bindings_node = get_node("bindings")?;

    #[cfg(feature = "url-assets")]
    let cache = cached_path::CacheBuilder::new()
        .dir(dirs.cache_dir().to_path_buf())
        .client_builder(reqwest::blocking::ClientBuilder::new().user_agent("tetris"))
        .build()?;

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
            sound.sink.set_volume(f);
        }
        // TODO: dedup
        if let Some(meta) = sound_node.get("meta") {
            for node in meta.iter_children() {
                let path =
                    node.entries().first().map(KdlEntry::value).and_then(KdlValue::as_string);
                #[cfg(feature = "url-assets")]
                let path = path.and_then(|p| cache.cached_path(p).ok());
                if let Some(path) = path
                    && let Ok(variant) = Meta::from_str(node.name().value()).map(Into::into)
                    && let Ok(decoded) = Rodio::decode(&path)
                {
                    sound.sounds.insert(variant, decoded);
                } else {
                    log::warn!("failed to load sound for '{}'", node.name().value())
                }
            }
        }
        if let Some(meta) = sound_node.get("action") {
            for node in meta.iter_children() {
                let path =
                    node.entries().first().map(KdlEntry::value).and_then(KdlValue::as_string);
                #[cfg(feature = "url-assets")]
                let path = path.and_then(|p| cache.cached_path(p).ok());
                if let Some(path) = path
                    && let Ok(variant) = Action::from_str(node.name().value()).map(Into::into)
                    && let Ok(decoded) = Rodio::decode(&path)
                {
                    sound.sounds.insert(variant, decoded);
                } else {
                    log::warn!("failed to load sound for '{}'", node.name().value())
                }
            }
        }
        if let Some(meta) = sound_node.get("clear") {
            for node in meta.iter_children() {
                let path =
                    node.entries().first().map(KdlEntry::value).and_then(KdlValue::as_string);
                #[cfg(feature = "url-assets")]
                let path = path.and_then(|p| cache.cached_path(p).ok());
                if let Some(path) = path
                    && let Ok(variant) = Clear::from_str(node.name().value()).map(Into::into)
                    && let Ok(decoded) = Rodio::decode(&path)
                {
                    sound.sounds.insert(variant, decoded);
                } else {
                    log::warn!("failed to load sound for '{}'", node.name().value())
                }
            }
        }
    }
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
        s => {
            if let Some(c) = s.chars().next()
                && s.chars().count() == 1
            {
                c
            } else {
                return None;
            }
        }
    })
}

fn get_binding(name: &str, bindings: &KdlDocument) -> Result<char> {
    bindings
        .get_arg(name)
        .and_then(KdlValue::as_string)
        .context(format!("need a binding for '{name}'"))
        .and_then(|s| get_key(s).context(format!("invalid key name '{s}' for '{name}' binding")))
}
