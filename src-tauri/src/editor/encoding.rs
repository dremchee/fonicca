//! Streaming export encoders. Only small PCM/encoded blocks are buffered.
mod containers;

use super::{Loop, Marker, append_markers, write_wav};
use crate::{
    audio::AudioRead,
    error::{Error, Result},
};
use std::{
    fs::File,
    io::{BufWriter, Seek, SeekFrom, Write},
    num::{NonZeroU8, NonZeroU32},
    path::Path,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    #[default]
    Wav,
    Mp3,
    Ogg,
    Flac,
    Aiff,
    M4a,
    Caf,
}
impl Format {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Wav => "wav",
            Self::Mp3 => "mp3",
            Self::Ogg => "ogg",
            Self::Flac => "flac",
            Self::Aiff => "aiff",
            Self::M4a => "m4a",
            Self::Caf => "caf",
        }
    }

    pub fn for_source(path: &Path) -> Self {
        if path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("aifc")) {
            return Self::Aiff;
        }
        Self::from_path(path).unwrap_or_default()
    }

    pub fn from_path(path: &Path) -> Result<Self> {
        match path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str()
        {
            "wav" | "wave" => Ok(Self::Wav),
            "mp3" => Ok(Self::Mp3),
            "ogg" | "oga" => Ok(Self::Ogg),
            "flac" => Ok(Self::Flac),
            "aif" | "aiff" => Ok(Self::Aiff),
            "m4a" | "mp4" => Ok(Self::M4a),
            "caf" => Ok(Self::Caf),
            _ => Err(codec_error("choose a WAV, MP3, OGG, FLAC, AIFF, M4A or CAF destination")),
        }
    }
}

const BLOCK: usize = 8192;
fn sample(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(-1.0, 1.0)
    } else {
        0.0
    }
}
fn codec_error(error: impl std::fmt::Display) -> Error {
    Error::Other(format!("audio export: {error}"))
}

/// Re-encode a complete temporary stem without holding it all in memory.
pub(crate) fn transcode(
    source: &Path,
    target: &Path,
    format: Format,
    cancelled: &impl Fn() -> bool,
) -> Result<()> {
    let (pcm, channels, rate) = super::storage::decode_cancellable(
        source, super::storage::RAM_CEILING, cancelled,
    )?;
    let audio = super::blocks::BlockAudio::from_storage(pcm, channels, rate);
    write_encoded(target, format, &audio, &|| {
        if cancelled() {
            return Err(codec_error("cancelled"));
        }
        Ok(())
    })
}

pub fn write(
    path: &Path,
    format: Format,
    audio: &impl AudioRead,
    bits: Option<u32>,
    lp: Option<Loop>,
    markers: &[Marker],
) -> Result<()> {
    if format == Format::Wav {
        write_wav(path, audio, bits, lp)?;
        return append_markers(path, markers);
    }
    write_encoded(path, format, audio, &|| Ok(()))
}

fn write_encoded(
    path: &Path,
    format: Format,
    audio: &impl AudioRead,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    check()?;
    if audio.channels() == 0 || audio.sample_rate() == 0 || audio.frames() == 0 {
        return Err(codec_error("no audio to export"));
    }
    let channel_limit = match format {
        Format::Mp3 | Format::Ogg | Format::M4a => 2,
        Format::Flac => 8,
        Format::Aiff => i16::MAX as usize,
        Format::Caf => u32::MAX as usize / 4,
        Format::Wav => unreachable!(),
    };
    if audio.channels() > channel_limit {
        return Err(codec_error(format!("{} supports up to {channel_limit} channels", format.extension())));
    }
    match format {
        Format::Mp3 => mp3(path, audio, check),
        Format::Ogg => ogg(path, audio, check),
        Format::Flac => containers::flac(path, audio, check),
        Format::Aiff => containers::aiff(path, audio, check),
        Format::M4a => containers::m4a(path, audio, check),
        Format::Caf => containers::caf(path, audio, check),
        Format::Wav => unreachable!(),
    }
}

