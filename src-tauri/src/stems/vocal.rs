//! Native vocal-target inference. External Hann STFT/iSTFT and bounded overlap-add.
use super::{Model, check, dsp::RATE, failure};
use crate::{audio::Pcm, error::Result};
use ort::{
    session::{Session, builder::GraphOptimizationLevel},
    value::Tensor,
};
use realfft::{RealFftPlanner, num_complex::Complex32};
use std::{path::Path, sync::atomic::AtomicBool};

#[derive(Clone, Copy)]
struct Contract {
    fft: usize,
    hop: usize,
    bins: usize,
    frames: usize,
    segment: usize,
    overlap: usize,
}
impl Contract {
    fn for_model(model: Model) -> Result<Self> {
        match model {
            Model::MelRoformer | Model::BsRoformer => Ok(Self {
                fft: 2048,
                hop: 441,
                bins: 1025,
                frames: 801,
                segment: 352800,
                overlap: 2,
            }),
            Model::Mdx23c => Ok(Self {
                fft: 8192,
                hop: 1024,
                bins: 4096,
                frames: 256,
                segment: 261120,
                overlap: 4,
            }),
            _ => Err(failure("unsupported vocal model")),
        }
    }
    fn window(self) -> Vec<f32> {
        (0..self.fft)
            .map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / self.fft as f32).cos())
            .collect()
    }
}
fn reflect(i: isize, n: usize) -> usize {
    if n <= 1 {
        return 0;
    }
    let period = 2 * (n - 1);
    let i = i.rem_euclid(period as isize) as usize;
    if i < n { i } else { period - i }
}
// [L real, L imag, R real, R imag], frequency-major.
fn analyse(pcm: &Pcm, c: Contract) -> Result<Vec<f32>> {
    let fft = RealFftPlanner::<f32>::new().plan_fft_forward(c.fft);
    let window = c.window();
    let mut input = fft.make_input_vec();
    let mut output = fft.make_output_vec();
    let mut scratch = fft.make_scratch_vec();
    let plane = c.bins * c.frames;
    let mut spec = vec![0.0; 4 * plane];
    for ch in 0..2 {
        for t in 0..c.frames {
            for (i, v) in input.iter_mut().enumerate() {
                let at = reflect(
                    (t * c.hop + i) as isize - (c.fft / 2) as isize,
                    pcm.frames(),
                );
                *v = pcm.samples[at * pcm.channels + ch.min(pcm.channels - 1)] * window[i];
            }
            fft.process_with_scratch(&mut input, &mut output, &mut scratch)
                .map_err(failure)?;
            for (f, v) in output.iter().take(c.bins).enumerate() {
                spec[2 * ch * plane + f * c.frames + t] = v.re;
                spec[(2 * ch + 1) * plane + f * c.frames + t] = v.im;
            }
        }
    }
    Ok(spec)
}
fn reconstruct(spec: &[f32], c: Contract) -> Result<Vec<f32>> {
    let plane = c.bins * c.frames;
    if spec.len() != 4 * plane || spec.iter().any(|v| !v.is_finite()) {
        return Err(failure("invalid vocal spectrum"));
    }
    let fft = RealFftPlanner::<f32>::new().plan_fft_inverse(c.fft);
    let window = c.window();
    let mut input = fft.make_input_vec();
    let mut output = fft.make_output_vec();
    let mut scratch = fft.make_scratch_vec();
    let total = (c.frames - 1) * c.hop + c.fft;
    let mut weights = vec![0.0; total];
    for t in 0..c.frames {
        for (i, w) in window.iter().enumerate() {
            weights[t * c.hop + i] += w * w;
        }
    }
    let mut result = vec![0.0; c.segment * 2];
    for ch in 0..2 {
        let mut wave = vec![0.0; total];
        for t in 0..c.frames {
            input.fill(Complex32::default()); // MDX omits Nyquist.
            for (f, v) in input.iter_mut().take(c.bins).enumerate() {
                *v = Complex32::new(
                    spec[2 * ch * plane + f * c.frames + t],
                    spec[(2 * ch + 1) * plane + f * c.frames + t],
                );
            }
            input[0].im = 0.0;
            input[c.fft / 2].im = 0.0;
            fft.process_with_scratch(&mut input, &mut output, &mut scratch)
                .map_err(failure)?;
            for (i, v) in output.iter().enumerate() {
                wave[t * c.hop + i] += v * window[i] / c.fft as f32;
            }
        }
        for i in 0..c.segment {
            let at = c.fft / 2 + i;
            result[2 * i + ch] = wave[at] / weights[at].max(1e-10);
        }
    }
    Ok(result)
}
pub struct Separator {
    session: Session,
    model: Model,
    contract: Contract,
}
impl Separator {
    pub fn load(dir: &Path, model: Model) -> Result<Self> {
        let contract = Contract::for_model(model)?;
        let file = super::models::model_file(model, 0);
        let session = Session::builder()
            .map_err(failure)?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(failure)?
            .with_intra_threads(std::thread::available_parallelism().map_or(2, |n| n.get().min(4)))
            .map_err(failure)?
            .with_intra_op_spinning(false)
            .map_err(failure)?
            .with_execution_providers([ort::ep::CPU::default().with_arena_allocator(false).build()])
            .map_err(failure)?
            .commit_from_file(dir.join(file))
            .map_err(failure)?;
        Ok(Self {
            session,
            model,
            contract,
        })
    }
    fn segment(&mut self, pcm: &Pcm, cancel: &AtomicBool) -> Result<Vec<f32>> {
        check(cancel)?;
        let c = self.contract;
        let mut spec = analyse(pcm, c)?;
        let plane = c.bins * c.frames;
        if self.model == Model::BsRoformer {
            let mut data = Vec::with_capacity(spec.len());
            for t in 0..c.frames {
                for f in 0..c.bins {
                    for ch in 0..2 {
                        data.push(spec[2 * ch * plane + f * c.frames + t]);
                        data.push(spec[(2 * ch + 1) * plane + f * c.frames + t]);
                    }
                }
            }
            let input = Tensor::from_array(([1, c.frames, 4 * c.bins], data)).map_err(failure)?;
            let outputs = self.session.run(ort::inputs![input]).map_err(failure)?;
            check(cancel)?;
            let (shape, mask) = outputs[0].try_extract_tensor::<f32>().map_err(failure)?;
            if shape.as_ref() != [1, 1, (2 * c.bins) as i64, c.frames as i64, 2]
                || mask.iter().any(|v| !v.is_finite())
            {
                return Err(failure("invalid BS-RoFormer mask"));
            }
            for f in 0..c.bins {
                for ch in 0..2 {
                    for t in 0..c.frames {
                        let at = f * c.frames + t;
                        let m = ((f * 2 + ch) * c.frames + t) * 2;
                        let r = spec[2 * ch * plane + at];
                        let i = spec[(2 * ch + 1) * plane + at];
                        spec[2 * ch * plane + at] = r * mask[m] - i * mask[m + 1];
                        spec[(2 * ch + 1) * plane + at] = r * mask[m + 1] + i * mask[m];
                    }
                }
            }
        } else {
            let input = Tensor::from_array(([1, 4, c.bins, c.frames], spec)).map_err(failure)?;
            let outputs = self.session.run(ort::inputs![input]).map_err(failure)?;
            check(cancel)?;
            let (shape, values) = outputs[0].try_extract_tensor::<f32>().map_err(failure)?;
            let expected = if self.model == Model::Mdx23c {
                vec![1, 2, 4, c.bins as i64, c.frames as i64]
            } else {
                vec![1, 4, c.bins as i64, c.frames as i64]
            };
            if shape.as_ref() != expected.as_slice() || values.iter().any(|v| !v.is_finite()) {
                return Err(failure("invalid vocal model output"));
            }
            spec = values[..4 * plane].to_vec(); // MDX trained order: vocals, other.
        }
        reconstruct(&spec, c)
    }
    pub fn separate(
        &mut self,
        pcm: &Pcm,
        cancel: &AtomicBool,
        progress: impl Fn(f64),
        emit: impl FnMut(&[Vec<f32>]) -> Result<()>,
    ) -> Result<()> {
        demix(pcm, self.contract, cancel, progress, emit, |input| {
            self.segment(input, cancel)
        })
    }
}
fn demix(
    pcm: &Pcm,
    c: Contract,
    cancel: &AtomicBool,
    progress: impl Fn(f64),
    mut emit: impl FnMut(&[Vec<f32>]) -> Result<()>,
    mut infer: impl FnMut(&Pcm) -> Result<Vec<f32>>,
) -> Result<()> {
    let n = pcm.frames();
    if n == 0 || !(1..=2).contains(&pcm.channels) || pcm.sample_rate != RATE {
        return Err(failure("expected mono or stereo 44.1 kHz audio"));
    }
    let step = c.segment / c.overlap;
    let border = if n > 2 * (c.segment - step) {
        c.segment - step
    } else {
        0
    };
    let extended = n + 2 * border;
    let fade = c.segment / 10;
    let mut sum = vec![0.0; c.segment * 2];
    let mut weights = vec![0.0; c.segment];
    // Consume every overlapping window, including the final short window.
    for start in (0..extended).step_by(step) {
        check(cancel)?;
        let length = c.segment.min(extended - start);
        let last = start + c.segment >= extended;
        let mut samples = vec![0.0; c.segment * 2];
        for i in 0..c.segment {
            let at = if i < length {
                i
            } else if length > c.segment / 2 {
                reflect(i as isize, length)
            } else {
                continue;
            };
            let source = reflect((start + at) as isize - border as isize, n);
            for ch in 0..2 {
                samples[i * 2 + ch] = pcm.samples[source * pcm.channels + ch.min(pcm.channels - 1)];
            }
        }
        let vocals = infer(&Pcm {
            samples,
            channels: 2,
            sample_rate: RATE,
        })?;
        if vocals.len() != c.segment * 2 || vocals.iter().any(|v| !v.is_finite()) {
            return Err(failure("invalid vocal segment"));
        }
        for i in 0..length {
            let mut w = 1.0f32;
            if start > 0 && i < fade {
                w = i as f32 / (fade - 1) as f32;
            }
            if !last && i >= c.segment - fade {
                w = w.min((c.segment - 1 - i) as f32 / (fade - 1) as f32);
            }
            weights[i] += w;
            for ch in 0..2 {
                sum[2 * i + ch] += vocals[2 * i + ch] * w;
            }
        }
        check(cancel)?;
        let flush = step.min(extended - start);
        let begin = border.saturating_sub(start).min(flush);
        let end = (border + n).saturating_sub(start).min(flush);
        if begin < end {
            let out = (begin..end)
                .flat_map(|i| {
                    [
                        sum[2 * i] / weights[i].max(1e-10),
                        sum[2 * i + 1] / weights[i].max(1e-10),
                    ]
                })
                .collect();
            emit(&[out])?;
        }
        sum.copy_within(flush * 2.., 0);
        sum[(c.segment - flush) * 2..].fill(0.0);
        weights.copy_within(flush.., 0);
        weights[c.segment - flush..].fill(0.0);
        progress((start + flush) as f64 / extended as f64);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hann_stft_round_trip_keeps_stereo_and_edges() {
        let c = Contract {
            fft: 64,
            hop: 16,
            bins: 33,
            frames: 17,
            segment: 256,
            overlap: 2,
        };
        let pcm = Pcm {
            samples: (0..c.segment)
                .flat_map(|i| [(i as f32 * 0.17).sin() * 0.7, (i as f32 * 0.31).cos() * 0.2])
                .collect(),
            channels: 2,
            sample_rate: RATE,
        };
        let output = reconstruct(&analyse(&pcm, c).unwrap(), c).unwrap();
        assert!(
            pcm.samples
                .iter()
                .zip(output)
                .all(|(a, b)| (a - b).abs() < 2e-6)
        );
    }
    #[test]
    fn overlap_is_bounded_and_preserves_short_long_mono_and_stereo_audio() {
        let c = Contract {
            fft: 64,
            hop: 16,
            bins: 33,
            frames: 17,
            segment: 256,
            overlap: 4,
        };
        for n in [1, 63, 64, 128, 255, 256, 257, 512, 769] {
            for channels in [1, 2] {
                let pcm = Pcm {
                    samples: (0..n * channels)
                        .map(|i| (i as f32 * 0.13).sin() * 0.5)
                        .collect(),
                    channels,
                    sample_rate: RATE,
                };
                let mut result = Vec::new();
                let mut progress = Vec::new();
                let cancel = AtomicBool::new(false);
                let p = std::cell::RefCell::new(&mut progress);
                demix(
                    &pcm,
                    c,
                    &cancel,
                    |v| p.borrow_mut().push(v),
                    |stems| {
                        result.extend_from_slice(&stems[0]);
                        Ok(())
                    },
                    |input| Ok(input.samples.clone()),
                )
                .unwrap();
                assert_eq!(result.len(), 2 * n);
                for i in 0..n {
                    for ch in 0..2 {
                        assert!(
                            (result[2 * i + ch] - pcm.samples[i * channels + ch.min(channels - 1)])
                                .abs()
                                < 2e-6
                        );
                    }
                }
                assert_eq!(progress.last(), Some(&1.0));
                assert!(progress.windows(2).all(|w| w[0] <= w[1]));
            }
        }
    }
    #[test]
    fn cancelled_vocal_inference_never_emits() {
        let c = Contract::for_model(Model::MelRoformer).unwrap();
        let pcm = Pcm {
            samples: vec![0.0; 8],
            channels: 1,
            sample_rate: RATE,
        };
        let cancel = AtomicBool::new(true);
        assert!(
            demix(
                &pcm,
                c,
                &cancel,
                |_| {},
                |_| panic!("must not emit"),
                |_| panic!("must not infer")
            )
            .is_err()
        );
        cancel.store(false, std::sync::atomic::Ordering::Relaxed);
        assert!(
            demix(
                &pcm,
                c,
                &cancel,
                |_| {},
                |_| panic!("must not emit after cancelled inference"),
                |input| {
                    cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                    Ok(input.samples.clone())
                }
            )
            .is_err()
        );
    }
    #[test]
    #[ignore = "requires VOCAL_MODELS and VOCAL_REFERENCE; real model and independent torch STFT parity"]
    fn real_vocal_models_match_reference() {
        let dir = std::path::PathBuf::from(std::env::var("VOCAL_MODELS").unwrap());
        let refs = std::path::PathBuf::from(std::env::var("VOCAL_REFERENCE").unwrap());
        let read = |name: &str| -> Vec<f32> {
            std::fs::read(refs.join(name))
                .unwrap()
                .chunks_exact(4)
                .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
                .collect()
        };
        for model in [Model::MelRoformer, Model::BsRoformer, Model::Mdx23c] {
            if std::env::var("VOCAL_MODEL").is_ok_and(|id| id != model.id()) {
                continue;
            }
            let c = Contract::for_model(model).unwrap();
            let pcm = Pcm {
                samples: read(&format!("{}-input.f32", model.id())),
                channels: 2,
                sample_rate: RATE,
            };
            assert_eq!(pcm.frames(), c.segment);
            let spec = analyse(&pcm, c).unwrap();
            std::fs::write(
                refs.join(format!("{}-native-spec.f32", model.id())),
                spec.iter()
                    .flat_map(|v| v.to_le_bytes())
                    .collect::<Vec<_>>(),
            )
            .unwrap();
            let now = std::time::Instant::now();
            let output = Separator::load(&dir, model)
                .unwrap()
                .segment(&pcm, &AtomicBool::new(false))
                .unwrap();
            std::fs::write(
                refs.join(format!("{}-native-output.f32", model.id())),
                output
                    .iter()
                    .flat_map(|v| v.to_le_bytes())
                    .collect::<Vec<_>>(),
            )
            .unwrap();
            let expected = read(&format!("{}-output.f32", model.id()));
            assert_eq!(output.len(), expected.len());
            let signal = expected.iter().map(|v| (*v as f64).powi(2)).sum::<f64>();
            let noise = expected
                .iter()
                .zip(&output)
                .map(|(a, b)| (*a as f64 - *b as f64).powi(2))
                .sum::<f64>();
            let snr = 10.0 * (signal / noise.max(1e-30)).log10();
            eprintln!(
                "{model:?} native/reference {snr:.2} dB, {:?}",
                now.elapsed()
            );
            assert!(snr > 55.0, "{model:?}: {snr}");
        }
    }
}
