use crate::audio::AudioRead;
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
    requests: Mutex<u64>,
    /// Serialize heavy preparation without blocking stop, controls or output changes.
    preparing: Mutex<()>,
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

/// Immutable source snapshot: no range copy when starting, looping or seeking.
#[derive(Clone)]
struct SharedSource<A> { audio:A,start:usize,end:usize,position:usize,begin:usize,looping:bool,cursor:usize }
impl<A:AudioRead> Iterator for SharedSource<A> {
    type Item=f32;
    fn next(&mut self)->Option<f32> {if self.position>=self.end {if self.looping && self.start<self.end {self.position=self.start}else{return None}}let sample=self.audio.sample_with_cursor(self.position,&mut self.cursor);self.position+=1;Some(sample)}
    fn size_hint(&self)->(usize,Option<usize>) {if self.looping {(0,None)}else{let n=self.end-self.position;(n,Some(n))}}
}
impl<A:AudioRead+Clone> rodio::Source for SharedSource<A> {
    fn current_span_len(&self)->Option<usize> {Some(if self.looping && self.position==self.end {self.end-self.start}else{self.end-self.position})}
    fn channels(&self)->std::num::NonZero<u16> {std::num::NonZero::new(self.audio.channels() as u16).unwrap()}
    fn sample_rate(&self)->std::num::NonZero<u32> {std::num::NonZero::new(self.audio.sample_rate()).unwrap()}
    fn total_duration(&self)->Option<Duration> {if self.looping {None}else{Some(Duration::from_secs_f64((self.end-self.begin) as f64/self.audio.channels().max(1) as f64/self.audio.sample_rate() as f64))}}
    fn try_seek(&mut self,pos:Duration)->std::result::Result<(),rodio::source::SeekError> {
        let ch=self.audio.channels().max(1);
        let offset=(pos.as_secs_f64()*self.audio.sample_rate() as f64) as usize;
        let target=self.begin.saturating_add(offset.saturating_mul(ch));
        self.position=if self.looping {self.start+(target-self.start)%(self.end-self.start)}else{target.min(self.end)};
        Ok(())
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
            requests: Mutex::new(0), preparing: Mutex::new(()),
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

    /// Capture intent before database/decoder work begins, not after it completes.
    pub(crate) fn begin_request(&self) -> u64 {
        let mut generation = self.requests.lock().unwrap();
        *generation = generation.wrapping_add(1);
        *generation
    }

    fn request_guard(&self, request: u64) -> Result<std::sync::MutexGuard<'_, u64>> {
        let guard = self.requests.lock().unwrap();
        if *guard != request { return Err(Error::Other("playback superseded".into())); }
        Ok(guard)
    }

    /// A failed latest load cannot leave an unrepresented older source audible.
    pub(crate) fn fail_request(&self, request: u64) {
        let mut generation = self.requests.lock().unwrap();
        if *generation == request {
            *generation = generation.wrapping_add(1);
            self.current.lock().unwrap().take();
        }
    }

    /// Plays from `start` (seconds in the file). Preparation stays off the mixer
    /// lock, allowing stop and output changes while stretching a preview.
    pub(crate) fn play_request(&self, request: u64, path: &Path, start: f64, adjust: Adjust) -> Result<Adjust> {
        // Queued superseded requests fail before decoding. One old render may
        // finish, but rapid clicks cannot render many PCM copies in parallel.
        let _preparing = self.preparing.lock().unwrap();
        let result = self.prepare_file(request, path, start, adjust);
        if result.is_err() { self.fail_request(request); }
        result
    }

