//! Level metering of what the preview player is playing: a source wrapper adds
//! up peak and mean square per channel on the audio thread; the UI drains them
//! (`playback_levels`) a few dozen times a second.

use realfft::{RealFftPlanner, RealToComplex};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use rodio::source::SeekError;
use rodio::{ChannelCount, SampleRate, Source};
use serde::Serialize;

/// Channels metered: left and right (mono uses the first).
const METERED: usize = 2;
/// Samples between hand-overs to the shared accumulator, to keep locking rare.
const FLUSH_EVERY: usize = 512;

const FFT_SIZE: usize = 4096;
const BANDS: usize = 16;

#[derive(Clone)]
struct SpectrumWindow {
    samples: [[f32; FFT_SIZE]; METERED],
    position: usize,
    filled: usize,
    rate: u32,
}
impl Default for SpectrumWindow {
    fn default() -> Self {
        Self {
            samples: [[0.0; FFT_SIZE]; METERED],
            position: 0,
            filled: 0,
            rate: 0,
        }
    }
}
impl SpectrumWindow {
    // Called only by the UI command, after releasing the audio-thread lock.
    fn bands(&self, channels: usize, volume: f32) -> Vec<f32> {
        let mut output = vec![0.0; BANDS];
        if self.filled == 0 || self.rate == 0 || volume <= 0.0 {
            return output;
        }
        static FFT: OnceLock<Arc<dyn RealToComplex<f32>>> = OnceLock::new();
        let fft = FFT.get_or_init(|| RealFftPlanner::<f32>::new().plan_fft_forward(FFT_SIZE));
        let mut powers = vec![0.0f32; FFT_SIZE / 2 + 1];
        let mut input = fft.make_input_vec();
        let mut spectrum = fft.make_output_vec();
        let mut scratch = fft.make_scratch_vec();
        for channel in 0..channels {
            for (i, sample) in input.iter_mut().enumerate() {
                let hann = 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / FFT_SIZE as f32).cos();
                *sample = self.samples[channel][(self.position + i) % FFT_SIZE] * hann;
            }
            if fft
                .process_with_scratch(&mut input, &mut spectrum, &mut scratch)
                .is_err()
            {
                return output;
            }
            for (power, bin) in powers.iter_mut().zip(&spectrum) {
                *power += bin.norm_sqr() / channels as f32;
            }
        }
        let resolution = self.rate as f32 / FFT_SIZE as f32;
        let ratio = (16000.0f32 / 40.0).powf(1.0 / (BANDS - 1) as f32);
        for (index, value) in output.iter_mut().enumerate() {
            let center = 40.0 * ratio.powi(index as i32);
            if center > self.rate as f32 / 2.0 {
                continue;
            }
            let start = ((center / ratio.sqrt() / resolution).ceil() as usize).max(1);
            let end = ((center * ratio.sqrt() / resolution).floor() as usize).min(powers.len() - 1);
            let nearest = ((center / resolution).round() as usize).clamp(1, powers.len() - 1);
            let power = if start <= end {
                powers[start..=end].iter().copied().fold(0.0f32, f32::max)
            } else {
                powers[nearest]
            };
            // Hann coherent-gain correction: a full-scale sine peaks near 1.
            *value = power.sqrt() * 4.0 / FFT_SIZE as f32 * volume;
        }
        output
    }
}

#[derive(Default, Clone, Copy)]
struct Acc {
    peak: [f32; METERED],
    sum_sq: [f64; METERED],
    count: [u64; METERED],
}

impl Acc {
    fn add(&mut self, other: &Acc) {
        for c in 0..METERED {
            self.peak[c] = self.peak[c].max(other.peak[c]);
            self.sum_sq[c] += other.sum_sq[c];
            self.count[c] += other.count[c];
        }
    }
}

/// Levels since the previous read, linear 0..1 (above 1 means clipping).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Levels {
    pub peak: Vec<f32>,
    pub rms: Vec<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spectrum: Option<Vec<f32>>,
}

