//! Download of the optional sound model into the app data folder.

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::clap;
use crate::error::{Error, Result};

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
