//! Single HTDemucs model, with upstream 7.8-second windows and 25% overlap.
use super::{check, dsp::RATE, failure};
use crate::{audio::Pcm, error::Result};
use ort::{
    session::{Session, builder::GraphOptimizationLevel},
    value::Tensor,
};
use std::{path::Path, sync::atomic::AtomicBool};
const SEGMENT: usize = 343980;
const STRIDE: usize = SEGMENT * 3 / 4;

fn normalization(pcm: &Pcm) -> Result<(f64, f64)> {
    if pcm.frames() == 0
        || !(1..=2).contains(&pcm.channels)
        || pcm.sample_rate != RATE
        || pcm.samples.iter().any(|s| !s.is_finite())
    {
        return Err(failure(
            "HTDemucs expects finite, non-empty mono or stereo 44.1 kHz audio",
        ));
    }
    let n = pcm.frames();
    let mono = |i: usize| {
        pcm.samples[i * pcm.channels..(i + 1) * pcm.channels]
            .iter()
            .map(|s| *s as f64)
            .sum::<f64>()
            / pcm.channels as f64
    };
    let mean = (0..n).map(mono).sum::<f64>() / n as f64;
    let variance =
        (0..n).map(|i| (mono(i) - mean).powi(2)).sum::<f64>() / n.saturating_sub(1).max(1) as f64;
    Ok((mean, variance.sqrt().max(1e-8)))
}
fn window(i: usize) -> f32 {
    (i + 1).min(SEGMENT - i) as f32 / (SEGMENT / 2) as f32
}

// A short TensorChunk is centred in the trained 7.8-second window. Context
// comes from the original track where available; only track boundaries are zero.
fn input(pcm: &Pcm, offset: usize, mean: f64, scale: f64) -> (Vec<f32>, usize, usize) {
    let length = SEGMENT.min(pcm.frames() - offset);
    let pad = (SEGMENT - length) / 2;
    let mut data = vec![0.0; 2 * SEGMENT];
    for i in 0..SEGMENT {
        let source = offset as isize + i as isize - pad as isize;
        if source < 0 || source as usize >= pcm.frames() {
            continue;
        }
        for channel in 0..2 {
            data[channel * SEGMENT + i] = ((pcm.samples
                [source as usize * pcm.channels + channel.min(pcm.channels - 1)]
                as f64
                - mean)
                / scale) as f32;
        }
    }
    (data, pad, length)
}

