//! Capture edited audio without materializing PCM. Mono conversion and FFT
//! resampling run on the transcription worker, with the model's existing input.
use super::{failure, RATE};
use crate::audio::{AudioRead, Pcm};
use crate::editor::{blocks::BlockAudio, memory, Span};
use crate::error::Result;

pub(crate) fn snapshot(audio: &BlockAudio, range: Span) -> Result<BlockAudio> {
    if range.start >= range.end
        || range.end > audio.frames()
        || (range.end - range.start) as f64 / audio.sample_rate as f64 > 600.0
    {
        return Err(crate::error::Error::Other(
            "select a non-empty fragment up to 10 minutes".into(),
        ));
    }
    Ok(audio.slice_frames(range.start, range.end))
}

fn validate_format(audio: &impl AudioRead) -> Result<()> {
    if audio.frames() == 0
        || !(1..=2).contains(&audio.channels())
        || audio.sample_rate() == 0
        || audio.frames() as f64 / audio.sample_rate() as f64 > 600.0
    {
        return Err(failure("use finite mono or stereo audio up to 10 minutes"));
    }
    Ok(())
}

fn mono_samples(audio: &BlockAudio) -> Result<Vec<f32>> {
    let mut output = Vec::with_capacity(audio.frames());
    for start in (0..audio.frames()).step_by(16384) {
        let end = (start + 16384).min(audio.frames());
        // A crossing between pieces materializes at most one small block.
        // Contiguous RAM/mapped pieces are read directly as borrowed slices.
        let samples = audio.range(start * audio.channels..end * audio.channels);
        if samples.iter().any(|v| !v.is_finite()) {
            return Err(failure("use finite mono or stereo audio up to 10 minutes"));
        }
        if audio.channels == 1 {
            output.extend_from_slice(&samples);
        } else {
            output.extend(samples.chunks_exact(audio.channels).map(|frame| {
                (frame.iter().map(|v| *v as f64).sum::<f64>() / frame.len() as f64) as f32
            }));
        }
    }
    Ok(output)
}

/// Owned file decoding keeps the existing move-only mono path.
pub(crate) fn prepare(mut pcm: Pcm) -> Result<Pcm> {
    validate_format(&pcm)?;
    if pcm.samples.iter().any(|v| !v.is_finite()) {
        return Err(failure("use finite mono or stereo audio up to 10 minutes"));
    }
    if pcm.channels != 1 {
        pcm.samples = pcm
            .samples
            .chunks_exact(pcm.channels)
            .map(|frame| (frame.iter().map(|v| *v as f64).sum::<f64>() / frame.len() as f64) as f32)
            .collect();
        pcm.channels = 1;
    }
    crate::batch::resample(&mut pcm, RATE)?;
    Ok(pcm)
}

