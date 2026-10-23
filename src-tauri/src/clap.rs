//! CLAP audio encoder (laion/clap-htsat-unfused, int8 ONNX): turns up to 10 s of
//! audio into a 512-dim embedding used for categories and similarity search.
//!
//! Feature extraction mirrors transformers' `ClapFeatureExtractor` for the unfused
//! model: 48 kHz mono, first 10 s (repeat-padded if shorter), 1024-point STFT with
//! hop 480 and a periodic Hann window, 64 Slaney mel bands (50 Hz–14 kHz), power dB.

use std::path::Path;
use std::sync::{Arc, LazyLock, Mutex};

use ort::session::builder::GraphOptimizationLevel;
use ort::session::Session;
use ort::value::Tensor;
use realfft::RealFftPlanner;
use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Resampler};

use crate::audio::Pcm;
use crate::error::{Error, Result};

pub const SAMPLE_RATE: u32 = 48_000;
pub const DIM: usize = 512;
const N_FFT: usize = 1024;
const HOP: usize = 480;
const N_MELS: usize = 64;
const N_BINS: usize = N_FFT / 2 + 1;
const CLIP: usize = 10 * SAMPLE_RATE as usize;
const FRAMES: usize = CLIP / HOP + 1;

/// Bump when the model or preprocessing changes, so embeddings are recomputed.
pub const EMBEDDING_VERSION: i64 = 1;

/// Pinned download of the model, verified by size and SHA-256.
pub const MODEL_FILE: &str = "clap-htsat-unfused-audio-int8.onnx";
pub const MODEL_URL: &str = "https://huggingface.co/Xenova/clap-htsat-unfused/resolve/c28f2883575e590e04d3146ff0713c2448d691ba/onnx/audio_model_quantized.onnx";
pub const MODEL_SHA256: &str = "3fcff2c8824e7bcb83a983f2a49edab3b60cbcf4872ac70efee517355173bd1f";
pub const MODEL_SIZE: u64 = 34_301_667;

// --- Features ------------------------------------------------------------------

fn hz_to_mel(hz: f64) -> f64 {
    // Slaney scale: linear below 1 kHz, logarithmic above.
    let logstep = 27.0 / 6.4f64.ln();
    if hz >= 1000.0 { 15.0 + (hz / 1000.0).ln() * logstep } else { 3.0 * hz / 200.0 }
}

fn mel_to_hz(mel: f64) -> f64 {
    let logstep = 6.4f64.ln() / 27.0;
    if mel >= 15.0 { 1000.0 * (logstep * (mel - 15.0)).exp() } else { 200.0 * mel / 3.0 }
}

/// Slaney-normalized triangular filters, `[mel][bin]`.
static MEL_FILTERS: LazyLock<Vec<[f64; N_BINS]>> = LazyLock::new(|| {
    let (lo, hi) = (hz_to_mel(50.0), hz_to_mel(14_000.0));
    let edges: Vec<f64> = (0..N_MELS + 2).map(|i| mel_to_hz(lo + (hi - lo) * i as f64 / (N_MELS + 1) as f64)).collect();
    let bin_hz: Vec<f64> = (0..N_BINS).map(|b| b as f64 * (SAMPLE_RATE / 2) as f64 / (N_BINS - 1) as f64).collect();
    (0..N_MELS)
        .map(|m| {
            let (left, center, right) = (edges[m], edges[m + 1], edges[m + 2]);
            let norm = 2.0 / (right - left);
            let mut row = [0.0; N_BINS];
            for (b, &f) in bin_hz.iter().enumerate() {
                let down = (f - left) / (center - left);
                let up = (right - f) / (right - center);
                row[b] = down.min(up).max(0.0) * norm;
            }
            row
        })
        .collect()
});

/// Periodic Hann window.
static WINDOW: LazyLock<Vec<f64>> =
    LazyLock::new(|| (0..N_FFT).map(|i| 0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / N_FFT as f64).cos()).collect());

/// Mono, 48 kHz, exactly 10 s: longer audio is cut, shorter audio is repeated
/// whole as many times as fits and then zero-padded.
pub fn prepare(pcm: &Pcm) -> Result<Vec<f32>> {
    let mut mono = pcm.mono();
    // Resample only what we need (plus a little filter margin).
    let needed = (CLIP as f64 * pcm.sample_rate as f64 / SAMPLE_RATE as f64) as usize + 4096;
    mono.truncate(needed);
    if pcm.sample_rate != SAMPLE_RATE && !mono.is_empty() {
        mono = resample(&mono, pcm.sample_rate)?;
    }
    mono.truncate(CLIP);
    if mono.is_empty() {
        return Err(Error::Audio("no audio".into()));
    }
    if mono.len() < CLIP {
        let repeats = CLIP / mono.len();
        let one = mono.clone();
        for _ in 1..repeats {
            mono.extend_from_slice(&one);
        }
        mono.resize(CLIP, 0.0);
    }
    Ok(mono)
}

fn resample(mono: &[f32], rate_in: u32) -> Result<Vec<f32>> {
    let err = |e: &dyn std::fmt::Display| Error::Audio(format!("resampling: {e}"));
    let mut resampler =
        Fft::<f32>::new(rate_in as usize, SAMPLE_RATE as usize, 1024, 1, FixedSync::Both).map_err(|e| err(&e))?;
    let input = InterleavedSlice::new(mono, 1, mono.len()).map_err(|e| err(&e))?;
    let output = resampler.process_all(&input, mono.len(), None).map_err(|e| err(&e))?;
    Ok(output.take_data())
}

