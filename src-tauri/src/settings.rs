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

/// Interface language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    En,
    Ru,
}

/// A theme's eight base colours for one mode, as `#rrggbb`; the UI derives the rest.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Palette {
    pub background: String,
    pub surface: String,
    pub text: String,
    pub muted: String,
    pub border: String,
    pub accent: String,
    pub danger: String,
    pub wave: String,
}

impl Palette {
    fn colors(&self) -> [&String; 8] {
        [&self.background, &self.surface, &self.text, &self.muted, &self.border, &self.accent, &self.danger, &self.wave]
    }

    fn is_valid(&self) -> bool {
        self.colors().iter().all(|c| is_hex_color(c))
    }
}

/// `#rgb` or `#rrggbb`.
fn is_hex_color(s: &str) -> bool {
    s.strip_prefix('#').is_some_and(|h| matches!(h.len(), 3 | 6) && h.chars().all(|c| c.is_ascii_hexdigit()))
}

/// A theme made by the user. Built-in themes live in the UI and are referred to by id.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CustomTheme {
    pub id: String,
    pub name: String,
    pub light: Palette,
    pub dark: Palette,
}

pub const BUILT_IN_THEMES: &[&str] = &["fonicca", "graphite", "midnight", "tape", "moss", "neon"];
const DEFAULT_THEME: &str = "fonicca";
const MAX_CUSTOM_THEMES: usize = 200;
const MAX_THEME_NAME: usize = 60;

impl CustomTheme {
    /// Trims the name; `None` for a theme whose colours are not valid.
    fn sanitized(mut self) -> Option<Self> {
        if self.id.trim().is_empty() || !self.light.is_valid() || !self.dark.is_valid() {
            return None;
        }
        self.name = self.name.trim().chars().take(MAX_THEME_NAME).collect();
        if self.name.is_empty() {
            self.name = "Untitled".into();
        }
        Some(self)
    }
}

/// How waveforms are drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WaveStyle {
    /// Filled min/max.
    Peaks,
    /// Min/max with the RMS level inside in a darker tone.
    PeaksRms,
    /// Rounded bars, mirrored around the centre.
    Bars,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WaveChannels {
    /// One lane with the channels mixed.
    Mix,
    /// One lane per channel.
    Split,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WaveScale {
    Linear,
    /// Logarithmic amplitude: quiet tails become visible.
    Db,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WaveColors {
    Theme,
    /// Low/mid/high energy as red/green/blue.
    Frequency,
}

/// Independent drawing options for an editor window.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WaveformSettings {
    pub wave_style: WaveStyle,
    pub wave_channels: WaveChannels,
    pub wave_scale: WaveScale,
    pub wave_colors: WaveColors,
}

impl Default for WaveformSettings {
    fn default() -> Self {
        Self { wave_style: WaveStyle::PeaksRms, wave_channels: WaveChannels::Split,
            wave_scale: WaveScale::Linear, wave_colors: WaveColors::Theme }
    }
}

/// Missing or unknown fields fall back to defaults, so older and newer files both load.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Light, dark or following the system.
    pub theme: Theme,
    /// Id of the colour theme: built-in or one of `custom_themes`.
    pub color_theme: String,
    pub custom_themes: Vec<CustomTheme>,
    pub language: Language,
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
    pub wave_style: WaveStyle,
    pub wave_channels: WaveChannels,
    pub wave_scale: WaveScale,
    pub wave_colors: WaveColors,
    pub editor_waveform: WaveformSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            color_theme: DEFAULT_THEME.into(),
            custom_themes: Vec::new(),
            language: Language::En,
            watch_folders: true,
            autoplay: true,
            volume: 1.0,
            analysis_threads: None,
            sync_enabled: false,
            project_bpm: None,
            project_key: None,
            wave_style: WaveStyle::PeaksRms,
            wave_channels: WaveChannels::Split,
            wave_scale: WaveScale::Linear,
            wave_colors: WaveColors::Theme,
            editor_waveform: WaveformSettings::default(),
        }
    }
}

impl Settings {
    /// Clamps values a hand-edited file or a buggy caller could put out of range.
    fn sanitized(mut self) -> Self {
        self.volume = if self.volume.is_finite() { self.volume.clamp(0.0, 1.0) } else { 1.0 };
        self.analysis_threads = self.analysis_threads.map(|n| n.clamp(1, cores()));
        self.project_bpm = self.project_bpm.filter(|b| b.is_finite()).map(|b| b.clamp(20.0, 400.0));
        // Drop broken or duplicate custom themes; an unknown selection falls back to the default.
        let mut ids = std::collections::HashSet::new();
        self.custom_themes = std::mem::take(&mut self.custom_themes)
            .into_iter()
            .filter_map(CustomTheme::sanitized)
            .filter(|t| !BUILT_IN_THEMES.contains(&t.id.as_str()) && ids.insert(t.id.clone()))
            .take(MAX_CUSTOM_THEMES)
            .collect();
        if !BUILT_IN_THEMES.contains(&self.color_theme.as_str()) && !ids.contains(&self.color_theme) {
            self.color_theme = DEFAULT_THEME.into();
        }
        self
    }
}

const THEME_FILE_FORMAT: &str = "fonicca-theme";

/// What a `.fonicca-theme.json` file holds: a theme without its local id.
#[derive(Serialize, Deserialize)]
struct ThemeFile {
    format: String,
    version: u32,
    name: String,
    light: Palette,
    dark: Palette,
}

