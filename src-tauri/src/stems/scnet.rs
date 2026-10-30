//! SCNet-large and XL IHF STFT contracts and bounded-memory overlapping segments.
//! See docs/research/stem-separation.md for the pinned export and attribution.
use super::{Model, check, dsp::RATE, failure};
use crate::{audio::Pcm, error::Result};
use ort::{
    session::{Session, builder::GraphOptimizationLevel},
    value::Tensor,
};
use rayon::prelude::*;
use realfft::{RealFftPlanner, num_complex::Complex32};
use std::{path::Path, sync::atomic::AtomicBool};
const FFT: usize = 4096;
const HOP: usize = 1024;
const BINS: usize = FFT / 2 + 1;
const FRAMES: usize = 476;
const PADDED: usize = (FRAMES - 1) * HOP;
const SEGMENT: usize = 485100;
const STEP: usize = SEGMENT / 4;
const FADE: usize = SEGMENT / 10;
const PLANE: usize = BINS * FRAMES;

fn reflect(index: isize, length: usize) -> usize {
    if length <= 1 {
        return 0;
    }
    let period = 2 * (length - 1) as isize;
    let i = index.rem_euclid(period) as usize;
    if i < length { i } else { period as usize - i }
}

// Rectangular window, centred reflection, orthonormal FFT. The model sees
// L-real, L-imaginary, R-real, R-imaginary, each frequency-major [F, T].
fn analyse(pcm: &Pcm) -> Result<Vec<f32>> {
    if pcm.frames() > SEGMENT || pcm.channels == 0 {
        return Err(failure("invalid SCNet segment"));
    }
    let fft = RealFftPlanner::<f32>::new().plan_fft_forward(FFT);
    let mut input = fft.make_input_vec();
    let mut output = fft.make_output_vec();
    let mut scratch = fft.make_scratch_vec();
    let mut spec = vec![0.0; 4 * PLANE];
    for channel in 0..2 {
        for frame in 0..FRAMES {
            for (i, v) in input.iter_mut().enumerate() {
                let pos = reflect((frame * HOP + i) as isize - (FFT / 2) as isize, PADDED);
                *v = if pos < pcm.frames() {
                    pcm.samples[pos * pcm.channels + channel.min(pcm.channels - 1)]
                } else {
                    0.0
                };
            }
            fft.process_with_scratch(&mut input, &mut output, &mut scratch)
                .map_err(failure)?;
            for (bin, value) in output.iter().enumerate() {
                spec[2 * channel * PLANE + bin * FRAMES + frame] = value.re / 64.0;
                spec[(2 * channel + 1) * PLANE + bin * FRAMES + frame] = value.im / 64.0;
            }
        }
    }
    Ok(spec)
}
fn reconstruct(spec: &[f32], length: usize) -> Result<Vec<f32>> {
    if spec.len() != 4 * PLANE || length > SEGMENT {
        return Err(failure("invalid SCNet spectrum"));
    }
    let fft = RealFftPlanner::<f32>::new().plan_fft_inverse(FFT);
    let mut input = fft.make_input_vec();
    let mut output = fft.make_output_vec();
    let mut scratch = fft.make_scratch_vec();
    let total = PADDED + FFT;
    let mut weights = vec![0.0f32; total];
    for frame in 0..FRAMES {
        for v in &mut weights[frame * HOP..frame * HOP + FFT] {
            *v += 1.0;
        }
    }
    let mut samples = vec![0.0; length * 2];
    for channel in 0..2 {
        let mut wave = vec![0.0f32; total];
        for frame in 0..FRAMES {
            for (bin, v) in input.iter_mut().enumerate() {
                *v = Complex32::new(
                    spec[2 * channel * PLANE + bin * FRAMES + frame],
                    spec[(2 * channel + 1) * PLANE + bin * FRAMES + frame],
                );
            }
            // A real inverse requires DC and Nyquist to be real. Estimated
            // imaginary endpoints are ignored by torch.istft as well.
            input[0].im = 0.0;
            input[BINS - 1].im = 0.0;
            fft.process_with_scratch(&mut input, &mut output, &mut scratch)
                .map_err(failure)?;
            for (i, value) in output.iter().enumerate() {
                wave[frame * HOP + i] += value / 64.0;
            }
        }
        for i in 0..length {
            samples[i * 2 + channel] = wave[FFT / 2 + i] / weights[FFT / 2 + i];
        }
    }
    Ok(samples)
}

