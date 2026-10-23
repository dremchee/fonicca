use std::fs::File;
use std::path::Path;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::{FormatOptions, FormatReader};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use crate::error::{Error, Result};

pub const AUDIO_EXTENSIONS: &[&str] = &[
    "wav", "wave", "aif", "aiff", "aifc", "flac", "mp3", "ogg", "oga", "m4a", "mp4", "caf",
];


#[derive(Debug, Default, Clone)]
pub struct AudioInfo {
    pub duration: Option<f64>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u32>,
    pub bit_depth: Option<u32>,
}

pub fn is_audio_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| AUDIO_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

fn open_format(path: &Path) -> Result<Box<dyn FormatReader>> {
    let file = File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe().format(
        &hint,
        mss,
        &FormatOptions::default(),
        &MetadataOptions::default(),
    )?;
    Ok(probed.format)
}

/// Reads stream parameters from the container header without decoding audio.
/// Falls back to parsing the RIFF header for WAVs with codecs symphonia can't open.
pub fn probe(path: &Path) -> Result<AudioInfo> {
    probe_symphonia(path).or_else(|e| probe_riff(path).ok_or(e))
}

fn probe_symphonia(path: &Path) -> Result<AudioInfo> {
    let format = open_format(path)?;
    let track = format
        .default_track()
        .ok_or_else(|| Error::Audio("no audio track".into()))?;
    let p = &track.codec_params;
    let duration = match (p.n_frames, p.time_base, p.sample_rate) {
        (Some(n), Some(tb), _) => {
            let t = tb.calc_time(n);
            Some(t.seconds as f64 + t.frac)
        }
        (Some(n), None, Some(sr)) => Some(n as f64 / sr as f64),
        _ => None,
    };
    Ok(AudioInfo {
        duration,
        sample_rate: p.sample_rate,
        channels: p.channels.map(|c| c.count() as u32),
        bit_depth: p.bits_per_sample,
    })
}

/// Cheap content fingerprint: xxh3 over the size plus the first and last 64 KiB.
/// Stable across renames and moves; reading whole files would make scans I/O-bound.
pub fn fingerprint(path: &Path, size: u64) -> Result<i64> {
    use std::io::{Read, Seek, SeekFrom};
    const CHUNK: u64 = 64 * 1024;

    let mut f = File::open(path)?;
    let mut hasher = xxhash_rust::xxh3::Xxh3::new();
    hasher.update(&size.to_le_bytes());
    let mut buf = Vec::with_capacity(CHUNK as usize);
    (&mut f).take(CHUNK).read_to_end(&mut buf)?;
    hasher.update(&buf);
    if size > CHUNK {
        f.seek(SeekFrom::Start(size.saturating_sub(CHUNK).max(CHUNK)))?;
        buf.clear();
        f.take(CHUNK).read_to_end(&mut buf)?;
        hasher.update(&buf);
    }
    Ok(hasher.digest() as i64)
}

/// Minimal RIFF/WAVE header reader: `fmt `, `fact` and `data` chunks only.
fn probe_riff(path: &Path) -> Option<AudioInfo> {
    use std::io::{Read, Seek, SeekFrom};

    let mut f = File::open(path).ok()?;
    let mut header = [0u8; 12];
    f.read_exact(&mut header).ok()?;
    if &header[0..4] != b"RIFF" || &header[8..12] != b"WAVE" {
        return None;
    }
    let (mut channels, mut sample_rate, mut byte_rate, mut bits) = (0u16, 0u32, 0u32, 0u16);
    let (mut fact_frames, mut data_len) = (None, None);
    let mut chunk = [0u8; 8];
    while f.read_exact(&mut chunk).is_ok() {
        let size = u32::from_le_bytes(chunk[4..8].try_into().ok()?);
        match &chunk[0..4] {
            b"fmt " => {
                let mut fmt = [0u8; 16];
                f.read_exact(&mut fmt).ok()?;
                channels = u16::from_le_bytes([fmt[2], fmt[3]]);
                sample_rate = u32::from_le_bytes(fmt[4..8].try_into().ok()?);
                byte_rate = u32::from_le_bytes(fmt[8..12].try_into().ok()?);
                bits = u16::from_le_bytes([fmt[14], fmt[15]]);
                f.seek(SeekFrom::Current(size as i64 - 16 + (size & 1) as i64)).ok()?;
            }
            b"fact" if size >= 4 => {
                let mut n = [0u8; 4];
                f.read_exact(&mut n).ok()?;
                fact_frames = Some(u32::from_le_bytes(n));
                f.seek(SeekFrom::Current(size as i64 - 4 + (size & 1) as i64)).ok()?;
            }
            b"data" => {
                data_len = Some(size);
                break;
            }
            _ => {
                f.seek(SeekFrom::Current(size as i64 + (size & 1) as i64)).ok()?;
            }
        }
    }
    if sample_rate == 0 {
        return None;
    }
    let duration = match (fact_frames, data_len) {
        (Some(n), _) if n > 0 => Some(n as f64 / sample_rate as f64),
        (_, Some(len)) if byte_rate > 0 => Some(len as f64 / byte_rate as f64),
        _ => None,
    };
    Some(AudioInfo {
        duration,
        sample_rate: Some(sample_rate),
        channels: (channels > 0).then_some(channels as u32),
        bit_depth: (bits > 0).then_some(bits as u32),
    })
}

