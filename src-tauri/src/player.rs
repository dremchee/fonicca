use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use rodio::{Decoder, DeviceSinkBuilder, Player as RodioPlayer};
use rodio::cpal::{self, traits::{DeviceTrait, HostTrait}};
use serde::Serialize;
use rodio::mixer::Mixer;

use crate::error::{Error, Result};
use crate::meter::{Levels, MeterTap, Metered};
use crate::sync::{self, Adjust};

/// Longer files play unadjusted: they are rendered in one pass before playback.
const MAX_ADJUST_SECONDS: f64 = 60.0;

/// Preview player. The output device lives on its own thread (cpal streams are not
/// `Send` on every platform); only the mixer handle is shared.
pub struct Player {
    mixer: Mutex<Option<Mixer>>,
    output: Option<mpsc::Sender<OutputRequest>>,
    device_id: Mutex<Option<String>>,
    current: Mutex<Option<RodioPlayer>>,
    volume: Mutex<f32>,
    /// Maps playback time to time in the file, so positions and seeks stay in
    /// the file's (or edit buffer's) own time.
    timing: Mutex<Timing>,
    /// Levels of what is playing, for the level meter.
    meter: Arc<MeterTap>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputDevices {
    pub devices: Vec<AudioDevice>,
    pub active_device_id: Option<String>,
}

/// Lists capture devices without opening a microphone stream or recording audio.
pub fn input_devices() -> Result<Vec<AudioDevice>> {
    let host = cpal::default_host();
    let default_id = host.default_input_device().and_then(|d| d.id().ok());
    Ok(host.input_devices().map_err(|e| Error::Audio(e.to_string()))?
        .filter_map(|device| {
            let id = device.id().ok()?;
            let name = device.description().ok()?.name().to_owned();
            Some(AudioDevice { is_default: Some(&id) == default_id.as_ref(), id: id.to_string(), name })
        }).collect())
}

pub fn validate_input_device(id: Option<&str>, devices: &[AudioDevice]) -> Result<()> {
    if id.is_none_or(|id| devices.iter().any(|device| device.id == id)) {
        Ok(())
    } else {
        Err(Error::Audio("input device is not available".into()))
    }
}

struct OutputRequest {
    id: Option<String>,
    reply: mpsc::Sender<Result<(Mixer, Option<String>)>>,
}

fn open_output(id: Option<&str>) -> Result<(rodio::MixerDeviceSink, Option<String>)> {
    let host = cpal::default_host();
    let device = if let Some(id) = id {
        host.output_devices().map_err(|e| Error::Audio(e.to_string()))?
            .find(|d| d.id().ok().is_some_and(|v| v.to_string() == id))
            .ok_or_else(|| Error::Audio("output device is not available".into()))?
    } else {
        host.default_output_device().ok_or_else(|| Error::Audio("no output device".into()))?
    };
    let device_id = device.id().ok().map(|v| v.to_string());
    let mut sink = DeviceSinkBuilder::from_device(device)
        .and_then(|b| b.open_sink_or_fallback())
        .map_err(|e| Error::Audio(e.to_string()))?;
    sink.log_on_drop(false);
    Ok((sink, device_id))
}

#[derive(Clone, Copy)]
struct Timing {
    /// Seconds of the source per second of playback (≠ 1 when stretched).
    scale: f64,
    /// Source time where playback started from (for buffer regions).
    offset: f64,
    /// Length of a looping region; positions wrap within it.
    loop_len: Option<f64>,
    /// Initial position within the region, retained when its bounds change.
    phase: f64,
}

impl Timing {
    const PLAIN: Timing = Timing { scale: 1.0, offset: 0.0, loop_len: None, phase: 0.0 };