/// Only a mono model input is allocated from shared editor pieces. The original
/// interleaved selection is never copied into a second owned PCM buffer.
pub(crate) fn prepare_snapshot(audio: &BlockAudio) -> Result<Pcm> {
    validate_format(audio)?;
    let output_frames = if audio.sample_rate == RATE || audio.frames() < 2 {
        0
    } else {
        (audio.frames() as f64 * RATE as f64 / audio.sample_rate as f64).ceil() as usize
    };
    // The existing resampler retains mono input and output while converting.
    let count = audio
        .frames()
        .checked_add(output_frames)
        .and_then(|count| count.checked_add(16384 * audio.channels))
        .ok_or_else(|| failure("audio length is too large"))?;
    memory::check_samples(count, memory::pcm_budget())?;
    let mut pcm = Pcm {
        samples: mono_samples(audio)?,
        channels: 1,
        sample_rate: audio.sample_rate,
    };
    crate::batch::resample(&mut pcm, RATE)?;
    Ok(pcm)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::{Curve, Op};

    // The previous owned interleaved input/downmix/resample sequence.
    fn reference(mut pcm: Pcm) -> Pcm {
        if pcm.channels != 1 {
            pcm.samples = pcm
                .samples
                .chunks_exact(pcm.channels)
                .map(|frame| {
                    (frame.iter().map(|v| *v as f64).sum::<f64>() / frame.len() as f64) as f32
                })
                .collect();
            pcm.channels = 1;
        }
        crate::batch::resample(&mut pcm, RATE).unwrap();
        pcm
    }

    #[test]
    fn snapshot_shares_edited_pieces_and_survives_later_edits_and_close() {
        let mut audio = BlockAudio::from(Pcm {
            samples: (0..1000).map(|n| n as f32 / 1000.0).collect(),
            channels: 2,
            sample_rate: 1000,
        });
        audio
            .try_apply(&Op::Delete {
                start: 20,
                end: 100,
            })
            .unwrap();
        audio
            .try_apply(&Op::FadeOut {
                start: 200,
                end: 300,
                curve: Curve::Smooth,
            })
            .unwrap();
        let selected = snapshot(
            &audio,
            Span {
                start: 10,
                end: 350,
            },
        )
        .unwrap();
        assert!(selected.samples.shares_storage(&audio.samples));
        let expected = Pcm {
            samples: audio.range(20..700).into_owned(),
            channels: 2,
            sample_rate: 1000,
        };
        audio
            .try_apply(&Op::Silence { start: 0, end: 400 })
            .unwrap();
        drop(audio);
        assert_eq!(selected.samples, expected.samples);
        let actual = prepare_snapshot(&selected).unwrap();
        assert_eq!(actual.samples, reference(expected).samples);
    }

    #[test]
    fn prepared_snapshot_and_owned_file_match_previous_model_input() {
        for frames in [1, 17, 1024, 4097, 16385, 32771] {
            for channels in [1, 2] {
                for sample_rate in [8000, 22050, 44100, 48000, 96000] {
                    let pcm = Pcm {
                        samples: (0..frames * channels)
                            .map(|n| ((n as f32 * 0.017).sin() + (n % 7) as f32 / 8.0) / 2.0)
                            .collect(),
                        channels,
                        sample_rate,
                    };
                    let expected = reference(pcm.clone());
                    let owned = prepare(pcm.clone()).unwrap();
                    let audio = BlockAudio::from(pcm);
                    let actual = prepare_snapshot(&audio).unwrap();
                    assert_eq!((actual.channels, actual.sample_rate), (1, RATE));
                    assert_eq!(
                        actual.samples, expected.samples,
                        "{frames} frames, {channels} channels, {sample_rate} Hz"
                    );
                    assert_eq!(owned.samples, expected.samples);
                }
            }
        }
        let pcm = Pcm {
            samples: vec![0.1; 100],
            channels: 1,
            sample_rate: RATE,
        };
        let pointer = pcm.samples.as_ptr();
        assert_eq!(
            prepare(pcm).unwrap().samples.as_ptr(),
            pointer,
            "already-prepared owned input must not be copied"
        );
    }

    #[test]
    fn invalid_ranges_formats_non_finite_and_excessive_duration_are_rejected() {
        let audio = BlockAudio::from(Pcm {
            samples: vec![0.2; 1202],
            channels: 2,
            sample_rate: 1,
        });
        for range in [
            Span { start: 0, end: 0 },
            Span { start: 5, end: 4 },
            Span { start: 0, end: 602 },
            Span { start: 0, end: 601 },
        ] {
            assert!(snapshot(&audio, range).is_err());
        }
        assert_eq!(
            snapshot(&audio, Span { start: 1, end: 601 })
                .unwrap()
                .frames(),
            600
        );
        for pcm in [
            Pcm {
                samples: vec![],
                channels: 1,
                sample_rate: RATE,
            },
            Pcm {
                samples: vec![f32::NAN, 0.0],
                channels: 2,
                sample_rate: RATE,
            },
            Pcm {
                samples: vec![f32::INFINITY],
                channels: 1,
                sample_rate: RATE,
            },
            Pcm {
                samples: vec![0.0; 3],
                channels: 3,
                sample_rate: RATE,
            },
            Pcm {
                samples: vec![0.0],
                channels: 1,
                sample_rate: 0,
            },
            Pcm {
                samples: vec![0.0; 601],
                channels: 1,
                sample_rate: 1,
            },
        ] {
            assert!(prepare(pcm.clone()).is_err());
            assert!(prepare_snapshot(&BlockAudio::from(pcm)).is_err());
        }
        let mut samples = vec![0.1; 32772];
        *samples.last_mut().unwrap() = f32::INFINITY;
        let source = BlockAudio::from(Pcm {
            samples,
            channels: 2,
            sample_rate: 44100,
        });
        let before = source.clone();
        assert!(
            prepare_snapshot(&source).is_err(),
            "a bad sample in a later block must not reach the model"
        );
        assert!(source.samples.same_snapshot(&before.samples));
    }
}
