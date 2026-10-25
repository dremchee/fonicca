//! Pinned optional Spleeter and SCNet model packs. Downloaded separately from the app.
use super::{Model, check, failure};
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
        name: "vocals.fp16.onnx",
        size: 19714141,
        hash: "db47148ab1c52709ce694893f532c91abfe3edc4d46238939570e036a22878ca",
    },
    Artifact {
        name: "drums.fp16.onnx",
        size: 19714140,
        hash: "7ae4002e5633634674f74dc3356d5875b0da894d59ce0f60e844bb8f9cb8aa92",
    },
    Artifact {
        name: "bass.fp16.onnx",
        size: 19714139,
        hash: "ba4c4949a27222492cca49859901a873b4b71461dc48c7c5a51f93d31eb11f55",
    },
    Artifact {
        name: "other.fp16.onnx",
        size: 19714140,
        hash: "3cc59116cb7195946ab9596d8ca25984d09c0f8a70db8cf85d063132f97bc61d",
    },
];
pub const SCNET_TOTAL: u64 = 45_082_486;
const SCNET_REVISION: &str = "95f22d9e68f6cab78aef358f1a0de248d4edf805";
const SCNET_FILES: [Artifact; 1] = [Artifact {
    name: "scnet-large.int8.onnx",
    size: SCNET_TOTAL,
    hash: "b2dc586a1e0e6c0afe4915e9057ea29111397b7bbf589edc3d85de91de79cd72",
}];
pub const XL_TOTAL: u64 = 221_482_626;
const XL_REVISION: &str = "30b749fd691312b1e4b3f8fe79df0b51cde836cd";
const XL_FILES: [Artifact; 1] = [Artifact {
    name: "scnet_xl_wide_v5_fp32.onnx",
    size: XL_TOTAL,
    hash: "6e2e46b669fa368247afbe867aaa61317425f3f0568309fd6e0c79671c39f0b6",
}];
pub const DEMUCS_TOTAL: u64 = 316_446_953;
const DEMUCS_REVISION: &str = "d54ed9eb60e258ea82131c6ee14578628816456a";
const DEMUCS_FILES: [Artifact; 1] = [Artifact {
    name: "htdemucs.onnx",
    size: DEMUCS_TOTAL,
    hash: "68d0bf16428ef66e692cdff8a9ccf28f1ef3f69440d57e58605a4cc55fcc5e74",
}];
fn files(model: Model) -> &'static [Artifact] {
    match model {
        Model::Spleeter => &FILES,
        Model::ScnetLarge => &SCNET_FILES,
        Model::ScnetXlIhf => &XL_FILES,
        Model::Htdemucs => &DEMUCS_FILES,
    }
}
#[cfg(test)]
pub fn directory(root: &Path) -> PathBuf {
    directory_for(root, Model::Spleeter)
}
pub fn directory_for(root: &Path, model: Model) -> PathBuf {
    root.join(match model {
        Model::Spleeter => "spleeter4-v1",
        Model::ScnetLarge => "scnet-large-v1",
        Model::ScnetXlIhf => "scnet-xl-ihf-v1",
        Model::Htdemucs => "htdemucs-v1",
    })
}
#[cfg(test)]
pub fn installed(root: &Path) -> bool {
    installed_for(root, Model::Spleeter)
}
pub fn installed_for(root: &Path, model: Model) -> bool {
    files(model).iter().all(|f| {
        std::fs::metadata(directory_for(root, model).join(f.name)).is_ok_and(|m| m.len() == f.size)
    })
}
pub fn total(model: Model) -> u64 {
    match model {
        Model::Spleeter => TOTAL,
        Model::ScnetLarge => SCNET_TOTAL,
        Model::ScnetXlIhf => XL_TOTAL,
        Model::Htdemucs => DEMUCS_TOTAL,
    }
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
#[cfg(test)]
pub fn ensure(root: &Path, cancel: &AtomicBool, progress: impl Fn(u64)) -> Result<PathBuf> {
    ensure_for(root, Model::Spleeter, cancel, progress)
}
pub fn ensure_for(
    root: &Path,
    model: Model,
    cancel: &AtomicBool,
    progress: impl Fn(u64),
) -> Result<PathBuf> {
    let dir = directory_for(root, model);
    std::fs::create_dir_all(&dir)?;
    if model == Model::Spleeter {
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
    } else if model == Model::Htdemucs {
        std::fs::write(
            dir.join("LICENSE-code-MIT.txt"),
            include_str!("../../../docs/licenses/demucs-MIT.txt"),
        )?;
        std::fs::write(
            dir.join("NOTICE.txt"),
            include_str!("../../../docs/licenses/htdemucs-NOTICE.txt"),
        )?;
    } else if model == Model::ScnetXlIhf {
        std::fs::write(
            dir.join("LICENSE-code-MIT.txt"),
            include_str!("../../../docs/licenses/scnet-MIT.txt"),
        )?;
        std::fs::write(
            dir.join("NOTICE.txt"),
            include_str!("../../../docs/licenses/scnet-xl-NOTICE.txt"),
        )?;
    } else {
        std::fs::write(
            dir.join("LICENSE-MIT.txt"),
            include_str!("../../../docs/licenses/scnet-MIT.txt"),
        )?;
        std::fs::write(
            dir.join("NOTICE.txt"),
            include_str!("../../../docs/licenses/scnet-NOTICE.txt"),
        )?;
    }
    ensure_pack(
        &dir,
        files(model),
        |name| match model {
            Model::Spleeter => format!(
                "https://huggingface.co/Best-Practice/spleeter-4stems-onnx/resolve/{REVISION}/{name}"
            ),
            Model::ScnetLarge => format!(
                "https://huggingface.co/audiojs/scnet-large/resolve/{SCNET_REVISION}/{name}"
            ),
            Model::ScnetXlIhf => {
                format!("https://huggingface.co/Ryan5453/unblend/resolve/{XL_REVISION}/{name}")
            }
            Model::Htdemucs => format!(
                "https://huggingface.co/StemSplitio/htdemucs-onnx/resolve/{DEMUCS_REVISION}/{name}"
            ),
        },
        cancel,
        progress,
    )
}

pub const BASIC_PITCH_TOTAL: u64 = 230_444;
pub const BASIC_PITCH_FILE: &str = "nmp.onnx";
const BASIC_PITCH_REVISION: &str = "fa5997af0a8210982619003269994a1be25eddf3";
const BASIC_PITCH_FILES: [Artifact; 1] = [Artifact {
    name: BASIC_PITCH_FILE,
    size: BASIC_PITCH_TOTAL,
    hash: "2c3c1d144bfa61ad236e92e169c13535c880469a12a047d4e73451f2c059a0ec",
}];
pub fn basic_pitch_directory(root: &Path) -> PathBuf {
    root.join("basic-pitch-v1")
}
pub fn basic_pitch_installed(root: &Path) -> bool {
    std::fs::metadata(basic_pitch_directory(root).join(BASIC_PITCH_FILE))
        .is_ok_and(|m| m.len() == BASIC_PITCH_TOTAL)
}
pub fn ensure_basic_pitch(
    root: &Path,
    cancel: &AtomicBool,
    progress: impl Fn(u64),
) -> Result<PathBuf> {
    let dir = basic_pitch_directory(root);
    std::fs::create_dir_all(&dir)?;
    std::fs::write(
        dir.join("LICENSE.txt"),
        include_str!("../../../docs/licenses/basic-pitch-Apache-2.0.txt"),
    )?;
    std::fs::write(
        dir.join("NOTICE.txt"),
        include_str!("../../../docs/licenses/basic-pitch-NOTICE.txt"),
    )?;
    ensure_pack(
        &dir,
        &BASIC_PITCH_FILES,
        |name| {
            format!(
                "https://raw.githubusercontent.com/spotify/basic-pitch/{BASIC_PITCH_REVISION}/basic_pitch/saved_models/icassp_2022/{name}"
            )
        },
        cancel,
        progress,
    )
}
pub fn remove_basic_pitch(root: &Path) -> Result<()> {
    let dir = basic_pitch_directory(root);
    if std::fs::symlink_metadata(&dir).is_ok_and(|m| m.file_type().is_symlink()) {
        std::fs::remove_file(dir)?;
        return Ok(());
    }
    for name in [BASIC_PITCH_FILE, "nmp.part", "LICENSE.txt", "NOTICE.txt"] {
        remove_file(&dir.join(name))?;
    }
    match std::fs::remove_dir(&dir) {
        Ok(()) => Ok(()),
        Err(e)
            if matches!(
                e.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::DirectoryNotEmpty
            ) =>
        {
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}

pub const APOLLO_TOTAL: u64 = 66_541_845;
const APOLLO_FILES: [Artifact; 1] = [Artifact {
    name: "pytorch_model.bin",
    size: APOLLO_TOTAL,
    hash: "99d9af7f1ff20e63c393035513a655392818d66b4d7fc23d658175c1f15e8d76",
}];
pub fn ensure_apollo(root: &Path, cancel: &AtomicBool, progress: impl Fn(u64)) -> Result<PathBuf> {
    let dir = root.join("apollo-v1");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(
        dir.join("LICENSE.txt"),
        include_str!("../../../docs/licenses/apollo-BY-SA-4.0.txt"),
    )?;
    std::fs::write(
        dir.join("NOTICE.txt"),
        include_str!("../../../docs/licenses/apollo-NOTICE.txt"),
    )?;
    let dir = ensure_pack(
        &dir,
        &APOLLO_FILES,
        |name| {
            format!(
                "https://huggingface.co/JusperLee/Apollo/resolve/c68bd80fdd9c0d93d2f4a833cb154624f660a561/{name}"
            )
        },
        cancel,
        progress,
    )?;
    let graph = dir.join("apollo.onnx");
    if std::fs::read(&graph).ok().as_deref() != Some(crate::apollo::GRAPH) {
        let partial = graph.with_extension("part");
        std::fs::write(&partial, crate::apollo::GRAPH)?;
        std::fs::rename(&partial, &graph)?;
    }
    Ok(dir)
}
pub fn apollo_installed(root: &Path) -> bool {
    std::fs::metadata(root.join("apollo-v1/pytorch_model.bin"))
        .is_ok_and(|m| m.len() == APOLLO_TOTAL)
}

/// Only remove the pack's own artifacts. Audio or other user-added files in the
/// directory survive; deleting an unknown ID never turns into an arbitrary path.
pub fn remove_pack(root: &Path, model: Option<Model>) -> Result<()> {
    let dir = model.map_or_else(|| root.join("apollo-v1"), |m| directory_for(root, m));
    if std::fs::symlink_metadata(&dir).is_ok_and(|m| m.file_type().is_symlink()) {
        std::fs::remove_file(&dir)?;
        return Ok(());
    }
    if model.is_none() {
        remove_file(&dir.join("apollo.onnx"))?;
        remove_file(&dir.join("apollo.part"))?;
    }
    let artifacts = model.map_or(APOLLO_FILES.as_slice(), files);
    for artifact in artifacts {
        remove_file(&dir.join(artifact.name))?;
        remove_file(&dir.join(artifact.name).with_extension("part"))?;
    }
    for notice in [
        "LICENSE.txt",
        "LICENSE-MIT.txt",
        "LICENSE-code-MIT.txt",
        "LICENSE-Apache-2.0.txt",
        "NOTICE.txt",
        "provenance.json",
    ] {
        remove_file(&dir.join(notice))?;
    }
    match std::fs::remove_dir(&dir) {
        Ok(()) => Ok(()),
        Err(e)
            if matches!(
                e.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::DirectoryNotEmpty
            ) =>
        {
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}
fn remove_file(path: &Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}
fn ensure_pack(
    dir: &Path,
    artifacts: &[Artifact],
    url: impl Fn(&str) -> String,
    cancel: &AtomicBool,
    progress: impl Fn(u64),
) -> Result<PathBuf> {
    let mut received = 0;
    for f in artifacts {
        check(cancel)?;
        let target = dir.join(f.name);
        if valid(&target, f) {
            received += f.size;
            progress(received);
            continue;
        }
        let partial = target.with_extension("part");
        let result = (|| {
            let url = url(f.name);
            let response = ureq::get(&url)
                .config()
                .timeout_global(Some(std::time::Duration::from_secs(600)))
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
            if !valid(&partial, f) {
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
    Ok(dir.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deleting_packs_preserves_audio_other_models_and_unrelated_files() {
        let root = tempfile::tempdir().unwrap();
        for model in Model::ALL.into_iter().map(Some).chain([None]) {
            let dir = model.map_or_else(
                || root.path().join("apollo-v1"),
                |m| directory_for(root.path(), m),
            );
            std::fs::create_dir_all(&dir).unwrap();
            let artifacts = model.map_or(APOLLO_FILES.as_slice(), files);
            for artifact in artifacts {
                std::fs::write(dir.join(artifact.name), b"model").unwrap();
                std::fs::write(dir.join(artifact.name).with_extension("part"), b"partial").unwrap();
            }
            std::fs::write(dir.join("NOTICE.txt"), b"notice").unwrap();
            std::fs::write(dir.join("vocals.wav"), b"keep this audio").unwrap();
            std::fs::write(root.path().join("other-model.onnx"), b"keep other model").unwrap();
            remove_pack(root.path(), model).unwrap();
            for artifact in artifacts {
                assert!(!dir.join(artifact.name).exists());
                assert!(!dir.join(artifact.name).with_extension("part").exists());
            }
            assert_eq!(
                std::fs::read(dir.join("vocals.wav")).unwrap(),
                b"keep this audio"
            );
            assert_eq!(
                std::fs::read(root.path().join("other-model.onnx")).unwrap(),
                b"keep other model"
            );
            remove_pack(root.path(), model).unwrap();
        }
    }
    #[cfg(unix)]
    #[test]
    fn deleting_a_linked_pack_does_not_delete_its_external_target() {
        let root = tempfile::tempdir().unwrap();
        let external = tempfile::tempdir().unwrap();
        std::fs::write(external.path().join("pytorch_model.bin"), b"external").unwrap();
        std::os::unix::fs::symlink(external.path(), root.path().join("apollo-v1")).unwrap();
        remove_pack(root.path(), None).unwrap();
        assert!(!root.path().join("apollo-v1").exists());
        assert_eq!(
            std::fs::read(external.path().join("pytorch_model.bin")).unwrap(),
            b"external"
        );
    }
    #[test]
    fn packs_have_separate_storage_and_complete_manifests() {
        let root = tempfile::tempdir().unwrap();
        assert_ne!(
            directory_for(root.path(), Model::Spleeter),
            directory_for(root.path(), Model::ScnetLarge)
        );
        for model in Model::ALL {
            let dir = directory_for(root.path(), model);
            std::fs::create_dir_all(&dir).unwrap();
            assert!(!installed_for(root.path(), model));
            for artifact in files(model) {
                std::fs::File::create(dir.join(artifact.name))
                    .unwrap()
                    .set_len(artifact.size)
                    .unwrap();
            }
            assert!(installed_for(root.path(), model));
            assert_eq!(
                files(model).iter().map(|f| f.size).sum::<u64>(),
                total(model)
            );
            std::fs::remove_file(dir.join(files(model)[0].name)).unwrap();
            assert!(!installed_for(root.path(), model));
        }
        assert_eq!(
            serde_json::to_string(&Model::ScnetLarge).unwrap(),
            "\"scnetLarge\""
        );
        assert!(serde_json::from_str::<Model>("\"unknown\"").is_err());
    }
    #[test]
    #[ignore = "requires SCNET_XL_MODELS; checks the actual XL pack and install notices"]
    fn xl_manifest_and_installation_are_verified() {
        let cache = PathBuf::from(std::env::var("SCNET_XL_MODELS").unwrap());
        let root = tempfile::tempdir().unwrap();
        let dir = directory_for(root.path(), Model::ScnetXlIhf);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::hard_link(cache.join(XL_FILES[0].name), dir.join(XL_FILES[0].name)).unwrap();
        let result = ensure_for(
            root.path(),
            Model::ScnetXlIhf,
            &AtomicBool::new(false),
            |_| {},
        )
        .unwrap();
        assert_eq!(result, dir);
        assert!(installed_for(root.path(), Model::ScnetXlIhf));
        assert!(!installed_for(root.path(), Model::ScnetLarge));
        assert!(dir.join("NOTICE.txt").is_file());
        assert!(dir.join("LICENSE-code-MIT.txt").is_file());
    }
    #[test]
    #[ignore = "requires DEMUCS_MODELS; verifies installed HTDemucs weights and notices"]
    fn htdemucs_manifest_and_installation_are_verified() {
        let dir = PathBuf::from(std::env::var("DEMUCS_MODELS").unwrap());
        assert!(valid(&dir.join(DEMUCS_FILES[0].name), &DEMUCS_FILES[0]));
        let result = ensure_for(
            dir.parent().unwrap(),
            Model::Htdemucs,
            &AtomicBool::new(false),
            |_| {},
        )
        .unwrap();
        assert_eq!(result, dir);
        assert!(installed_for(dir.parent().unwrap(), Model::Htdemucs));
        assert!(dir.join("LICENSE-code-MIT.txt").is_file());
        assert!(
            std::fs::read_to_string(dir.join("NOTICE.txt"))
                .unwrap()
                .contains("scientific purposes")
        );
    }
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
    #[ignore = "downloads the pinned Basic Pitch model; requires network"]
    fn basic_pitch_download_delete_and_redownload_preserves_audio() {
        let root = tempfile::tempdir().unwrap();
        let dir = ensure_basic_pitch(root.path(), &AtomicBool::new(false), |_| {}).unwrap();
        assert!(basic_pitch_installed(root.path()));
        assert!(valid(&dir.join(BASIC_PITCH_FILE), &BASIC_PITCH_FILES[0]));
        assert!(dir.join("LICENSE.txt").is_file());
        assert!(dir.join("NOTICE.txt").is_file());
        std::fs::write(dir.join("keep.wav"), b"original audio").unwrap();
        remove_basic_pitch(root.path()).unwrap();
        assert!(!basic_pitch_installed(root.path()));
        assert_eq!(std::fs::read(dir.join("keep.wav")).unwrap(), b"original audio");
        ensure_basic_pitch(root.path(), &AtomicBool::new(false), |_| {}).unwrap();
        assert!(valid(&dir.join(BASIC_PITCH_FILE), &BASIC_PITCH_FILES[0]));
        assert_eq!(std::fs::read(dir.join("keep.wav")).unwrap(), b"original audio");
    }

    #[test]
    #[ignore = "downloads the pinned Apollo checkpoint into a temporary directory; requires network"]
    fn downloads_and_verifies_apollo_with_application_downloader() {
        let root = tempfile::tempdir().unwrap();
        let bytes = std::cell::Cell::new(0);
        let dir = ensure_apollo(root.path(), &AtomicBool::new(false), |n| bytes.set(n)).unwrap();
        assert_eq!(bytes.get(), APOLLO_TOTAL);
        assert!(valid(&dir.join(APOLLO_FILES[0].name), &APOLLO_FILES[0]));
        assert!(dir.join("LICENSE.txt").is_file());
        assert!(dir.join("NOTICE.txt").is_file());
        remove_pack(root.path(), None).unwrap();
        assert!(!dir.exists());
    }
    #[test]
    #[ignore = "downloads the pinned SCNet-large model; requires network"]
    fn downloads_and_verifies_scnet_with_application_downloader() {
        let root = tempfile::tempdir().unwrap();
        let received = std::cell::Cell::new(0);
        let dir = ensure_for(
            root.path(),
            Model::ScnetLarge,
            &AtomicBool::new(false),
            |n| received.set(n),
        )
        .unwrap();
        assert_eq!(received.get(), SCNET_TOTAL);
        assert!(installed_for(root.path(), Model::ScnetLarge));
        assert!(!installed_for(root.path(), Model::Spleeter));
        assert!(dir.join("LICENSE-MIT.txt").is_file());
        assert!(dir.join("NOTICE.txt").is_file());
        assert!(valid(&dir.join(SCNET_FILES[0].name), &SCNET_FILES[0]));
    }
    #[test]
    #[ignore = "downloads one pinned model; requires STEM_MODELS and network"]
    fn downloads_and_verifies_a_model_with_the_application_downloader() {
        let cache = PathBuf::from(std::env::var("STEM_MODELS").unwrap());
        let root = tempfile::tempdir().unwrap();
        let dir = directory(root.path());
        std::fs::create_dir_all(&dir).unwrap();
        for f in &FILES[1..] {
            let name = f.name;
            std::fs::hard_link(cache.join(&name), dir.join(&name)).unwrap();
        }
        let progress = std::cell::Cell::new(0);
        let result = ensure(root.path(), &AtomicBool::new(false), |n| progress.set(n)).unwrap();
        assert!(installed(root.path()));
        assert_eq!(result, dir);
        assert_eq!(progress.get(), TOTAL);
        assert!(FILES.iter().all(|f| valid(&dir.join(f.name), f)));
    }
}
