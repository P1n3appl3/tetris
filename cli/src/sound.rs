use std::{fs::File, path::Path};

use anyhow::Result;
use rodio::{
    Decoder, OutputStream, OutputStreamBuilder, source::Source, static_buffer::StaticSamplesBuffer,
};

use tetris::sound;

pub struct Rodio {
    pub volume: f64,
    stream: OutputStream,
}

impl Rodio {
    pub fn new() -> Result<Self> {
        let stream = OutputStreamBuilder::from_default_device()?.open_stream()?;
        Ok(Self { stream, volume: 0.5 })
    }

    pub fn decode(path: &Path) -> Result<StaticSamplesBuffer> {
        let decoder = Decoder::try_from(File::open(path)?)?;
        let (channels, rate, samples) = (
            decoder.channels(),
            decoder.sample_rate(),
            decoder.into_iter().collect::<Vec<f32>>().leak(),
        );
        Ok(StaticSamplesBuffer::new(channels, rate, samples))
    }
}

impl sound::Sink for Rodio {
    type Asset = StaticSamplesBuffer;
    // TODO: cloning the sound on every play seems bad. even if volume is dynamic it
    // should maybe cache the amplified sounds
    fn play(&self, s: &Self::Asset) -> Result<()> {
        let sink = rodio::Sink::connect_new(self.stream.mixer());
        sink.append(s.clone().amplify_normalized(self.volume as f32));
        sink.detach();
        Ok(())
    }

    fn set_volume(&mut self, level: f64) {
        debug_assert!((0.0..=1.0).contains(&level));
        self.volume = level;
    }
}
