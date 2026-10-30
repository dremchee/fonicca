//! Opt-in reproducible workload. Uses private synthetic files, never library audio.
use super::{Curve, Op, Session};
use crate::audio::AudioRead;
use serde_json::json;
use std::{collections::BTreeMap, io::Write, path::Path, time::Instant};

fn fixture(path: &Path, seconds: u32) {
    fixture_format(path, seconds, 2, 44_100);
}

fn fixture_format(path: &Path, seconds: u32, channels: u32, rate: u32) {
    let frame_bytes = channels * 4;
    let bytes = seconds.checked_mul(rate).unwrap().checked_mul(frame_bytes).unwrap();
    let mut writer = std::io::BufWriter::new(std::fs::File::create(path).unwrap());
    writer.write_all(b"RIFF").unwrap();
    writer.write_all(&(36 + bytes).to_le_bytes()).unwrap();
    writer.write_all(b"WAVEfmt ").unwrap();
    for value in [16u32, channels << 16 | 3, rate, rate * frame_bytes, 32 << 16 | frame_bytes] {
        writer.write_all(&value.to_le_bytes()).unwrap();
    }
    writer.write_all(b"data").unwrap();
    writer.write_all(&bytes.to_le_bytes()).unwrap();
    let mut block = Vec::with_capacity((rate * frame_bytes) as usize);
    for frame in 0..rate {
        let t = frame as f32 / rate as f32;
        for frequency in (0..channels).map(|channel| 220.0 + channel as f32 * 110.0) {
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
            timed(&mut samples, "gain_full", || session.try_apply(Op::Gain { start: 0, end: frames, db: -3.0 }).unwrap());
            session.undo();
            timed(&mut samples, "normalize_full", || session.try_apply(Op::Normalize { start: 0, end: frames, peak_db: -1.0 }).unwrap());
            session.undo();
            for (name, processing) in [
                ("stretch_short", super::processing::Processing::Stretch { semitones: 0.0, ratio: 1.1 }),
                ("lufs_short", super::processing::Processing::Lufs { target: -18.0, ceiling: -1.0 }),
            ] {
                timed(&mut samples, name, || {
                    let audio = session.audio();
                    let pcm = crate::audio::Pcm { samples: audio.range(0..frames.min(3 * rate) * 2).into_owned(), channels: 2, sample_rate: 44100 };
                    super::processing::render(pcm, 0, frames, processing).unwrap()
                });
            }
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

fn rss(system: &mut sysinfo::System) -> Option<u64> {
    let pid = sysinfo::get_current_pid().ok()?;
    system.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::Some(&[pid]), true,
        sysinfo::ProcessRefreshKind::nothing().with_memory(),
    );
    system.process(pid).map(|process| process.memory())
}

fn locked<T>(session: &std::sync::Mutex<Session>, work: impl FnOnce(&mut Session) -> T) -> (T, serde_json::Value) {
    let waiting = Instant::now();
    let mut guard = session.lock().unwrap();
    let held = Instant::now();
    let wait_ms = waiting.elapsed().as_secs_f64() * 1000.0;
    let value = work(&mut guard);
    let held_ms = held.elapsed().as_secs_f64() * 1000.0;
    drop(guard);
    (value, json!({ "waitMs": wait_ms, "heldMs": held_ms }))
}

#[test]
#[ignore = "opt-in command-equivalent copy/processing/commit lock and native RSS workload"]
fn editor_processing_lock_benchmark() {
    let reference = std::env::var("FONICCA_LOCK_BENCH_REFERENCE").is_ok_and(|value| value == "1");
    enum Input { Reference(crate::audio::Pcm, u64), Snapshot(super::processing::Snapshot) }
    let durations = std::env::var("FONICCA_BENCH_SECONDS").unwrap_or_else(|_| "30,180".into());
    let repeats: usize = std::env::var("FONICCA_BENCH_REPEATS").unwrap_or_else(|_| "3".into()).parse().unwrap();
    assert!((1..=20).contains(&repeats));
    let root = tempfile::tempdir().unwrap();
    let mut system = sysinfo::System::new();
    let mut observations = Vec::new();
    for seconds in durations.split(',').map(|n| n.parse::<u32>().unwrap()) {
        assert!((1..=3600).contains(&seconds));
        let input = root.path().join("input.wav");
        fixture(&input, seconds);
        for repetition in 0..repeats {
            let before_open = rss(&mut system);
            let session = std::sync::Mutex::new(Session::open(1, &input, Some(32)).unwrap());
            let after_open = rss(&mut system);
            let frames = session.lock().unwrap().audio().frames();
            let mut cases = Vec::new();
            for (label, end, processing) in [
                ("lufs_short", frames.min(3 * 44100), super::processing::Processing::Lufs { target: -18.0, ceiling: -1.0 }),
                ("lufs_full", frames, super::processing::Processing::Lufs { target: -18.0, ceiling: -1.0 }),
                ("stretch_short", frames.min(3 * 44100), super::processing::Processing::Stretch { semitones: 0.0, ratio: 1.1 }),
            ] {
                let before_prepare = rss(&mut system);
                // Reference is the command path at 67b93dc: make owned PCM under
                // the session mutex, render outside, then apply the op under it.
                let (prepared, prepare_lock) = locked(&session, |session| {
                    if !reference { return Input::Snapshot(super::processing::Snapshot::new(session, 0, end).unwrap()); }
                    let audio = session.audio();
                    super::memory::check_samples(end * audio.channels, super::memory::pcm_budget()).unwrap();
                    Input::Reference(crate::audio::Pcm { samples: audio.range(0..end * audio.channels).into_owned(),
                        channels: audio.channels, sample_rate: audio.sample_rate }, session.revision())
                });
                let after_prepare = rss(&mut system);
                let started = Instant::now();
                let (audio, revision, rendered) = match prepared {
                    Input::Reference(pcm, revision) => (None, revision, super::processing::render(pcm, 0, frames, processing).unwrap()),
                    Input::Snapshot(snapshot) => {
                        let output = snapshot.render(processing).unwrap();
                        (Some(output.audio), output.revision, output.rendered)
                    }
                };
                let worker_ms = started.elapsed().as_secs_f64() * 1000.0;
                let after_worker = rss(&mut system);
                let (_, commit_lock) = locked(&session, |session| {
                    if let Some(audio) = audio { session.apply_rendered(revision, rendered.op, audio).unwrap(); }
                    else { assert_eq!(session.revision(), revision); session.try_apply(rendered.op).unwrap(); }
                });
                let after_commit = rss(&mut system);
                let storage = serde_json::to_value(session.lock().unwrap().diagnostics()).unwrap();
                let (_, undo_lock) = locked(&session, Session::undo);
                cases.push(json!({ "case": label, "targetFrames": end, "copiedBytesUnderPrepareLock": if reference { end * 2 * 4 } else { 0 },
                    "prepareLock": prepare_lock, "workerMs": worker_ms, "commitLock": commit_lock,
                    "undoLock": undo_lock, "storageAfterCommit": storage,
                    "rssBytes": { "beforePrepare": before_prepare, "afterPrepare": after_prepare,
                        "afterWorker": after_worker, "afterCommit": after_commit, "afterUndo": rss(&mut system) } }));
            }
            drop(session);
            observations.push(json!({ "seconds": seconds, "repetition": repetition + 1,
                "rssBeforeOpen": before_open, "rssAfterOpen": after_open, "rssAfterClose": rss(&mut system), "cases": cases }));
        }
    }
    let report = json!({ "schema": 1, "implementation": if reference { "67b93dc command path" } else { "immutable worker snapshot" },
        "profile": if cfg!(debug_assertions) { "dev-test" } else { "release-test" },
        "os": std::env::consts::OS, "arch": std::env::consts::ARCH,
        "note": "Sequential synthetic stereo 44.1kHz float WAV. Native test-process RSS includes mmap pages/allocator retention; it is not PCM heap or WebView memory. Uncontended real Mutex; command-equivalent phases exclude IPC/UI and queue delay.",
        "observations": observations });
    let text = serde_json::to_string_pretty(&report).unwrap();
    if let Ok(path) = std::env::var("FONICCA_LOCK_BENCH_OUTPUT") { std::fs::write(path, &text).unwrap(); }
    println!("{text}");
}

#[test]
#[ignore = "opt-in command-equivalent paste conversion, storage and lock workload"]
fn editor_paste_lock_benchmark() {
    let reference = std::env::var("FONICCA_LOCK_BENCH_REFERENCE").is_ok_and(|value| value == "1");
    let durations = std::env::var("FONICCA_BENCH_SECONDS").unwrap_or_else(|_| "30,180".into());
    let repeats: usize = std::env::var("FONICCA_BENCH_REPEATS").unwrap_or_else(|_| "3".into()).parse().unwrap();
    assert!((1..=20).contains(&repeats));
    let root = tempfile::tempdir().unwrap();
    let mut system = sysinfo::System::new();
    let mut observations = Vec::new();
    for seconds in durations.split(',').map(|n| n.parse::<u32>().unwrap()) {
        assert!((1..=1200).contains(&seconds));
        let input = root.path().join("input.wav");
        fixture(&input, seconds);
        let formats = [("identity", 2, 44100), ("stereo_to_mono_48k", 1, 48000)];
        for (case, channels, sample_rate) in formats {
            fixture_format(&root.path().join(format!("{case}.wav")), seconds, channels, sample_rate);
        }
        for repetition in 0..repeats {
            for (case, channels, sample_rate) in formats {
                let before_open = rss(&mut system);
                let source = Session::open(1, &input, Some(32)).unwrap();
                let clip = super::clipboard::Clip::from_range(source.audio(), 0, source.audio().frames()).unwrap();
                drop(source);
                let target = root.path().join(format!("{case}.wav"));
                let session = std::sync::Mutex::new(Session::open(2, &target, Some(32)).unwrap());
                let position = session.lock().unwrap().audio().frames() / 2;
                let before_paste = rss(&mut system);
                let phases = if reference {
                    let (shares_clipboard, paste_lock) = locked(&session, |session| {
                        let audio = session.audio();
                        let output_frames = (clip.info().frames as f64 * sample_rate as f64 / 44100.0).round().max(1.0) as usize;
                        super::memory::check_samples(clip.info().frames.max(output_frames) * channels as usize, super::memory::pcm_budget() / 2).unwrap();
                        let samples = std::sync::Arc::new(super::clipboard::reference_convert(&clip, channels as usize, sample_rate));
                        let shared = clip.shares_samples(&samples);
                        let result_frames = audio.frames() + samples.len() / channels as usize;
                        super::memory::check_growth(audio.frames(), result_frames, channels as usize).unwrap();
                        session.try_apply(Op::Replace { start: position, end: position, samples }).unwrap();
                        shared
                    });
                    json!({ "pasteLock": paste_lock, "workerMs": 0.0, "sharesClipboard": shares_clipboard })
                } else {
                    let (snapshot, capture_lock) = locked(&session, |session| super::paste::Snapshot::new(session, clip.clone(), position, position));
                    let started = Instant::now();
                    let output = snapshot.render().unwrap();
                    let worker_ms = started.elapsed().as_secs_f64() * 1000.0;
                    let shares_clipboard = matches!(&output.op, Op::Replace { samples, .. } if clip.shares_samples(samples));
                    let (_, commit_lock) = locked(&session, |session| session.apply_rendered(output.revision, output.op, output.audio).unwrap());
                    json!({ "captureLock": capture_lock, "workerMs": worker_ms, "commitLock": commit_lock, "sharesClipboard": shares_clipboard })
                };
                let storage = serde_json::to_value(session.lock().unwrap().diagnostics()).unwrap();
                let clipboard_bytes = clip.info().frames * 2 * 4;
                let stored_pcm = storage["ramBytes"].as_u64().unwrap() + storage["diskBytes"].as_u64().unwrap()
                    + if phases["sharesClipboard"] == true { 0 } else { clipboard_bytes as u64 };
                let after_paste = rss(&mut system);
                let (_, undo_lock) = locked(&session, Session::undo);
                observations.push(json!({ "seconds": seconds, "repetition": repetition + 1, "case": case,
                    "targetChannels": channels, "targetSampleRate": sample_rate, "phases": phases, "undoLock": undo_lock,
                    "clipboardBytes": clipboard_bytes, "storageAfterPaste": storage, "uniqueStoredPcmBytesIncludingClipboard": stored_pcm,
                    "rssBytes": { "beforeOpen": before_open, "beforePaste": before_paste, "afterPaste": after_paste, "afterUndo": rss(&mut system) } }));
            }
        }
    }
    let report = json!({ "schema": 1, "implementation": if reference { "ba2ac2e synchronous paste" } else { "immutable clipboard worker" },
        "profile": if cfg!(debug_assertions) { "dev-test" } else { "release-test" }, "os": std::env::consts::OS, "arch": std::env::consts::ARCH,
        "note": "Synthetic private stereo float WAV; same source, target format and owned clipboard per mode. Reference conversion runs inside the paste lock; candidate conversion and splicing run on the worker. Uncontended mutex phases exclude IPC/UI/queue delay. Clipboard storage is external to session diagnostics and shares its PCM with candidate identity paste. Native RSS includes mmap pages/allocator retention, not WebView memory.",
        "observations": observations });
    let text = serde_json::to_string_pretty(&report).unwrap();
    if let Ok(path) = std::env::var("FONICCA_LOCK_BENCH_OUTPUT") { std::fs::write(path, &text).unwrap(); }
    println!("{text}");
}

#[test]
#[ignore = "opt-in transcription preparation and session lock workload, no model required"]
fn editor_transcription_lock_benchmark() {
    use crate::basic_pitch::input as transcription;
    use crate::audio::Pcm;
    enum Input { Owned(Pcm), Shared(super::blocks::BlockAudio) }
    let reference = std::env::var("FONICCA_LOCK_BENCH_REFERENCE").is_ok_and(|value| value == "1");
    let durations = std::env::var("FONICCA_BENCH_SECONDS").unwrap_or_else(|_| "30,180,600".into());
    let repeats: usize = std::env::var("FONICCA_BENCH_REPEATS").unwrap_or_else(|_| "3".into()).parse().unwrap();
    assert!((1..=20).contains(&repeats));
    let root = tempfile::tempdir().unwrap();
    let mut system = sysinfo::System::new();
    let mut observations = Vec::new();
    for seconds in durations.split(',').map(|n| n.parse::<u32>().unwrap()) {
        assert!((1..=600).contains(&seconds));
        let path = root.path().join("input.wav");
        fixture(&path, seconds);
        for repetition in 0..repeats {
            let session = std::sync::Mutex::new(Session::open(1, &path, Some(32)).unwrap());
            let audio = session.lock().unwrap().audio().clone();
            let frames = audio.frames();
            let before_capture = rss(&mut system);
            let (input, capture_lock) = locked(&session, |session| {
                let audio = session.audio();
                if reference { Input::Owned(Pcm { samples: audio.range(0..frames * 2).into_owned(), channels: 2, sample_rate: 44100 }) }
                else { Input::Shared(transcription::snapshot(audio, super::Span { start: 0, end: frames }).unwrap()) }
            });
            let after_capture = rss(&mut system);
            let started = Instant::now();
            let prepared = match input {
                Input::Owned(pcm) => transcription::prepare(pcm).unwrap(),
                Input::Shared(audio) => transcription::prepare_snapshot(&audio).unwrap(),
            };
            let worker_ms = started.elapsed().as_secs_f64() * 1000.0;
            assert_eq!((prepared.channels, prepared.sample_rate), (1, 22050));
            assert!(audio.samples.same_snapshot(&session.lock().unwrap().audio().samples));
            let after_prepare = rss(&mut system);
            let output_bytes = prepared.samples.capacity() * size_of::<f32>();
            let model_frames = prepared.frames();
            drop(prepared);
            observations.push(json!({ "seconds": seconds, "repetition": repetition + 1,
                "captureLock": capture_lock, "workerMs": worker_ms,
                "copiedInterleavedBytesUnderLock": if reference { frames * 2 * 4 } else { 0 },
                "modelFrames": model_frames, "modelInputBytes": output_bytes,
                "rssBytes": { "beforeCapture": before_capture, "afterCapture": after_capture, "afterPrepare": after_prepare, "afterDroppingPrepared": rss(&mut system) } }));
        }
    }
    let report = json!({ "schema": 1, "implementation": if reference { "e9daaa1 owned selection before transcription" } else { "shared edited selection; mono prepared on worker" },
        "profile": if cfg!(debug_assertions) { "dev-test" } else { "release-test" }, "os": std::env::consts::OS, "arch": std::env::consts::ARCH,
        "note": "Private synthetic stereo 44.1 kHz WAV. Uncontended session mutex; no UI/IPC/job startup/model inference. Existing mono downmix precision and FFT resampler retained. RSS includes mapped pages and allocator retention; phase boundaries can miss transient peaks and exclude WebView memory. Mono input/output and FFT scratch are still allocated on the worker.",
        "observations": observations });
    let text = serde_json::to_string_pretty(&report).unwrap();
    if let Ok(path) = std::env::var("FONICCA_LOCK_BENCH_OUTPUT") { std::fs::write(path, &text).unwrap(); }
    println!("{text}");
}

#[test]
#[ignore = "opt-in file save/session lock workload with private WAVs and backups"]
fn editor_save_lock_benchmark() {
    use super::saving::{self, SaveMode};
    let reference = std::env::var("FONICCA_LOCK_BENCH_REFERENCE").is_ok_and(|value| value == "1");
    let durations = std::env::var("FONICCA_BENCH_SECONDS").unwrap_or_else(|_| "30,180".into());
    let repeats: usize = std::env::var("FONICCA_BENCH_REPEATS").unwrap_or_else(|_| "3".into()).parse().unwrap();
    assert!((1..=20).contains(&repeats));
    let root = tempfile::tempdir().unwrap();
    let mut system = sysinfo::System::new();
    let mut observations = Vec::new();
    for seconds in durations.split(',').map(|n| n.parse::<u32>().unwrap()) {
        assert!((1..=1200).contains(&seconds));
        for repetition in 0..repeats {
            let run = root.path().join(format!("{seconds}-{repetition}"));
            std::fs::create_dir(&run).unwrap();
            let source = run.join("input.wav");
            fixture(&source, seconds);
            let session = std::sync::Mutex::new(Session::open(1, &source, Some(32)).unwrap());
            for case in ["first_save", "save_existing_with_backup"] {
                let frames = session.lock().unwrap().audio().frames();
                session.lock().unwrap().try_apply(Op::Gain { start: 0, end: frames.min(44100), db: -3.0 }).unwrap();
                let before_save = rss(&mut system);
                let backups = run.join("backups");
                let mut locks = Vec::new();
                let started = Instant::now();
                let output = if reference {
                    let (output, held) = locked(&session, |session| saving::save_session(session, &backups, SaveMode::Save, None, None).unwrap());
                    locks.push(held);
                    output
                } else {
                    let (snapshot, held) = locked(&session, |session| saving::Snapshot::new(session, SaveMode::Save, None, None).unwrap());
                    locks.push(held);
                    snapshot.write_with(&backups, |output| {
                        let (_, held) = locked(&session, |session| snapshot.commit(Some(session), &output, None).unwrap());
                        locks.push(held);
                        Ok(output)
                    }).unwrap()
                };
                let total_ms = started.elapsed().as_secs_f64() * 1000.0;
                let after_save = rss(&mut system);
                let file_bytes = std::fs::metadata(&output.path).unwrap().len();
                let backup_bytes = output.backup.as_ref().map(|path| std::fs::metadata(path).unwrap().len());
                assert!(!session.lock().unwrap().state().dirty);
                assert_eq!(backup_bytes.is_some(), case == "save_existing_with_backup");
                observations.push(json!({ "seconds": seconds, "repetition": repetition + 1, "case": case,
                    "sessionLocks": locks, "totalCoreMs": total_ms, "fileBytes": file_bytes, "backupBytes": backup_bytes,
                    "rssBytes": { "beforeSave": before_save, "afterSave": after_save } }));
            }
        }
    }
    let report = json!({ "schema": 1, "implementation": if reference { "synchronous policy under session mutex" } else { "snapshot; independent file writer; guarded saved anchor" },
        "profile": if cfg!(debug_assertions) { "dev-test" } else { "release-test" }, "os": std::env::consts::OS, "arch": std::env::consts::ARCH,
        "note": "Private synthetic stereo float WAV. Reference holds the session mutex around the same file policy; it isolates critical-section scope rather than comparing historical binaries. Candidate includes snapshot capture, real writes, backup, rename, saved-anchor commit and state DTO. Both include the independent uncontended writer mutex. No DB indexing, UI/IPC/worker queue delay or file fsync durability guarantee. Warm filesystem; RSS includes mmap/allocator and phase samples are not peak memory.",
        "observations": observations });
    let text = serde_json::to_string_pretty(&report).unwrap();
    if let Ok(path) = std::env::var("FONICCA_LOCK_BENCH_OUTPUT") { std::fs::write(path, &text).unwrap(); }
    println!("{text}");
}

#[test]
#[ignore = "opt-in slice detection/export and editor lock workload"]
fn editor_slices_lock_benchmark() {
    let reference = std::env::var("FONICCA_LOCK_BENCH_REFERENCE").is_ok_and(|value| value == "1");
    let durations = std::env::var("FONICCA_BENCH_SECONDS").unwrap_or_else(|_| "30,180".into());
    let repeats: usize = std::env::var("FONICCA_BENCH_REPEATS").unwrap_or_else(|_| "3".into()).parse().unwrap();
    assert!((1..=20).contains(&repeats));
    let root = tempfile::tempdir().unwrap();
    let mut system = sysinfo::System::new();
    let mut observations = Vec::new();
    for seconds in durations.split(',').map(|n| n.parse::<u32>().unwrap()) {
        assert!((1..=1200).contains(&seconds));
        let path = root.path().join("input.wav");
        fixture(&path, seconds);
        for repetition in 0..repeats {
            let session = std::sync::Mutex::new(Session::open(1, &path, Some(32)).unwrap());
            let frames = session.lock().unwrap().audio().frames();
            for (case, by) in [("grid", super::SliceBy::Grid { origin: 0.0, step: 22050.0 }),
                ("transients", super::SliceBy::Transients { sensitivity: 0.5, min_ms: 50.0 }),
                ("silence", super::SliceBy::Silence { threshold_db: -40.0, min_silence_ms: 100.0 })] {
                let mut locks = Vec::new();
                let before = rss(&mut system);
                let started = Instant::now();
                let spans = if reference {
                    let (spans, held) = locked(&session, |session| super::slices::slices(session.audio(), 0, frames, by, true));
                    locks.push(held);
                    spans
                } else {
                    let (snapshot, held) = locked(&session, |session| super::SliceSnapshot::new(session));
                    locks.push(held);
                    let result = snapshot.detect(0, frames, by, true);
                    let (spans, held) = locked(&session, |session| result.finish(session).unwrap());
                    locks.push(held);
                    spans
                };
                let total_ms = started.elapsed().as_secs_f64() * 1000.0;
                observations.push(json!({ "seconds": seconds, "repetition": repetition + 1, "case": case,
                    "sessionLocks": locks, "totalCoreMs": total_ms, "spanCount": spans.len(),
                    "rssBytes": { "before": before, "after": rss(&mut system) } }));
            }
            let spans = (0..4).map(|n| super::Span { start: n * frames / 4, end: (n + 1) * frames / 4 }).collect();
            let mut locks = Vec::new();
            let before = rss(&mut system);
            let started = Instant::now();
            let (dir, count) = if reference {
                let (output, held) = locked(&session, |session| super::exporting::Snapshot::new(session, spans).unwrap().write().unwrap());
                locks.push(held);
                output
            } else {
                let (snapshot, held) = locked(&session, |session| super::exporting::Snapshot::new(session, spans).unwrap());
                locks.push(held);
                snapshot.write().unwrap()
            };
            let total_ms = started.elapsed().as_secs_f64() * 1000.0;
            assert_eq!(count, 4);
            let output_bytes: u64 = std::fs::read_dir(&dir).unwrap().map(|entry| entry.unwrap().metadata().unwrap().len()).sum();
            observations.push(json!({ "seconds": seconds, "repetition": repetition + 1, "case": "export_four_regions",
                "sessionLocks": locks, "totalCoreMs": total_ms, "spanCount": count, "fileBytes": output_bytes,
                "rssBytes": { "before": before, "after": rss(&mut system) } }));
        }
    }
    let report = json!({ "schema": 1, "implementation": if reference { "detection/export under session mutex" } else { "shared snapshots; worker detection/export" },
        "profile": if cfg!(debug_assertions) { "dev-test" } else { "release-test" }, "os": std::env::consts::OS, "arch": std::env::consts::ARCH,
        "note": "Private synthetic stereo float WAV. Detection uses the unchanged grid/transient/silence algorithm and zero snapping. Export writes four equal WAVs with the same policy in both modes; reference includes snapshot setup inside the long critical section and isolates lock scope rather than comparing historical binaries. No library rescan, UI/IPC or worker queue delay. Warm filesystem and uncontended mutex. Phase RSS includes mmap/allocator, misses transient peaks and excludes WebView.",
        "observations": observations });
    let text = serde_json::to_string_pretty(&report).unwrap();
    if let Ok(path) = std::env::var("FONICCA_LOCK_BENCH_OUTPUT") { std::fs::write(path, &text).unwrap(); }
    println!("{text}");
}
