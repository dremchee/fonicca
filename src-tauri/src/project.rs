//! A sample-based arrangement. Preview and WAV export use the same frame clock.
use crate::{
    AppState,
    audio::{self, Pcm},
    error::{Error, Result},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};

pub const RATE: u32 = 48_000;
const MAX_SECONDS: f64 = 600.0;
const MAX_BYTES: usize = 256 * 1024 * 1024;
fn invalid(s: &str) -> Error {
    Error::Other(s.into())
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Clip {
    pub id: String,
    pub path: String,
    pub name: String,
    pub source_duration: f64,
    pub start: f64,
    pub offset: f64,
    pub duration: f64,
    pub repeats: u32,
    pub fade_in: f64,
    pub fade_out: f64,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: String,
    pub name: String,
    pub gain_db: f64,
    pub pan: f64,
    pub mute: bool,
    pub solo: bool,
    pub clips: Vec<Clip>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Project {
    pub version: u32,
    pub name: String,
    pub bpm: f64,
    pub master_db: f64,
    pub tracks: Vec<Track>,
}
impl Project {
    pub fn duration(&self) -> f64 {
        self.tracks
            .iter()
            .flat_map(|t| &t.clips)
            .map(|c| c.start + c.duration * c.repeats as f64)
            .fold(0.0, f64::max)
    }
    pub fn validate(&self) -> Result<()> {
        let range = |v: f64, a: f64, b: f64| v.is_finite() && (a..=b).contains(&v);
        if self.version != 1
            || !range(self.bpm, 30.0, 300.0)
            || !range(self.master_db, -60.0, 0.0)
            || self.name.len() > 512
            || self.tracks.len() > 32
        {
            return Err(invalid(
                "invalid project settings (up to 32 tracks, 30–300 BPM)",
            ));
        }
        let mut ids = std::collections::HashSet::new();
        let mut clips = 0;
        for t in &self.tracks {
            if !ids.insert(&t.id) || !range(t.gain_db, -60.0, 12.0) || !range(t.pan, -1.0, 1.0) {
                return Err(invalid("invalid track settings"));
            }
            for c in &t.clips {
                clips += 1;
                if !ids.insert(&c.id)
                    || c.path.is_empty()
                    || !range(c.start, 0.0, MAX_SECONDS)
                    || !range(c.offset, 0.0, MAX_SECONDS)
                    || !range(c.duration, 1.0 / RATE as f64, MAX_SECONDS)
                    || !(1..=64).contains(&c.repeats)
                    || !range(c.fade_in, 0.0, c.duration)
                    || !range(c.fade_out, 0.0, c.duration)
                    || !range(c.source_duration, 1.0 / RATE as f64, MAX_SECONDS)
                    || c.offset + c.duration > c.source_duration + 1e-8
                    || c.start + c.duration * c.repeats as f64 > MAX_SECONDS
                {
                    return Err(invalid(
                        "invalid clip; project and source ranges must fit within 10 minutes",
                    ));
                }
            }
        }
        if clips > 256 {
            return Err(invalid("a project can contain up to 256 clips"));
        }
        Ok(())
    }
}

struct Voice {
    clip: Clip,
    pcm: Arc<Pcm>,
    left: f32,
    right: f32,
}
pub struct Mix {
    voices: Vec<Voice>,
    frame: usize,
    channel: usize,
    end: usize,
    total_frames: usize,
    stereo: [f32; 2],
}
impl Mix {
    fn from_audio(project: &Project, files: &HashMap<String, Arc<Pcm>>, start: f64) -> Self {
        let solo = project.tracks.iter().any(|t| t.solo);
        let mut voices = Vec::new();
        for t in &project.tracks {
            if t.mute || (solo && !t.solo) {
                continue;
            }
            let gain = 10f64.powf((t.gain_db + project.master_db) / 20.0) as f32;
            // Balance law: centre preserves stereo, endpoints silence the opposite channel.
            let left = gain * (1.0 - t.pan.max(0.0)) as f32;
            let right = gain * (1.0 + t.pan.min(0.0)) as f32;
            for clip in &t.clips {
                if let Some(pcm) = files.get(&clip.path) {
                    voices.push(Voice {
                        clip: clip.clone(),
                        pcm: Arc::clone(pcm),
                        left,
                        right,
                    });
                }
            }
        }
        Self {
            voices,
            frame: (start * RATE as f64).round() as usize,
            channel: 0,
            end: (project.duration() * RATE as f64).ceil() as usize,
            total_frames: ((project.duration() - start).max(0.0) * RATE as f64).ceil() as usize,
            stereo: [0.0; 2],
        }
    }
    fn sample_frame(&self) -> [f32; 2] {
        let mut out = [0.0; 2];
        for v in &self.voices {
            let c = &v.clip;
            let begin = (c.start * RATE as f64).round() as usize;
            let length = (c.duration * RATE as f64).round().max(1.0) as usize;
            if self.frame < begin || self.frame - begin >= length * c.repeats as usize {
                continue;
            }
            let time = ((self.frame - begin) % length) as f64 / RATE as f64;
            let index = (c.offset + time) * v.pcm.sample_rate as f64;
            let frame = index.floor() as usize;
            let fraction = (index - frame as f64) as f32;
            if frame >= v.pcm.frames() {
                continue;
            }
            let envelope = if c.fade_in > 0.0 {
                (time / c.fade_in).min(1.0)
            } else {
                1.0
            } * if c.fade_out > 0.0 {
                ((c.duration - time) / c.fade_out).clamp(0.0, 1.0)
            } else {
                1.0
            };
            for ch in 0..2 {
                let source_ch = ch.min(v.pcm.channels - 1);
                let a = v.pcm.samples[frame * v.pcm.channels + source_ch];
                let next = (frame + 1).min(v.pcm.frames() - 1);
                let b = v.pcm.samples[next * v.pcm.channels + source_ch];
                out[ch] += (a + (b - a) * fraction)
                    * envelope as f32
                    * if ch == 0 { v.left } else { v.right };
            }
        }
        out
    }
}
impl Iterator for Mix {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.frame >= self.end {
            return None;
        }
        if self.channel == 0 {
            self.stereo = self.sample_frame();
        }
        let sample = self.stereo[self.channel];
        self.channel += 1;
        if self.channel == 2 {
            self.channel = 0;
            self.frame += 1;
        }
        Some(sample)
    }
}
impl rodio::Source for Mix {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> std::num::NonZero<u16> {
        std::num::NonZero::new(2).unwrap()
    }
    fn sample_rate(&self) -> std::num::NonZero<u32> {
        std::num::NonZero::new(RATE).unwrap()
    }
    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f64(
            self.total_frames as f64 / RATE as f64,
        ))
    }
}