/// Decoded PCM, interleaved.
pub struct Pcm {
    pub samples: Vec<f32>,
    pub channels: usize,
    pub sample_rate: u32,
}

impl Pcm {
    pub fn frames(&self) -> usize {
        self.samples.len() / self.channels.max(1)
    }

    pub fn mono(&self) -> Vec<f32> {
        let ch = self.channels.max(1);
        self.samples.chunks(ch).map(|f| f.iter().sum::<f32>() / ch as f32).collect()
    }
}

/// Decodes up to `max_seconds` of audio into memory.
pub fn decode(path: &Path, max_seconds: f64) -> Result<Pcm> {
    let mut format = open_format(path)?;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| Error::Audio("no audio track".into()))?;
    let track_id = track.id;
    let mut decoder =
        symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default())?;

    let mut pcm = Pcm { samples: Vec::new(), channels: 0, sample_rate: 0 };
    let mut buf: Option<SampleBuffer<f32>> = None;
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymphoniaError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(SymphoniaError::ResetRequired) => break,
            Err(e) => return Err(e.into()),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(e) => return Err(e.into()),
        };
        let spec = *decoded.spec();
        pcm.channels = spec.channels.count().max(1);
        pcm.sample_rate = spec.rate;
        let sb = buf.get_or_insert_with(|| SampleBuffer::new(decoded.capacity() as u64, spec));
        if sb.capacity() < decoded.capacity() * pcm.channels {
            *sb = SampleBuffer::new(decoded.capacity() as u64, spec);
        }
        sb.copy_interleaved_ref(decoded);
        pcm.samples.extend_from_slice(sb.samples());
        if pcm.frames() as f64 >= max_seconds * pcm.sample_rate as f64 {
            break;
        }
    }
    if pcm.sample_rate == 0 {
        return Err(Error::Audio("no audio decoded".into()));
    }
    Ok(pcm)
}

/// Decodes the whole file in chunks, calling `f(interleaved, channels, sample_rate)`
/// for each, without keeping the audio in memory.
pub fn stream(path: &Path, mut f: impl FnMut(&[f32], usize, u32)) -> Result<()> {
    let mut format = open_format(path)?;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| Error::Audio("no audio track".into()))?;
    let track_id = track.id;
    let mut decoder =
        symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default())?;
    let mut buf: Option<SampleBuffer<f32>> = None;
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymphoniaError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(SymphoniaError::ResetRequired) => break,
            Err(e) => return Err(e.into()),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(e) => return Err(e.into()),
        };
        let spec = *decoded.spec();
        let channels = spec.channels.count().max(1);
        let sb = buf.get_or_insert_with(|| SampleBuffer::new(decoded.capacity() as u64, spec));
        if sb.capacity() < decoded.capacity() * channels {
            *sb = SampleBuffer::new(decoded.capacity() as u64, spec);
        }
        sb.copy_interleaved_ref(decoded);
        f(sb.samples(), channels, spec.rate);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    /// `PEAKS_FILE=/path/to/file cargo test -- --ignored --nocapture overview_timing`
    #[test]
    #[ignore]
    fn overview_timing() {
        let path = std::env::var("PEAKS_FILE").expect("set PEAKS_FILE");
        let t = std::time::Instant::now();
        let o = crate::overview::Overview::compute(std::path::Path::new(&path), None).unwrap();
        println!("{} bytes for 2000 columns in {:?}", o.render(2000, true).len(), t.elapsed());
    }
}
