//! User preferences in `settings.json` next to the library database. Kept apart
//! from the database so resetting the library keeps them, and readable by hand.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::error::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    System,
    Light,
    Dark,
}

/// Missing or unknown fields fall back to defaults, so older and newer files both load.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub theme: Theme,
    pub watch_folders: bool,
    /// Play a sample when the focus moves to it with the arrow keys.
    pub autoplay: bool,
    /// Preview volume, 0..=1.
    pub volume: f32,
    /// Analysis threads; `None` picks half the cores.
    pub analysis_threads: Option<usize>,
    /// Preview loops in the project tempo and tonal samples in the project key.
    pub sync_enabled: bool,
    pub project_bpm: Option<f64>,
    /// Key name like "Am" or "F#".
    pub project_key: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            watch_folders: true,
            autoplay: true,
            volume: 1.0,
            analysis_threads: None,
            sync_enabled: false,
            project_bpm: None,
            project_key: None,
        }
    }
}

impl Settings {
    /// Clamps values a hand-edited file or a buggy caller could put out of range.
    fn sanitized(mut self) -> Self {
        self.volume = if self.volume.is_finite() { self.volume.clamp(0.0, 1.0) } else { 1.0 };
        self.analysis_threads = self.analysis_threads.map(|n| n.clamp(1, cores()));
        self.project_bpm = self.project_bpm.filter(|b| b.is_finite()).map(|b| b.clamp(20.0, 400.0));
        self
    }
}

pub fn cores() -> usize {
    std::thread::available_parallelism().map_or(2, |n| n.get())
}

/// Thread count for `analysis_threads: None`: leave half the cores for the UI,
/// playback, scanning and the user's DAW.
pub fn default_threads() -> usize {
    (cores() / 2).max(1)
}

pub struct SettingsStore {
    path: PathBuf,
    current: Mutex<Settings>,
}

impl SettingsStore {
    /// Loads the file; a missing or unreadable one gives defaults.
    pub fn load(path: PathBuf) -> Self {
        let current = match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice::<Settings>(&bytes).unwrap_or_else(|e| {
                eprintln!("ignoring invalid {}: {e}", path.display());
                Settings::default()
            }),
            Err(_) => Settings::default(),
        };
        Self { path, current: Mutex::new(current.sanitized()) }
    }

    pub fn get(&self) -> Settings {
        self.current.lock().unwrap().clone()
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Saves `next` and returns the previous settings so the caller can apply the difference.
    pub fn replace(&self, next: Settings) -> Result<(Settings, Settings)> {
        let next = next.sanitized();
        let mut current = self.current.lock().unwrap();
        // Through a temporary file, so a crash mid-write never leaves a truncated file.
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(&next).expect("settings serialize"))?;
        std::fs::rename(&tmp, &self.path)?;
        let prev = std::mem::replace(&mut *current, next.clone());
        Ok((prev, next))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_and_out_of_range_files_load() {
        let dir = std::env::temp_dir().join(format!("samplette-settings-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        std::fs::write(&path, r#"{"volume": 3.5, "analysisThreads": 100000, "unknown": 1}"#).unwrap();

        let store = SettingsStore::load(path.clone());
        let s = store.get();
        assert_eq!(s.volume, 1.0);
        assert_eq!(s.analysis_threads, Some(cores()));
        assert!(s.watch_folders && s.autoplay && s.theme == Theme::System);

        store.replace(Settings { theme: Theme::Dark, volume: 0.5, project_bpm: Some(1000.0), ..s }).unwrap();
        let reloaded = SettingsStore::load(path).get();
        assert_eq!((reloaded.theme, reloaded.volume, reloaded.project_bpm), (Theme::Dark, 0.5, Some(400.0)));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