/// Shared between the playing source and the UI.
#[derive(Default)]
pub struct MeterTap {
    acc: Mutex<Acc>,
    channels: Mutex<usize>,
    spectrum: Mutex<SpectrumWindow>,
}

impl MeterTap {
    /// Takes what was played since the last call, scaled by the output volume.
    /// One value per channel: two for stereo and more, one for mono.
    #[cfg(test)]
    pub fn drain(&self, volume: f32) -> Levels {
        self.drain_with_spectrum(volume, false)
    }

    pub fn drain_with_spectrum(&self, volume: f32, include_spectrum: bool) -> Levels {
        let acc = std::mem::take(&mut *self.acc.lock().unwrap());
        let n = (*self.channels.lock().unwrap()).clamp(1, METERED);
        let rms = |c: usize| {
            if acc.count[c] == 0 {
                0.0
            } else {
                (acc.sum_sq[c] / acc.count[c] as f64).sqrt() as f32
            }
        };
        Levels {
            peak: (0..n).map(|c| acc.peak[c] * volume).collect(),
            rms: (0..n).map(|c| rms(c) * volume).collect(),
            spectrum: include_spectrum.then(|| {
                let window = self.spectrum.lock().unwrap().clone();
                window.bands(n, volume)
            }),
        }
    }

    pub fn reset(&self) {
        *self.acc.lock().unwrap() = Acc::default();
        let mut window = self.spectrum.lock().unwrap();
        let rate = window.rate;
        *window = SpectrumWindow {
            rate,
            ..Default::default()
        };
    }
}

/// Passes samples through unchanged while feeding a `MeterTap`.
pub struct Metered<S> {
    inner: S,
    tap: Arc<MeterTap>,
    local: Acc,
    /// Position within the current frame, to tell channels apart.
    index: usize,
    since_flush: usize,
    frame: [f32; METERED],
    waveform: [[f32; FLUSH_EVERY]; METERED],
    waveform_frames: usize,
}

impl<S: Source> Metered<S> {
    pub fn new(inner: S, tap: Arc<MeterTap>) -> Self {
        tap.reset();
        *tap.channels.lock().unwrap() = inner.channels().get() as usize;
        tap.spectrum.lock().unwrap().rate = inner.sample_rate().get();
        Self {
            inner,
            tap,
            local: Acc::default(),
            index: 0,
            since_flush: 0,
            frame: [0.0; METERED],
            waveform: [[0.0; FLUSH_EVERY]; METERED],
            waveform_frames: 0,
        }
    }

    fn flush(&mut self) {
        // Never block the audio thread: if the UI is reading, keep adding up locally.
        if let Ok(mut shared) = self.tap.acc.try_lock() {
            shared.add(&self.local);
            self.local = Acc::default();
        }
        // Only a bounded copy on the audio thread; contention drops this
        // visual snapshot rather than delaying playback. FFT runs in drain.
        if self.waveform_frames > 0 {
            if let Ok(mut window) = self.tap.spectrum.try_lock() {
                for i in 0..self.waveform_frames {
                    let position = window.position;
                    for c in 0..METERED {
                        window.samples[c][position] = self.waveform[c][i];
                    }
                    window.position = (position + 1) % FFT_SIZE;
                    window.filled = (window.filled + 1).min(FFT_SIZE);
                }
            }
            self.waveform_frames = 0;
        }
        self.since_flush = 0;
    }
}

impl<S: Source> Iterator for Metered<S> {
    type Item = S::Item;