pub struct Separator {
    session: Session,
    model: Model,
}
impl Separator {
    pub fn load_for(dir: &Path, model: Model) -> Result<Self> {
        let file = match model {
            Model::ScnetLarge | Model::ScnetXlIhf => super::models::model_file(model, 0),
            _ => return Err(failure("unsupported SCNet variant")),
        };
        // XL benefits from up to six performance cores on Apple silicon.
        // Avoid scheduling extra workers on efficiency cores on smaller Macs.
        let max_threads = if model == Model::ScnetXlIhf
            && cfg!(all(target_os = "macos", target_arch = "aarch64"))
        {
            std::process::Command::new("/usr/sbin/sysctl")
                .args(["-n", "hw.perflevel0.physicalcpu"])
                .output()
                .ok()
                .filter(|output| output.status.success())
                .and_then(|output| String::from_utf8(output.stdout).ok())
                .and_then(|cores| cores.trim().parse::<usize>().ok())
                .unwrap_or(4)
                .clamp(1, 6)
        } else {
            4
        };
        let threads = std::thread::available_parallelism().map_or(2, |n| n.get().min(max_threads));
        let mut builder = Session::builder()
            .map_err(failure)?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(failure)?
            .with_intra_threads(threads)
            .map_err(failure)?;
        if model == Model::ScnetXlIhf {
            builder = builder
                .with_intra_op_spinning(false)
                .map_err(failure)?
                .with_execution_providers([ort::ep::CPU::default()
                    .with_arena_allocator(false)
                    .build()])
                .map_err(failure)?;
        }
        let session = builder.commit_from_file(dir.join(file)).map_err(failure)?;
        Ok(Self { session, model })
    }
    fn segment(&mut self, pcm: &Pcm, cancel: &AtomicBool) -> Result<Vec<Vec<f32>>> {
        check(cancel)?;
        if self.model == Model::ScnetXlIhf {
            let spec = analyse(pcm)?;
            let mut real = Vec::with_capacity(2 * PLANE);
            let mut imag = Vec::with_capacity(2 * PLANE);
            for ch in 0..2 {
                real.extend_from_slice(&spec[2 * ch * PLANE..(2 * ch + 1) * PLANE]);
                imag.extend_from_slice(&spec[(2 * ch + 1) * PLANE..(2 * ch + 2) * PLANE]);
            }
            let real = Tensor::from_array(([1usize, 2, BINS, FRAMES], real)).map_err(failure)?;
            let imag = Tensor::from_array(([1usize, 2, BINS, FRAMES], imag)).map_err(failure)?;
            let outputs = self
                .session
                .run(ort::inputs!["spec_real"=>real, "spec_imag"=>imag])
                .map_err(failure)?;
            check(cancel)?;
            let (real_shape, real) = outputs["out_spec_real"]
                .try_extract_tensor::<f32>()
                .map_err(failure)?;
            let (imag_shape, imag) = outputs["out_spec_imag"]
                .try_extract_tensor::<f32>()
                .map_err(failure)?;
            let expected = [1, 4, 2, BINS as i64, FRAMES as i64];
            if real_shape.as_ref() != expected
                || imag_shape.as_ref() != expected
                || real.iter().chain(imag).any(|v| !v.is_finite())
            {
                return Err(failure("invalid SCNet XL IHF model output"));
            }
            return [3usize, 0, 1, 2]
                // Each stem has independent inverse FFTs; finish all four
                // before advancing to the next sequential model segment.
                .into_par_iter()
                .map(|source| {
                    check(cancel)?;
                    let mut packed = Vec::with_capacity(4 * PLANE);
                    for ch in 0..2 {
                        let offset = (source * 2 + ch) * PLANE;
                        packed.extend_from_slice(&real[offset..offset + PLANE]);
                        packed.extend_from_slice(&imag[offset..offset + PLANE]);
                    }
                    reconstruct(&packed, SEGMENT)
                })
                .collect();
        }
        let input =
            Tensor::from_array(([1usize, 4, BINS, FRAMES], analyse(pcm)?)).map_err(failure)?;
        let outputs = self
            .session
            .run(ort::inputs!["mix_spec" => input])
            .map_err(failure)?;
        check(cancel)?;
        let (shape, values) = outputs["stems_spec"]
            .try_extract_tensor::<f32>()
            .map_err(failure)?;
        if shape.as_ref() != [1, 16, BINS as i64, FRAMES as i64]
            || values.iter().any(|v| !v.is_finite())
        {
            return Err(failure("invalid SCNet model output"));
        }
        // Graph: drums, bass, other, vocals. App: vocals, drums, bass, other.
        [3, 0, 1, 2]
            .into_par_iter()
            .map(|source| {
                check(cancel)?;
                reconstruct(
                    &values[source * 4 * PLANE..(source + 1) * 4 * PLANE],
                    SEGMENT,
                )
            })
            .collect()
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
            self.model == Model::ScnetLarge,
            cancel,
            progress,
            emit,
            |segment| self.segment(segment, cancel),
        )
    }
}

