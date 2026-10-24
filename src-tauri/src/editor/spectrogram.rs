//! Cached, log-frequency STFT over current editor audio. Columns are max-pooled
//! for the viewport so narrow transients survive zooming out.
use crate::audio::Pcm;
use crate::error::{Error, Result};
use realfft::RealFftPlanner;

pub const BINS: usize = 256;
pub const FLOOR_DB: f32 = -100.0;

pub struct Spectrogram {
    frames: usize,
    sample_rate: u32,
    fft_size: usize,
    hop: usize,
    columns: usize,
    /// Column-major, low to high frequency. 0 = -100 dBFS, 255 = 0 dBFS.
    values: Vec<u8>,
}

impl Spectrogram {
    pub fn build(pcm: &Pcm) -> Result<Self> {
        if pcm.sample_rate == 0 || pcm.channels == 0 {
            return Err(Error::Audio("invalid audio format for spectrogram".into()));
        }
        // About 85 ms of frequency resolution, 75% overlap. The time step and
        // cache size stay comparable at different sample rates (under 32 MB/20 min).
        let fft_size = ((pcm.sample_rate as f64 * 0.085) as usize)
            .next_power_of_two()
            .clamp(512, 16384);
        let hop = (fft_size / 4).max(pcm.frames().div_ceil(131072));
        let columns = pcm.frames().div_ceil(hop);
        let mut values = vec![0; columns * BINS];
        let fft = RealFftPlanner::<f32>::new().plan_fft_forward(fft_size);
        let window: Vec<f32> = (0..fft_size)
            .map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / fft_size as f32).cos())
            .collect();
        let normalization = (2.0 / window.iter().sum::<f32>()).powi(2) / pcm.channels as f32;
        let mut input = fft.make_input_vec();
        let mut spectrum = fft.make_output_vec();
        let mut scratch = fft.make_scratch_vec();
        let mut power = vec![0.0f32; spectrum.len()];
        let max_hz = pcm.sample_rate as f64 / 2.0;
        let min_hz = 20.0f64.min(max_hz / 16.0);
        let bands: Vec<_> = (0..BINS)
            .map(|bin| {
                let hz = |i| min_hz * (max_hz / min_hz).powf(i as f64 / BINS as f64);
                let low = hz(bin) * fft_size as f64 / pcm.sample_rate as f64;
                let high = hz(bin + 1) * fft_size as f64 / pcm.sample_rate as f64;
                let lo = (low.ceil() as usize).clamp(1, fft_size / 2);
                let hi = (high.floor() as usize).clamp(1, fft_size / 2);
                if lo <= hi {
                    (lo, hi)
                } else {
                    let nearest = ((low * high).sqrt().round() as usize).clamp(1, fft_size / 2);
                    (nearest, nearest)
                }
            })
            .collect();
        for column in 0..columns {
            power.fill(0.0);
            let centre = column * hop;
            for channel in 0..pcm.channels {
                for (i, sample) in input.iter_mut().enumerate() {
                    let frame = centre as isize + i as isize - (fft_size / 2) as isize;
                    let value = if frame >= 0 && (frame as usize) < pcm.frames() {
                        pcm.samples[frame as usize * pcm.channels + channel]
                    } else {
                        0.0
                    };
                    *sample = if value.is_finite() {
                        value * window[i]
                    } else {
                        0.0
                    };
                }
                fft.process_with_scratch(&mut input, &mut spectrum, &mut scratch)
                    .map_err(|e| Error::Audio(format!("spectrogram FFT: {e}")))?;
                for (i, (power, value)) in power.iter_mut().zip(&spectrum).enumerate() {
                    // Nyquist has no mirrored partner in a real FFT.
                    *power += value.norm_sqr()
                        * normalization
                        * if i == fft_size / 2 { 0.25 } else { 1.0 };
                }
            }
            for (bin, &(lo, hi)) in bands.iter().enumerate() {
                let peak = power[lo..=hi].iter().copied().fold(0.0f32, f32::max);
                let db = 10.0 * peak.max(1e-10).log10();
                values[column * BINS + bin] =
                    (((db - FLOOR_DB) / -FLOOR_DB).clamp(0.0, 1.0) * 255.0).round() as u8;
            }
        }
        Ok(Self {
            frames: pcm.frames(),
            sample_rate: pcm.sample_rate,
            fft_size,
            hop,
            columns,
            values,
        })
    }

    /// Header: version, start, end, columns, bins, sample rate, FFT size, hop (u32 LE).
    /// Then column-major magnitudes as u8. Pool all windows touched by a pixel.
    pub fn range(&self, start: usize, end: usize, columns: usize) -> Vec<u8> {
        let start = start.min(self.frames);
        let end = end.clamp(start, self.frames);
        let columns = if start == end {
            0
        } else {
            columns.clamp(1, 2048)
        };
        let mut bytes = Vec::with_capacity(32 + columns * BINS);
        for value in [
            1,
            start,
            end,
            columns,
            BINS,
            self.sample_rate as usize,
            self.fft_size,
            self.hop,
        ] {
            bytes.extend_from_slice(&(value as u32).to_le_bytes());
        }
        for column in 0..columns {
            let a = start + (end - start) * column / columns;
            let b = start + (end - start) * (column + 1) / columns;
            let first = (a / self.hop).min(self.columns - 1);
            let last = b.div_ceil(self.hop).max(first + 1).min(self.columns);
            let mut pooled = [0u8; BINS];
            for source in first..last {
                for (peak, value) in pooled
                    .iter_mut()
                    .zip(&self.values[source * BINS..(source + 1) * BINS])
                {
                    *peak = (*peak).max(*value);
                }
            }
            bytes.extend_from_slice(&pooled);
        }
        bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::{self, Marker, Op, Session};
    use std::sync::Arc;

    fn tone(hz: f32, channels: usize) -> Pcm {
        let samples = (0..48000)
            .flat_map(|i| {
                let sample = 0.5 * (std::f32::consts::TAU * hz * i as f32 / 48000.0).sin();
                (0..channels).map(move |ch| if ch == 0 { sample } else { -sample })
            })
            .collect();
        Pcm {
            samples,
            channels,
            sample_rate: 48000,
        }
    }

    #[test]
    fn tone_is_in_the_correct_log_band_at_the_correct_level() {
        let s = Spectrogram::build(&tone(1000.0, 1)).unwrap();
        let middle = &s.values[(s.columns / 2) * BINS..(s.columns / 2 + 1) * BINS];
        let (bin, &peak) = middle.iter().enumerate().max_by_key(|(_, v)| *v).unwrap();
        let hz = 20.0f64 * (24000.0f64 / 20.0).powf((bin as f64 + 0.5) / BINS as f64);
        assert!((hz - 1000.0).abs() < 100.0, "peak at {hz} Hz");
        let db = peak as f32 / 255.0 * 100.0 - 100.0;
        assert!((-8.0..-5.0).contains(&db), "half-scale tone at {db} dBFS");
    }

    #[test]
    fn opposite_phase_stereo_retains_the_same_energy_as_mono() {
        let mono = Spectrogram::build(&tone(440.0, 1)).unwrap();
        let stereo = Spectrogram::build(&tone(440.0, 2)).unwrap();
        assert_eq!(mono.values.len(), stereo.values.len());
        assert!(
            mono.values
                .iter()
                .zip(&stereo.values)
                .all(|(a, b)| a.abs_diff(*b) <= 1)
        );
        assert!(stereo.values.iter().any(|v| *v > 200));
    }

    #[test]
    fn silence_empty_audio_and_clamped_ranges_pack_safely() {
        let silent = Pcm {
            samples: vec![0.0; 4800],
            channels: 1,
            sample_rate: 48000,
        };
        let s = Spectrogram::build(&silent).unwrap();
        assert!(s.values.iter().all(|v| *v == 0));
        let bytes = s.range(0, usize::MAX, usize::MAX);
        assert_eq!(bytes.len(), 32 + 2048 * BINS);
        let words: Vec<u32> = bytes[..32]
            .chunks_exact(4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
            .collect();
        assert_eq!(&words[..6], [1, 0, 4800, 2048, 256, 48000]);
        assert_eq!(s.range(10000, 20000, 10).len(), 32);
        let empty = Pcm {
            samples: vec![],
            channels: 1,
            sample_rate: 48000,
        };
        assert_eq!(Spectrogram::build(&empty).unwrap().range(0, 0, 1).len(), 32);
    }

    #[test]
    fn zoomed_out_columns_keep_a_short_transient() {
        let mut pcm = Pcm {
            samples: vec![0.0; 48000],
            channels: 1,
            sample_rate: 48000,
        };
        pcm.samples[35000..35016].fill(1.0);
        let s = Spectrogram::build(&pcm).unwrap();
        assert!(s.range(0, 10000, 1)[32..].iter().all(|v| *v == 0));
        assert!(s.range(0, 48000, 1)[32..].iter().any(|v| *v > 100));
    }

    #[test]
    fn cache_survives_markers_but_invalidates_on_audio_edits_undo_and_redo() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tone.wav");
        editor::write_wav(&path, &tone(440.0, 1), Some(32), None).unwrap();
        let mut s = Session::open(1, &path, Some(32)).unwrap();
        let version = s.audio_revision();
        let cached = Arc::new(Spectrogram::build(s.audio()).unwrap());
        s.cache_spectrogram(version, cached.clone()).unwrap();
        s.apply(Op::SetMarkers {
            markers: vec![Marker {
                frame: 100,
                skip: false,
            }],
        });
        s.undo();
        s.redo();
        assert_eq!(s.audio_revision(), version);
        assert!(Arc::ptr_eq(&cached, &s.spectrogram().unwrap()));
        s.apply(Op::Gain {
            start: 0,
            end: 48000,
            db: -6.0,
        });
        assert!(s.spectrogram().is_none());
        assert!(s.cache_spectrogram(version, cached.clone()).is_err());
        s.cache_spectrogram(s.audio_revision(), cached.clone())
            .unwrap();
        s.undo();
        assert!(s.spectrogram().is_none());
        s.cache_spectrogram(s.audio_revision(), cached).unwrap();
        s.redo();
        assert!(s.spectrogram().is_none());
    }
}
