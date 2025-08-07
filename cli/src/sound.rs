use std::{collections::HashMap, fs::File, io::BufReader};

use anyhow::{Result, anyhow};
use directories::ProjectDirs;
use rodio::{
    Decoder, OutputStream, OutputStreamHandle, source::Source, static_buffer::StaticSamplesBuffer,
};
use tetris::sound::Sound;

pub struct Rodio {
    pub volume: f32,
    _stream: OutputStream,
    handle: OutputStreamHandle,
    sounds: HashMap<Sound, StaticSamplesBuffer<f32>>,
    #[cfg(feature = "url-assets")]
    cache: cached_path::Cache,
}

impl Rodio {
    pub fn new(dirs: &ProjectDirs) -> Result<Self> {
        let (_stream, handle) = OutputStream::try_default()?;

        #[cfg(feature = "url-assets")]
        let cache = cached_path::CacheBuilder::new()
            .dir(dirs.cache_dir().to_path_buf())
            .client_builder(reqwest::blocking::ClientBuilder::new().user_agent("tetris"))
            .build()?;

        Ok(Self {
            _stream,
            volume: 0.5,
            handle,
            sounds: HashMap::new(),
            #[cfg(feature = "url-assets")]
            cache,
        })
    }
}

impl tetris::sound::Sink for Rodio {
    type Asset<'a> = &'a str;
    fn add_sound<T>(&mut self, sound: T, filename: Self::Asset<'_>) -> Result<()> {
        #[cfg(feature = "url-assets")]
        let filename = self.cache.cached_path(&filename)?;

        let decoder = Decoder::new(BufReader::new(File::open(filename)?))?;
        let (channels, rate, samples) = (
            decoder.channels(),
            decoder.sample_rate(),
            decoder.convert_samples().collect::<Vec<_>>().leak(),
        );
        self.sounds.insert(sound.into(), StaticSamplesBuffer::new(channels, rate, samples));
        Ok(())
    }

    fn play<T>(&self, s: T) -> Result<()> {
        if let Some(sound) = self.sounds.get(s.into()) {
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
