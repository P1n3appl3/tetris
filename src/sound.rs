use std::io::BufRead;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use strum::EnumString;

#[derive(Copy, Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Sound {
    Action(Action),
    Meta(Meta),
    Clear(Clear),
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Serialize, Deserialize, EnumString)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum Meta {
    Ready,
    Go,
    Lose,
    Win,
    Fault,
    Garbage,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Serialize, Deserialize, EnumString)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum Action {
    Move,
    Rotate,
    Spin,
    MiniSpin,
    Land,
    HardDrop,
    SoftDrop,
    Gravity,
    Lock,
    Hold,
    NoHold,
    // TODO: land but for hitting a wall, no- variants for rotate/move
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Serialize, Deserialize, EnumString)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum Clear {
    Single,
    Double,
    Triple,
    Quad,
    Tspin,
    TspinDouble,
    TSpinTriple,
    // TODO: make these modifiers since any permutation of them can be true for a given clear.
    // if i get a 5combo mini-zspin-double b2b pc which sounds should play?
    // all of them? highest precedence? configurable per sound for the current settings?
    // PerfectClear,
    // Combo(NonZeroU8),
    // ComboBreak,
    // BackToBack(NonZeroU8),
}

pub trait Sink {
    type Asset;
    // TODO: asyncify this to lazy-load sounds while playing?
    fn add_sound(&mut self, encoded_audio: impl BufRead) -> Result<Self::Asset>;
    fn play(&self, decoded_audio: Self::Asset) -> Result<()>;
    fn set_volume(&mut self, level: f32);
}

pub struct NullSink;

impl Sink for NullSink {
    type Asset = ();
    fn add_sound(&mut self, encoded_audio: impl BufRead) -> Result<Self::Asset> {
        Ok(())
    }

    fn play(&self, decoded_audio: Self::Asset) -> Result<()> {
        Ok(())
    }

    fn set_volume(&mut self, level: f32) {}
}

impl From<Meta> for Sound {
    fn from(value: Meta) -> Self {
        Self::Meta(value)
    }
}

impl From<Action> for Sound {
    fn from(value: Action) -> Self {
        Self::Action(value)
    }
}

impl From<Clear> for Sound {
    fn from(value: Clear) -> Self {
        Self::Clear(value)
    }
}