    fn next(&mut self) -> Option<Self::Item> {
        let Some(sample) = self.inner.next() else {
            self.flush();
            return None;
        };
        let channels = self.inner.channels().get() as usize;
        let c = self.index % channels;
        if c < METERED {
            let s: f32 = sample;
            self.frame[c] = s;
            self.local.peak[c] = self.local.peak[c].max(s.abs());
            self.local.sum_sq[c] += f64::from(s * s);
            self.local.count[c] += 1;
        }
        self.index = (self.index + 1) % channels;
        if self.index == 0 {
            for c in 0..METERED {
                self.waveform[c][self.waveform_frames] = self.frame[c];
            }
            self.waveform_frames += 1;
        }
        self.since_flush += 1;
        if self.since_flush >= FLUSH_EVERY {
            self.flush();
        }
        Some(sample)
    }
}

impl<S: Source> Source for Metered<S> {
    fn current_span_len(&self) -> Option<usize> {
        self.inner.current_span_len()
    }

    fn channels(&self) -> ChannelCount {
        self.inner.channels()
    }

    fn sample_rate(&self) -> SampleRate {
        self.inner.sample_rate()
    }

    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }

    fn try_seek(&mut self, pos: Duration) -> Result<(), SeekError> {
        // Seeks land on frame boundaries.
        self.index = 0;
        self.inner.try_seek(pos)?;
        self.local = Acc::default();
        self.waveform_frames = 0;
        self.since_flush = 0;
        self.frame = [0.0; METERED];
        self.tap.reset();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spectrum_tracks_frequency_preserves_opposite_phase_stereo_and_scales_volume() {
        let mut dominant = Vec::new();
        for frequency in [187.5f32, 6000.0] {
            let tap = Arc::new(MeterTap::default());
            let samples: Vec<f32> = (0..FFT_SIZE)
                .flat_map(|i| {
                    let s = 0.5 * (std::f32::consts::TAU * frequency * i as f32 / 48000.0).sin();
                    [s, -s]
                })
                .collect();
            let source = rodio::buffer::SamplesBuffer::new(
                ChannelCount::new(2).unwrap(),
                SampleRate::new(48000).unwrap(),
                samples.clone(),
            );
            let played: Vec<_> = Metered::new(source, Arc::clone(&tap)).collect();
            assert_eq!(played, samples);
            assert!(tap.drain(1.0).spectrum.is_none());
            let spectrum = tap.drain_with_spectrum(1.0, true).spectrum.unwrap();
            let index = spectrum
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.total_cmp(b.1))
                .unwrap()
                .0;
            assert!(spectrum[index] > 0.45);
            dominant.push(index);
            let quiet = tap.drain_with_spectrum(0.25, true).spectrum.unwrap();
            for (a, b) in spectrum.iter().zip(quiet) {
                assert!((a * 0.25 - b).abs() < 1e-6);
            }
            tap.reset();
            assert!(
                tap.drain_with_spectrum(1.0, true)
                    .spectrum
                    .unwrap()
                    .iter()
                    .all(|v| *v == 0.0)
            );
        }
        assert!(dominant[1] > dominant[0] + 6);
    }

    #[test]
    fn meters_each_channel_and_scales_by_volume() {
        let tap = Arc::new(MeterTap::default());
        // Left: a full-scale square wave; right: silence with one 0.5 spike.
        let mut samples = Vec::new();
        for i in 0..2048 {
            samples.push(if i % 2 == 0 { 1.0 } else { -1.0 });
            samples.push(if i == 100 { -0.5 } else { 0.0 });
        }
        let source = rodio::buffer::SamplesBuffer::new(
            ChannelCount::new(2).unwrap(),
            SampleRate::new(48_000).unwrap(),
            samples,
        );
        let played: Vec<f32> = Metered::new(source, Arc::clone(&tap)).collect();
        assert_eq!(played.len(), 4096);

        let levels = tap.drain(0.5);
        assert_eq!(levels.peak, vec![0.5, 0.25]);
        assert!((levels.rms[0] - 0.5).abs() < 1e-6);
        assert!(levels.rms[1] < 0.01);
        // Drained: nothing new until more is played.
        assert_eq!(tap.drain(1.0).peak, vec![0.0, 0.0]);
    }
}
