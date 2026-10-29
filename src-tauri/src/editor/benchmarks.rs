//! Opt-in reproducible workload. Uses private synthetic files, never library audio.
use super::{Curve, Op, Session};
use crate::audio::AudioRead;
use serde_json::json;
use std::{collections::BTreeMap, io::Write, path::Path, time::Instant};

fn fixture(path: &Path, seconds: u32) {
    let rate = 44_100u32;
    let bytes = seconds.checked_mul(rate).unwrap().checked_mul(8).unwrap();
    let mut writer = std::io::BufWriter::new(std::fs::File::create(path).unwrap());
    writer.write_all(b"RIFF").unwrap();
    writer.write_all(&(36 + bytes).to_le_bytes()).unwrap();
    writer.write_all(b"WAVEfmt ").unwrap();
    for value in [16u32, 0x0002_0003, rate, rate * 8, 0x0020_0008] {
        writer.write_all(&value.to_le_bytes()).unwrap();
    }
    writer.write_all(b"data").unwrap();
    writer.write_all(&bytes.to_le_bytes()).unwrap();
    let mut block = Vec::with_capacity(rate as usize * 8);
    for frame in 0..rate {
        let t = frame as f32 / rate as f32;
        for frequency in [220.0, 330.0] {
            let amplitude = 0.1 + 0.5 * (-12.0 * t).exp();
            block.extend_from_slice(&(amplitude * (t * frequency * std::f32::consts::TAU).sin()).to_le_bytes());
        }
    }
    for _ in 0..seconds { writer.write_all(&block).unwrap(); }
    writer.flush().unwrap();
}

fn timed<T>(samples: &mut BTreeMap<&'static str, Vec<f64>>, name: &'static str, work: impl FnOnce() -> T) -> T {
    let started = Instant::now();
    let result = work();
    samples.entry(name).or_default().push(started.elapsed().as_secs_f64() * 1000.0);
    result
}

#[test]
#[ignore = "opt-in performance workload; FONICCA_BENCH_SECONDS=30,180,1200,3600 FONICCA_BENCH_OUTPUT=/tmp/report.json"]
fn editor_workload_benchmark() {
    let durations = std::env::var("FONICCA_BENCH_SECONDS").unwrap_or_else(|_| "30,180".into());
    let repetitions: usize = std::env::var("FONICCA_BENCH_REPEATS").unwrap_or_else(|_| "3".into()).parse().unwrap();
    assert!((1..=20).contains(&repetitions));
    let root = tempfile::tempdir().unwrap();
    let mut reports = Vec::new();
    for seconds in durations.split(',').map(|n| n.parse::<u32>().unwrap()) {
        assert!((1..=3600).contains(&seconds));
        let input = root.path().join("input.wav");
        fixture(&input, seconds);
        let mut samples = BTreeMap::new();
        let mut memory = serde_json::Value::Null;
        for _ in 0..repetitions {
            let mut session = timed(&mut samples, "open_decode", || Session::open(1, &input, Some(32)).unwrap());
            let frames = session.audio().frames();
            let rate = session.audio().sample_rate as usize;
            timed(&mut samples, "wave_first", || session.wave(0, frames, 1200, true, false));
            timed(&mut samples, "wave_cached", || session.wave(0, frames, 1200, true, false));
            timed(&mut samples, "wave_pan", || session.wave(frames / 4, frames / 2, 1200, true, false));
            timed(&mut samples, "wave_zoom", || session.wave(frames / 2, (frames / 2 + rate).min(frames), 1200, true, false));
            timed(&mut samples, "fade_short", || session.try_apply(Op::FadeOut { start: 0, end: frames.min(3 * rate), curve: Curve::Linear }).unwrap());
            timed(&mut samples, "wave_after_fade", || session.wave(0, frames, 1200, true, false));
            timed(&mut samples, "undo_fade", || session.undo());
            timed(&mut samples, "redo_fade", || session.redo());
            timed(&mut samples, "delete", || session.try_apply(Op::Delete { start: frames / 3, end: (frames / 3 + rate).min(frames) }).unwrap());
            timed(&mut samples, "undo_delete", || session.undo());
            timed(&mut samples, "fade_full", || session.try_apply(Op::FadeOut { start: 0, end: frames, curve: Curve::Smooth }).unwrap());
            timed(&mut samples, "undo_full_fade", || session.undo());
            timed(&mut samples, "save", || super::write_wav(&root.path().join("output.wav"), session.audio(), Some(32), None).unwrap());
            memory = serde_json::to_value(session.diagnostics()).unwrap();
        }
        let operations: BTreeMap<_, _> = samples.into_iter().map(|(name, values)| {
            let mut sorted = values.clone();
            sorted.sort_by(f64::total_cmp);
            let percentile = |p: f64| sorted[(sorted.len() as f64 * p).ceil() as usize - 1];
            (name, json!({ "samplesMs": values, "p50Ms": percentile(0.5), "p95Ms": percentile(0.95) }))
        }).collect();
        reports.push(json!({ "seconds": seconds, "sampleRate": 44100, "channels": 2,
            "repetitions": repetitions, "operations": operations, "storage": memory }));
    }
    let report = json!({ "schema": 1, "profile": if cfg!(debug_assertions) { "dev-test" } else { "release-test" },
        "os": std::env::consts::OS, "arch": std::env::consts::ARCH,
        "note": "Sequential warm-filesystem runs; no audio device or WebView. Wave includes cache generation. Storage is not process RSS.",
        "scenarios": reports });
    let text = serde_json::to_string_pretty(&report).unwrap();
    if let Ok(path) = std::env::var("FONICCA_BENCH_OUTPUT") { std::fs::write(path, &text).unwrap(); }
    println!("{text}");
}