/// Validate the requested output rate instead of only relabelling the header.
pub fn write_at_rate(
    path: &Path,
    format: Format,
    audio: &impl AudioRead,
    bits: Option<u32>,
    lp: Option<Loop>,
    markers: &[Marker],
    rate: Option<u32>,
) -> Result<()> {
    let rate = rate.unwrap_or(audio.sample_rate());
    if !(1..=384000).contains(&rate) || audio.sample_rate() == 0 || audio.channels() == 0 {
        return Err(codec_error("invalid sample rate or channel count"));
    }
    if format == Format::Mp3
        && ![8000, 11025, 12000, 16000, 22050, 24000, 32000, 44100, 48000].contains(&rate)
    {
        return Err(codec_error(
            "MP3 requires a supported sample rate up to 48 kHz",
        ));
    }
    if rate == audio.sample_rate() {
        return write(path, format, audio, bits, lp, markers);
    }
    let converted = super::resampling::convert(audio, rate)?;
    let frame = |f: usize| {
        ((f as u128 * rate as u128 + audio.sample_rate() as u128 / 2) / audio.sample_rate() as u128)
            .min(converted.frames() as u128) as usize
    };
    let lp = lp
        .map(|lp| Loop {
            start: frame(lp.start),
            end: frame(lp.end),
        })
        .filter(|lp| lp.start < lp.end);
    let markers: Vec<_> = markers
        .iter()
        .map(|m| Marker {
            frame: frame(m.frame),
            skip: m.skip,
        })
        .filter(|m| m.frame < converted.frames())
        .collect();
    write(path, format, &converted, bits, lp, &markers)
}

fn mp3(path: &Path, audio: &impl AudioRead, check: &dyn Fn() -> Result<()>) -> Result<()> {
    use mp3lame_encoder::{Bitrate, Builder, DualPcm, FlushGap, MonoPcm, Quality};
    let mut encoder = Builder::new()
        .ok_or_else(|| codec_error("cannot allocate MP3 encoder"))?
        .with_num_channels(audio.channels() as u8)
        .map_err(codec_error)?
        .with_sample_rate(audio.sample_rate())
        .map_err(codec_error)?
        .with_brate(if audio.sample_rate() >= 32000 {
            Bitrate::Kbps320
        } else if audio.sample_rate() >= 16000 {
            Bitrate::Kbps160
        } else {
            Bitrate::Kbps64
        })
        .map_err(codec_error)?
        .with_quality(Quality::Best)
        .map_err(codec_error)?
        .with_to_write_vbr_tag(true)
        .map_err(codec_error)?
        .build()
        .map_err(codec_error)?;
    let mut file = BufWriter::new(File::create(path)?);
    let mut left = Vec::with_capacity(BLOCK);
    let mut right = Vec::with_capacity(BLOCK);
    let mut encoded = Vec::with_capacity(mp3lame_encoder::max_required_buffer_size(BLOCK));
    let mut input = audio.iter_samples();
    loop {
        check()?;
        left.clear();
        right.clear();
        encoded.clear();
        for _ in 0..BLOCK {
            let Some(&value) = input.next() else { break };
            left.push(sample(value));
            if audio.channels() == 2 {
                right.push(sample(*input.next().unwrap()));
            }
        }
        if left.is_empty() {
            break;
        }
        if audio.channels() == 1 {
            encoder.encode_to_vec(MonoPcm(&left), &mut encoded)
        } else {
            encoder.encode_to_vec(
                DualPcm {
                    left: &left,
                    right: &right,
                },
                &mut encoded,
            )
        }
        .map_err(codec_error)?;
        file.write_all(&encoded)?;
    }
    encoded.clear();
    encoder
        .flush_to_vec::<FlushGap>(&mut encoded)
        .map_err(codec_error)?;
    file.write_all(&encoded)?;
    // Replace LAME's placeholder with the final frame count and encoder delay/
    // padding. This preserves duration and enables gapless decoding of selections.
    let tag_size = encoder.lame_tag_size();
    if tag_size > 0 {
        let mut tag = Vec::with_capacity(tag_size);
        encoder
            .lame_tag_encode_to_vec(&mut tag)
            .ok_or_else(|| codec_error("cannot finish MP3 seek header"))?;
        file.seek(SeekFrom::Start(encoder.id3v2_tag_size() as u64))?;
        file.write_all(&tag)?;
    }
    file.flush()?;
    Ok(())
}