fn prepare(project: &Project, start: f64) -> Result<Mix> {
    project.validate()?;
    if !start.is_finite() || start < 0.0 || start >= project.duration() {
        return Err(invalid("place the cursor inside a non-empty project"));
    }
    let mut needed = HashMap::<String, f64>::new();
    let solo = project.tracks.iter().any(|t| t.solo);
    for track in &project.tracks {
        if track.mute || (solo && !track.solo) {
            continue;
        }
        for c in &track.clips {
            needed
                .entry(c.path.clone())
                .and_modify(|n| *n = n.max(c.offset + c.duration))
                .or_insert(c.offset + c.duration);
        }
    }
    let mut files = HashMap::new();
    let mut bytes = 0;
    for (path, seconds) in needed {
        let info = audio::probe(Path::new(&path)).map_err(|e| invalid(&format!("{path}: {e}")))?;
        if info.channels.is_some_and(|channels| channels > 2) {
            return Err(invalid(&format!(
                "{path}: projects support mono and stereo sources"
            )));
        }
        let estimate = ((seconds + 0.1)
            * info.sample_rate.unwrap_or(RATE).max(RATE) as f64
            * info.channels.unwrap_or(2) as f64
            * 4.0) as usize;
        if bytes + estimate > MAX_BYTES {
            return Err(invalid(
                "project audio exceeds the 256 MiB memory limit; trim clips or use fewer sources",
            ));
        }
        let mut pcm = audio::decode(Path::new(&path), seconds + 0.1)
            .map_err(|e| invalid(&format!("{path}: {e}")))?;
        if pcm.channels == 0 || pcm.sample_rate == 0 || pcm.samples.iter().any(|s| !s.is_finite()) {
            return Err(invalid("invalid source audio"));
        }
        if (pcm.frames() as f64 + 1.0) / (pcm.sample_rate as f64) < seconds {
            return Err(invalid(&format!(
                "{path}: clip extends past the source audio"
            )));
        }
        // Use the existing band-limited converter before the audio callback.
        // Playback then only reads and sums samples; no resampling allocation on the sound thread.
        crate::batch::resample(&mut pcm, RATE)?;
        bytes += pcm.samples.len() * 4;
        if bytes > MAX_BYTES {
            return Err(invalid("project audio exceeds the 256 MiB memory limit"));
        }
        files.insert(path, Arc::new(pcm));
    }
    Ok(Mix::from_audio(project, &files, start))
}

