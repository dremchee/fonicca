//! Download of the optional sound model into the app data folder.

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::clap;
use crate::error::{Error, Result};

/// A lease covers the whole operation, including the transition from preparing
/// to inference. A stale UI snapshot cannot delete files while they are in use.
#[derive(Clone, Default)]
pub struct Operations(std::sync::Arc<std::sync::Mutex<std::collections::HashMap<&'static str, Operation>>>);
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Operation { Using, Downloading, Deleting }
pub struct Lease { operations: Operations, id: &'static str }
impl Operations {
    pub fn is_busy(&self) -> bool { !self.0.lock().unwrap().is_empty() }
    pub fn acquire(&self, id: &'static str, operation: Operation) -> Result<Lease> {
        let mut active = self.0.lock().unwrap();
        if active.contains_key(id) {
            return Err(Error::Other("model is in use or being downloaded; try again after it finishes".into()));
        }
        active.insert(id, operation);
        Ok(Lease { operations: self.clone(), id })
    }
    pub fn state(&self, id: &str) -> Option<Operation> { self.0.lock().unwrap().get(id).copied() }
}
impl Drop for Lease {
    fn drop(&mut self) { self.operations.0.lock().unwrap().remove(self.id); }
}
/// Only the named optional packs are supported by the generic settings actions.
#[derive(Clone, Copy)]
pub enum OptionalModel { Stem(crate::stems::Model), Apollo, BasicPitch }
impl OptionalModel {
    pub fn id(self) -> &'static str {
        match self { Self::Stem(model) => model.id(), Self::Apollo => "apollo", Self::BasicPitch => "basicPitch" }
    }
    pub fn ensure(self, root: &Path, cancel: &std::sync::atomic::AtomicBool, progress: impl Fn(u64)) -> Result<PathBuf> {
        match self {
            Self::Stem(model) => crate::stems::models::ensure_for(root, model, cancel, progress),
            Self::Apollo => crate::stems::models::ensure_apollo(root, cancel, progress),
            Self::BasicPitch => crate::stems::models::ensure_basic_pitch(root, cancel, progress),
        }
    }
    pub fn remove(self, root: &Path) -> Result<()> {
        match self {
            Self::Stem(model) => crate::stems::models::remove_pack(root, Some(model)),
            Self::Apollo => crate::stems::models::remove_pack(root, None),
            Self::BasicPitch => crate::stems::models::remove_basic_pitch(root),
        }
    }
}
pub fn optional_model(id: &str) -> Result<OptionalModel> {
    match id {
        "apollo" => Ok(OptionalModel::Apollo),
        "basicPitch" => Ok(OptionalModel::BasicPitch),
        _ => crate::stems::Model::ALL.into_iter().find(|m| m.id() == id)
            .map(OptionalModel::Stem).ok_or_else(|| Error::Other("unknown optional model".into())),
    }
}

pub const DOWNLOAD_EVENT: &str = "model-download";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub received: u64,
    pub total: u64,
    pub done: bool,
    pub error: Option<String>,
}

pub fn model_path(models_dir: &Path) -> PathBuf {
    models_dir.join(clap::MODEL_FILE)
}