pub fn write_theme_file(path: &Path, theme: &CustomTheme) -> Result<()> {
    let file = ThemeFile {
        format: THEME_FILE_FORMAT.into(),
        version: 1,
        name: theme.name.clone(),
        light: theme.light.clone(),
        dark: theme.dark.clone(),
    };
    std::fs::write(path, serde_json::to_vec_pretty(&file).expect("theme serialize"))?;
    Ok(())
}

/// Reads a shared theme; the caller gives it an id. Errors on anything that is not a valid theme.
pub fn read_theme_file(path: &Path) -> Result<CustomTheme> {
    let invalid = || crate::error::Error::Other("not a Fonicca theme".into());
    let file: ThemeFile = serde_json::from_slice(&std::fs::read(path)?).map_err(|_| invalid())?;
    if file.format != THEME_FILE_FORMAT {
        return Err(invalid());
    }
    CustomTheme { id: "imported".into(), name: file.name, light: file.light, dark: file.dark }.sanitized().ok_or_else(invalid)
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
            Ok(bytes) => serde_json::from_slice::<serde_json::Value>(&bytes).and_then(|mut data| {
                // Before profiles were split, these four options were shared.
                // Copy them only once; subsequent loads preserve independent values.
                if let Some(object) = data.as_object_mut() {
                    if !object.contains_key("editorWaveform") {
                        let options: serde_json::Map<String, serde_json::Value> =
                            ["waveStyle", "waveChannels", "waveScale", "waveColors"].into_iter()
                                .filter_map(|key| object.get(key).map(|value| (key.to_string(), value.clone())))
                                .collect();
                        object.insert("editorWaveform".into(), serde_json::Value::Object(options));
                    }
                }
                serde_json::from_value::<Settings>(data)
            }).unwrap_or_else(|e| {
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

    fn palette(accent: &str) -> Palette {
        let c = |s: &str| s.to_string();
        Palette {
            background: c("#000000"),
            surface: c("#111"),
            text: c("#ffffff"),
            muted: c("#999999"),
            border: c("#222222"),
            accent: c(accent),
            danger: c("#ff0000"),
            wave: c("#555555"),
        }
    }

    #[test]
    fn broken_custom_themes_are_dropped_and_selection_falls_back() {
        let good = CustomTheme { id: "mine".into(), name: "  Mine  ".into(), light: palette("#123456"), dark: palette("#abc") };
        let bad_color = CustomTheme { id: "bad".into(), light: palette("red"), ..good.clone() };
        let shadows_builtin = CustomTheme { id: "neon".into(), ..good.clone() };
        let s = Settings {
            color_theme: "bad".into(),
            custom_themes: vec![good.clone(), bad_color, shadows_builtin, good.clone()],
            ..Settings::default()
        }
        .sanitized();
        assert_eq!(s.custom_themes.len(), 1);
        assert_eq!(s.custom_themes[0].name, "Mine");
        assert_eq!(s.color_theme, DEFAULT_THEME);

        let kept = Settings { color_theme: "mine".into(), custom_themes: vec![good], ..Settings::default() }.sanitized();
        assert_eq!(kept.color_theme, "mine");
    }

    #[test]
    fn theme_files_round_trip_and_reject_other_json() {
        let dir = std::env::temp_dir().join(format!("fonicca-themes-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.json");
        let theme = CustomTheme { id: "x".into(), name: "Tape 2".into(), light: palette("#a8650b"), dark: palette("#e6a23c") };
        write_theme_file(&path, &theme).unwrap();
        let back = read_theme_file(&path).unwrap();
        assert_eq!((back.name.as_str(), &back.light, &back.dark), ("Tape 2", &theme.light, &theme.dark));

        std::fs::write(&path, r#"{"format": "something-else", "version": 1}"#).unwrap();
        assert!(read_theme_file(&path).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn partial_and_out_of_range_files_load() {
        let dir = std::env::temp_dir().join(format!("fonicca-settings-{}", std::process::id()));
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

    #[test]
    fn legacy_waveform_options_migrate_to_independent_editor_settings() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, r#"{"waveStyle":"bars","waveChannels":"mix","waveScale":"db","waveColors":"frequency"}"#).unwrap();
        let store = SettingsStore::load(path.clone());
        let mut settings = store.get();
        assert_eq!(settings.editor_waveform, WaveformSettings {
            wave_style: WaveStyle::Bars, wave_channels: WaveChannels::Mix,
            wave_scale: WaveScale::Db, wave_colors: WaveColors::Frequency,
        });
        settings.editor_waveform.wave_style = WaveStyle::Peaks;
        settings.editor_waveform.wave_channels = WaveChannels::Split;
        store.replace(settings).unwrap();
        let restored = SettingsStore::load(path.clone());
        let mut settings = restored.get();
        assert_eq!(settings.wave_style, WaveStyle::Bars);
        assert_eq!(settings.wave_channels, WaveChannels::Mix);
        assert_eq!(settings.editor_waveform.wave_style, WaveStyle::Peaks);
        assert_eq!(settings.editor_waveform.wave_channels, WaveChannels::Split);
        settings.wave_colors = WaveColors::Theme;
        restored.replace(settings).unwrap();
        let settings = SettingsStore::load(path).get();
        assert_eq!(settings.wave_colors, WaveColors::Theme);
        assert_eq!(settings.editor_waveform.wave_colors, WaveColors::Frequency);
    }
}
