//! The editor's clipboard, shared by all editor windows. Capturing a paste
//! shares immutable samples; conversion never holds either editor or clipboard locks.

use std::sync::{Arc, Mutex};

use serde::Serialize;

use crate::audio::AudioRead;
use crate::error::{Error, Result};

static CLIPBOARD: Mutex<Option<Clip>> = Mutex::new(None);

#[derive(Clone)]
pub struct Clip {
    samples: Arc<Vec<f32>>,
    channels: usize,
    sample_rate: u32,
}

/// What the clipboard holds, for the page to tell whether paste can work.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipInfo {
    pub frames: usize,
    pub channels: usize,
    pub sample_rate: u32,
}

impl Clip {
    fn frames(&self) -> usize {
        self.samples.len() / self.channels.max(1)
    }

    pub(super) fn from_range(pcm: &impl AudioRead, start: usize, end: usize) -> Option<Self> {
        let ch = pcm.channels().max(1);
        let frames = pcm.frames();
        let (start, end) = (start.min(frames), end.min(frames));
        (end > start).then(|| Self {
            samples: Arc::new(pcm.range(start * ch..end * ch).into_owned()),
            channels: pcm.channels(),
            sample_rate: pcm.sample_rate(),
        })
    }

    pub(super) fn info(&self) -> ClipInfo {
        ClipInfo {
            frames: self.frames(),
            channels: self.channels,
            sample_rate: self.sample_rate,
        }
    }

    /// Same-format pastes reuse storage, including across undo history. Other
    /// formats keep only the final output and a bounded remapping block.
    pub(super) fn render(&self, channels: usize, sample_rate: u32) -> Result<Arc<Vec<f32>>> {
        let to_ch = channels.max(1);
        let from_ch = self.channels.max(1);
        if from_ch == to_ch && self.sample_rate == sample_rate {
            return Ok(self.samples.clone());
        }
        let frames = self.info().frames;
        let resample = self.sample_rate != sample_rate && frames >= 2;
        let ratio = self.sample_rate.max(1) as f64 / sample_rate.max(1) as f64;
        let output_frames = if resample {
            (frames as f64 / ratio).round().max(1.0) as usize
        } else {
            frames
        };
        let count = output_frames
            .checked_mul(to_ch)
            .ok_or_else(|| Error::Other("audio length is too large".into()))?;
        const BLOCK_FRAMES: usize = 16384;
        let scratch_count = if resample {
            frames.min(BLOCK_FRAMES + 1).checked_mul(to_ch)
        } else {
            Some(0)
        }
        .ok_or_else(|| Error::Other("audio length is too large".into()))?;
        let allocation = count
            .checked_add(scratch_count)
            .ok_or_else(|| Error::Other("audio length is too large".into()))?;
        super::memory::check_samples(allocation, super::memory::pcm_budget() / 2)?;
        let frame = |f: usize, output: &mut Vec<f32>| {
            let src = &self.samples[f * from_ch..(f + 1) * from_ch];
            for c in 0..to_ch {
                output.push(if from_ch == to_ch {
                    src[c]
                } else if from_ch == 1 {
                    src[0]
                } else if to_ch == 1 {
                    src.iter().sum::<f32>() / from_ch as f32
                } else {
                    src[c.min(from_ch - 1)]
                });
            }
        };
        let mut output = Vec::with_capacity(count);
        if !resample {
            for f in 0..frames {
                frame(f, &mut output);
            }
        } else {
            // One source block plus the next interpolation endpoint. All
            // positions use the global output index to preserve exact samples.
            let mut remapped = Vec::with_capacity(scratch_count);
            let mut i = 0;
            for start in (0..frames).step_by(BLOCK_FRAMES) {
                let end = (start + BLOCK_FRAMES).min(frames);
                if i >= output_frames {
                    break;
                }
                if ((i as f64 * ratio).floor() as usize).min(frames - 1) >= end {
                    continue;
                }
                let source_end = (end + 1).min(frames);
                remapped.clear();
                if from_ch == to_ch {
                    remapped
                        .extend_from_slice(&self.samples[start * from_ch..source_end * from_ch]);
                } else {
                    for f in start..source_end {
                        frame(f, &mut remapped);
                    }
                }
                while i < output_frames {
                    let pos = i as f64 * ratio;
                    let a = (pos.floor() as usize).min(frames - 1);
                    if a >= end {
                        break;
                    }
                    let b = (a + 1).min(frames - 1);
                    let t = (pos - a as f64) as f32;
                    for c in 0..to_ch {
                        output.push(
                            remapped[(a - start) * to_ch + c] * (1.0 - t)
                                + remapped[(b - start) * to_ch + c] * t,
                        );
                    }
                    i += 1;
                }
            }
        }
        Ok(Arc::new(output))
    }