/// Present and complete. The hash is checked once, at download time.
pub fn is_installed(models_dir: &Path) -> bool {
    std::fs::metadata(model_path(models_dir)).is_ok_and(|m| m.len() == clap::MODEL_SIZE)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Storage {
    pub(crate) id: &'static str,
    path: String,
    installed: bool,
    pub(crate) downloading: bool,
    pub(crate) busy: bool,
    size: u64,
    download_size: u64,
}

/// File lengths on disk, including partial downloads and notices. Do not follow
/// symlinks: they can point outside the model folder or create a cycle.
pub fn disk_size(path: &Path) -> u64 {
    let Ok(metadata) = std::fs::symlink_metadata(path) else {
        return 0;
    };
    if metadata.is_file() {
        return metadata.len();
    }
    if !metadata.is_dir() {
        return 0;
    }
    std::fs::read_dir(path).map_or(0, |entries| {
        entries.flatten().map(|entry| disk_size(&entry.path())).sum()
    })
}

pub fn storage(root: &Path, clap_downloading: bool, downloading: impl Fn(crate::stems::Model) -> bool) -> Vec<Storage> {
    let clap = model_path(root);
    let mut entries = vec![Storage {
        id: "clap",
        path: clap.to_string_lossy().into_owned(),
        installed: is_installed(root),
        downloading: clap_downloading,
        busy: clap_downloading,
        size: disk_size(&clap) + disk_size(&clap.with_extension("part")),
        download_size: clap::MODEL_SIZE,
    }];
    entries.extend(crate::stems::Model::ALL.map(|model| {
        let dir = crate::stems::models::directory_for(root, model);
        Storage {
            id: model.id(), path: dir.to_string_lossy().into_owned(),
            installed: crate::stems::models::installed_for(root, model),
            downloading: downloading(model), busy: false, size: disk_size(&dir),
            download_size: crate::stems::models::total(model),
        }
    }));
    // Whole-track restoration uses a separate Apollo model pack.
    let apollo_dir = root.join(crate::stems::models::APOLLO_DIRECTORY);
    let apollo_size = disk_size(&apollo_dir);
    {
        const APOLLO_SIZE: u64 = crate::stems::models::APOLLO_TOTAL;
        entries.push(Storage {
            id: "apollo",
            path: apollo_dir.to_string_lossy().into_owned(),
            installed: std::fs::metadata(apollo_dir.join(crate::stems::models::APOLLO_FILE))
                .is_ok_and(|m| m.len() == APOLLO_SIZE),
            downloading: false,
            busy: false,
            size: apollo_size,
            download_size: APOLLO_SIZE,
        });
    }
    let pitch = crate::stems::models::basic_pitch_directory(root);
    entries.push(Storage {
        id: "basicPitch", path: pitch.to_string_lossy().into_owned(),
        installed: crate::stems::models::basic_pitch_installed(root),
        downloading: false, busy: false, size: disk_size(&pitch),
        download_size: crate::stems::models::BASIC_PITCH_TOTAL,
    });
    entries
}

/// Downloads the pinned model to a temporary file, verifies size and SHA-256,
/// then moves it into place, so a partial download never looks installed.
pub fn download(models_dir: &Path, on_progress: impl Fn(DownloadProgress)) -> Result<PathBuf> {
    std::fs::create_dir_all(models_dir)?;
    let target = model_path(models_dir);
    let partial = target.with_extension("part");

    let response = ureq::get(clap::MODEL_URL).call().map_err(|e| Error::Other(format!("download failed: {e}")))?;
    let total = response
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok())
        .unwrap_or(clap::MODEL_SIZE);
    let mut reader = response.into_body().into_reader();
    let mut file = File::create(&partial)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 256 * 1024];
    let mut received = 0u64;
    let mut last_report = 0u64;
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])?;
        hasher.update(&buf[..n]);
        received += n as u64;
        if received - last_report >= 512 * 1024 {
            last_report = received;
            on_progress(DownloadProgress { received, total, done: false, error: None });
        }
    }
    file.sync_all()?;
    drop(file);

    let hash: String = hasher.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if received != clap::MODEL_SIZE || hash != clap::MODEL_SHA256 {
        let _ = std::fs::remove_file(&partial);
        return Err(Error::Other("downloaded model is corrupt (checksum mismatch)".into()));
    }
    std::fs::rename(&partial, &target)?;
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_leases_prevent_download_delete_and_inference_races() {
        let operations = Operations::default();
        let using = operations.acquire("htdemucs", Operation::Using).unwrap();
        assert!(operations.acquire("htdemucs", Operation::Deleting).is_err());
        assert!(operations.acquire("htdemucs", Operation::Downloading).is_err());
        let other = operations.acquire("apollo", Operation::Downloading).unwrap();
        drop(using);
        let delete = operations.acquire("htdemucs", Operation::Deleting).unwrap();
        assert!(operations.acquire("htdemucs", Operation::Using).is_err());
        drop(delete);
        drop(other);
        assert!(operations.state("htdemucs").is_none());
        assert!(operations.state("apollo").is_none());
        for id in ["../htdemucs", "/tmp/model", "unknown", "clap"] { assert!(optional_model(id).is_err()); }
        assert!(matches!(optional_model("htdemucs").unwrap(), OptionalModel::Stem(_)));
        assert!(matches!(optional_model("apollo").unwrap(), OptionalModel::Apollo));
        assert!(matches!(optional_model("basicPitch").unwrap(), OptionalModel::BasicPitch));
    }
    #[test]
    fn storage_counts_partial_downloads_without_marking_them_installed() {
        let root = tempfile::tempdir().unwrap();
        let clap = model_path(root.path());
        std::fs::write(clap.with_extension("part"), [0; 7]).unwrap();
        let spleeter = crate::stems::models::directory(root.path());
        std::fs::create_dir_all(&spleeter).unwrap();
        std::fs::write(spleeter.join("vocals.fp16.onnx"), [0; 11]).unwrap();
        std::fs::write(spleeter.join("drums.fp16.part"), [0; 13]).unwrap();
        std::fs::write(spleeter.join("NOTICE.txt"), [0; 3]).unwrap();
        let models = storage(root.path(), true, |_| false);
        assert_eq!(models[0].size, 7);
        assert!(models[0].downloading);
        assert!(!models[0].installed);
        assert_eq!(models[1].size, 27);
        assert!(!models[1].installed);
        assert!(!models[1].downloading);
        assert_eq!(disk_size(root.path()), 34);

        std::fs::remove_file(clap.with_extension("part")).unwrap();
        File::create(&clap).unwrap().set_len(clap::MODEL_SIZE).unwrap();
        let models = storage(root.path(), false, |_| false);
        assert!(models[0].installed);
        assert_eq!(models[0].size, clap::MODEL_SIZE);
        assert_eq!(disk_size(root.path()), clap::MODEL_SIZE + 27);
    }

    #[test]
    fn missing_models_have_zero_disk_usage() {
        let root = tempfile::tempdir().unwrap();
        let models = storage(root.path(), false, |_| false);
        assert_eq!(models.len(), crate::stems::Model::ALL.len() + 3);
        assert!(models.iter().all(|m| !m.installed && m.size == 0 && m.download_size > 0));
    }

    #[test]
    fn downloaded_apollo_is_visible_as_a_comparison_model() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("apollo-v1");
        std::fs::create_dir(&dir).unwrap();
        let weights = dir.join("pytorch_model.bin");
        std::fs::write(&weights, [0; 7]).unwrap();
        let inventory = storage(root.path(), false, |_| false);
        let model = inventory.iter().find(|m| m.id == "apollo").expect("downloaded Apollo should be listed");
        assert!(!model.installed);
        assert_eq!(model.size, 7);
        assert_eq!(model.path, dir.to_string_lossy());
        File::create(&weights).unwrap().set_len(66_541_845).unwrap();
        let inventory = storage(root.path(), false, |_| false);
        let model = inventory.iter().find(|m| m.id == "apollo").unwrap();
        assert!(model.installed);
        assert_eq!(model.download_size, 66_541_845);
        assert!(!model.downloading);
        std::fs::remove_file(weights).unwrap();
        assert!(!storage(root.path(), false, |_| false).iter().find(|m| m.id == "apollo").unwrap().installed);
    }

    #[cfg(unix)]
    #[test]
    fn disk_usage_does_not_follow_symlinks() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("unrelated"), [0; 31]).unwrap();
        std::os::unix::fs::symlink(outside.path(), root.path().join("outside")).unwrap();
        std::os::unix::fs::symlink(root.path(), root.path().join("cycle")).unwrap();
        assert_eq!(disk_size(root.path()), 0);
    }

    /// Network test: `cargo test -- --ignored --nocapture downloads_and_verifies`
    #[test]
    #[ignore]
    fn downloads_and_verifies() {
        let dir = tempfile::tempdir().unwrap();
        let reports = std::cell::Cell::new(0);
        let path = super::download(dir.path(), |_| reports.set(reports.get() + 1)).unwrap();
        assert!(super::is_installed(dir.path()));
        assert_eq!(std::fs::metadata(path).unwrap().len(), crate::clap::MODEL_SIZE);
        assert!(reports.get() > 10);
        assert!(!dir.path().join("clap-htsat-unfused-audio-int8.part").exists());
    }
}