    fn source_time(&self, played: f64) -> f64 {
        let t = played * self.scale + self.phase;
        self.offset + self.loop_len.filter(|l| *l > 0.0).map_or(t, |l| t % l)
    }
}

impl Player {
    pub fn new(preferred_device: Option<String>) -> Self {
        let (ready_tx, ready_rx) = mpsc::channel();
        let (tx, rx) = mpsc::channel::<OutputRequest>();
        std::thread::Builder::new()
            .name("audio-output".into())
            .spawn(move || {
                // A disconnected saved interface must not prevent app startup.
                let initial = open_output(preferred_device.as_deref()).or_else(|error| {
                    eprintln!("failed to open preferred audio output: {error}");
                    open_output(None)
                });
                let mut sink = match initial {
                    Ok((sink, id)) => {
                        let _ = ready_tx.send((Some(sink.mixer().clone()), id));
                        Some(sink)
                    }
                    Err(error) => {
                        eprintln!("failed to open audio output: {error}");
                        let _ = ready_tx.send((None, None));
                        None
                    }
                };
                while let Ok(request) = rx.recv() {
                    // Open first: a failed switch leaves the previous stream alive.
                    let result = open_output(request.id.as_deref()).map(|(next, id)| {
                        let mixer = next.mixer().clone();
                        sink = Some(next);
                        (mixer, id)
                    });
                    let _ = request.reply.send(result);
                }
                drop(sink);
            })
            .expect("spawn audio thread");
        let (mixer, id) = ready_rx.recv().unwrap_or((None, None));
        Self {
            mixer: Mutex::new(mixer),
            output: Some(tx),
            device_id: Mutex::new(id),
            current: Mutex::new(None),
            volume: Mutex::new(1.0),
            timing: Mutex::new(Timing::PLAIN),
            meter: Arc::default(),
        }
    }

    pub fn output_devices(&self) -> Result<OutputDevices> {
        let host = cpal::default_host();
        let default_id = host.default_output_device().and_then(|d| d.id().ok());
        let devices = host.output_devices().map_err(|e| Error::Audio(e.to_string()))?
            .filter_map(|device| {
                let id = device.id().ok()?;
                let name = device.description().ok()?.name().to_owned();
                Some(AudioDevice { is_default: Some(&id) == default_id.as_ref(), id: id.to_string(), name })
            }).collect();
        Ok(OutputDevices { devices, active_device_id: self.device_id.lock().unwrap().clone() })
    }

    /// Preview and editor share this route. Successful changes stop playback.
    pub fn set_output_device(&self, id: Option<String>) -> Result<()> {
        let mut mixer = self.mixer.lock().unwrap();
        let tx = self.output.as_ref().ok_or_else(|| Error::Audio("audio output thread unavailable".into()))?;
        let (reply, rx) = mpsc::channel();
        tx.send(OutputRequest { id, reply }).map_err(|e| Error::Audio(e.to_string()))?;
        let (next, id) = rx.recv().map_err(|e| Error::Audio(e.to_string()))??;
        self.stop();
        *mixer = Some(next);
        *self.device_id.lock().unwrap() = id;
        Ok(())
    }

