//! Periodic-Hann STFT, soft ratio masks and weighted overlap-add. No audio is
//! normalised per stem: adding the four stems reconstructs the input mixture.
use crate::{
    audio::Pcm,
    error::{Error, Result},
};
use ort::{
    session::{Session, builder::GraphOptimizationLevel},
    value::Tensor,
};
use realfft::{RealFftPlanner, num_complex::Complex32};
use std::path::Path;
const FFT: usize = 4096;
const HOP: usize = 1024;
const PAD: usize = FFT - HOP;
const BINS: usize = FFT / 2 + 1;
const TIME: usize = 512;
const FREQ: usize = 1024;
pub const RATE: u32 = 44100;
pub const NAMES: [&str; 4] = ["vocals", "drums", "bass", "other"];
fn err(e: impl std::fmt::Display) -> Error {
    Error::Audio(format!("stem separation: {e}"))
}
fn window() -> Vec<f32> {
    (0..FFT)
        .map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / FFT as f32).cos())
        .collect()
}
struct Spectrum {
    data: Vec<Complex32>,
    frames: usize,
    length: usize,
}
fn analyse(pcm: &Pcm) -> Result<(Spectrum, Vec<f32>)> {
    let frames = (PAD + pcm.frames()).div_ceil(HOP);
    if frames > TIME {
        return Err(err("audio block exceeds model window"));
    }
    let fft = RealFftPlanner::<f32>::new().plan_fft_forward(FFT);
    let win = window();
    let mut input = fft.make_input_vec();
    let mut output = fft.make_output_vec();
    let mut scratch = fft.make_scratch_vec();
    let mut data = vec![Complex32::new(0.0, 0.0); 2 * frames * BINS];
    let mut magnitude = vec![0.0; 2 * TIME * FREQ];
    for ch in 0..2 {
        for frame in 0..frames {
            for i in 0..FFT {
                let pos = frame * HOP + i;
                input[i] = if pos >= PAD && pos - PAD < pcm.frames() {
                    pcm.samples[(pos - PAD) * pcm.channels + ch.min(pcm.channels - 1)] * win[i]
                } else {
                    0.0
                };
            }
            fft.process_with_scratch(&mut input, &mut output, &mut scratch)
                .map_err(err)?;
            data[(ch * frames + frame) * BINS..(ch * frames + frame + 1) * BINS]
                .copy_from_slice(&output);
            for b in 0..FREQ {
                magnitude[(ch * TIME + frame) * FREQ + b] = output[b].norm();
            }
        }
    }
    Ok((
        Spectrum {
            data,
            frames,
            length: pcm.frames(),
        },
        magnitude,
    ))
}
fn reconstruct(s: &Spectrum, masks: &[f32]) -> Result<Vec<f32>> {
    let fft = RealFftPlanner::<f32>::new().plan_fft_inverse(FFT);
    let win = window();
    let mut input = fft.make_input_vec();
    let mut output = fft.make_output_vec();
    let mut scratch = fft.make_scratch_vec();
    let total = (s.frames - 1) * HOP + FFT;
    let mut weights = vec![0.0f32; total];
    for frame in 0..s.frames {
        for i in 0..FFT {
            weights[frame * HOP + i] += win[i] * win[i];
        }
    }
    let mut result = vec![0.0; s.length * 2];
    for ch in 0..2 {
        let mut wave = vec![0.0f32; total];
        for frame in 0..s.frames {
            let low = &masks[(ch * TIME + frame) * FREQ..(ch * TIME + frame + 1) * FREQ];
            let high = low.iter().sum::<f32>() / FREQ as f32;
            for b in 0..BINS {
                input[b] = s.data[(ch * s.frames + frame) * BINS + b]
                    * if b < FREQ { low[b] } else { high };
            }
            fft.process_with_scratch(&mut input, &mut output, &mut scratch)
                .map_err(err)?;
            for i in 0..FFT {
                wave[frame * HOP + i] += output[i] * win[i] / FFT as f32;
            }
        }
        for i in 0..s.length {
            result[i * 2 + ch] = wave[PAD + i] / weights[PAD + i].max(1e-8);
        }
    }
    Ok(result)
}
fn ratio_masks(estimates: &mut [Vec<f32>]) -> Result<()> {
    if estimates.len() != 4
        || estimates
            .iter()
            .any(|x| x.len() != 2 * TIME * FREQ || x.iter().any(|v| !v.is_finite()))
    {
        return Err(err("invalid model output"));
    }
    for i in 0..2 * TIME * FREQ {
        let sum = estimates
            .iter()
            .map(|s| s[i] as f64 * s[i] as f64)
            .sum::<f64>()
            + 1e-10;
        for stem in estimates.iter_mut() {
            stem[i] = ((stem[i] as f64 * stem[i] as f64 + 2.5e-11) / sum) as f32;
        }
    }
    Ok(())
}
pub struct Separator {
    sessions: Vec<Session>,
}
impl Separator {
    pub fn load(dir: &Path) -> Result<Self> {
        let threads = std::thread::available_parallelism().map_or(2, |n| n.get().min(4));
        let mut sessions = Vec::new();
        for name in NAMES {
            let session = Session::builder()
                .map_err(err)?
                .with_optimization_level(GraphOptimizationLevel::Level3)
                .map_err(err)?
                .with_intra_threads(threads)
                .map_err(err)?
                .commit_from_file(dir.join(format!("{name}.fp16.onnx")))
                .map_err(err)?;
            sessions.push(session);
        }
        Ok(Self { sessions })
    }
    pub fn separate(&mut self, pcm: &Pcm, check: impl Fn() -> Result<()>) -> Result<Vec<Vec<f32>>> {
        let (spec, magnitudes) = analyse(pcm)?;
        let mut estimates = Vec::new();
        for session in &mut self.sessions {
            check()?;
            let input =
                Tensor::from_array(([2usize, 1, TIME, FREQ], magnitudes.clone())).map_err(err)?;
            let outputs = session.run(ort::inputs!["x"=>input]).map_err(err)?;
            let (_, values) = outputs["y"].try_extract_tensor::<f32>().map_err(err)?;
            estimates.push(values.to_vec());
        }
        check()?;
        ratio_masks(&mut estimates)?;
        estimates
            .iter()
            .map(|mask| reconstruct(&spec, mask))
            .collect()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stft_reconstructs_first_last_and_short_samples_without_shift() {
        for n in [1, 1024, 5000, 44100] {
            let pcm = Pcm {
                samples: (0..n * 2).map(|i| (i as f32 * 0.23).sin() * 0.4).collect(),
                channels: 2,
                sample_rate: RATE,
            };
            let (spec, _) = analyse(&pcm).unwrap();
            let out = reconstruct(&spec, &vec![1.0; 2 * TIME * FREQ]).unwrap();
            assert_eq!(out.len(), pcm.samples.len());
            assert!(
                out.iter()
                    .zip(&pcm.samples)
                    .all(|(a, b)| (a - b).abs() < 1e-5)
            );
        }
    }
    #[test]
    fn ratio_masks_preserve_silence_and_sum_to_the_original() {
        let mut estimates = (0..4)
            .map(|s| vec![s as f32; 2 * TIME * FREQ])
            .collect::<Vec<_>>();
        ratio_masks(&mut estimates).unwrap();
        assert!((estimates.iter().map(|s| s[0]).sum::<f32>() - 1.0).abs() < 1e-6);
        let mut silent = vec![vec![0.0; 2 * TIME * FREQ]; 4];
        ratio_masks(&mut silent).unwrap();
        assert_eq!(silent[0][0], 0.25);
    }
    #[test]
    #[ignore = "requires downloaded Spleeter weights; STEM_MODELS and STEM_AUDIO"]
    fn real_model_inference() {
        let dir = std::env::var("STEM_MODELS").unwrap();
        let path = std::env::var("STEM_AUDIO").unwrap();
        let mut pcm = crate::audio::decode(Path::new(&path), 10.0).unwrap();
        crate::batch::resample(&mut pcm, RATE).unwrap();
        pcm.samples.truncate(8 * RATE as usize * pcm.channels);
        let start = std::time::Instant::now();
        let mut model = Separator::load(Path::new(&dir)).unwrap();
        let load = start.elapsed();
        let stems = model.separate(&pcm, || Ok(())).unwrap();
        let n = pcm.frames();
        let mut max = 0.0f32;
        for i in 0..n * 2 {
            let sum = stems.iter().map(|s| s[i]).sum::<f32>();
            let original = pcm.samples[(i / 2) * pcm.channels + (i % 2).min(pcm.channels - 1)];
            max = max.max((sum - original).abs());
        }
        eprintln!(
            "load={load:?}; total={:?}; audio={:.2}s; reconstruction max={max}",
            start.elapsed(),
            n as f64 / RATE as f64
        );
        assert!(max < 1e-4);
        if let Ok(out) = std::env::var("STEM_OUTPUT") {
            std::fs::create_dir_all(&out).unwrap();
            for (name, samples) in NAMES.into_iter().zip(stems) {
                crate::editor::write_wav(
                    &Path::new(&out).join(format!("{name}.wav")),
                    &Pcm {
                        samples,
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
