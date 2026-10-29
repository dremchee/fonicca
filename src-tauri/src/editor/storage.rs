//! Private immutable PCM storage. Small files stay in RAM; larger files spill
//! during decoding, so no complete RAM buffer is needed before mapping them.
use crate::{
    audio::{self},
    error::{Error, Result},
};
use memmap2::Mmap;
use std::{
    fs::File,
    io::{BufWriter, Write},
    path::Path,
    sync::Arc,
};

pub(super) const RAM_CEILING: usize = 32 * 1024 * 1024;

pub(super) struct DiskPcm {
    // Fields drop in declaration order: unmap before closing the private file.
    map: Mmap,
    _file: File,
}
impl DiskPcm {
    fn map(file: File) -> Result<Self> {
        // SAFETY: tempfile() creates a process-private unnamed file. No handle
        // is exposed; after this point the file is never modified or truncated.
        let map = unsafe { Mmap::map(&file) }?;
        if map.len() % size_of::<f32>() != 0 || (map.as_ptr() as usize) % align_of::<f32>() != 0 {
            return Err(Error::Audio("invalid editor PCM cache alignment".into()));
        }
        Ok(Self { map, _file: file })
    }
    fn samples(&self) -> &[f32] {
        // SAFETY: map() checked alignment/length. The writer stored native f32
        // bytes, all bit patterns are valid f32, and the map is immutable/alive.
        unsafe {
            std::slice::from_raw_parts(
                self.map.as_ptr().cast::<f32>(),
                self.map.len() / size_of::<f32>(),
            )
        }
    }
}

pub(super) enum Storage {
    Ram(Vec<f32>),
    Shared(Arc<Vec<f32>>),
    Disk(DiskPcm),
}
impl std::ops::Deref for Storage {
    type Target = [f32];
    fn deref(&self) -> &[f32] {
        match self {
            Self::Ram(v) => v,
            Self::Shared(v) => v,
            Self::Disk(d) => d.samples(),
        }
    }
}

pub(super) struct Builder {
    ram: Vec<f32>,
    writer: Option<BufWriter<File>>,
    limit: usize,
    samples: usize,
    bytes: Vec<u8>,
}
impl Builder {
    pub(super) fn new(limit: usize) -> Self {
        Self {
            ram: Vec::new(),
            writer: None,
            limit,
            samples: 0,
            bytes: Vec::with_capacity(65536),
        }
    }
    fn write(&mut self, samples: &[f32]) -> Result<()> {
        let writer = self.writer.as_mut().expect("disk writer exists");
        for part in samples.chunks(16384) {
            self.bytes.clear();
            for sample in part {
                self.bytes.extend_from_slice(&sample.to_ne_bytes());
            }
            writer.write_all(&self.bytes)?;
        }
        Ok(())
    }
    pub(super) fn append(&mut self, samples: &[f32]) -> Result<()> {
        let next = self
            .samples
            .checked_add(samples.len())
            .filter(|n| *n <= isize::MAX as usize / size_of::<f32>())
            .ok_or_else(|| Error::Audio("audio exceeds the editor's address space".into()))?;
        if self.writer.is_none() && next > self.limit / size_of::<f32>() {
            self.writer = Some(BufWriter::with_capacity(65536, tempfile::tempfile()?));
            let ram = std::mem::take(&mut self.ram);
            self.write(&ram)?;
        }
        if self.writer.is_some() {
            self.write(samples)?;
        } else {
            self.ram
                .try_reserve(samples.len())
                .map_err(|_| Error::Audio("not enough memory for editor audio".into()))?;
            self.ram.extend_from_slice(samples);
        }
        self.samples = next;
        Ok(())
    }
    pub(super) fn finish(self) -> Result<Storage> {
        match self.writer {
            Some(writer) => {
                let file = writer.into_inner().map_err(|e| Error::Io(e.into_error()))?;
                Ok(Storage::Disk(DiskPcm::map(file)?))
            }
            None => Ok(Storage::Ram(self.ram)),
        }
    }
}

pub(super) fn decode(path: &Path, ram_budget: usize) -> Result<(Arc<Storage>, usize, u32)> {
    let mut builder = Builder::new(ram_budget.min(RAM_CEILING));
    let (mut channels, mut rate) = (0, 0);
    audio::try_stream(path, |samples, ch, sr| {
        channels = ch;
        rate = sr;
        builder.append(samples)
    })?;
    if rate == 0 || channels == 0 {
        return Err(Error::Audio("no audio decoded".into()));
    }
    Ok((Arc::new(builder.finish()?), channels, rate))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::Pcm;
    use crate::{
        audio::AudioRead,
        editor::{Op, blocks::BlockAudio},
    };
    #[test]
    fn spilling_preserves_samples_and_snapshots_keep_the_private_file_alive() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("source.wav");
        let original = Pcm {
            samples: (0..20000).map(|i| (i as f32 * 0.1).sin() * 0.5).collect(),
            channels: 2,
            sample_rate: 44100,
        };
        crate::editor::write_wav(&path, &original, Some(32), None).unwrap();
        let (storage, ch, rate) = decode(&path, 128).unwrap();
        assert!(matches!(storage.as_ref(), Storage::Disk(_)));
        assert_eq!(&**storage, original.samples.as_slice());
        let weak = Arc::downgrade(&storage);
        let mut audio = BlockAudio::from_storage(storage, ch, rate);
        let snapshot = audio.clone();
        audio.apply(&Op::Gain {
            start: 10,
            end: 100,
            db: -6.0,
        });
        assert_eq!(snapshot.sample(20), original.samples[20]);
        assert!(weak.upgrade().is_some());
        drop(audio);
        assert!(weak.upgrade().is_some());
        drop(snapshot);
        assert!(weak.upgrade().is_none());
        let (small, _, _) = decode(&path, 100000).unwrap();
        assert!(matches!(small.as_ref(), Storage::Ram(_)));
    }
    #[test]
    fn large_incremental_output_spills_without_a_second_complete_ram_buffer() {
        let mut builder = Builder::new(1024);
        for _ in 0..40 {
            builder.append(&[0.25; 128]).unwrap();
        }
        assert!(builder.writer.is_some());
        assert!(builder.ram.is_empty());
        let stored = builder.finish().unwrap();
        assert!(matches!(stored, Storage::Disk(_)));
        assert_eq!(stored.len(), 5120);
        assert!(stored.iter().all(|v| *v == 0.25));
    }

    #[test]
    fn failing_sink_aborts_streaming_instead_of_decoding_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("source.wav");
        crate::editor::write_wav(
            &path,
            &Pcm {
                samples: vec![0.2; 20000],
                channels: 1,
                sample_rate: 1000,
            },
            Some(16),
            None,
        )
        .unwrap();
        let mut calls = 0;
        let result = audio::try_stream(&path, |_, _, _| {
            calls += 1;
            Err(Error::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                "simulated full cache disk",
            )))
        });
        assert_eq!(calls, 1);
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("simulated full cache disk")
        );
    }
}