#[derive(Default)]
pub struct Transport {
    revision: u64,
    active: bool,
    pending: Vec<Vec<i64>>,
}
#[tauri::command]
pub async fn project_play(state: State<'_, AppState>, project: Project, start: f64) -> Result<()> {
    let revision = {
        let mut transport = state.project.lock().unwrap();
        transport.revision += 1;
        transport.active = false;
        state.project_player.stop();
        transport.revision
    };
    let mix = tauri::async_runtime::spawn_blocking(move || prepare(&project, start))
        .await
        .map_err(|e| invalid(&e.to_string()))??;
    let mut transport = state.project.lock().unwrap();
    if transport.revision != revision {
        return Err(invalid("playback cancelled"));
    }
    state.project_player.play_project(mix, start)?;
    transport.active = true;
    Ok(())
}
#[tauri::command]
pub fn project_stop(state: State<'_, AppState>) {
    let mut transport = state.project.lock().unwrap();
    transport.revision += 1;
    if transport.active {
        state.project_player.stop();
    }
    transport.active = false;
}
#[tauri::command]
pub fn project_position(state: State<'_, AppState>) -> Option<f64> {
    let transport = state.project.lock().unwrap();
    if transport.active {
        state.project_player.position()
    } else {
        None
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSample {
    id: i64,
    path: String,
    name: String,
    duration: f64,
}
#[tauri::command]
pub fn project_samples(state: State<'_, AppState>, ids: Vec<i64>) -> Result<Vec<ProjectSample>> {
    if ids.len() > 32 {
        return Err(invalid("add up to 32 samples at once"));
    }
    let conn = state.db.lock().unwrap();
    ids.iter()
        .map(|id| {
            conn.query_row(
                "SELECT id,path,name,duration FROM samples WHERE id=?1",
                [id],
                |r| {
                    Ok(ProjectSample {
                        id: r.get(0)?,
                        path: r.get(1)?,
                        name: r.get(2)?,
                        duration: r.get::<_, Option<f64>>(3)?.unwrap_or(0.0),
                    })
                },
            )
            .map_err(Error::from)
        })
        .collect()
}
fn write_atomic(path: &Path, write: impl FnOnce(&Path) -> Result<()>) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| invalid("choose an output folder"))?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp = parent.join(format!(".fonicca-{}-{stamp}.tmp", std::process::id()));
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)?;
    let result = write(&temp).and_then(|_| std::fs::rename(&temp, path).map_err(Error::from));
    if result.is_err() {
        let _ = std::fs::remove_file(temp);
    }
    result
}
fn protect_sources(project: &Project, path: &Path) -> Result<()> {
    let target = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if project.tracks.iter().flat_map(|t| &t.clips).any(|c| {
        std::fs::canonicalize(&c.path).unwrap_or_else(|_| PathBuf::from(&c.path)) == target
    }) {
        return Err(invalid(
            "choose a destination different from a source audio file",
        ));
    }
    Ok(())
}
#[tauri::command]
pub async fn project_save(project: Project, path: String) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || {
        project.validate()?;
        let path = Path::new(&path);
        protect_sources(&project, path)?;
        let bytes = serde_json::to_vec_pretty(&project).map_err(|e| invalid(&e.to_string()))?;
        if bytes.len() > 1024 * 1024 {
            return Err(invalid("project file is too large"));
        }
        write_atomic(path, |temp| {
            std::fs::write(temp, bytes)?;
            Ok(())
        })
    })
    .await
    .map_err(|e| invalid(&e.to_string()))?
}
#[tauri::command]
pub async fn project_load(path: String) -> Result<Project> {
    tauri::async_runtime::spawn_blocking(move || {
        use std::io::Read;
        let mut bytes = Vec::new();
        std::fs::File::open(path)?
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() > 1024 * 1024 {
            return Err(invalid("project file is too large"));
        }
        let p: Project = serde_json::from_slice(&bytes).map_err(|e| invalid(&e.to_string()))?;
        p.validate()?;
        Ok(p)
    })
    .await
    .map_err(|e| invalid(&e.to_string()))?
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Exported {
    peak_db: f64,
    seconds: f64,
}
#[tauri::command]
pub async fn project_export(project: Project, path: String) -> Result<Exported> {
    tauri::async_runtime::spawn_blocking(move || export(&project, Path::new(&path)))
        .await
        .map_err(|e| invalid(&e.to_string()))?
}
fn export(project: &Project, path: &Path) -> Result<Exported> {
    protect_sources(project, path)?;
    let mix = prepare(project, 0.0)?;
    let mut peak = 0.0f32;
    write_atomic(path, |temp| {
        let mut writer = hound::WavWriter::create(
            temp,
            hound::WavSpec {
                channels: 2,
                sample_rate: RATE,
                bits_per_sample: 32,
                sample_format: hound::SampleFormat::Float,
            },
        )
        .map_err(|e| invalid(&e.to_string()))?;
        for sample in mix {
            peak = peak.max(sample.abs());
            writer
                .write_sample(sample)
                .map_err(|e| invalid(&e.to_string()))?;
        }
        writer.finalize().map_err(|e| invalid(&e.to_string()))?;
        Ok(())
    })?;
    Ok(Exported {
        peak_db: 20.0 * (peak.max(1e-10) as f64).log10(),
        seconds: project.duration(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn clip(id: &str, start: f64, offset: f64, duration: f64) -> Clip {
        Clip {
            id: id.into(),
            path: "test.wav".into(),
            name: "Test".into(),
            source_duration: 600.0,
            start,
            offset,
            duration,
            repeats: 1,
            fade_in: 0.0,
            fade_out: 0.0,
        }
    }
    fn project(clips: Vec<Clip>) -> Project {
        Project {
            version: 1,
            name: "Test".into(),
            bpm: 120.0,
            master_db: 0.0,
            tracks: vec![Track {
                id: "track".into(),
                name: "Track".into(),
                gain_db: 0.0,
                pan: 0.0,
                mute: false,
                solo: false,
                clips,
            }],
        }
    }
    fn mix(p: &Project, samples: Vec<f32>, channels: usize, rate: u32, start: f64) -> Vec<f32> {
        let files = HashMap::from([(
            "test.wav".into(),
            Arc::new(Pcm {
                samples,
                channels,
                sample_rate: rate,
            }),
        )]);
        Mix::from_audio(p, &files, start).collect()
    }
    #[test]
    fn clips_share_a_clock_and_sum_at_exact_start_frames() {
        let p = project(vec![
            clip("a", 0.0, 0.0, 4.0 / RATE as f64),
            clip("b", 2.0 / RATE as f64, 0.0, 2.0 / RATE as f64),
        ]);
        assert_eq!(
            mix(&p, vec![0.25; 4], 1, RATE, 0.0),
            vec![0.25, 0.25, 0.25, 0.25, 0.5, 0.5, 0.5, 0.5]
        );
        assert_eq!(
            mix(&p, vec![0.25; 4], 1, RATE, 2.0 / RATE as f64),
            vec![0.5; 4]
        );
    }
    #[test]
    fn source_offsets_repeats_and_linear_resampling_are_deterministic() {
        let mut c = clip("a", 0.0, 1.0 / 24_000.0, 4.0 / RATE as f64);
        c.repeats = 2;
        let out = mix(&project(vec![c]), vec![0.0, 0.2, 0.4, 0.6], 1, 24_000, 0.0);
        for (actual, expected) in out
            .iter()
            .step_by(2)
            .zip([0.2, 0.3, 0.4, 0.5, 0.2, 0.3, 0.4, 0.5])
        {
            assert!((actual - expected).abs() < 1e-6);
        }
    }
    #[test]
    fn mute_solo_stereo_pan_and_master_gain_apply_before_summing() {
        let mut p = project(vec![clip("a", 0.0, 0.0, 2.0 / RATE as f64)]);
        let mut second = p.tracks[0].clone();
        second.id = "second".into();
        second.clips[0].id = "b".into();
        second.solo = true;
        second.pan = -1.0;
        p.tracks.push(second);
        p.master_db = -6.020599913;
        let out = mix(&p, vec![0.4, 0.8, 0.4, 0.8], 2, RATE, 0.0);
        assert!((out[0] - 0.2).abs() < 1e-6);
        assert_eq!(out[1], 0.0);
        p.tracks[1].mute = true;
        assert!(
            mix(&p, vec![0.4; 4], 2, RATE, 0.0)
                .iter()
                .all(|s| *s == 0.0)
        );
    }
    #[test]
    fn fades_restart_on_each_repeat() {
        let mut c = clip("a", 0.0, 0.0, 4.0 / RATE as f64);
        c.repeats = 2;
        c.fade_in = 2.0 / RATE as f64;
        c.fade_out = 2.0 / RATE as f64;
        let out = mix(&project(vec![c]), vec![1.0; 4], 1, RATE, 0.0);
        assert_eq!(
            out.iter().step_by(2).copied().collect::<Vec<_>>(),
            vec![0.0, 0.5, 1.0, 0.5, 0.0, 0.5, 1.0, 0.5]
        );
    }
    #[test]
    fn project_round_trip_preserves_edits_and_rejects_invalid_ranges() {
        let mut p = project(vec![clip("a", 1.0, 0.0, 0.5)]);
        p.validate().unwrap();
        let restored: Project = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
        assert_eq!(restored.duration(), 1.5);
        assert_eq!(restored.tracks[0].clips[0].path, "test.wav");
        p.tracks[0].clips[0].repeats = 0;
        assert!(p.validate().is_err());
        p.tracks[0].clips[0].repeats = 1;
        p.tracks[0].pan = f64::NAN;
        assert!(p.validate().is_err());
        p.tracks[0].pan = 0.0;
        p.tracks[0].clips[0].start = 600.0;
        assert!(p.validate().is_err());
    }
    #[test]
    fn wav_export_matches_preview_and_failed_export_keeps_existing_files() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.wav");
        let output = dir.path().join("mix.wav");
        crate::editor::write_wav(
            &source,
            &Pcm {
                samples: vec![0.25; 4800],
                channels: 1,
                sample_rate: RATE,
            },
            Some(32),
            None,
        )
        .unwrap();
        let source_bytes = std::fs::read(&source).unwrap();
        let mut p = project(vec![clip("a", 0.0, 0.0, 0.1)]);
        p.tracks[0].clips[0].path = source.to_str().unwrap().into();
        let expected: Vec<_> = prepare(&p, 0.0).unwrap().collect();
        let report = export(&p, &output).unwrap();
        assert_eq!(report.seconds, 0.1);
        assert!(report.peak_db < 0.0);
        let actual: Vec<f32> = hound::WavReader::open(&output)
            .unwrap()
            .samples::<f32>()
            .map(|s| s.unwrap())
            .collect();
        assert_eq!(actual, expected);
        assert!(export(&p, &source).is_err());
        assert_eq!(std::fs::read(&source).unwrap(), source_bytes);
        let before = std::fs::read(&output).unwrap();
        p.tracks[0].clips[0].duration = 0.5;
        assert!(export(&p, &output).is_err());
        assert_eq!(std::fs::read(&output).unwrap(), before);
        p.tracks[0].clips[0].path = "/missing/source.wav".into();
        assert!(prepare(&p, 0.0).is_err());
    }
}

#[derive(Serialize)]
pub struct Waveform {
    duration: f64,
    peaks: Vec<f32>,
}
#[tauri::command]
pub async fn project_wave(path: String) -> Result<Waveform> {
    tauri::async_runtime::spawn_blocking(move || {
        let path = Path::new(&path);
        let info = audio::probe(path)?;
        let overview = crate::overview::Overview::compute(path, None)?;
        let data = overview.render(512, false);
        let peaks = data[16..]
            .chunks_exact(12)
            .map(|column| {
                let min = i16::from_le_bytes([column[0], column[1]]) as f32 / 32767.0;
                let max = i16::from_le_bytes([column[2], column[3]]) as f32 / 32767.0;
                min.abs().max(max.abs())
            })
            .collect();
        Ok(Waveform {
            duration: info.duration.unwrap_or(0.0),
            peaks,
        })
    })
    .await
    .map_err(|e| invalid(&e.to_string()))?
}

#[cfg(test)]
mod file_tests {
    use super::*;
    #[test]
    fn project_file_round_trip_and_invalid_save_preserve_previous_contents() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir
            .path()
            .join("loop.fonicca.json")
            .to_str()
            .unwrap()
            .to_string();
        let p = Project {
            version: 1,
            name: "Loop".into(),
            bpm: 95.0,
            master_db: -9.0,
            tracks: vec![],
        };
        tauri::async_runtime::block_on(project_save(p.clone(), path.clone())).unwrap();
        let loaded = tauri::async_runtime::block_on(project_load(path.clone())).unwrap();
        assert_eq!(loaded.bpm, 95.0);
        assert_eq!(loaded.name, "Loop");
        let bytes = std::fs::read(&path).unwrap();
        let mut bad = p;
        bad.bpm = 0.0;
        assert!(tauri::async_runtime::block_on(project_save(bad, path.clone())).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        std::fs::write(&path, b"{broken").unwrap();
        assert!(tauri::async_runtime::block_on(project_load(path)).is_err());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}

#[cfg(test)]
mod rate_tests {
    use super::*;
    #[test]
    fn different_source_rates_produce_the_same_timeline_length() {
        let dir = tempfile::tempdir().unwrap();
        for rate in [22_050, 44_100, 48_000, 96_000] {
            let path = dir.path().join(format!("source-{rate}.wav"));
            let pcm = Pcm {
                samples: (0..rate / 10)
                    .map(|f| (f as f32 / rate as f32 * 440.0 * std::f32::consts::TAU).sin() * 0.25)
                    .collect(),
                channels: 1,
                sample_rate: rate,
            };
            crate::editor::write_wav(&path, &pcm, Some(32), None).unwrap();
            let p = Project {
                version: 1,
                name: "Rates".into(),
                bpm: 120.0,
                master_db: 0.0,
                tracks: vec![Track {
                    id: "t".into(),
                    name: "Track".into(),
                    gain_db: 0.0,
                    pan: 0.0,
                    mute: false,
                    solo: false,
                    clips: vec![Clip {
                        id: "c".into(),
                        path: path.to_str().unwrap().into(),
                        name: "Tone".into(),
                        source_duration: 0.1,
                        start: 0.0,
                        offset: 0.0,
                        duration: 0.1,
                        repeats: 2,
                        fade_in: 0.0,
                        fade_out: 0.0,
                    }],
                }],
            };
            let out: Vec<_> = prepare(&p, 0.0).unwrap().collect();
            assert_eq!(out.len(), 19_200);
            assert!(out.iter().all(|s| s.is_finite() && s.abs() < 0.3));
            assert!(out.iter().any(|s| s.abs() > 0.2));
            assert_eq!(out[..9600], out[9600..]);
        }
    }
}

pub const WINDOW_LABEL: &str = "project";
impl Transport {
    fn enqueue(&mut self, ids: Vec<i64>) -> Result<()> {
        if ids.len() > 32 || ids.iter().any(|id| *id <= 0) {
            return Err(invalid("add up to 32 samples at once"));
        }
        if !ids.is_empty() {
            if self.pending.len() >= 32 {
                return Err(invalid("wait for the project to finish adding samples"));
            }
            self.pending.push(ids);
        }
        Ok(())
    }
    fn take_samples(&mut self) -> Vec<Vec<i64>> {
        std::mem::take(&mut self.pending)
    }
}
#[tauri::command]
pub fn open_project_window(
    app: AppHandle,
    state: State<'_, AppState>,
    ids: Vec<i64>,
) -> Result<()> {
    if ids.len() > 32 || ids.iter().any(|id| *id <= 0) {
        return Err(invalid("add up to 32 samples at once"));
    }
    if app.get_webview_window(WINDOW_LABEL).is_none() {
        let builder = WebviewWindowBuilder::new(
            &app,
            WINDOW_LABEL,
            WebviewUrl::App("index.html?project=1".into()),
        )
        .title("Fonicca — Project")
        .inner_size(1120.0, 720.0)
        .min_inner_size(760.0, 540.0)
        .visible(false);
        #[cfg(target_os = "macos")]
        let builder = builder
            .title_bar_style(tauri::TitleBarStyle::Overlay)
            .hidden_title(true);
        builder.build()?;
        crate::show_window_fallback(&app, WINDOW_LABEL);
    }
    state.project.lock().unwrap().enqueue(ids)?;
    if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
        let _ = window.unminimize();
        window.set_focus()?;
        // Initial requests stay queued until the new page has installed its listener.
        window.emit("project:samples-ready", ())?;
    }
    Ok(())
}
#[tauri::command]
pub fn project_take_samples(
    state: State<'_, AppState>,
    window: tauri::Window,
) -> Result<Vec<Vec<i64>>> {
    if window.label() != WINDOW_LABEL {
        return Err(invalid("only the project window can receive samples"));
    }
    Ok(state.project.lock().unwrap().take_samples())
}
pub fn release(app: &AppHandle) {
    if let Some(state) = app.try_state::<AppState>() {
        let mut transport = state.project.lock().unwrap();
        transport.revision += 1;
        transport.active = false;
        state.project_player.stop();
    }
}
#[tauri::command]
pub fn project_levels(state: State<'_, AppState>) -> Option<crate::meter::Levels> {
    state.project_player.levels()
}
#[tauri::command]
pub async fn project_sources(paths: Vec<String>) -> Result<Vec<ProjectSample>> {
    tauri::async_runtime::spawn_blocking(move || {
        if paths.len() > 32 {
            return Err(invalid("add up to 32 samples at once"));
        }
        paths
            .into_iter()
            .map(|path| {
                if !audio::is_audio_file(Path::new(&path)) {
                    return Err(invalid("drop audio files onto a track"));
                }
                let info = audio::probe(Path::new(&path))?;
                let name = Path::new(&path)
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                Ok(ProjectSample {
                    id: 0,
                    path,
                    name,
                    duration: info.duration.unwrap_or(0.0),
                })
            })
            .collect()
    })
    .await
    .map_err(|e| invalid(&e.to_string()))?
}

#[cfg(test)]
mod window_tests {
    use super::*;
    #[test]
    fn initial_and_later_requests_are_delivered_once_in_order() {
        let mut transport = Transport::default();
        transport.enqueue(vec![1, 2]).unwrap();
        transport.enqueue(vec![]).unwrap();
        transport.enqueue(vec![3]).unwrap();
        assert_eq!(transport.take_samples(), vec![vec![1, 2], vec![3]]);
        assert!(transport.take_samples().is_empty());
        transport.enqueue(vec![4]).unwrap();
        assert_eq!(transport.take_samples(), vec![vec![4]]);
        assert!(transport.enqueue(vec![0]).is_err());
        assert!(transport.enqueue(vec![1; 33]).is_err());
    }
}
