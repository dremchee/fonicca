//! The editor's clipboard, shared by all editor windows, so audio copied from one
//! file can be pasted into another.

use std::sync::Mutex;

use serde::Serialize;

use crate::audio::Pcm;

static CLIPBOARD: Mutex<Option<Pcm>> = Mutex::new(None);

/// What the clipboard holds, for the page to tell whether paste can work.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipInfo {
    pub frames: usize,
    pub channels: usize,
    pub sample_rate: u32,
}

/// Copies `[start, end)` of `pcm` to the clipboard; false for an empty range.
pub fn copy(pcm: &Pcm, start: usize, end: usize) -> bool {
    let ch = pcm.channels.max(1);
    let frames = pcm.frames();
    let (start, end) = (start.min(frames), end.min(frames));
    if end <= start {
        return false;
    }
    let clip = Pcm { samples: pcm.samples[start * ch..end * ch].to_vec(), channels: pcm.channels, sample_rate: pcm.sample_rate };
    *CLIPBOARD.lock().unwrap() = Some(clip);
    true
}

pub fn info() -> Option<ClipInfo> {
    CLIPBOARD
        .lock()
        .unwrap()
        .as_ref()
        .map(|c| ClipInfo { frames: c.frames(), channels: c.channels, sample_rate: c.sample_rate })
}

/// The clipboard's audio in the given format (interleaved), ready to splice in.
pub fn take_as(channels: usize, sample_rate: u32) -> Option<Vec<f32>> {
    CLIPBOARD.lock().unwrap().as_ref().map(|c| convert(c, channels, sample_rate))
}

/// Converts audio to another channel count (mono is copied to every channel, more
/// channels are averaged down) and sample rate (linear interpolation, good enough
/// for pasting a phrase from a file at another rate).
fn convert(clip: &Pcm, channels: usize, sample_rate: u32) -> Vec<f32> {
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

    #[test]
    fn converts_channels_and_rate() {
        let stereo = Pcm { samples: vec![0.2, 0.4, 0.6, 0.8], channels: 2, sample_rate: 1000 };
        // Same format: unchanged.
        assert_eq!(convert(&stereo, 2, 1000), stereo.samples);
        // Down to mono: averaged.
        let mono = convert(&stereo, 1, 1000);
        assert!((mono[0] - 0.3).abs() < 1e-6 && (mono[1] - 0.7).abs() < 1e-6);
        // Mono up to stereo: copied.
        let m = Pcm { samples: vec![0.5, -0.5], channels: 1, sample_rate: 1000 };
        assert_eq!(convert(&m, 2, 1000), [0.5, 0.5, -0.5, -0.5]);
        // Twice the rate: twice the frames, interpolated in between.
        let up = convert(&Pcm { samples: vec![0.0, 1.0, 0.0], channels: 1, sample_rate: 1000 }, 1, 2000);
        assert_eq!(up.len(), 6);
        assert!((up[1] - 0.5).abs() < 1e-6 && (up[2] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn copy_and_take() {
        let p = Pcm { samples: (0..10).map(|i| i as f32).collect(), channels: 1, sample_rate: 1000 };
        assert!(!copy(&p, 5, 5));
        assert!(copy(&p, 2, 5));
        assert_eq!(info(), Some(ClipInfo { frames: 3, channels: 1, sample_rate: 1000 }));
        assert_eq!(take_as(1, 1000).unwrap(), [2.0, 3.0, 4.0]);
    }
}
