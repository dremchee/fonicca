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
    /// Maps playback time to time in the file, so positions and seeks stay in
    /// the file's (or edit buffer's) own time.
    timing: Mutex<Timing>,
}

#[derive(Clone, Copy)]
struct Timing {
    /// Seconds of the source per second of playback (≠ 1 when stretched).
    scale: f64,
    /// Source time where playback started from (for buffer regions).
    offset: f64,
    /// Length of a looping region; positions wrap within it.
    loop_len: Option<f64>,
}

impl Timing {
    const PLAIN: Timing = Timing { scale: 1.0, offset: 0.0, loop_len: None };

    fn source_time(&self, played: f64) -> f64 {
        let t = played * self.scale;
        self.offset + self.loop_len.filter(|l| *l > 0.0).map_or(t, |l| t % l)
    }
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
            timing: Mutex::new(Timing::PLAIN),
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
                *self.timing.lock().unwrap() = Timing { scale, ..Timing::PLAIN };
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
        *self.timing.lock().unwrap() = Timing::PLAIN;
        *self.current.lock().unwrap() = Some(player);
        Ok(Adjust::NONE)
    }

    /// Plays frames `[start, end)` of an in-memory buffer (the editor), optionally
    /// looping. Positions are reported in seconds of the buffer.
    pub fn play_buffer(&self, pcm: &crate::audio::Pcm, start: usize, end: usize, looping: bool) -> Result<()> {
        use rodio::Source;
        let mixer = self.mixer.as_ref().ok_or_else(|| Error::Audio("no output device".into()))?;
        let ch = pcm.channels.max(1);
        let (start, end) = (start.min(pcm.frames()), end.min(pcm.frames()));
        if end <= start {
            return Ok(());
        }
        let channels = std::num::NonZero::new(ch as u16).ok_or_else(|| Error::Audio("no channels".into()))?;
        let rate = std::num::NonZero::new(pcm.sample_rate).ok_or_else(|| Error::Audio("no sample rate".into()))?;
        let buffer = rodio::buffer::SamplesBuffer::new(channels, rate, pcm.samples[start * ch..end * ch].to_vec());
        let player = RodioPlayer::connect_new(mixer);
        player.set_volume(*self.volume.lock().unwrap());
        if looping {
            player.append(buffer.repeat_infinite());
        } else {
            player.append(buffer);
        }
        let sr = pcm.sample_rate as f64;
        *self.timing.lock().unwrap() = Timing {
            scale: 1.0,
            offset: start as f64 / sr,
            loop_len: looping.then(|| (end - start) as f64 / sr),
        };
        *self.current.lock().unwrap() = Some(player);
        Ok(())
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
            let t = *self.timing.lock().unwrap();
            let played = ((pos - t.offset) / t.scale).max(0.0);
            p.try_seek(Duration::from_secs_f64(played)).map_err(|e| Error::Audio(e.to_string()))?;
        }
        Ok(())
    }

    /// Current position in seconds of the file, or `None` when nothing is playing.
    pub fn position(&self) -> Option<f64> {
        let timing = *self.timing.lock().unwrap();
        let mut guard = self.current.lock().unwrap();
        match guard.as_ref() {
            Some(p) if !p.empty() => Some(timing.source_time(p.get_pos().as_secs_f64())),
            Some(_) => {
                guard.take();
                None
            }
            None => None,
        }
    }
}
