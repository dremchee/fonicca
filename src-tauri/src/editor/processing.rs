//! Render expensive edits once off-thread and keep their result in undo history.
use super::Op;
use crate::audio::Pcm;
use crate::error::{Error, Result};
use ebur128::{EbuR128, Mode};
use serde::{Deserialize, Serialize};
use signalsmith_stretch::Stretch;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(tag = "mode", rename_all = "camelCase")]
pub enum Processing {
    Stretch { semitones: f64, ratio: f64 },
    Lufs { target: f64, ceiling: f64 },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoudnessReport {
    pub before: f64,
    pub after: f64,
    pub gain_db: f64,
    pub limited: bool,
}

pub struct Rendered {
    pub op: Op,
    pub end: usize,
    pub loudness: Option<LoudnessReport>,
}

fn invalid(message: &str) -> Error {
    Error::Other(message.into())
}

fn integrated(pcm: &Pcm) -> Result<f64> {
    let mut meter = EbuR128::new(pcm.channels as u32, pcm.sample_rate, Mode::I)
        .map_err(|e| invalid(&e.to_string()))?;
    meter
        .add_frames_f32(&pcm.samples)
        .map_err(|e| invalid(&e.to_string()))?;
    meter
        .loudness_global()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or_else(|| invalid("LUFS cannot be measured: select at least 400 ms of audible audio"))
}

pub fn render(
    mut pcm: Pcm,
    start: usize,
    total_frames: usize,
    processing: Processing,
) -> Result<Rendered> {
    let frames = pcm.frames();
    if frames == 0 || pcm.samples.iter().any(|s| !s.is_finite()) {
        return Err(invalid("select a non-empty range of finite audio"));
    }
    let end = start + frames;
    match processing {
        Processing::Stretch { semitones, ratio } => {
            if !semitones.is_finite()
                || !(-24.0..=24.0).contains(&semitones)
                || !ratio.is_finite()
                || !(0.25..=4.0).contains(&ratio)
            {
                return Err(invalid(
                    "pitch must be between -24 and 24 semitones; duration between 25% and 400%",
                ));
            }
            if frames < 64 {
                return Err(invalid(
                    "select at least 64 audio frames to change pitch or duration",
                ));
            }
            let output_frames = (frames as f64 * ratio).round().max(1.0) as usize;
            if total_frames - frames + output_frames > pcm.sample_rate as usize * 20 * 60 {
                return Err(invalid(
                    "the result would be longer than the editor's 20-minute limit",
                ));
            }
            let output = stretch(&pcm, semitones, ratio)?;
            Ok(Rendered {
                end: start + output.frames(),
                op: Op::Stretch {
                    start,
                    end,
                    samples: Arc::new(output.samples),
                },
                loudness: None,
            })
        }
        Processing::Lufs { target, ceiling } => {
            if !target.is_finite()
                || !(-36.0..=-5.0).contains(&target)
                || !ceiling.is_finite()
                || !(-12.0..=0.0).contains(&ceiling)
            {
                return Err(invalid("target must be between -36 and -5 LUFS; sample peak ceiling between -12 and 0 dBFS"));
            }
            let before = integrated(&pcm)?;
            let peak = pcm
                .samples
                .iter()
                .fold(0.0f64, |m, s| m.max(s.abs() as f64));
            let desired = target - before;
            let gain_db = desired.min(ceiling - 20.0 * peak.log10());
            let gain = 10f64.powf(gain_db / 20.0) as f32;
            pcm.samples.iter_mut().for_each(|s| *s *= gain);
            let after = integrated(&pcm)?;
            Ok(Rendered {
                op: Op::Gain {
                    start,
                    end,
                    db: gain_db as f32,
                },
                end,
                loudness: Some(LoudnessReport {
                    before,
                    after,
                    gain_db,
                    limited: gain_db < desired - 0.01,
                }),
            })
        }
    }
}

/// `exact` needs two output-latency windows. Pad short input with silence,
/// then remove only the padding so every result has the requested frame count.
fn stretch(pcm: &Pcm, semitones: f64, ratio: f64) -> Result<Pcm> {
    let mut processor = Stretch::preset_default(pcm.channels as u32, pcm.sample_rate);
    processor.set_transpose_factor_semitones(semitones as f32, None);
    let wanted = (pcm.frames() as f64 * ratio).round().max(1.0) as usize;
    let minimum_input = ((processor.output_latency() * 2 + 1) as f64 / ratio).ceil() as usize;
    let padded_frames = pcm
        .frames()
        .max(processor.input_latency())
        .max(minimum_input);
    let mut input = std::borrow::Cow::Borrowed(pcm.samples.as_slice());
    if padded_frames > pcm.frames() {
        input.to_mut().resize(padded_frames * pcm.channels, 0.0);
    }
    let output_frames = (padded_frames as f64 * ratio).round() as usize;
    let mut samples = vec![0.0; output_frames * pcm.channels];
    if !processor.exact(input.as_ref(), &mut samples) {
        return Err(invalid(
            "the selected audio is too short for these pitch/time settings",
        ));
    }
    samples.truncate(wanted * pcm.channels);
    Ok(Pcm {
        samples,
        channels: pcm.channels,
        sample_rate: pcm.sample_rate,
    })
}

/// Scale positions inside the region and shift positions after it.
pub fn retimed_frame(point: usize, start: usize, end: usize, inserted: usize) -> usize {
    if point <= start {
        point
    } else if point >= end {
        point - (end - start) + inserted
    } else {
        start + ((point - start) as f64 * inserted as f64 / (end - start) as f64).round() as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::{self, Loop, Marker, Session};

    fn sine(seconds: f64, amplitude: f32) -> Pcm {
        let rate = 48000;
        let samples = (0..(seconds * rate as f64) as usize)
            .map(|i| amplitude * (std::f32::consts::TAU * 440.0 * i as f32 / rate as f32).sin())
            .collect();
        Pcm {
            samples,
            channels: 1,
            sample_rate: rate,
        }
    }

    fn session(pcm: &Pcm) -> (tempfile::TempDir, Session) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("source.wav");
        editor::write_wav(&path, pcm, Some(32), None).unwrap();
        let s = Session::open(1, &path, Some(32)).unwrap();
        (dir, s)
    }

    fn frequency(samples: &[f32], rate: u32) -> f64 {
        let middle = &samples[samples.len() / 4..samples.len() * 3 / 4];
        let crossings = middle
            .windows(2)
            .filter(|w| w[0] <= 0.0 && w[1] > 0.0)
            .count();
        crossings as f64 * rate as f64 / middle.len() as f64
    }

    #[test]
    fn pitch_and_duration_are_independent_and_preserve_channels() {
        let mono = sine(1.0, 0.2);
        let stereo = Pcm {
            samples: mono.samples.iter().flat_map(|s| [*s, -*s]).collect(),
            channels: 2,
            sample_rate: 48000,
        };
        let result = render(
            stereo,
            0,
            48000,
            Processing::Stretch {
                semitones: 12.0,
                ratio: 1.5,
            },
        )
        .unwrap();
        let Op::Stretch { samples, .. } = result.op else {
            panic!("expected stretch")
        };
        assert_eq!(samples.len(), 72000 * 2);
        let left: Vec<f32> = samples.chunks_exact(2).map(|f| f[0]).collect();
        assert!((frequency(&left, 48000) - 880.0).abs() < 8.0);
        assert!(samples.iter().all(|s| s.is_finite()));
        let phase_error = samples
            .chunks_exact(2)
            .map(|f| (f[0] + f[1]).abs())
            .fold(0.0f32, f32::max);
        let right: Vec<f32> = samples.chunks_exact(2).map(|f| f[1]).collect();
        assert!((frequency(&right, 48000) - 880.0).abs() < 8.0);
        // STFT interpolation is not sample-exact across inverted channels.
        assert!(phase_error < 1e-3);

        let result = render(
            sine(1.0, 0.2),
            0,
            48000,
            Processing::Stretch {
                semitones: 0.0,
                ratio: 0.5,
            },
        )
        .unwrap();
        let Op::Stretch { samples, .. } = result.op else {
            panic!("expected stretch")
        };
        assert_eq!(samples.len(), 24000);
        assert!((frequency(&samples, 48000) - 440.0).abs() < 8.0);
    }

    #[test]
    fn short_samples_do_not_turn_into_silence() {
        for ratio in [0.25, 1.0, 4.0] {
            let result = render(
                sine(0.01, 0.2),
                0,
                480,
                Processing::Stretch {
                    semitones: 7.0,
                    ratio,
                },
            )
            .unwrap();
            let Op::Stretch { samples, .. } = result.op else {
                panic!("expected stretch")
            };
            assert_eq!(samples.len(), (480.0 * ratio).round() as usize);
            assert!(
                samples.iter().any(|s| s.abs() > 1e-5),
                "silent result at ratio {ratio}"
            );
        }
    }

    #[test]
    fn lufs_hits_target_without_touching_audio_outside_the_selection() {
        let original = sine(2.0, 0.05);
        let (_dir, mut s) = session(&original);
        let input = Pcm {
            samples: s.audio().samples[24000..72000].to_vec(),
            channels: 1,
            sample_rate: 48000,
        };
        let result = render(
            input,
            24000,
            96000,
            Processing::Lufs {
                target: -18.0,
                ceiling: -0.3,
            },
        )
        .unwrap();
        let report = result.loudness.unwrap();
        assert!(!report.limited);
        assert!((report.after + 18.0).abs() < 0.1);
        let before = s.audio().samples.clone();
        s.apply(result.op);
        assert_eq!(s.audio().samples[..24000], before[..24000]);
        assert_eq!(s.audio().samples[72000..], before[72000..]);
        s.undo();
        assert_eq!(s.audio().samples, before);
        s.redo();
        let measured = integrated(&Pcm {
            samples: s.audio().samples[24000..72000].to_vec(),
            channels: 1,
            sample_rate: 48000,
        })
        .unwrap();
        assert!((measured + 18.0).abs() < 0.1);
    }

    #[test]
    fn lufs_respects_peak_ceiling_and_reports_unreachable_target() {
        let mut input = sine(1.0, 0.01);
        input.samples[24000] = 1.0;
        let before = input.samples.clone();
        let result = render(
            input,
            0,
            48000,
            Processing::Lufs {
                target: -5.0,
                ceiling: -1.0,
            },
        )
        .unwrap();
        let report = result.loudness.unwrap();
        assert!(report.limited && report.after < -5.0);
        let mut output = Pcm {
            samples: before,
            channels: 1,
            sample_rate: 48000,
        };
        editor::apply(&mut output, &result.op);
        let peak = output.samples.iter().fold(0.0f32, |a, b| a.max(b.abs()));
        assert!(20.0 * peak.log10() <= -1.0 + 1e-4);
    }

    #[test]
    fn silence_short_ranges_and_invalid_parameters_do_not_produce_edits() {
        let silent = Pcm {
            samples: vec![0.0; 48000],
            channels: 1,
            sample_rate: 48000,
        };
        assert!(render(
            silent,
            0,
            48000,
            Processing::Lufs {
                target: -14.0,
                ceiling: -0.3
            }
        )
        .is_err());
        assert!(render(
            sine(0.1, 0.2),
            0,
            4800,
            Processing::Lufs {
                target: -14.0,
                ceiling: -0.3
            }
        )
        .is_err());
        assert!(render(
            sine(0.001, 0.2),
            0,
            48,
            Processing::Stretch {
                semitones: 0.0,
                ratio: 2.0
            }
        )
        .is_err());
        for (semitones, ratio) in [(f64::NAN, 1.0), (25.0, 1.0), (0.0, 0.0), (0.0, 4.1)] {
            assert!(render(
                sine(0.01, 0.2),
                0,
                480,
                Processing::Stretch { semitones, ratio }
            )
            .is_err());
        }
        assert!(render(
            sine(1.0, 0.2),
            0,
            48000 * 1200,
            Processing::Stretch {
                semitones: 0.0,
                ratio: 2.0
            }
        )
        .is_err());
    }

    #[test]
    fn stretching_preserves_surrounding_audio_and_retimes_metadata_through_undo_and_save() {
        let (dir, mut s) = session(&sine(1.0, 0.2));
        s.apply(Op::SetLoop {
            start: 12000,
            end: 36000,
        });
        s.apply(Op::SetMarkers {
            markers: vec![
                Marker {
                    frame: 12000,
                    skip: false,
                },
                Marker {
                    frame: 36000,
                    skip: true,
                },
                Marker {
                    frame: 42000,
                    skip: false,
                },
            ],
        });
        let before = s.audio().samples.clone();
        let result = render(
            Pcm {
                samples: before[12000..36000].to_vec(),
                channels: 1,
                sample_rate: 48000,
            },
            12000,
            48000,
            Processing::Stretch {
                semitones: 0.0,
                ratio: 2.0,
            },
        )
        .unwrap();
        s.apply(result.op);
        assert_eq!(s.audio().frames(), 72000);
        assert_eq!(s.audio().samples[..12000], before[..12000]);
        assert_eq!(s.audio().samples[60000..], before[36000..]);
        assert_eq!(
            s.loop_points(),
            Some(Loop {
                start: 12000,
                end: 60000
            })
        );
        assert_eq!(
            s.markers().iter().map(|m| m.frame).collect::<Vec<_>>(),
            [12000, 60000, 66000]
        );
        s.undo();
        assert_eq!(s.audio().samples, before);
        assert_eq!(
            s.loop_points(),
            Some(Loop {
                start: 12000,
                end: 36000
            })
        );
        s.redo();
        let path = dir.path().join("saved.wav");
        editor::write_wav(&path, s.audio(), Some(32), s.loop_points()).unwrap();
        editor::append_markers(&path, s.markers()).unwrap();
        let reopened = Session::open(2, &path, Some(32)).unwrap();
        assert_eq!(reopened.audio().frames(), 72000);
        assert_eq!(reopened.loop_points(), s.loop_points());
        assert_eq!(reopened.markers(), s.markers());
        assert_eq!(retimed_frame(24000, 12000, 36000, 48000), 36000);
    }

    #[test]
    fn asynchronous_results_cannot_overwrite_a_newer_edit_or_undo() {
        let (_dir, mut s) = session(&sine(1.0, 0.1));
        let revision = s.revision();
        let op = || Op::Gain {
            start: 0,
            end: 48000,
            db: -3.0,
        };
        s.apply(op());
        s.undo();
        assert!(s.apply_processed(revision, op()).is_err());
        let current = s.revision();
        s.apply_processed(current, op()).unwrap();
        assert!(s.state().dirty);
    }
}
