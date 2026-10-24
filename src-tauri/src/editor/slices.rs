//! Cutting a range into slices: on a beat grid, at transients, or around silence.

use serde::{Deserialize, Serialize};

use super::zero_crossing;
use crate::audio::Pcm;

/// A slice, `[start, end)` frames.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(tag = "mode", rename_all = "camelCase")]
pub enum SliceBy {
    /// Lines at `origin + k * step` frames (the editor's bar grid).
    #[serde(rename_all = "camelCase")]
    Grid { origin: f64, step: f64 },
    /// At attacks; `sensitivity` 0…1, slices at least `min_ms` long.
    #[serde(rename_all = "camelCase")]
    Transients { sensitivity: f32, min_ms: f32 },
    /// The sounding parts between silences quieter than `threshold_db` and longer than `min_silence_ms`.
    #[serde(rename_all = "camelCase")]
    Silence { threshold_db: f32, min_silence_ms: f32 },
}

/// Analysis hop and window, in frames.
const HOP: usize = 256;
const WINDOW: usize = 512;
/// Cut points move to a zero crossing within this much (a 50 Hz period).
const ZERO_RADIUS_S: f64 = 0.01;
/// Slices shorter than this are dropped.
const MIN_SLICE_S: f64 = 0.01;

/// Slices of `[start, end)`. Cuts between neighbouring slices move to zero crossings when `zero` is set.
pub fn slices(pcm: &Pcm, start: usize, end: usize, by: SliceBy, zero: bool) -> Vec<Span> {
    let frames = pcm.frames();
    let (start, end) = (start.min(frames), end.min(frames).max(start.min(frames)));
    let sr = pcm.sample_rate.max(1) as f64;
    let min_len = (MIN_SLICE_S * sr) as usize;
    if end - start < min_len.max(2) {
        return Vec::new();
    }
    let radius = (ZERO_RADIUS_S * sr) as usize;
    let snap = |f: usize| if zero { zero_crossing(pcm, f, radius).clamp(start, end) } else { f };

    let spans = match by {
        SliceBy::Grid { origin, step } => {
            // At most a few thousand slices; a tiny step is a mistake, not a request.
            if !(step.is_finite() && step >= sr * 0.005) {
                return Vec::new();
            }
            let first = ((start as f64 - origin) / step).ceil() as i64;
            let cuts: Vec<usize> = (first..)
                .map(|k| (origin + k as f64 * step).round())
                .take_while(|&f| f < end as f64)
                .take(4096)
                .filter(|&f| f > start as f64)
                .map(|f| snap(f as usize))
                .collect();
            contiguous(start, end, &cuts)
        }
        SliceBy::Transients { sensitivity, min_ms } => {
            let min_gap = ((min_ms.max(10.0) as f64 / 1000.0) * sr) as usize;
            let cuts: Vec<usize> = onsets(pcm, start, end, sensitivity, min_gap)
                .into_iter()
                .filter(|&f| f > start + min_gap / 2)
                .map(snap)
                .collect();
            contiguous(start, end, &cuts)
        }
        SliceBy::Silence { threshold_db, min_silence_ms } => {
            let min_silence = ((min_silence_ms.max(5.0) as f64 / 1000.0) * sr) as usize;
            sounding(pcm, start, end, threshold_db, min_silence)
        }
    };
    let mut spans: Vec<Span> = spans.into_iter().filter(|s| s.end >= s.start + min_len).collect();
    spans.dedup_by(|b, a| b.start == a.start);
    spans
}

/// Back-to-back slices from cut points.
fn contiguous(start: usize, end: usize, cuts: &[usize]) -> Vec<Span> {
    let mut points = vec![start];
    points.extend(cuts.iter().copied().filter(|&c| c > start && c < end));
    points.sort_unstable();
    points.dedup();
    points.push(end);
    points.windows(2).map(|w| Span { start: w[0], end: w[1] }).collect()
}

/// Mixed-down level per hop, in log10 RMS, of `signal(frame)` over a window.
fn levels(start: usize, end: usize, signal: impl Fn(usize) -> f32) -> Vec<f32> {
    let n = (end - start) / HOP + 1;
    (0..n)
        .map(|i| {
            let a = start + i * HOP;
            let b = (a + WINDOW).min(end);
            if b <= a {
                return -10.0;
            }
            let sq: f32 = (a..b).map(|f| signal(f).powi(2)).sum();
            ((sq / (b - a) as f32).sqrt() + 1e-9).log10()
        })
        .collect()
}

fn mix(pcm: &Pcm, f: usize) -> f32 {
    let ch = pcm.channels.max(1);
    pcm.samples[f * ch..(f + 1) * ch].iter().sum::<f32>() / ch as f32
}

