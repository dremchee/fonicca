use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::{mpsc, Mutex};
use std::time::Duration;

use rodio::{Decoder, DeviceSinkBuilder, Player as RodioPlayer};
use rodio::mixer::Mixer;

use crate::error::{Error, Result};
use crate::sync::{self, Adjust};

/// Longer files play unadjusted: they are rendered in one pass before playback.
const MAX_ADJUST_SECONDS: f64 = 60.0;

/// Preview player. The output device lives on its own thread (cpal streams are not
/// `Send` on every platform); only the mixer handle is shared.
pub struct Player {
    mixer: Option<Mixer>,
    current: Mutex<Option<RodioPlayer>>,
    volume: Mutex<f32>,
    /// Seconds of the original file per second of playback (≠ 1 when stretched),
    /// so positions and seeks stay in the file's own time.
    time_scale: Mutex<f64>,
}

impl Player {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("audio-output".into())
            .spawn(move || match DeviceSinkBuilder::open_default_sink() {
                Ok(mut sink) => {
                    sink.log_on_drop(false);
                    let _ = tx.send(Some(sink.mixer().clone()));
                    // Keep the stream alive for the lifetime of the app.
                    loop {
                        std::thread::park();
                    }
                }
                Err(e) => {
                    eprintln!("failed to open audio output: {e}");
                    let _ = tx.send(None);
                }
            })
            .expect("spawn audio thread");
        Self {
            mixer: rx.recv().ok().flatten(),
            current: Mutex::new(None),
            volume: Mutex::new(1.0),
            time_scale: Mutex::new(1.0),
        }
    }

    /// Plays from `start` (seconds in the file). Returns the adjustment actually
    /// applied: none for files too long to render up front.
    pub fn play(&self, path: &Path, start: f64, adjust: Adjust) -> Result<Adjust> {
        let mixer = self.mixer.as_ref().ok_or_else(|| Error::Audio("no output device".into()))?;
        if !adjust.is_none() {
            let pcm = crate::audio::decode(path, MAX_ADJUST_SECONDS + 0.5)?;
            if (pcm.frames() as f64) < MAX_ADJUST_SECONDS * pcm.sample_rate as f64 {
                let out = sync::render(&pcm, adjust);
                let channels = std::num::NonZero::new(out.channels as u16).ok_or_else(|| Error::Audio("no channels".into()))?;
                let rate = std::num::NonZero::new(out.sample_rate).ok_or_else(|| Error::Audio("no sample rate".into()))?;
                let player = RodioPlayer::connect_new(mixer);
                player.set_volume(*self.volume.lock().unwrap());
                player.append(rodio::buffer::SamplesBuffer::new(channels, rate, out.samples));
                let scale = 1.0 / adjust.stretch;
                if start > 0.0 {
                    let _ = player.try_seek(Duration::from_secs_f64(start / scale));
                }
                *self.time_scale.lock().unwrap() = scale;
                *self.current.lock().unwrap() = Some(player);
                return Ok(adjust);
            }
        }

        let file = File::open(path)?;
        let len = file.metadata()?.len();
        let mut builder = Decoder::builder()
            .with_data(BufReader::new(file))
            .with_byte_len(len)
            .with_seekable(true);
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            builder = builder.with_hint(ext);
        }
        let source = builder.build().map_err(|e| Error::Audio(e.to_string()))?;

        let player = RodioPlayer::connect_new(mixer);
        player.set_volume(*self.volume.lock().unwrap());
        player.append(source);
        if start > 0.0 {
            let _ = player.try_seek(Duration::from_secs_f64(start));
        }
        // Replacing the previous player drops it, which stops its sound.
        *self.time_scale.lock().unwrap() = 1.0;
        *self.current.lock().unwrap() = Some(player);
        Ok(Adjust::NONE)
    }

    /// Preview volume, 0..=1; also applies to the sound playing now.
    pub fn set_volume(&self, volume: f32) {
        *self.volume.lock().unwrap() = volume;
        if let Some(p) = self.current.lock().unwrap().as_ref() {
            p.set_volume(volume);
        }
    }

    pub fn stop(&self) {
        self.current.lock().unwrap().take();
    }

    pub fn seek(&self, pos: f64) -> Result<()> {
        if let Some(p) = self.current.lock().unwrap().as_ref() {
            let scale = *self.time_scale.lock().unwrap();
            p.try_seek(Duration::from_secs_f64(pos / scale)).map_err(|e| Error::Audio(e.to_string()))?;
        }
        Ok(())
    }

    /// Current position in seconds of the file, or `None` when nothing is playing.
    pub fn position(&self) -> Option<f64> {
        let scale = *self.time_scale.lock().unwrap();
        let mut guard = self.current.lock().unwrap();
        match guard.as_ref() {
            Some(p) if !p.empty() => Some(p.get_pos().as_secs_f64() * scale),
            Some(_) => {
                guard.take();
                None
            }
            None => None,
        }
    }
}