/// Log-mel spectrogram, `FRAMES × N_MELS` values in frame-major order.
pub fn log_mel(clip: &[f32]) -> Vec<f32> {
    debug_assert_eq!(clip.len(), CLIP);
    // Centered frames: reflect-pad by half a window on both sides.
    let pad = N_FFT / 2;
    let n = clip.len();
    let padded: Vec<f64> = (0..n + 2 * pad)
        .map(|i| {
            let j = i as isize - pad as isize;
            let j = if j < 0 { -j } else if j as usize >= n { 2 * (n as isize - 1) - j } else { j };
            clip[j as usize] as f64
        })
        .collect();

    let fft = RealFftPlanner::<f64>::new().plan_fft_forward(N_FFT);
    let mut frame = fft.make_input_vec();
    let mut spectrum = fft.make_output_vec();
    let mut power = [0.0f64; N_BINS];
    let mut out = Vec::with_capacity(FRAMES * N_MELS);
    for t in 0..FRAMES {
        let start = t * HOP;
        for (i, x) in frame.iter_mut().enumerate() {
            *x = padded[start + i] * WINDOW[i];
        }
        fft.process(&mut frame, &mut spectrum).expect("fft sizes match");
        for (p, c) in power.iter_mut().zip(&spectrum) {
            *p = c.norm_sqr();
        }
        for filter in MEL_FILTERS.iter() {
            let energy: f64 = filter.iter().zip(&power).map(|(w, p)| w * p).sum();
            out.push((10.0 * energy.max(1e-10).log10()) as f32);
        }
    }
    out
}

// --- Model ---------------------------------------------------------------------

pub struct Model {
    session: Mutex<Session>,
}

impl Model {
    pub fn load(path: &Path, threads: usize) -> Result<Arc<Self>> {
        let err = |e: ort::Error| Error::Other(format!("loading sound model: {e}"));
        let session = Session::builder()
            .map_err(err)?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| err(e.into()))?
            .with_intra_threads(threads)
            .map_err(|e| err(e.into()))?
            .commit_from_file(path)
            .map_err(err)?;
        Ok(Arc::new(Self { session: Mutex::new(session) }))
    }

    /// L2-normalized embeddings for a batch of log-mel spectrograms.
    pub fn embed(&self, features: &[Vec<f32>]) -> Result<Vec<Vec<f32>>> {
        if features.is_empty() {
            return Ok(Vec::new());
        }
        let err = |e: ort::Error| Error::Other(format!("sound model: {e}"));
        let data: Vec<f32> = features.iter().flatten().copied().collect();
        let input = Tensor::from_array(([features.len(), 1, FRAMES, N_MELS], data)).map_err(err)?;
        let mut session = self.session.lock().unwrap();
        let outputs = session.run(ort::inputs!["input_features" => input]).map_err(err)?;
        let (_, flat) = outputs["audio_embeds"].try_extract_tensor::<f32>().map_err(err)?;
        Ok(flat
            .chunks(DIM)
            .map(|v| {
                let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-12);
                v.iter().map(|x| x / norm).collect()
            })
            .collect())
    }
}

pub fn to_bytes(embedding: &[f32]) -> Vec<u8> {
    embedding.iter().flat_map(|v| v.to_le_bytes()).collect()
}

pub fn from_bytes(bytes: &[u8]) -> Vec<f32> {
    bytes.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect()
}

pub fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference() -> serde_json::Value {
        serde_json::from_str(include_str!("../tests/data/clap_reference.json")).unwrap()
    }

    fn data(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data").join(name)
    }

    #[test]
    fn features_match_transformers() {
        let reference = reference();
        for name in ["ref_short.wav", "ref_long.wav"] {
            let pcm = crate::audio::decode(&data(name), 30.0).unwrap();
            let mel = log_mel(&prepare(&pcm).unwrap());
            assert_eq!(mel.len(), FRAMES * N_MELS);
            let r = &reference[name];
            // First three frames, 64 mel values each.
            let head: Vec<f32> = r["features_head"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|frame| frame.as_array().unwrap().iter().map(|v| v.as_f64().unwrap() as f32))
                .collect();
            for (i, (a, b)) in mel[..N_MELS * 3].iter().zip(&head).enumerate() {
                assert!((a - b).abs() < 1e-2, "{name} value {i}: {a} vs {b}");
            }
            let mean = mel.iter().map(|&v| v as f64).sum::<f64>() / mel.len() as f64;
            assert!((mean - r["features_mean"].as_f64().unwrap()).abs() < 1e-3, "{name} mean {mean}");
        }
    }

    #[test]
    fn mel_scale_round_trips() {
        for hz in [50.0, 500.0, 1000.0, 4000.0, 14000.0] {
            assert!((mel_to_hz(hz_to_mel(hz)) - hz).abs() < 1e-6);
        }
    }

    /// Needs the downloaded model: `CLAP_MODEL=… cargo test -- --ignored embeddings_match`
    #[test]
    #[ignore]
    fn embeddings_match_transformers() {
        let model = Model::load(Path::new(&std::env::var("CLAP_MODEL").unwrap()), 2).unwrap();
        let reference = reference();
        let names = ["ref_short.wav", "ref_long.wav"];
        let features: Vec<Vec<f32>> = names
            .iter()
            .map(|n| log_mel(&prepare(&crate::audio::decode(&data(n), 30.0).unwrap()).unwrap()))
            .collect();
        let embeddings = model.embed(&features).unwrap();
        for (name, e) in names.iter().zip(&embeddings) {
            let r: Vec<f32> =
                reference[*name]["embedding"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap() as f32).collect();
            let cos = dot(e, &r);
            println!("{name}: cosine {cos:.5}");
            assert!(cos > 0.999, "{name}: cosine {cos}");
        }
    }
}