// All four predictions share one inference and one bounded overlap buffer.
fn demix(
    pcm: &Pcm,
    cancel: &AtomicBool,
    progress: impl Fn(f64),
    mut infer: impl FnMut(Vec<f32>) -> Result<Vec<f32>>,
    mut emit: impl FnMut(&[Vec<f32>]) -> Result<()>,
) -> Result<()> {
    check(cancel)?;
    let (mean, scale) = normalization(pcm)?;
    let chunks = pcm.frames().div_ceil(STRIDE);
    let mut accumulated = vec![vec![0.0f32; SEGMENT * 2]; 4];
    let mut weights = vec![0.0f32; SEGMENT];
    for (chunk, offset) in (0..pcm.frames()).step_by(STRIDE).enumerate() {
        check(cancel)?;
        let (data, pad, length) = input(pcm, offset, mean, scale);
        let output = infer(data)?;
        check(cancel)?;
        if output.len() != 4 * 2 * SEGMENT || output.iter().any(|s| !s.is_finite()) {
            return Err(failure("invalid HTDemucs output"));
        }
        for i in 0..length {
            let w = window(i);
            weights[i] += w;
            // Graph: drums, bass, other, vocals. App: vocals, drums, bass, other.
            for (stem, source) in [3, 0, 1, 2].into_iter().enumerate() {
                for ch in 0..2 {
                    accumulated[stem][i * 2 + ch] +=
                        output[(source * 2 + ch) * SEGMENT + pad + i] * w;
                }
            }
        }
        let flush = STRIDE.min(length);
        let block = accumulated
            .iter()
            .map(|stem| {
                (0..flush)
                    .flat_map(|i| {
                        [
                            (stem[i * 2] as f64 / weights[i] as f64 * scale) as f32,
                            (stem[i * 2 + 1] as f64 / weights[i] as f64 * scale) as f32,
                        ]
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        if block.iter().flatten().any(|v| !v.is_finite()) {
            return Err(failure("non-finite HTDemucs audio"));
        }
        emit(&block)?;
        progress((chunk + 1) as f64 / chunks as f64);
        for stem in &mut accumulated {
            stem.rotate_left(STRIDE * 2);
            stem[(SEGMENT - STRIDE) * 2..].fill(0.0);
        }
        weights.rotate_left(STRIDE);
        weights[SEGMENT - STRIDE..].fill(0.0);
    }
    Ok(())
}

pub struct Separator {
    session: Session,
}
impl Separator {
    pub fn load(dir: &Path) -> Result<Self> {
        let threads = std::thread::available_parallelism().map_or(2, |n| n.get().min(4));
        let session = Session::builder()
            .map_err(failure)?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(failure)?
            .with_intra_threads(threads)
            .map_err(failure)?
            .with_intra_op_spinning(false)
            .map_err(failure)?
            .with_execution_providers([ort::ep::CPU::default().with_arena_allocator(false).build()])
            .map_err(failure)?
            .commit_from_file(dir.join("htdemucs.onnx"))
            .map_err(failure)?;
        Ok(Self { session })
    }
    pub fn separate(
        &mut self,
        pcm: &Pcm,
        cancel: &AtomicBool,
        progress: impl Fn(f64),
        emit: impl FnMut(&[Vec<f32>]) -> Result<()>,
    ) -> Result<()> {
        demix(
            pcm,
            cancel,
            progress,
            |data| {
                let tensor = Tensor::from_array(([1usize, 2, SEGMENT], data)).map_err(failure)?;
                let outputs = self
                    .session
                    .run(ort::inputs!["mix" => tensor])
                    .map_err(failure)?;
                let (shape, values) = outputs["stems"]
                    .try_extract_tensor::<f32>()
                    .map_err(failure)?;
                if shape.as_ref() != [1, 4, 2, SEGMENT as i64] {
                    return Err(failure("invalid HTDemucs tensor shape"));
                }
                Ok(values.to_vec())
            },
            emit,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn overlap_preserves_boundaries_gain_channels_and_source_order() {
        for (n, channels) in [
            (1, 1),
            (333, 2),
            (SEGMENT - 1, 1),
            (SEGMENT + 53, 2),
            (SEGMENT * 2 + 127, 2),
        ] {
            let pcm = Pcm {
                samples: (0..n * channels)
                    .map(|i| 0.1 + (i as f32 * 0.037).sin() * 0.4)
                    .collect(),
                channels,
                sample_rate: RATE,
            };
            let (mean, _) = normalization(&pcm).unwrap();
            let mut out = vec![Vec::new(); 4];
            let mut progress = Vec::new();
            demix(
                &pcm,
                &AtomicBool::new(false),
                |_| {},
                |data| {
                    Ok((1..=4)
                        .flat_map(|scale| data.iter().map(move |v| v * scale as f32))
                        .collect())
                },
                |block| {
                    for i in 0..4 {
                        out[i].extend_from_slice(&block[i]);
                    }
                    progress.push(block[0].len());
                    Ok(())
                },
            )
            .unwrap();
            assert!(!progress.is_empty());
            for (stem, scale) in out.iter().zip([4.0, 1.0, 2.0, 3.0]) {
                assert_eq!(stem.len(), n * 2);
                for i in 0..n {
                    for ch in 0..2 {
                        let expected = (pcm.samples[i * channels + ch.min(channels - 1)] as f64
                            - mean)
                            * scale;
                        assert!((stem[i * 2 + ch] as f64 - expected).abs() < 2e-6);
                    }
                }
            }
        }
    }
    #[test]
    fn cancellation_and_invalid_predictions_do_not_emit_audio() {
        let pcm = Pcm {
            samples: vec![0.2; 100],
            channels: 1,
            sample_rate: RATE,
        };
        assert!(
            demix(
                &pcm,
                &AtomicBool::new(true),
                |_| {},
                |_| panic!("inference after cancel"),
                |_| panic!("write after cancel")
            )
            .is_err()
        );
        let cancel = AtomicBool::new(false);
        assert!(
            demix(
                &pcm,
                &cancel,
                |_| {},
                |data| {
                    cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                    Ok(data.repeat(4))
                },
                |_| panic!("write after cancel")
            )
            .is_err()
        );
        for prediction in [vec![], vec![f32::NAN; 8 * SEGMENT]] {
            assert!(
                demix(
                    &pcm,
                    &AtomicBool::new(false),
                    |_| {},
                    |_| Ok(prediction.clone()),
                    |_| panic!("invalid audio written")
                )
                .is_err()
            );
        }
        let invalid = Pcm {
            samples: vec![],
            channels: 1,
            sample_rate: RATE,
        };
        assert!(normalization(&invalid).is_err());
    }
    #[test]
    #[ignore = "requires DEMUCS_MODELS and STEM_AUDIO, optionally DEMUCS_REFERENCE and STEM_OUTPUT"]
    fn real_model_inference() {
        let dir = std::env::var("DEMUCS_MODELS").unwrap();
        let audio = std::env::var("STEM_AUDIO").unwrap();
        let mut pcm = crate::audio::decode(Path::new(&audio), 10.0).unwrap();
        crate::batch::resample(&mut pcm, RATE).unwrap();
        pcm.samples.truncate(8 * RATE as usize * pcm.channels);
        let mut model = Separator::load(Path::new(&dir)).unwrap();
        let mut stems = vec![Vec::new(); 4];
        let start = std::time::Instant::now();
        model
            .separate(
                &pcm,
                &AtomicBool::new(false),
                |p| eprintln!("progress={p:.3}"),
                |block| {
                    for i in 0..4 {
                        stems[i].extend_from_slice(&block[i]);
                    }
                    Ok(())
                },
            )
            .unwrap();
        eprintln!(
            "HTDemucs total={:?}, audio={}s",
            start.elapsed(),
            pcm.frames() as f64 / RATE as f64
        );
        for (i, name) in super::super::dsp::NAMES.iter().enumerate() {
            assert_eq!(stems[i].len(), pcm.frames() * 2);
            assert!(stems[i].iter().all(|v| v.is_finite()));
            if let Ok(reference) = std::env::var("DEMUCS_REFERENCE") {
                let expected =
                    crate::audio::decode(&Path::new(&reference).join(format!("{name}.wav")), 10.0)
                        .unwrap();
                assert_eq!(expected.samples.len(), stems[i].len());
                let error = stems[i]
                    .iter()
                    .zip(&expected.samples)
                    .map(|(a, b)| (*a as f64 - *b as f64).powi(2))
                    .sum::<f64>();
                let power = expected
                    .samples
                    .iter()
                    .map(|v| (*v as f64).powi(2))
                    .sum::<f64>();
                eprintln!(
                    "{name} native/reference SNR={} dB",
                    10.0 * (power / error.max(1e-20)).log10()
                );
                assert!(error / power.max(1e-20) < 1e-6);
            }
            if let Ok(output) = std::env::var("STEM_OUTPUT") {
                std::fs::create_dir_all(&output).unwrap();
                crate::editor::write_wav(
                    &Path::new(&output).join(format!("{name}.wav")),
                    &Pcm {
                        samples: stems[i].clone(),
                        channels: 2,
                        sample_rate: RATE,
                    },
                    Some(32),
                    None,
                )
                .unwrap();
            }
        }
    }
}