    /// Plays from `start` (seconds in the file). Returns the adjustment actually
    /// applied: none for files too long to render up front.
    pub fn play(&self, path: &Path, start: f64, adjust: Adjust) -> Result<Adjust> {
        let output = self.mixer.lock().unwrap();
        let mixer = output.as_ref().ok_or_else(|| Error::Audio("no output device".into()))?;
        if !adjust.is_none() {
            let pcm = crate::audio::decode(path, MAX_ADJUST_SECONDS + 0.5)?;
            if (pcm.frames() as f64) < MAX_ADJUST_SECONDS * pcm.sample_rate as f64 {
                let out = sync::render(&pcm, adjust);
                let channels = std::num::NonZero::new(out.channels as u16).ok_or_else(|| Error::Audio("no channels".into()))?;
                let rate = std::num::NonZero::new(out.sample_rate).ok_or_else(|| Error::Audio("no sample rate".into()))?;
                let player = RodioPlayer::connect_new(mixer);
                player.set_volume(*self.volume.lock().unwrap());
                player.append(Metered::new(rodio::buffer::SamplesBuffer::new(channels, rate, out.samples), Arc::clone(&self.meter)));
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
        player.append(Metered::new(source, Arc::clone(&self.meter)));
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
        self.play_buffer_from(pcm, start, end, looping, start)
    }

    /// Changes an editor region without moving the playhead back to its beginning.
    pub fn update_buffer_range(&self, pcm: &crate::audio::Pcm, start: usize, end: usize, looping: bool) -> Result<bool> {
        let Some(seconds) = self.position() else { return Ok(false) };
        let position = (seconds * pcm.sample_rate as f64).round() as usize;
        let end = end.min(pcm.frames());
        if end <= start || (position >= end && !looping) {
            self.stop();
            return Ok(false);
        }
        let position = if position >= end { start } else { position.max(start) };
        self.play_buffer_from(pcm, start, end, looping, position)?;
        Ok(true)
    }

    fn play_buffer_from(&self, pcm: &crate::audio::Pcm, start: usize, end: usize, looping: bool, position: usize) -> Result<()> {
        use rodio::Source;
        let output = self.mixer.lock().unwrap();
        let mixer = output.as_ref().ok_or_else(|| Error::Audio("no output device".into()))?;
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
        let sr = pcm.sample_rate as f64;
        let phase = position.saturating_sub(start) as f64 / sr;
        let skip = Duration::from_secs_f64(phase);
        if looping {
            player.append(Metered::new(buffer.repeat_infinite().skip_duration(skip), Arc::clone(&self.meter)));
        } else {
            player.append(Metered::new(buffer.skip_duration(skip), Arc::clone(&self.meter)));
        }
        *self.timing.lock().unwrap() = Timing {
            scale: 1.0,
            offset: start as f64 / sr,
            loop_len: looping.then(|| (end - start) as f64 / sr),
            phase,
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

    /// Keep the decoder and its exact source position for a later resume.
    pub fn pause(&self) -> Option<f64> {
        let timing = *self.timing.lock().unwrap();
        let guard = self.current.lock().unwrap();
        let player = guard.as_ref().filter(|p| !p.empty())?;
        player.pause();
        Some(timing.source_time(player.get_pos().as_secs_f64()))
    }

    pub fn resume(&self) -> bool {
        let guard = self.current.lock().unwrap();
        let Some(player) = guard.as_ref().filter(|p| !p.empty()) else { return false };
        player.play();
        true
    }

    pub fn seek(&self, pos: f64) -> Result<()> {
        if let Some(p) = self.current.lock().unwrap().as_ref() {
            let t = *self.timing.lock().unwrap();
            let played = ((pos - t.offset - t.phase) / t.scale).max(0.0);
            p.try_seek(Duration::from_secs_f64(played)).map_err(|e| Error::Audio(e.to_string()))?;
        }
        Ok(())
    }

    /// Peak and RMS per channel since the last call, as heard (volume applied);
    /// `None` when nothing is playing.
    pub fn levels(&self, spectrum: bool) -> Option<Levels> {
        let playing = self.current.lock().unwrap().as_ref().is_some_and(|p| !p.empty() && !p.is_paused());
        playing.then(|| self.meter.drain_with_spectrum(*self.volume.lock().unwrap(), spectrum))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recording_input_validation_accepts_system_and_available_devices_only() {
        let devices = vec![AudioDevice { id: "test:microphone".into(), name: "Microphone".into(), is_default: true }];
        assert!(validate_input_device(None, &[]).is_ok());
        assert!(validate_input_device(Some("test:microphone"), &devices).is_ok());
        assert!(validate_input_device(Some("test:missing"), &devices).is_err());
    }

    #[test]
    #[ignore = "requires an available system audio output"]
    fn system_output_can_be_listed_selected_and_reopened() {
        let player = Player::new(Some("missing:disconnected-interface".into()));
        let inputs = input_devices().unwrap();
        assert!(!inputs.is_empty());
        assert!(inputs.iter().all(|device| !device.id.is_empty() && !device.name.is_empty()));
        let outputs = player.output_devices().unwrap();
        assert!(!outputs.devices.is_empty());
        let active = outputs.active_device_id.unwrap();
        assert!(outputs.devices.iter().any(|d| d.id == active));
        player.set_output_device(Some(active.clone())).unwrap();
        assert_eq!(player.output_devices().unwrap().active_device_id, Some(active));
        assert!(player.set_output_device(Some("missing:unknown-interface".into())).is_err());
        player.set_output_device(None).unwrap();
        assert!(player.output_devices().unwrap().active_device_id.is_some());
    }

    #[test]
    fn device_switch_stops_playback_but_keeps_volume() {
        let (mixer, _output) = rodio::mixer::mixer(
            std::num::NonZero::new(1).unwrap(), std::num::NonZero::new(48000).unwrap(),
        );
        let (tx, rx) = mpsc::channel::<OutputRequest>();
        let replacement = mixer.clone();
        let worker = std::thread::spawn(move || {
            let request = rx.recv().unwrap();
            assert_eq!(request.id.as_deref(), Some("test:interface"));
            request.reply.send(Ok((replacement, request.id))).unwrap();
        });
        let player = Player {
            mixer: Mutex::new(Some(mixer)), output: Some(tx), device_id: Mutex::new(None),
            current: Mutex::new(None), volume: Mutex::new(0.35),
            timing: Mutex::new(Timing::PLAIN), meter: Arc::default(),
        };
        let pcm = crate::audio::Pcm { samples: vec![0.25; 48000], channels: 1, sample_rate: 48000 };
        player.play_buffer(&pcm, 0, 48000, true).unwrap();
        assert!(player.position().is_some());
        player.set_output_device(Some("test:interface".into())).unwrap();
        assert!(player.position().is_none());
        assert!(!player.resume());
        assert_eq!(*player.volume.lock().unwrap(), 0.35);
        assert_eq!(player.device_id.lock().unwrap().as_deref(), Some("test:interface"));
        player.play_buffer(&pcm, 0, 48000, false).unwrap();
        assert!(player.position().is_some());
        worker.join().unwrap();
    }

    #[test]
    fn failed_device_switch_keeps_the_previous_source_and_output() {
        let (mixer, _output) = rodio::mixer::mixer(
            std::num::NonZero::new(1).unwrap(), std::num::NonZero::new(48000).unwrap(),
        );
        let (tx, rx) = mpsc::channel::<OutputRequest>();
        let worker = std::thread::spawn(move || {
            let request = rx.recv().unwrap();
            request.reply.send(Err(Error::Audio("device unavailable".into()))).unwrap();
        });
        let player = Player {
            mixer: Mutex::new(Some(mixer)), output: Some(tx), device_id: Mutex::new(Some("test:old".into())),
            current: Mutex::new(None), volume: Mutex::new(0.5),
            timing: Mutex::new(Timing::PLAIN), meter: Arc::default(),
        };
        let pcm = crate::audio::Pcm { samples: vec![0.25; 48000], channels: 1, sample_rate: 48000 };
        player.play_buffer(&pcm, 0, 48000, true).unwrap();
        assert!(player.set_output_device(Some("test:missing".into())).is_err());
        assert!(player.position().is_some());
        assert_eq!(player.device_id.lock().unwrap().as_deref(), Some("test:old"));
        worker.join().unwrap();
    }

    #[test]
    fn resizing_editor_range_preserves_position_and_uses_new_loop_bounds() {
        let (mixer, mut output) = rodio::mixer::mixer(
            std::num::NonZero::new(1).unwrap(), std::num::NonZero::new(48000).unwrap(),
        );
        let player = Player {
            mixer: Mutex::new(Some(mixer)), output: None, device_id: Mutex::new(None), current: Mutex::new(None), volume: Mutex::new(1.0),
            timing: Mutex::new(Timing::PLAIN), meter: Arc::default(),
        };
        let pcm = crate::audio::Pcm { samples: vec![0.25; 48000 * 4], channels: 1, sample_rate: 48000 };
        player.play_buffer(&pcm, 48000, 48000 * 4, true).unwrap();
        for _ in 0..48000 { output.next(); }
        let before = player.position().unwrap();
        assert!(before > 1.9);
        assert!(player.update_buffer_range(&pcm, 72000, 144000, true).unwrap());
        assert!((player.position().unwrap() - before).abs() < 0.001);
        for _ in 0..60000 { output.next(); }
        let wrapped = player.position().unwrap();
        assert!((1.5..2.0).contains(&wrapped), "position after loop: {wrapped}");
        // A non-looping range shortened past the playhead ends rather than restarting.
        assert!(!player.update_buffer_range(&pcm, 0, 48000, false).unwrap());
        assert_eq!(player.position(), None);
        assert!(!player.update_buffer_range(&pcm, 0, 96000, true).unwrap());
    }

    #[test]
    fn resized_non_looping_region_continues_until_the_new_end() {
        let (mixer, mut output) = rodio::mixer::mixer(
            std::num::NonZero::new(1).unwrap(), std::num::NonZero::new(48000).unwrap(),
        );
        let player = Player {
            mixer: Mutex::new(Some(mixer)), output: None, device_id: Mutex::new(None), current: Mutex::new(None), volume: Mutex::new(1.0),
            timing: Mutex::new(Timing::PLAIN), meter: Arc::default(),
        };
        let pcm = crate::audio::Pcm { samples: vec![0.25; 48000 * 4], channels: 1, sample_rate: 48000 };
        player.play_buffer(&pcm, 48000, 192000, false).unwrap();
        for _ in 0..48000 { output.next(); }
        let before = player.position().unwrap();
        assert!(player.update_buffer_range(&pcm, 72000, 144000, false).unwrap());
        assert!((player.position().unwrap() - before).abs() < 0.001);
        for _ in 0..24000 { output.next(); }
        assert!(player.position().unwrap() > 2.4);
        for _ in 0..48000 { output.next(); }
        assert_eq!(player.position(), None);
    }

    #[test]
    fn pause_keeps_the_source_position_and_resume_continues_the_same_buffer() {
        // Drive a mixer manually: no sound device or wall-clock sleeps needed.
        let (mixer, mut output) = rodio::mixer::mixer(
            std::num::NonZero::new(1).unwrap(), std::num::NonZero::new(48000).unwrap(),
        );
        let sink = RodioPlayer::connect_new(&mixer);
        sink.append(rodio::buffer::SamplesBuffer::new(
            std::num::NonZero::new(1).unwrap(), std::num::NonZero::new(48000).unwrap(), vec![0.25; 48000],
        ));
        let player = Player {
            mixer: Mutex::new(Some(mixer)), output: None, device_id: Mutex::new(None), current: Mutex::new(Some(sink)), volume: Mutex::new(1.0),
            timing: Mutex::new(Timing { scale: 0.5, offset: 2.0, loop_len: None, phase: 0.0 }), meter: Arc::default(),
        };
        for _ in 0..4800 { output.next(); }
        let paused_at = player.pause().unwrap();
        assert!(paused_at > 2.0);
        assert!(player.current.lock().unwrap().as_ref().unwrap().is_paused());
        for _ in 0..4800 { output.next(); }
        // Rodio applies controls and publishes position at 5 ms boundaries.
        // Once that boundary is processed, paused playback cannot advance.
        let settled = player.position().unwrap();
        assert!((settled - paused_at).abs() <= 0.005 * 0.5 + f64::EPSILON);
        for _ in 0..4800 { output.next(); }
        assert_eq!(player.position(), Some(settled));
        assert!(player.levels(false).is_none());
        assert!(player.resume());
        for _ in 0..4800 { output.next(); }
        assert!(player.position().unwrap() > paused_at);
        player.stop();
        assert_eq!(player.position(), None);
        assert_eq!(player.pause(), None);
        assert!(!player.resume());
    }
}