/// Attack positions: rises of the high-passed level (the first difference brings out
/// clicks and hits over sustained low end) well above its recent past, a few ms early
/// so the slice keeps the very start of the hit.
fn onsets(pcm: &Pcm, start: usize, end: usize, sensitivity: f32, min_gap: usize) -> Vec<usize> {
    let diff = |f: usize| if f > 0 { mix(pcm, f) - mix(pcm, f - 1) } else { 0.0 };
    let level = levels(start, end, diff);
    let peak = level.iter().copied().fold(f32::MIN, f32::max);
    // A rise of 12 dB at the lowest sensitivity, 2 dB at the highest (log10 units: 20 dB = 1).
    let rise = 0.6 - 0.5 * sensitivity.clamp(0.0, 1.0);
    // Ignore what is more than 50 dB under the loudest part.
    let floor = peak - 2.5;
    let pre_roll = (pcm.sample_rate as usize / 200).min(HOP); // 5 ms
    let lookback = 8; // ~45 ms at 44.1 kHz
    let mut out: Vec<usize> = Vec::new();
    for i in 1..level.len() {
        if level[i] < floor {
            continue;
        }
        let past = level[i.saturating_sub(lookback)..i].iter().copied().fold(f32::MAX, f32::min);
        let novelty = level[i] - past;
        // The top of the rise: the next hop is not higher.
        let is_peak = level.get(i + 1).is_none_or(|&next| next <= level[i]);
        if novelty >= rise && is_peak {
            // Back to where the rise began.
            let mut j = i;
            while j > 0 && level[j - 1] < level[j] && i - j < lookback {
                j -= 1;
            }
            let at = (start + j * HOP).saturating_sub(pre_roll).max(start);
            if out.last().is_none_or(|&last| at >= last + min_gap) {
                out.push(at);
            }
        }
    }
    out
}

/// The parts louder than `threshold_db`, joined across silences shorter than `min_silence`.
fn sounding(pcm: &Pcm, start: usize, end: usize, threshold_db: f32, min_silence: usize) -> Vec<Span> {
    let level = levels(start, end, |f| mix(pcm, f));
    let threshold = threshold_db / 20.0;
    let mut spans: Vec<Span> = Vec::new();
    let mut open: Option<usize> = None;
    for (i, &l) in level.iter().enumerate() {
        let at = (start + i * HOP).min(end);
        match (l > threshold, open) {
            (true, None) => open = Some(at),
            (false, Some(s)) => {
                // The window reaches ahead: the sound lasts until this hop's window ends.
                spans.push(Span { start: s, end: (at + WINDOW).min(end) });
                open = None;
            }
            _ => {}
        }
    }
    if let Some(s) = open {
        spans.push(Span { start: s, end });
    }
    // Short gaps are part of the sound (a flam, a breath).
    let mut joined: Vec<Span> = Vec::new();
    for s in spans {
        match joined.last_mut() {
            Some(prev) if s.start <= prev.end + min_silence => prev.end = prev.end.max(s.end),
            _ => joined.push(s),
        }
    }
    joined
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: usize = 44100;

    /// One second of silence with decaying 200 Hz hits at the given times (s).
    fn hits(times: &[f64], seconds: f64) -> Pcm {
        let n = (seconds * SR as f64) as usize;
        let mut samples = vec![0.0f32; n];
        for &t in times {
            let at = (t * SR as f64) as usize;
            for i in 0..(SR / 5).min(n - at) {
                let x = i as f32 / SR as f32;
                samples[at + i] += 0.8 * (x * 200.0 * std::f32::consts::TAU).sin() * (-x * 30.0).exp();
            }
        }
        Pcm { samples, channels: 1, sample_rate: SR as u32 }
    }

    #[test]
    fn grid_cuts_back_to_back() {
        let p = hits(&[], 2.0);
        let s = slices(&p, 0, 2 * SR, SliceBy::Grid { origin: 0.0, step: SR as f64 / 2.0 }, false);
        assert_eq!(s.len(), 4);
        assert_eq!(s[0], Span { start: 0, end: SR / 2 });
        assert_eq!(s[3].end, 2 * SR);
        // A shifted origin and a range that starts between lines.
        let s = slices(&p, SR / 4, 2 * SR, SliceBy::Grid { origin: 100.0, step: SR as f64 }, false);
        assert_eq!(s.iter().map(|s| s.start).collect::<Vec<_>>(), [SR / 4, SR + 100]);
        // A silly step yields nothing rather than thousands of slices.
        assert!(slices(&p, 0, 2 * SR, SliceBy::Grid { origin: 0.0, step: 3.0 }, false).is_empty());
    }

    #[test]
    fn transients_find_each_hit() {
        let times = [0.1, 0.5, 0.75, 1.4];
        let p = hits(&times, 2.0);
        let s = slices(&p, 0, 2 * SR, SliceBy::Transients { sensitivity: 0.5, min_ms: 50.0 }, true);
        // The lead-in before the first hit, then one slice per hit.
        assert_eq!(s.len(), times.len() + 1, "{s:?}");
        for (span, t) in s[1..].iter().zip(times) {
            let hit = (t * SR as f64) as usize;
            // Starts just before the hit (pre-roll, hop, zero snap), never after it.
            assert!(span.start <= hit && hit - span.start < SR / 50, "{span:?} for {t}");
        }
        assert_eq!(s.last().unwrap().end, 2 * SR);
    }

    #[test]
    fn silence_keeps_the_sounding_parts() {
        let p = hits(&[0.2, 0.9], 1.5);
        let s = slices(&p, 0, p.frames(), SliceBy::Silence { threshold_db: -50.0, min_silence_ms: 100.0 }, false);
        assert_eq!(s.len(), 2, "{s:?}");
        let near = |f: usize, t: f64| (f as f64 / SR as f64 - t).abs() < 0.02;
        assert!(near(s[0].start, 0.2) && near(s[1].start, 0.9), "{s:?}");
        // Each runs until the hit has decayed below the threshold, well before the next.
        assert!(s[0].end > s[0].start + SR / 10 && s[0].end < s[1].start);
        // A long minimum silence joins them.
        let s = slices(&p, 0, p.frames(), SliceBy::Silence { threshold_db: -50.0, min_silence_ms: 1000.0 }, false);
        assert_eq!(s.len(), 1);
    }
}
