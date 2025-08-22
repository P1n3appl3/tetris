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
        let sound = sound.into();
        for sound in std::iter::once(sound).chain(sound.fallback()) {
            if let Some(sample) = self.sounds.get(&sound) {
                self.sink.play(sample)?;
                break;
            }
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

trait Fallback: Copy {
    fn fallback(self) -> impl Iterator<Item = Self>;
}

impl Fallback for Sound {
    fn fallback(self) -> impl Iterator<Item = Self> {
        use Sound::*;
        // TODO: this collect is dumb, use trait objects?
        match self {
            Action(a) => a.fallback().map(Action).collect::<Vec<_>>().into_iter(),
            Meta(m) => m.fallback().map(Meta).collect::<Vec<_>>().into_iter(),
            Clear(c) => c.fallback().map(Clear).collect::<Vec<_>>().into_iter(),
        }
    }
}

impl Fallback for Action {
    fn fallback(self) -> impl Iterator<Item = Self> {
        use Action::*;
        let e = [].iter();
        match self {
            Move => e,
            Rotate => e,
            Spin => [Rotate].iter(),
            MiniSpin => [Spin, Rotate].iter(),
            Land => e,
            HardDrop => [Lock].iter(),
            SoftDrop => [Gravity].iter(),
            Gravity => [SoftDrop].iter(),
            Lock => [HardDrop].iter(),
            Hold => e,
            NoHold => e,
        }
        .copied()
    }
}

impl Fallback for Meta {
    fn fallback(self) -> impl Iterator<Item = Self> {
        [].iter().copied()
    }
}

impl Fallback for Clear {
    fn fallback(self) -> impl Iterator<Item = Self> {
        use Clear::*;
        match self {
            Single => [].iter(),
            Double => [Single].iter(),
            Triple => [Double, Single].iter(),
            Quad => [Triple, Double, Single].iter(),
            Tspin => [Single].iter(),
            TspinDouble => [Tspin, Single].iter(),
            TSpinTriple => [TspinDouble, Tspin, Single].iter(),
        }
        .copied()
    }
}
