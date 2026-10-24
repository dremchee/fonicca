//! Pinned optional Spleeter model pack. Downloaded separately from the app.
use super::{check, failure};
use crate::error::Result;
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};
pub const REVISION: &str = "486643021f18a4d057d282fff1d11b89a30e9887";
pub const TOTAL: u64 = 78856560;
struct Artifact {
    name: &'static str,
    size: u64,
    hash: &'static str,
}
const FILES: [Artifact; 4] = [
    Artifact {
        name: "vocals",
        size: 19714141,
        hash: "db47148ab1c52709ce694893f532c91abfe3edc4d46238939570e036a22878ca",
    },
    Artifact {
        name: "drums",
        size: 19714140,
        hash: "7ae4002e5633634674f74dc3356d5875b0da894d59ce0f60e844bb8f9cb8aa92",
    },
    Artifact {
        name: "bass",
        size: 19714139,
        hash: "ba4c4949a27222492cca49859901a873b4b71461dc48c7c5a51f93d31eb11f55",
    },
    Artifact {
        name: "other",
        size: 19714140,
        hash: "3cc59116cb7195946ab9596d8ca25984d09c0f8a70db8cf85d063132f97bc61d",
    },
];
pub fn directory(root: &Path) -> PathBuf {
    root.join("spleeter4-v1")
}
pub fn installed(root: &Path) -> bool {
    FILES.iter().all(|f| {
        std::fs::metadata(directory(root).join(format!("{}.fp16.onnx", f.name)))
            .is_ok_and(|m| m.len() == f.size)
    })
}
fn valid(path: &Path, f: &Artifact) -> bool {
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    if !file.metadata().is_ok_and(|m| m.len() == f.size) {
        return false;
    }
    let mut hash = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        match file.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => hash.update(&buf[..n]),
            Err(_) => return false,
        }
    }
    hash.finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>()
        == f.hash
}
pub fn ensure(root: &Path, cancel: &AtomicBool, progress: impl Fn(u64)) -> Result<PathBuf> {
    let dir = directory(root);
    std::fs::create_dir_all(&dir)?;
    std::fs::write(
        dir.join("LICENSE-MIT.txt"),
        include_str!("../../../docs/licenses/spleeter-MIT.txt"),
    )?;
    std::fs::write(
        dir.join("LICENSE-Apache-2.0.txt"),
        include_str!("../../../docs/licenses/spleeter-onnx-Apache-2.0.txt"),
    )?;
    std::fs::write(
        dir.join("NOTICE.txt"),
        include_str!("../../../docs/licenses/spleeter-onnx-NOTICE.txt"),
    )?;
    let mut received = 0;
    for f in FILES {
        check(cancel)?;
        let target = dir.join(format!("{}.fp16.onnx", f.name));
        if valid(&target, &f) {
            received += f.size;
            progress(received);
            continue;
        }
        let partial = target.with_extension("part");
        let result = (|| {
            let url = format!(
                "https://huggingface.co/Best-Practice/spleeter-4stems-onnx/resolve/{REVISION}/{}.fp16.onnx",
                f.name
            );
            let response = ureq::get(&url)
                .config()
                .timeout_global(Some(std::time::Duration::from_secs(180)))
                .build()
                .call()
                .map_err(failure)?;
            let mut reader = response.into_body().into_reader();
            let mut file = std::fs::File::create(&partial)?;
            let mut bytes = 0;
            let mut buf = [0u8; 262144];
            loop {
                check(cancel)?;
                let n = reader.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                bytes += n as u64;
                if bytes > f.size {
                    return Err(failure("model download exceeds expected size"));
                }
                file.write_all(&buf[..n])?;
                progress(received + bytes);
            }
            file.sync_all()?;
            drop(file);
            if !valid(&partial, &f) {
                return Err(failure("model checksum mismatch"));
            }
            std::fs::rename(&partial, &target)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&partial);
        }
        result?;
        received += f.size;
    }
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn checksum_rejects_corrupt_downloads_of_the_expected_size() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("model.onnx");
        let model = Artifact {
            name: "test",
            size: 3,
            hash: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        };
        std::fs::write(&path, b"abc").unwrap();
        assert!(valid(&path, &model));
        std::fs::write(&path, b"abd").unwrap();
        assert!(!valid(&path, &model));
        std::fs::write(&path, b"ab").unwrap();
        assert!(!valid(&path, &model));
    }
    #[test]
    #[ignore = "requires pinned downloaded weights in STEM_MODELS"]
    fn downloaded_model_manifest_is_verified() {
        let dir = PathBuf::from(std::env::var("STEM_MODELS").unwrap());
        let result = ensure(dir.parent().unwrap(), &AtomicBool::new(false), |_| {}).unwrap();
        assert_eq!(result, dir);
        assert!(installed(dir.parent().unwrap()));
        assert!(dir.join("NOTICE.txt").is_file());
    }
}

#[cfg(test)]
mod network_tests {
    use super::*;
    #[test]
    #[ignore = "downloads one pinned model; requires STEM_MODELS and network"]
    fn downloads_and_verifies_a_model_with_the_application_downloader() {
        let cache = PathBuf::from(std::env::var("STEM_MODELS").unwrap());
        let root = tempfile::tempdir().unwrap();
        let dir = directory(root.path());
        std::fs::create_dir_all(&dir).unwrap();
        for f in &FILES[1..] {
            let name = format!("{}.fp16.onnx", f.name);
            std::fs::hard_link(cache.join(&name), dir.join(&name)).unwrap();
        }
        let progress = std::cell::Cell::new(0);
        let result = ensure(root.path(), &AtomicBool::new(false), |n| progress.set(n)).unwrap();
        assert!(installed(root.path()));
        assert_eq!(result, dir);
        assert_eq!(progress.get(), TOTAL);
        assert!(
            FILES
                .iter()
                .all(|f| valid(&dir.join(format!("{}.fp16.onnx", f.name)), f))
        );
    }
}
