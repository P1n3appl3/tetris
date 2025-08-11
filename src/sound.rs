use std::{collections::HashMap, hash::Hash};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use strum::EnumString;

#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Sound {
    Action(Action),
    Meta(Meta),
    Clear(Clear),
}

#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug, Serialize, Deserialize, EnumString)]
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

#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug, Serialize, Deserialize, EnumString)]
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

#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug, Serialize, Deserialize, EnumString)]
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

pub struct SoundPlayer<T: Sink> {
    pub sink: T,
    pub sounds: HashMap<Sound, T::Asset>,
}

impl<T: Sink> SoundPlayer<T> {
    pub fn play(&self, sound: impl Into<Sound>) -> Result<()> {
        if let Some(sample) = self.sounds.get(&sound.into()) {
            self.sink.play(sample)?;
        }
        Ok(())
    }
}

pub trait Sink {
    type Asset;
    fn play(&self, sample: &Self::Asset) -> Result<()>;
    fn set_volume(&mut self, level: f64);
}

#[derive(Default, Clone, Copy)]
pub struct NullSink;

impl Sink for NullSink {
    type Asset = ();

    fn play(&self, _sample: &Self::Asset) -> Result<()> {
        Ok(())
    }

    fn set_volume(&mut self, _level: f64) {}
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

impl<T: Sink + Default> Default for SoundPlayer<T> {
    fn default() -> Self {
        Self { sink: T::default(), sounds: HashMap::new() }
    }
}

impl<T: Sink> From<T> for SoundPlayer<T> {
    fn from(sink: T) -> Self {
        Self { sink, sounds: HashMap::new() }
    }
}
