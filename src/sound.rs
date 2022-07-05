use anyhow::{anyhow, Result};
use rodio::{
    source::Source, static_buffer::StaticSamplesBuffer, Decoder, OutputStream, OutputStreamHandle,
};

use std::{collections::HashMap, fs::File, io::BufReader};

pub struct Player {
    pub volume: f32,
    _stream: OutputStream,
    handle: OutputStreamHandle,
    sounds: HashMap<String, StaticSamplesBuffer<f32>>,
}

impl Player {
    pub fn new() -> Result<Self> {
        let (_stream, handle) = OutputStream::try_default()?;

        Ok(Self {
            _stream,
            handle,
            sounds: HashMap::new(),
            volume: 0.5,
        })
    }

    pub fn add_sound(&mut self, name: &str, filename: &str) -> Result<()> {
        let decoder = Decoder::new(BufReader::new(File::open(filename).unwrap())).unwrap();
        let (channels, rate, samples) = (
            decoder.channels(),
            decoder.sample_rate(),
            decoder.convert_samples().collect::<Vec<_>>().leak(),
        );
        self.sounds.insert(
            name.to_owned(),
            StaticSamplesBuffer::new(channels, rate, samples),
        );
        Ok(())
    }

    pub fn play(&self, s: &str) -> Result<()> {
        if let Some(sound) = self.sounds.get(s) {
            Ok(self.handle.play_raw(sound.clone().amplify(self.volume))?)
        } else {
            Err(anyhow!("couldn't find sound"))
        }
    }
}
