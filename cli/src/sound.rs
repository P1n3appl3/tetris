use anyhow::{Result, anyhow};
use rodio::{
    Decoder, OutputStream, OutputStreamHandle, source::Source,
    static_buffer::StaticSamplesBuffer,
};

use std::{collections::HashMap, fs::File, io::BufReader};

pub struct RodioPlayer {
    pub volume: f32,
    _stream: OutputStream,
    handle: OutputStreamHandle,
    sounds: HashMap<String, StaticSamplesBuffer<f32>>,
}

impl RodioPlayer {
    pub fn new() -> Result<Self> {
        let (_stream, handle) = OutputStream::try_default()?;

        Ok(Self { _stream, handle, sounds: HashMap::new(), volume: 0.5 })
    }
}

impl tetris::Sound for RodioPlayer {
    fn add_sound(&mut self, name: &str, filename: &str) -> Result<()> {
        let decoder = Decoder::new(BufReader::new(File::open(filename)?))?;
        let (channels, rate, samples) = (
            decoder.channels(),
            decoder.sample_rate(),
            decoder.convert_samples().collect::<Vec<_>>().leak(),
        );
        self.sounds
            .insert(name.to_owned(), StaticSamplesBuffer::new(channels, rate, samples));
        Ok(())
    }

    fn play(&self, s: &str) -> Result<()> {
        if let Some(sound) = self.sounds.get(s) {
            // TODO: cloning the sound on every seems bad. even if volume is dynamic it
            // should probably cache the amplified sounds
            Ok(self.handle.play_raw(sound.clone().amplify(self.volume))?)
        } else {
            Err(anyhow!("couldn't find sound"))
        }
    }

    fn set_volume(&mut self, level: f32) {
        debug_assert!((0.0..=1.0).contains(&level));
        self.volume = level;
    }
}
