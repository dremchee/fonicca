//! Adapted from Spotify's note_creation.py (Apache-2.0); see the packaged NOTICE.
use super::{check, failure};
use crate::error::Result;
use serde::Serialize;
use std::{cmp::Ordering, collections::BinaryHeap, sync::atomic::AtomicBool};
const KEYS: usize = 88;
const FRAME_THRESHOLD: f32 = 0.3;
const ONSET_THRESHOLD: f64 = 0.5;
const MIN_LENGTH: usize = 11;
const TOLERANCE: usize = 11;
#[derive(Debug, Clone, Serialize)]
pub struct Note {
    pub start: f64,
    pub end: f64,
    pub pitch: u8,
    pub velocity: u8,
}
// Same alignment correction as Spotify, including its training-window offset.
fn time(frame: usize) -> f64 {
    let hop = 256.0 / 22050.0;
    frame as f64 * hop - (frame / 172) as f64 * (hop * (172.0 - 43844.0 / 256.0) + 0.0018)
}
#[derive(Clone, Copy)]
struct Energy {
    value: f32,
    index: usize,
}
impl PartialEq for Energy {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && self.value == other.value
    }
}
impl Eq for Energy {}
impl PartialOrd for Energy {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Energy {
    fn cmp(&self, other: &Self) -> Ordering {
        self.value
            .total_cmp(&other.value)
            .then_with(|| other.index.cmp(&self.index))
    }
}
fn clear(remaining: &mut [f32], frame: usize, key: usize) {
    for p in key.saturating_sub(1)..=(key + 1).min(KEYS - 1) {
        remaining[frame * KEYS + p] = 0.0;
    }
}
fn add(notes: &mut Vec<Note>, frames: &[f32], start: usize, end: usize, key: usize, duration: f64) {
    if end <= start + MIN_LENGTH {
        return;
    }
    let amplitude = (start..end)
        .map(|i| frames[i * KEYS + key] as f64)
        .sum::<f64>()
        / (end - start) as f64;
    let start_time = time(start).max(0.0).min(duration);
    let end_time = time(end).max(0.0).min(duration);
    if end_time > start_time && amplitude > 0.0 {
        notes.push(Note {
            start: start_time,
            end: end_time,
            pitch: (key + 21) as u8,
            velocity: (amplitude * 127.0).round_ties_even().clamp(1.0, 127.0) as u8,
        });
    }
}
pub fn decode(
    frames: &[f32],
    predicted_onsets: &[f32],
    duration: f64,
    cancel: &AtomicBool,
) -> Result<Vec<Note>> {
    if frames.len() != predicted_onsets.len()
        || frames.len() % KEYS != 0
        || !duration.is_finite()
        || duration <= 0.0
        || frames
            .iter()
            .chain(predicted_onsets)
            .any(|v| !v.is_finite())
    {
        return Err(failure("invalid note predictions"));
    }
    check(cancel)?;
    let n = frames.len() / KEYS;
    if n < 3 {
        return Ok(vec![]);
    }
    let mut onsets: Vec<f64> = predicted_onsets.iter().map(|v| *v as f64).collect();
    let mut difference = vec![0.0f64; frames.len()];
    for t in 2..n {
        for p in 0..KEYS {
            let current = frames[t * KEYS + p] as f64;
            difference[t * KEYS + p] = (current - frames[(t - 1) * KEYS + p] as f64)
                .min(current - frames[(t - 2) * KEYS + p] as f64)
                .max(0.0);
        }
    }
    let maximum = difference.iter().copied().fold(0.0, f64::max);
    if maximum > 0.0 {
        let scale = onsets.iter().copied().fold(0.0, f64::max) / maximum;
        for (onset, delta) in onsets.iter_mut().zip(difference) {
            *onset = onset.max(delta * scale);
        }
    }
    let mut remaining = frames.to_vec();
    let mut notes = Vec::new();
    // argrelmax + reverse lexicographic onset order, matching Spotify.
    for start in (1..n - 1).rev() {
        check(cancel)?;
        for key in (0..KEYS).rev() {
            let v = onsets[start * KEYS + key];
            if v < ONSET_THRESHOLD
                || v <= onsets[(start - 1) * KEYS + key]
                || v <= onsets[(start + 1) * KEYS + key]
            {
                continue;
            }
            let mut end = start + 1;
            let mut quiet = 0;
            while end < n - 1 && quiet < TOLERANCE {
                quiet = if remaining[end * KEYS + key] < FRAME_THRESHOLD {
                    quiet + 1
                } else {
                    0
                };
                end += 1;
            }
            end -= quiet;
            if end <= start + MIN_LENGTH {
                continue;
            }
            for t in start..end {
                clear(&mut remaining, t, key);
            }
            add(&mut notes, frames, start, end, key, duration);
        }
    }
    // The reference repeatedly scans the whole matrix for argmax. A heap picks
    // the same maxima/ties while avoiding quadratic work on long recordings.
    let mut heap = BinaryHeap::from(
        remaining
            .iter()
            .enumerate()
            .filter(|(_, v)| **v > FRAME_THRESHOLD)
            .map(|(index, value)| Energy {
                index,
                value: *value,
            })
            .collect::<Vec<_>>(),
    );
    while let Some(energy) = heap.pop() {
        check(cancel)?;
        if remaining[energy.index] != energy.value {
            continue;
        }
        let mid = energy.index / KEYS;
        let key = energy.index % KEYS;
        remaining[energy.index] = 0.0;
        let mut i = mid + 1;
        let mut quiet = 0;
        while i < n - 1 && quiet < TOLERANCE {
            quiet = if remaining[i * KEYS + key] < FRAME_THRESHOLD {
                quiet + 1
            } else {
                0
            };
            clear(&mut remaining, i, key);
            i += 1;
        }
        let end = i.saturating_sub(1 + quiet);
        let mut i = mid as isize - 1;
        quiet = 0;
        while i > 0 && quiet < TOLERANCE {
            quiet = if remaining[i as usize * KEYS + key] < FRAME_THRESHOLD {
                quiet + 1
            } else {
                0
            };
            clear(&mut remaining, i as usize, key);
            i -= 1;
        }
        let start = (i + 1 + quiet as isize).max(0) as usize;
        add(&mut notes, frames, start, end, key, duration);
    }
    notes.sort_by(|a, b| {
        a.start
            .total_cmp(&b.start)
            .then(a.pitch.cmp(&b.pitch))
            .then(a.end.total_cmp(&b.end))
    });
    Ok(notes)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn silence_and_flat_energy_are_finite_and_short_notes_are_rejected() {
        let silent = vec![0.0; 100 * KEYS];
        assert!(
            decode(&silent, &silent, 1.0, &AtomicBool::new(false))
                .unwrap()
                .is_empty()
        );
        let mut frames = silent.clone();
        for t in 10..16 {
            frames[t * KEYS + 39] = 0.9;
        }
        assert!(
            decode(&frames, &silent, 1.0, &AtomicBool::new(false))
                .unwrap()
                .is_empty()
        );
        for t in 10..60 {
            frames[t * KEYS + 39] = 0.9;
        }
        let result = decode(&frames, &silent, 1.0, &AtomicBool::new(false)).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].pitch, 60);
        assert!(result[0].start >= 0.0 && result[0].end <= 1.0);
        assert!(decode(&frames, &silent, 1.0, &AtomicBool::new(true)).is_err());
    }
    #[test]
    fn polyphony_and_repeated_notes_preserve_midi_pitches() {
        let mut frames = vec![0.0; 180 * KEYS];
        let mut onsets = frames.clone();
        for pitch in [39, 43, 46] {
            for t in 10..140 {
                frames[t * KEYS + pitch] = 0.8;
            }
            onsets[10 * KEYS + pitch] = 0.9;
            onsets[80 * KEYS + pitch] = 0.9;
        }
        let result = decode(&frames, &onsets, 2.0, &AtomicBool::new(false)).unwrap();
        for pitch in [60, 64, 67] {
            assert_eq!(result.iter().filter(|n| n.pitch == pitch).count(), 2);
        }
        assert!(result.iter().all(|n| n.velocity > 0 && n.end > n.start));
        for t in 1..1000 {
            assert!(time(t) > time(t - 1));
        }
    }
}
