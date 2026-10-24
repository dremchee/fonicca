//! Level metering of what the preview player is playing: a source wrapper adds
//! up peak and mean square per channel on the audio thread; the UI drains them
//! (`playback_levels`) a few dozen times a second.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use rodio::source::SeekError;
use rodio::{ChannelCount, SampleRate, Source};
use serde::Serialize;

/// Channels metered: left and right (mono uses the first).
const METERED: usize = 2;
/// Samples between hand-overs to the shared accumulator, to keep locking rare.
const FLUSH_EVERY: usize = 512;

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
}

/// Shared between the playing source and the UI.
#[derive(Default)]
pub struct MeterTap {
    acc: Mutex<Acc>,
    channels: Mutex<usize>,
}

impl MeterTap {
    /// Takes what was played since the last call, scaled by the output volume.
    /// One value per channel: two for stereo and more, one for mono.
    pub fn drain(&self, volume: f32) -> Levels {
        let acc = std::mem::take(&mut *self.acc.lock().unwrap());
        let n = (*self.channels.lock().unwrap()).clamp(1, METERED);
        let rms = |c: usize| if acc.count[c] == 0 { 0.0 } else { (acc.sum_sq[c] / acc.count[c] as f64).sqrt() as f32 };
        Levels {
            peak: (0..n).map(|c| acc.peak[c] * volume).collect(),
            rms: (0..n).map(|c| rms(c) * volume).collect(),
        }
    }

    pub fn reset(&self) {
        *self.acc.lock().unwrap() = Acc::default();
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
}

impl<S: Source> Metered<S> {
    pub fn new(inner: S, tap: Arc<MeterTap>) -> Self {
        tap.reset();
        *tap.channels.lock().unwrap() = inner.channels().get() as usize;
        Self { inner, tap, local: Acc::default(), index: 0, since_flush: 0 }
    }

    fn flush(&mut self) {
        // Never block the audio thread: if the UI is reading, keep adding up locally.
        if let Ok(mut shared) = self.tap.acc.try_lock() {
            shared.add(&self.local);
            self.local = Acc::default();
        }
        self.since_flush = 0;
    }
}

impl<S: Source> Iterator for Metered<S> {
    type Item = S::Item;

    fn next(&mut self) -> Option<Self::Item> {
        let sample = self.inner.next()?;
        let channels = self.inner.channels().get() as usize;
        let c = self.index % channels;
        if c < METERED {
            let s: f32 = sample;
            self.local.peak[c] = self.local.peak[c].max(s.abs());
            self.local.sum_sq[c] += f64::from(s * s);
            self.local.count[c] += 1;
        }
        self.index = (self.index + 1) % channels;
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
        self.inner.try_seek(pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