    fn prepare_file(&self, request: u64, path: &Path, start: f64, adjust: Adjust) -> Result<Adjust> {
        drop(self.request_guard(request)?);
        if !adjust.is_none() {
            let pcm = crate::audio::decode(path, MAX_ADJUST_SECONDS + 0.5)?;
            drop(self.request_guard(request)?);
            if (pcm.frames() as f64) < MAX_ADJUST_SECONDS * pcm.sample_rate as f64 {
                let out = sync::render(&pcm, adjust);
                let channels = std::num::NonZero::new(out.channels as u16).ok_or_else(|| Error::Audio("no channels".into()))?;
                let rate = std::num::NonZero::new(out.sample_rate).ok_or_else(|| Error::Audio("no sample rate".into()))?;
                let scale = 1.0 / adjust.stretch;
                return self.install(request, rodio::buffer::SamplesBuffer::new(channels, rate, out.samples),
                    Timing { scale, ..Timing::PLAIN }, start / scale, adjust);
            }
        }
        let file = File::open(path)?;
        let len = file.metadata()?.len();
        let mut builder = Decoder::builder()
            .with_data(BufReader::new(file))
            .with_byte_len(len)
            .with_seekable(true);
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) { builder = builder.with_hint(ext); }
        let source = builder.build().map_err(|e| Error::Audio(e.to_string()))?;
        self.install(request, source, Timing::PLAIN, start, Adjust::NONE)
    }

    fn install<S: rodio::Source + Send + 'static>(&self, request: u64, source: S,
        timing: Timing, start: f64, applied: Adjust) -> Result<Adjust> {
        let output = self.mixer.lock().unwrap();
        // Always mixer -> request gate. Output switching uses the same order.
        let _request = self.request_guard(request)?;
        let mixer = output.as_ref().ok_or_else(|| Error::Audio("no output device".into()))?;
        let player = RodioPlayer::connect_new(mixer);
        player.set_volume(*self.volume.lock().unwrap());
        player.pause();
        player.append(Metered::new(source, Arc::clone(&self.meter)));
        if start > 0.0 { let _ = player.try_seek(Duration::from_secs_f64(start)); }
        *self.timing.lock().unwrap() = timing;
        let mut current = self.current.lock().unwrap();
        *current = Some(player);
        current.as_ref().unwrap().play();
        Ok(applied)
    }

    /// Plays frames `[start, end)` of an in-memory buffer (the editor), optionally
    /// looping. Positions are reported in seconds of the buffer.
    #[cfg(test)]
    pub fn play_buffer<A:AudioRead+Clone+Send+Sync+'static>(&self, pcm: &A, start: usize, end: usize, looping: bool) -> Result<()> {
        self.play_buffer_request(self.begin_request(), pcm, start, end, looping)
    }

    pub(crate) fn play_buffer_request<A:AudioRead+Clone+Send+Sync+'static>(&self, request: u64, pcm: &A, start: usize, end: usize, looping: bool) -> Result<()> {
        self.play_buffer_from(request, pcm, start, end, looping, start)
    }

    /// Changes an editor region without moving the playhead back to its beginning.
    #[cfg(test)]
    pub fn update_buffer_range<A:AudioRead+Clone+Send+Sync+'static>(&self, pcm: &A, start: usize, end: usize, looping: bool) -> Result<bool> {
        self.update_buffer_request(self.begin_request(), pcm, start, end, looping)
    }

    pub(crate) fn update_buffer_request<A:AudioRead+Clone+Send+Sync+'static>(&self, request: u64, pcm: &A, start: usize, end: usize, looping: bool) -> Result<bool> {
        let seconds = {
            let _request = self.request_guard(request)?;
            self.position_unlocked()
        };
        let Some(seconds) = seconds else { return Ok(false) };
        let position = (seconds * pcm.sample_rate() as f64).round() as usize;
        let end = end.min(pcm.frames());
        if end <= start || (position >= end && !looping) {
            self.fail_request(request);
            return Ok(false);
        }
        let position = if position >= end { start } else { position.max(start) };
        self.play_buffer_from(request, pcm, start, end, looping, position)?;
        Ok(true)
    }

    fn play_buffer_from<A:AudioRead+Clone+Send+Sync+'static>(&self, request: u64, pcm: &A, start: usize, end: usize, looping: bool, position: usize) -> Result<()> {
        let ch = pcm.channels().max(1);
        let (start, end) = (start.min(pcm.frames()), end.min(pcm.frames()));
        if end <= start {
            return Ok(());
        }
        let _channels = std::num::NonZero::new(ch as u16).ok_or_else(|| Error::Audio("no channels".into()))?;
        let _rate = std::num::NonZero::new(pcm.sample_rate()).ok_or_else(|| Error::Audio("no sample rate".into()))?;
        let buffer = SharedSource {audio:pcm.clone(),start:start*ch,end:end*ch,position:position*ch,begin:position*ch,looping,cursor:0};
        let sr = pcm.sample_rate() as f64;
        let phase = position.saturating_sub(start) as f64 / sr;
        let timing = Timing {
            scale: 1.0,
            offset: start as f64 / sr,
            loop_len: looping.then(|| (end - start) as f64 / sr),
            phase,
        };
        self.install(request, buffer, timing, 0.0, Adjust::NONE)?;
        Ok(())
    }

    /// Preview volume, 0..=1; also applies to the sound playing now.
    pub fn set_volume(&self, volume: f32) {
        let _request = self.requests.lock().unwrap();
        *self.volume.lock().unwrap() = volume;
        if let Some(p) = self.current.lock().unwrap().as_ref() {
            p.set_volume(volume);
        }
    }

    pub fn stop(&self) {
        let mut generation = self.requests.lock().unwrap();
        *generation = generation.wrapping_add(1);
        self.current.lock().unwrap().take();
    }

    /// Keep the decoder and its exact source position for a later resume.
    pub fn pause(&self) -> Option<f64> {
        let mut generation = self.requests.lock().unwrap();
        *generation = generation.wrapping_add(1);
        let timing = *self.timing.lock().unwrap();
        let guard = self.current.lock().unwrap();
        let player = guard.as_ref().filter(|p| !p.empty())?;
        player.pause();
        Some(timing.source_time(player.get_pos().as_secs_f64()))
    }

    pub fn resume(&self) -> bool {
        let mut generation = self.requests.lock().unwrap();
        *generation = generation.wrapping_add(1);
        let guard = self.current.lock().unwrap();
        let Some(player) = guard.as_ref().filter(|p| !p.empty()) else { return false };
        player.play();
        true
    }

    pub fn seek(&self, pos: f64) -> Result<()> {
        let mut generation = self.requests.lock().unwrap();
        *generation = generation.wrapping_add(1);
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
        let _request = self.requests.lock().unwrap();
        let playing = self.current.lock().unwrap().as_ref().is_some_and(|p| !p.empty() && !p.is_paused());
        playing.then(|| self.meter.drain_with_spectrum(*self.volume.lock().unwrap(), spectrum))
    }

    /// Current position in seconds of the file, or `None` when nothing is playing.
    pub fn position(&self) -> Option<f64> {
        let _request = self.requests.lock().unwrap();
        self.position_unlocked()
    }

    fn position_unlocked(&self) -> Option<f64> {
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

    fn fixture() -> (Player, impl Iterator<Item = f32>) {
        let (mixer, output) = rodio::mixer::mixer(
            std::num::NonZero::new(1).unwrap(), std::num::NonZero::new(48000).unwrap());
        (Player { requests: Mutex::new(0), preparing: Mutex::new(()), mixer: Mutex::new(Some(mixer)), output: None,
            device_id: Mutex::new(None), current: Mutex::new(None), volume: Mutex::new(1.0),
            timing: Mutex::new(Timing::PLAIN), meter: Arc::default() }, output)
    }
    fn tone(value: f32) -> rodio::buffer::SamplesBuffer {
        rodio::buffer::SamplesBuffer::new(std::num::NonZero::new(1).unwrap(),
            std::num::NonZero::new(48000).unwrap(), vec![value; 48000])
    }

    #[test]
    fn stop_rejects_a_preview_that_finishes_preparing_later() {
        let (player, mut output) = fixture();
        let old = player.begin_request();
        player.stop();
        assert!(player.install(old, tone(0.25), Timing::PLAIN, 0.0, Adjust::NONE).is_err(),
            "stop must invalidate unfinished preparation before it can attach a source");
        assert_eq!(player.position(), None);
        assert!(output.by_ref().take(1000).all(|value| value == 0.0));
    }

    #[test]
    fn a_new_preview_wins_even_when_an_older_preparation_finishes_last() {
        let (player, mut output) = fixture();
        let old = player.begin_request();
        let latest = player.begin_request();
        player.install(latest, tone(0.8), Timing::PLAIN, 0.0, Adjust::NONE).unwrap();
        assert!(player.install(old, tone(0.25), Timing::PLAIN, 0.0, Adjust::NONE).is_err(),
            "stale preparation must not replace the newer source");
        assert!(output.by_ref().take(1000).all(|value| (value - 0.8).abs() < 0.001));
    }

    #[test]
    fn controls_cancel_preparation_even_without_an_attached_source() {
        for action in ["pause", "resume", "seek"] {
            let (player, _) = fixture();
            let old = player.begin_request();
            match action {
                "pause" => { assert_eq!(player.pause(), None); }
                "resume" => { assert!(!player.resume()); }
                _ => player.seek(0.0).unwrap(),
            }
            assert!(player.install(old, tone(0.25), Timing::PLAIN, 0.0, Adjust::NONE).is_err(), "{action}");
        }
    }

    #[test]
    fn editor_and_preview_use_the_same_request_order() {
        let (player, _) = fixture();
        let old_preview = player.begin_request();
        let pcm = crate::audio::Pcm { samples: vec![0.8; 48000], channels: 1, sample_rate: 48000 };
        player.play_buffer(&pcm, 0, 48000, true).unwrap();
        assert!(player.install(old_preview, tone(0.25), Timing::PLAIN, 0.0, Adjust::NONE).is_err());
        let old_editor = player.begin_request();
        player.stop();
        assert!(player.play_buffer_request(old_editor, &pcm, 0, 48000, true).is_err());
        assert!(player.update_buffer_request(old_editor, &pcm, 0, 48000, true).is_err());
        assert_eq!(player.position(), None);
    }

    #[test]
    fn latest_load_failure_stops_old_audio_but_stale_failure_leaves_new_audio_alone() {
        let (player, _) = fixture();
        let missing = tempfile::tempdir().unwrap().path().join("missing.wav");
        let old = player.begin_request();
        let latest = player.begin_request();
        player.install(latest, tone(0.8), Timing::PLAIN, 0.0, Adjust::NONE).unwrap();
        let stale = player.play_request(old, &missing, 0.0, Adjust::NONE).unwrap_err();
        assert!(stale.to_string().contains("superseded"));
        assert!(player.position().is_some());
        let failed = player.begin_request();
        assert!(player.play_request(failed, &missing, 0.0, Adjust::NONE).is_err());
        assert_eq!(player.position(), None);
    }

    #[test]
    fn an_old_worker_cannot_attach_after_stop_while_its_preparation_was_running() {
        let (player, mut output) = fixture();
        let player = Arc::new(player);
        let worker_player = Arc::clone(&player);
        let (ready, started) = mpsc::channel();
        let (release, resume) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let request = worker_player.begin_request();
            let prepared = tone(0.25);
            ready.send(()).unwrap();
            resume.recv().unwrap();
            worker_player.install(request, prepared, Timing::PLAIN, 0.0, Adjust::NONE)
        });
        started.recv_timeout(Duration::from_secs(2)).unwrap();
        // Preparation owns neither the mixer nor the request gate.
        assert!(player.mixer.try_lock().is_ok());
        player.stop();
        release.send(()).unwrap();
        assert!(worker.join().unwrap().is_err());
        assert_eq!(player.position(), None);
        assert!(output.by_ref().take(1000).all(|value| value == 0.0));
    }

    #[test]
    fn file_preview_decodes_stretches_and_seeks_without_a_system_device() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("generated.wav");
        let mut writer = hound::WavWriter::create(&path, hound::WavSpec {
            channels: 1, sample_rate: 8000, bits_per_sample: 16, sample_format: hound::SampleFormat::Int,
        }).unwrap();
        for i in 0..4000 {
            writer.write_sample(((i as f32 * 220.0 * std::f32::consts::TAU / 8000.0).sin() * 12000.0) as i16).unwrap();
        }
        writer.finalize().unwrap();
        for adjustment in [Adjust::NONE, Adjust { stretch: 1.2, semitones: 2.0 }] {
            let (player, mut output) = fixture();
            let running = Arc::new(std::sync::atomic::AtomicBool::new(true));
            let consume = Arc::clone(&running);
            let worker = std::thread::spawn(move || {
                while consume.load(std::sync::atomic::Ordering::Relaxed) {
                    for _ in 0..256 { output.next(); }
                    std::thread::sleep(Duration::from_millis(1));
                }
                output
            });
            let request = player.begin_request();
            let result = player.play_request(request, &path, 0.1, adjustment);
            running.store(false, std::sync::atomic::Ordering::Relaxed);
            let mut output = worker.join().unwrap();
            assert_eq!(result.unwrap(), adjustment);
            assert!(output.by_ref().take(2000).any(|value| value.abs() > 0.01));
            assert!(player.position().unwrap() >= 0.1);
            assert!(!player.current.lock().unwrap().as_ref().unwrap().is_paused());
        }
    }

    #[test]
    fn superseded_preparation_waiters_do_not_open_their_files() {
        let (player, _) = fixture();
        let player = Arc::new(player);
        let preparing = player.preparing.lock().unwrap();
        let old = player.begin_request();
        let worker_player = Arc::clone(&player);
        let (ready, started) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let dir = tempfile::tempdir().unwrap();
            ready.send(()).unwrap();
            worker_player.play_request(old, &dir.path().join("never-created.wav"), 0.0, Adjust::NONE)
        });
        started.recv_timeout(Duration::from_secs(2)).unwrap();
        // Even with the preparation slot held, stop and a new intent are immediate.
        player.stop();
        let latest = player.begin_request();
        drop(preparing);
        assert!(worker.join().unwrap().unwrap_err().to_string().contains("superseded"));
        assert!(player.request_guard(latest).is_ok());
    }

    #[test]
    fn shared_source_keeps_old_audio_and_seeks_and_loops_without_range_copies() {
        use rodio::Source;
        let mut audio=crate::editor::blocks::BlockAudio::from(crate::audio::Pcm {samples:vec![0.1,0.2,0.3,0.4,0.5,0.6],channels:2,sample_rate:2});
        let mut source=SharedSource {audio:audio.clone(),start:0,end:6,position:2,begin:2,looping:true,cursor:0};
        assert!(source.audio.samples.shares_storage(&audio.samples));
        audio.apply(&crate::editor::Op::Silence{start:0,end:3});
        assert_eq!(source.next(),Some(0.3));
        assert_eq!(source.by_ref().take(5).collect::<Vec<_>>(),vec![0.4,0.5,0.6,0.1,0.2]);
        source.try_seek(Duration::from_secs_f64(0.5)).unwrap();
        assert_eq!(source.next(),Some(0.5));
        let mut finite=SharedSource {audio:source.audio.clone(),start:2,end:6,position:2,begin:2,looping:false,cursor:0};
        finite.try_seek(Duration::from_secs(99)).unwrap();
        assert_eq!(finite.next(),None);
    }

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
            requests: Mutex::new(0), preparing: Mutex::new(()),
            mixer: Mutex::new(Some(mixer)), output: Some(tx), device_id: Mutex::new(None),
            current: Mutex::new(None), volume: Mutex::new(0.35),
            timing: Mutex::new(Timing::PLAIN), meter: Arc::default(),
        };
        let pcm = crate::audio::Pcm { samples: vec![0.25; 48000], channels: 1, sample_rate: 48000 };
        player.play_buffer(&pcm, 0, 48000, true).unwrap();
        assert!(player.position().is_some());
        let pending = player.begin_request();
        player.set_output_device(Some("test:interface".into())).unwrap();
        assert!(player.install(pending, tone(0.25), Timing::PLAIN, 0.0, Adjust::NONE).is_err());
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
            requests: Mutex::new(0), preparing: Mutex::new(()),
            mixer: Mutex::new(Some(mixer)), output: Some(tx), device_id: Mutex::new(Some("test:old".into())),
            current: Mutex::new(None), volume: Mutex::new(0.5),
            timing: Mutex::new(Timing::PLAIN), meter: Arc::default(),
        };
        let pcm = crate::audio::Pcm { samples: vec![0.25; 48000], channels: 1, sample_rate: 48000 };
        player.play_buffer(&pcm, 0, 48000, true).unwrap();
        let pending = player.begin_request();
        assert!(player.set_output_device(Some("test:missing".into())).is_err());
        assert!(player.request_guard(pending).is_ok());
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
            requests: Mutex::new(0), preparing: Mutex::new(()),
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
            requests: Mutex::new(0), preparing: Mutex::new(()),
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
            requests: Mutex::new(0), preparing: Mutex::new(()),
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
