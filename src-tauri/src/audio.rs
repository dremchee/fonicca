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

/// Number of min/max pairs stored per sample for waveform drawing.
pub const PEAK_BUCKETS: usize = 1024;

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

/// Decodes the whole file and returns `PEAK_BUCKETS` interleaved (min, max) pairs
/// of the mono downmix, scaled to i8.
pub fn compute_peaks(path: &Path) -> Result<Vec<i8>> {
    const BLOCK: usize = 256;

    let mut format = open_format(path)?;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| Error::Audio("no audio track".into()))?;
    let track_id = track.id;
    let mut decoder =
        symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default())?;

    // Min/max per fixed block of frames; reduced to PEAK_BUCKETS at the end,
    // so the total length does not need to be known up front.
    let mut blocks: Vec<(f32, f32)> = Vec::new();
    let (mut lo, mut hi, mut filled) = (f32::MAX, f32::MIN, 0usize);
    let mut buf: Option<SampleBuffer<f32>> = None;

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymphoniaError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                break;
            }
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

        for frame in sb.samples().chunks(channels) {
            let v = frame.iter().sum::<f32>() / channels as f32;
            lo = lo.min(v);
            hi = hi.max(v);
            filled += 1;
            if filled == BLOCK {
                blocks.push((lo, hi));
                (lo, hi, filled) = (f32::MAX, f32::MIN, 0);
            }
        }
    }
    if filled > 0 {
        blocks.push((lo, hi));
    }

    let mut peaks = Vec::with_capacity(PEAK_BUCKETS * 2);
    if blocks.is_empty() {
        peaks.resize(PEAK_BUCKETS * 2, 0);
        return Ok(peaks);
    }
    for b in 0..PEAK_BUCKETS {
        let start = b * blocks.len() / PEAK_BUCKETS;
        let end = ((b + 1) * blocks.len() / PEAK_BUCKETS).max(start + 1).min(blocks.len());
        let (mut mn, mut mx) = (0f32, 0f32);
        for &(l, h) in &blocks[start.min(blocks.len() - 1)..end] {
            mn = mn.min(l);
            mx = mx.max(h);
        }
        peaks.push((mn.clamp(-1.0, 1.0) * 127.0) as i8);
        peaks.push((mx.clamp(-1.0, 1.0) * 127.0) as i8);
    }
    Ok(peaks)
}