// Same framing/normalization as the export's reference demix, with only one
// 11-second overlap buffer. Samples that no later segment can change are flushed
// immediately, so a ten-minute track doesn't allocate four whole-track outputs.
fn demix(
    pcm: &Pcm,
    normalize: bool,
    cancel: &AtomicBool,
    progress: impl Fn(f64),
    mut emit: impl FnMut(&[Vec<f32>]) -> Result<()>,
    mut infer: impl FnMut(&Pcm) -> Result<Vec<Vec<f32>>>,
) -> Result<()> {
    let n = pcm.frames();
    if n == 0 || !(1..=2).contains(&pcm.channels) || pcm.sample_rate != RATE {
        return Err(failure(
            "SCNet expects non-empty mono or stereo 44.1 kHz audio",
        ));
    }
    // XL IHF was trained without external input normalization. Preserve its
    // approved reference behavior rather than inheriting Large's normalization.
    let (mean, std) = if normalize {
        let mono = |i: usize| {
            pcm.samples[i * pcm.channels..(i + 1) * pcm.channels]
                .iter()
                .map(|v| *v as f64)
                .sum::<f64>()
                / pcm.channels as f64
        };
        let mean = (0..n).map(mono).sum::<f64>() / n as f64;
        let std = ((0..n).map(|i| (mono(i) - mean).powi(2)).sum::<f64>()
            / n.saturating_sub(1).max(1) as f64)
            .sqrt()
            + 1e-8;
        (mean, std)
    } else {
        (0.0, 1.0)
    };
    let border = if n > 2 * (SEGMENT - STEP) {
        SEGMENT - STEP
    } else {
        0
    };
    let extended = n + 2 * border;
    let segments = extended.saturating_sub(SEGMENT).div_ceil(STEP) + 1;
    let mut accumulated = vec![vec![0.0f32; SEGMENT * 2]; 4];
    let mut weights = vec![0.0f32; SEGMENT];
    let mut start = 0;
    loop {
        check(cancel)?;
        let length = SEGMENT.min(extended - start);
        let last = start + SEGMENT >= extended;
        let mut samples = vec![0.0; SEGMENT * 2];
        for i in 0..SEGMENT {
            let at = if i < length {
                i
            } else if length > SEGMENT / 2 {
                reflect(i as isize, length)
            } else {
                continue;
            };
            let source = reflect((start + at) as isize - border as isize, n);
            for ch in 0..2 {
                samples[i * 2 + ch] =
                    ((pcm.samples[source * pcm.channels + ch.min(pcm.channels - 1)] as f64 - mean)
                        / std) as f32;
            }
        }
        let stems = infer(&Pcm {
            samples,
            channels: 2,
            sample_rate: RATE,
        })?;
        if stems.len() != 4
            || stems
                .iter()
                .any(|s| s.len() != SEGMENT * 2 || s.iter().any(|v| !v.is_finite()))
        {
            return Err(failure("invalid SCNet segment output"));
        }
        for i in 0..length {
            let mut weight = 1.0;
            if start > 0 && i < FADE {
                weight = i as f32 / (FADE - 1) as f32;
            }
            if !last && i >= SEGMENT - FADE {
                weight = weight.min((SEGMENT - 1 - i) as f32 / (FADE - 1) as f32);
            }
            weights[i] += weight;
            for s in 0..4 {
                for ch in 0..2 {
                    accumulated[s][i * 2 + ch] += stems[s][i * 2 + ch] * weight;
                }
            }
        }
        check(cancel)?;
        let flush = if last { length } else { STEP };
        let begin = border.saturating_sub(start).min(flush);
        let end = (border + n).saturating_sub(start).min(flush);
        if begin < end {
            let block = accumulated
                .iter()
                .map(|stem| {
                    (begin..end)
                        .flat_map(|i| {
                            [
                                (stem[i * 2] as f64 / weights[i] as f64 * std) as f32,
                                (stem[i * 2 + 1] as f64 / weights[i] as f64 * std) as f32,
                            ]
                        })
                        .collect()
                })
                .collect::<Vec<Vec<f32>>>();
            emit(&block)?;
        }
        progress((start / STEP + 1) as f64 / segments as f64);
        if last {
            break;
        }
        weights.rotate_left(STEP);
        weights[SEGMENT - STEP..].fill(0.0);
        for stem in &mut accumulated {
            stem.rotate_left(STEP * 2);
            stem[(SEGMENT - STEP) * 2..].fill(0.0);
        }
        start += STEP;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "measures sequential vs bounded parallel synthesis; run without concurrent inference/builds"]
    fn benchmark_parallel_synthesis_preserves_samples() {
        let pcm = Pcm {
            samples: (0..SEGMENT * 2)
                .map(|i| (i as f32 * 0.017).sin() * 0.2)
                .collect(),
            channels: 2,
            sample_rate: RATE,
        };
        let spec = analyse(&pcm).unwrap();
        let spectra: Vec<Vec<f32>> = (0..4)
            .map(|source| {
                spec.iter()
                    .map(|sample| *sample * (source + 1) as f32 / 4.0)
                    .collect()
            })
            .collect();
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(4)
            .build()
            .unwrap();
        for round in 0..3 {
            let start = std::time::Instant::now();
            let serial: Vec<_> = spectra
                .iter()
                .map(|s| reconstruct(s, SEGMENT).unwrap())
                .collect();
            let sequential = start.elapsed();
            let start = std::time::Instant::now();
            let parallel: Vec<_> = pool.install(|| {
                spectra
                    .par_iter()
                    .map(|s| reconstruct(s, SEGMENT).unwrap())
                    .collect()
            });
            let concurrent = start.elapsed();
            assert_eq!(serial, parallel);
            eprintln!(
                "synthesis round {round}: sequential={sequential:?}, parallel={concurrent:?}"
            );
        }
    }
    #[test]
    fn rectangular_stft_round_trips_short_mono_and_stereo_without_shift() {
        for (frames, channels) in [(1, 1), (1300, 2), (SEGMENT, 2)] {
            let pcm = Pcm {
                samples: (0..frames * channels)
                    .map(|i| (i as f32 * 0.17).sin() * 0.3)
                    .collect(),
                channels,
                sample_rate: RATE,
            };
            let result = reconstruct(&analyse(&pcm).unwrap(), frames).unwrap();
            for i in 0..frames {
                for ch in 0..2 {
                    assert!(
                        (result[i * 2 + ch] - pcm.samples[i * channels + ch.min(channels - 1)])
                            .abs()
                            < 2e-6
                    );
                }
            }
        }
    }
    #[test]
    fn overlap_flush_preserves_length_and_gain_across_reflected_boundaries() {
        for n in [1, SEGMENT - 1, SEGMENT + STEP, 2 * SEGMENT] {
            let pcm = Pcm {
                samples: (0..n).map(|i| (i as f32 * 0.07).sin() * 0.2).collect(),
                channels: 1,
                sample_rate: RATE,
            };
            let mut out = Vec::new();
            demix(
                &pcm,
                true,
                &AtomicBool::new(false),
                |_| {},
                |s| {
                    out.extend_from_slice(&s[0]);
                    Ok(())
                },
                |seg| Ok(vec![seg.samples.clone(); 4]),
            )
            .unwrap();
            assert_eq!(out.len(), n * 2);
            let mean = pcm.samples.iter().map(|v| *v as f64).sum::<f64>() / n as f64;
            for i in 0..n {
                assert!((out[i * 2] as f64 - (pcm.samples[i] as f64 - mean)).abs() < 2e-6);
            }
        }
    }
    #[test]
    fn xl_pipeline_preserves_raw_level_and_dc_without_standardization() {
        for n in [1, SEGMENT + STEP, 2 * SEGMENT] {
            let pcm = Pcm {
                samples: (0..n * 2)
                    .map(|i| 0.13 + (i as f32 * 0.07).sin() * 0.2)
                    .collect(),
                channels: 2,
                sample_rate: RATE,
            };
            let mut out = Vec::new();
            demix(
                &pcm,
                false,
                &AtomicBool::new(false),
                |_| {},
                |s| {
                    out.extend_from_slice(&s[0]);
                    Ok(())
                },
                |seg| Ok(vec![seg.samples.clone(); 4]),
            )
            .unwrap();
            assert_eq!(out.len(), pcm.samples.len());
            assert!(
                out.iter()
                    .zip(&pcm.samples)
                    .all(|(a, b)| (a - b).abs() < 2e-6)
            );
        }
    }
    #[test]
    fn cancellation_prevents_inference_and_output() {
        let pcm = Pcm {
            samples: vec![0.1],
            channels: 1,
            sample_rate: RATE,
        };
        assert!(
            demix(
                &pcm,
                true,
                &AtomicBool::new(true),
                |_| {},
                |_| panic!("output after cancel"),
                |_| panic!("inference after cancel")
            )
            .is_err()
        );
    }
    #[test]
    #[ignore = "requires SCNET_MODELS and STEM_AUDIO; compares with a Python ONNX reference if SCNET_REFERENCE is set"]
    fn real_model_inference() {
        let dir = std::env::var("SCNET_MODELS").unwrap();
        let audio = std::env::var("STEM_AUDIO").unwrap();
        let mut pcm = crate::audio::decode(Path::new(&audio), 10.0).unwrap();
        crate::batch::resample(&mut pcm, RATE).unwrap();
        pcm.samples.truncate(8 * RATE as usize * pcm.channels);
        let started = std::time::Instant::now();
        let variant = if std::env::var("SCNET_VARIANT").as_deref() == Ok("xlIhf") {
            Model::ScnetXlIhf
        } else {
            Model::ScnetLarge
        };
        let mut model = Separator::load_for(Path::new(&dir), variant).unwrap();
        let mut stems = vec![Vec::new(); 4];
        model
            .separate(
                &pcm,
                &AtomicBool::new(false),
                |_| {},
                |chunk| {
                    for i in 0..4 {
                        stems[i].extend_from_slice(&chunk[i]);
                    }
                    Ok(())
                },
            )
            .unwrap();
        eprintln!(
            "{variant:?} total={:?}; duration={}s",
            started.elapsed(),
            pcm.frames() as f64 / RATE as f64
        );
        for (i, name) in super::super::dsp::NAMES.iter().enumerate() {
            assert_eq!(stems[i].len(), pcm.frames() * 2);
            assert!(stems[i].iter().all(|v| v.is_finite()));
            if let Ok(reference) = std::env::var("SCNET_REFERENCE") {
                let expected =
                    crate::audio::decode(&Path::new(&reference).join(format!("{name}.wav")), 10.0)
                        .unwrap();
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