    #[cfg(test)]
    pub(super) fn shares_samples(&self, samples: &Arc<Vec<f32>>) -> bool {
        Arc::ptr_eq(&self.samples, samples)
    }
}

/// Copies `[start, end)` of `pcm` to the clipboard; false for an empty range.
pub fn copy(pcm: &impl AudioRead, start: usize, end: usize) -> bool {
    let Some(clip) = Clip::from_range(pcm, start, end) else {
        return false;
    };
    // Free the previous large clip after releasing the clipboard mutex.
    let old = CLIPBOARD.lock().unwrap().replace(clip);
    drop(old);
    true
}

pub fn snapshot() -> Option<Clip> {
    CLIPBOARD.lock().unwrap().clone()
}

pub fn info() -> Option<ClipInfo> {
    snapshot().map(|clip| clip.info())
}

// Previous two-buffer conversion, kept only as a differential/benchmark reference.
#[cfg(test)]
pub(super) fn reference_convert(clip: &Clip, channels: usize, sample_rate: u32) -> Vec<f32> {
    let (from_ch, to_ch) = (clip.channels.max(1), channels.max(1));
    let frames = clip.frames();
    // Frame `f` in the target channel layout.
    let frame = |f: usize, out: &mut Vec<f32>| {
        let src = &clip.samples[f * from_ch..(f + 1) * from_ch];
        for c in 0..to_ch {
            out.push(if from_ch == to_ch {
                src[c]
            } else if from_ch == 1 {
                src[0]
            } else if to_ch == 1 {
                src.iter().sum::<f32>() / from_ch as f32
            } else {
                src[c.min(from_ch - 1)]
            });
        }
    };
    let mut remapped = Vec::with_capacity(frames * to_ch);
    for f in 0..frames {
        frame(f, &mut remapped);
    }
    if clip.sample_rate == sample_rate || frames < 2 {
        return remapped;
    }
    let ratio = clip.sample_rate as f64 / sample_rate.max(1) as f64;
    let out_frames = ((frames as f64) / ratio).round().max(1.0) as usize;
    let mut out = Vec::with_capacity(out_frames * to_ch);
    for i in 0..out_frames {
        let pos = i as f64 * ratio;
        let a = (pos.floor() as usize).min(frames - 1);
        let b = (a + 1).min(frames - 1);
        let t = (pos - a as f64) as f32;
        for c in 0..to_ch {
            out.push(remapped[a * to_ch + c] * (1.0 - t) + remapped[b * to_ch + c] * t);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::Pcm;

    fn clip(pcm: &Pcm) -> Clip {
        Clip {
            samples: Arc::new(pcm.samples.clone()),
            channels: pcm.channels,
            sample_rate: pcm.sample_rate,
        }
    }

    fn convert(pcm: &Pcm, channels: usize, rate: u32) -> Vec<f32> {
        clip(pcm).render(channels, rate).unwrap().as_ref().clone()
    }

    #[test]
    fn converts_channels_and_rate() {
        let stereo = Pcm {
            samples: vec![0.2, 0.4, 0.6, 0.8],
            channels: 2,
            sample_rate: 1000,
        };
        // Same format: unchanged.
        assert_eq!(convert(&stereo, 2, 1000), stereo.samples);
        // Down to mono: averaged.
        let mono = convert(&stereo, 1, 1000);
        assert!((mono[0] - 0.3).abs() < 1e-6 && (mono[1] - 0.7).abs() < 1e-6);
        // Mono up to stereo: copied.
        let m = Pcm {
            samples: vec![0.5, -0.5],
            channels: 1,
            sample_rate: 1000,
        };
        assert_eq!(convert(&m, 2, 1000), [0.5, 0.5, -0.5, -0.5]);
        // Twice the rate: twice the frames, interpolated in between.
        let up = convert(
            &Pcm {
                samples: vec![0.0, 1.0, 0.0],
                channels: 1,
                sample_rate: 1000,
            },
            1,
            2000,
        );
        assert_eq!(up.len(), 6);
        assert!((up[1] - 0.5).abs() < 1e-6 && (up[2] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn conversion_matches_previous_samples_for_rates_channels_and_short_inputs() {
        for frames in [0, 1, 2, 31, 1001, 16383, 16384, 16385, 32771] {
            for from_ch in [1, 2, 4] {
                for from_rate in [22050, 44100, 48000] {
                    let pcm = Pcm {
                        samples: (0..frames * from_ch)
                            .map(|n| ((n * 37 % 257) as f32 - 128.0) / 128.0)
                            .collect(),
                        channels: from_ch,
                        sample_rate: from_rate,
                    };
                    for to_ch in [1, 2, 3] {
                        for to_rate in [16000, 22050, 44100, 48000, 96000] {
                            assert_eq!(
                                convert(&pcm, to_ch, to_rate),
                                reference_convert(&clip(&pcm), to_ch, to_rate),
                                "{frames} frames, {from_ch}/{from_rate} -> {to_ch}/{to_rate}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn identity_preserves_storage_and_float_bits_and_checks_conversion_size() {
        let clip = Clip {
            samples: Arc::new(vec![-0.0, f32::from_bits(0x7fc0_0123), f32::INFINITY]),
            channels: 1,
            sample_rate: 44100,
        };
        let output = clip.render(1, 44100).unwrap();
        assert!(Arc::ptr_eq(&clip.samples, &output));
        assert_eq!(
            output.iter().map(|n| n.to_bits()).collect::<Vec<_>>(),
            clip.samples.iter().map(|n| n.to_bits()).collect::<Vec<_>>()
        );
        assert!(clip.render(usize::MAX, 48000).is_err());
    }

    #[test]
    fn copy_and_take() {
        let p = Pcm {
            samples: (0..10).map(|i| i as f32).collect(),
            channels: 1,
            sample_rate: 1000,
        };
        assert!(!copy(&p, 5, 5));
        assert!(copy(&p, 2, 5));
        assert_eq!(
            info(),
            Some(ClipInfo {
                frames: 3,
                channels: 1,
                sample_rate: 1000
            })
        );
        let captured = snapshot().unwrap();
        let source = captured.samples.as_ptr();
        let pasted = captured.render(1, 1000).unwrap();
        assert_eq!(pasted.as_slice(), [2.0, 3.0, 4.0]);
        assert_eq!(
            pasted.as_ptr(),
            source,
            "same-format paste must share the immutable clipboard samples"
        );
        assert!(copy(&p, 5, 10));
        assert_eq!(
            snapshot().unwrap().render(1, 1000).unwrap().as_slice(),
            [5.0, 6.0, 7.0, 8.0, 9.0]
        );
        assert_eq!(
            captured.render(1, 1000).unwrap().as_slice(),
            [2.0, 3.0, 4.0],
            "later copies cannot change an in-flight paste"
        );
        *CLIPBOARD.lock().unwrap() = None;
    }
}
