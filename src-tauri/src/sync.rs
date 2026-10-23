//! Preview in the project's tempo and key: loops are time-stretched to the
//! project BPM and tonal samples transposed to the project key.

use serde::{Deserialize, Serialize};
use signalsmith_stretch::Stretch;

use crate::audio::Pcm;

/// Project settings sent with a play request.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Target {
    pub bpm: Option<f64>,
    /// Key name like "Am" or "F#".
    pub key: Option<String>,
}

/// How a sample is changed for playback.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Adjust {
    /// Output length ÷ input length (2.0 plays at half speed).
    pub stretch: f64,
    pub semitones: f64,
}

impl Adjust {
    pub const NONE: Adjust = Adjust { stretch: 1.0, semitones: 0.0 };

    pub fn is_none(&self) -> bool {
        (self.stretch - 1.0).abs() < 1e-3 && self.semitones.abs() < 1e-3
    }
}

/// Stretching beyond this sounds bad; half/double time is used instead.
const MAX_STRETCH: f64 = 2.0;

/// Pitch class of the key's tonic in its relative major, so "Am" and "C" (same
/// notes) need no transposition between them.
fn major_equivalent(key: &str) -> Option<i32> {
    let (note, minor) = match key.strip_suffix('m') {
        Some(n) => (n, true),
        None => (key, false),
    };
    const NOTES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    let pc = NOTES.iter().position(|n| *n == note)? as i32;
    Some(if minor { (pc + 3) % 12 } else { pc })
}

/// Smallest transposition (−6…+5 semitones) from one key to another.
pub fn semitones_between(from: &str, to: &str) -> Option<f64> {
    let d = (major_equivalent(to)? - major_equivalent(from)?).rem_euclid(12);
    Some(if d > 5 { d - 12 } else { d } as f64)
}

/// Stretch that brings `sample_bpm` to `target_bpm`, preferring half or double
/// time when that keeps the change smaller (an 85 BPM loop in a 170 BPM project).
pub fn stretch_for(sample_bpm: f64, target_bpm: f64) -> Option<f64> {
    if sample_bpm <= 0.0 || target_bpm <= 0.0 {
        return None;
    }
    [sample_bpm, sample_bpm * 2.0, sample_bpm / 2.0]
        .iter()
        .map(|b| b / target_bpm)
        .filter(|s| (1.0 / MAX_STRETCH..=MAX_STRETCH).contains(s))
        .min_by(|a, b| a.ln().abs().total_cmp(&b.ln().abs()))
}

/// Tempo applies to loops with a known BPM; key to any sample with a known key.
pub fn plan(kind: Option<&str>, bpm: Option<f64>, key: Option<&str>, target: &Target) -> Adjust {
    let stretch = match (kind, bpm, target.bpm) {
        (Some("loop"), Some(b), Some(t)) => stretch_for(b, t).unwrap_or(1.0),
        _ => 1.0,
    };
    let semitones = match (key, target.key.as_deref()) {
        (Some(k), Some(t)) => semitones_between(k, t).unwrap_or(0.0),
        _ => 0.0,
    };
    Adjust { stretch, semitones }
}

/// Renders the adjusted audio in one pass (for short samples; latency is
/// compensated by the library's exact mode).
pub fn render(pcm: &Pcm, adjust: Adjust) -> Pcm {
    let channels = pcm.channels.max(1);
    let frames_in = pcm.frames();
    let frames_out = ((frames_in as f64) * adjust.stretch).round().max(1.0) as usize;
    let mut stretch = Stretch::preset_default(channels as u32, pcm.sample_rate);
    stretch.set_transpose_factor_semitones(adjust.semitones as f32, None);
    let mut out = vec![0.0f32; frames_out * channels];
    stretch.exact(&pcm.samples[..frames_in * channels], &mut out);
    Pcm { samples: out, channels, sample_rate: pcm.sample_rate }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transposes_by_the_smallest_interval_between_relative_keys() {
        assert_eq!(semitones_between("C", "D"), Some(2.0));
        assert_eq!(semitones_between("C", "A#"), Some(-2.0));
        assert_eq!(semitones_between("Am", "C"), Some(0.0)); // relative keys
        assert_eq!(semitones_between("Am", "Bm"), Some(2.0));
        assert_eq!(semitones_between("F#m", "Cm"), Some(-6.0));
        assert_eq!(semitones_between("H", "C"), None);
    }

    #[test]
    fn stretches_toward_the_nearest_of_half_normal_double_time() {
        assert_eq!(stretch_for(120.0, 120.0), Some(1.0));
        assert!((stretch_for(90.0, 100.0).unwrap() - 0.9).abs() < 1e-9);
        assert_eq!(stretch_for(85.0, 170.0), Some(1.0)); // double time
        assert_eq!(stretch_for(170.0, 85.0), Some(1.0)); // half time
        assert_eq!(stretch_for(0.0, 120.0), None);
    }

    #[test]
    fn plans_tempo_for_loops_and_key_for_tonal_samples() {
        let target = Target { bpm: Some(100.0), key: Some("D".into()) };
        let a = plan(Some("loop"), Some(90.0), Some("C"), &target);
        assert!((a.stretch - 0.9).abs() < 1e-9 && a.semitones == 2.0);
        // One-shots keep their length.
        let a = plan(Some("oneshot"), Some(90.0), Some("C"), &target);
        assert_eq!(a, Adjust { stretch: 1.0, semitones: 2.0 });
        assert!(plan(Some("loop"), None, None, &target).is_none());
    }

    #[test]
    fn render_changes_length_and_pitch() {
        let sr = 44100;
        // 1 s of 440 Hz.
        let samples: Vec<f32> = (0..sr).map(|i| (i as f32 / sr as f32 * 440.0 * std::f32::consts::TAU).sin() * 0.5).collect();
        let pcm = Pcm { samples, channels: 1, sample_rate: sr as u32 };
        let out = render(&pcm, Adjust { stretch: 1.5, semitones: 12.0 });
        assert_eq!(out.frames(), (sr as f64 * 1.5) as usize);
        // An octave up: zero crossings in the middle of the signal roughly double.
        let mid = &out.samples[out.samples.len() / 4..out.samples.len() * 3 / 4];
        let crossings = mid.windows(2).filter(|w| w[0] <= 0.0 && w[1] > 0.0).count() as f64;
        let seconds = mid.len() as f64 / sr as f64;
        let freq = crossings / seconds;
        assert!((freq - 880.0).abs() < 40.0, "{freq} Hz");
    }
}

#[cfg(test)]
mod timing {
    /// `cargo test --release -- --ignored --nocapture render_timing`
    #[test]
    #[ignore]
    fn render_timing() {
        for seconds in [4usize, 30] {
            let sr = 44100;
            let samples: Vec<f32> = (0..sr * seconds * 2).map(|i| ((i as f32) * 0.01).sin() * 0.3).collect();
            let pcm = crate::audio::Pcm { samples, channels: 2, sample_rate: sr as u32 };
            let t = std::time::Instant::now();
            let out = super::render(&pcm, super::Adjust { stretch: 0.92, semitones: 2.0 });
            println!("{seconds} s stereo loop: {:?} ({} frames)", t.elapsed(), out.frames());
        }
    }
}