fn ogg(path: &Path, audio: &impl AudioRead, check: &dyn Fn() -> Result<()>) -> Result<()> {
    use vorbis_rs::{VorbisBitrateManagementStrategy, VorbisEncoderBuilder};
    let mut file = BufWriter::new(File::create(path)?);
    let mut encoder = VorbisEncoderBuilder::new(
        NonZeroU32::new(audio.sample_rate()).unwrap(),
        NonZeroU8::new(audio.channels() as u8).unwrap(),
        &mut file,
    )
    .map_err(codec_error)?
    .bitrate_management_strategy(VorbisBitrateManagementStrategy::QualityVbr {
        target_quality: 0.6,
    })
    .build()
    .map_err(codec_error)?;
    let mut channels = vec![Vec::with_capacity(BLOCK); audio.channels()];
    let mut input = audio.iter_samples();
    loop {
        check()?;
        for channel in &mut channels {
            channel.clear();
        }
        for _ in 0..BLOCK {
            let Some(&value) = input.next() else { break };
            channels[0].push(sample(value));
            for channel in channels.iter_mut().skip(1) {
                channel.push(sample(*input.next().unwrap()));
            }
        }
        if channels[0].is_empty() {
            break;
        }
        encoder.encode_audio_block(&channels).map_err(codec_error)?;
    }
    encoder.finish().map_err(codec_error)?;
    file.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::{self, Pcm};
    fn signal(channels: usize) -> Pcm {
        Pcm {
            channels,
            sample_rate: 44100,
            samples: (0..44100)
                .flat_map(|i| {
                    (0..channels).map(move |ch| {
                        (i as f32 * std::f32::consts::TAU * (440.0 + ch as f32 * 440.0) / 44100.0)
                            .sin()
                            * 0.4
                    })
                })
                .collect(),
        }
    }
    #[test]
    fn codecs_round_trip_mono_stereo_duration_level_and_channel_identity() {
        let dir = tempfile::tempdir().unwrap();
        for (format, ext, magic) in [
            (Format::Wav, "wav", b"RIFF".as_slice()),
            (Format::Mp3, "mp3", &[]),
            (Format::Ogg, "ogg", b"OggS".as_slice()),
            (Format::Flac, "flac", b"fLaC".as_slice()),
            (Format::Aiff, "aiff", b"FORM".as_slice()),
            (Format::M4a, "m4a", &[]),
            (Format::Caf, "caf", b"caff".as_slice()),
        ] {
            for channels in [1, 2] {
                let source = signal(channels);
                let path = dir.path().join(format!("{channels}.{ext}"));
                write(&path, format, &source, Some(24), None, &[]).unwrap();
                let bytes = std::fs::read(&path).unwrap();
                assert!(bytes.starts_with(magic));
                assert!(bytes.len() > 1000);
                let decoded = audio::decode(&path, f64::INFINITY).unwrap();
                assert_eq!(decoded.channels, channels);
                assert_eq!(decoded.sample_rate, 44100);
                assert!(
                    (decoded.frames() as isize - source.frames() as isize).unsigned_abs() < 3000,
                    "{ext}: duration {}",
                    decoded.frames()
                );
                if matches!(format, Format::Flac | Format::Aiff | Format::M4a | Format::Caf) {
                    assert_eq!(decoded.frames(), source.frames(), "{ext}: exact duration");
                    let tolerance = if format == Format::M4a { 1.0 / 16_384.0 } else { 1e-6 };
                    assert!(
                        decoded.samples.iter().zip(&source.samples).all(|(a, b)| (a - b).abs() < tolerance),
                        "{ext}: samples differ beyond PCM rounding",
                    );
                }
                let rms = (decoded.samples.iter().map(|s| s * s).sum::<f32>()
                    / decoded.samples.len() as f32)
                    .sqrt();
                assert!((rms - 0.283).abs() < 0.04, "{ext}: level {rms}");
                // Distinct channel frequencies must survive encoding (no accidental downmix).
                for ch in 0..channels {
                    // Hysteresis excludes tiny codec ringing around silence and
                    // zero; count only full-amplitude cycles in the middle 0.8s.
                    let mut negative = false;
                    let mut crossings = 0usize;
                    for frame in decoded
                        .samples
                        .chunks_exact(channels)
                        .skip(4410)
                        .take(35280)
                    {
                        if frame[ch] < -0.05 {
                            negative = true;
                        }
                        if negative && frame[ch] > 0.05 {
                            crossings += 1;
                            negative = false;
                        }
                    }
                    let expected = ((440 + ch * 440) as f32 * 0.8) as isize;
                    assert!(
                        (crossings as isize - expected).unsigned_abs() < 4,
                        "{ext}: channel {ch}: {crossings}"
                    );
                }
            }
        }
    }
    #[test]
    fn invalid_formats_and_multichannel_compressed_exports_fail_before_creating_a_file() {
        let dir = tempfile::tempdir().unwrap();
        assert!(Format::from_path(Path::new("out.txt")).is_err());
        assert_eq!(
            Format::from_path(Path::new("out.MP3")).unwrap(),
            Format::Mp3
        );
        for format in [Format::Mp3, Format::Ogg] {
            let path = dir.path().join("invalid");
            assert!(write(&path, format, &signal(3), None, None, &[]).is_err());
            assert!(!path.exists());
        }
    }
}
