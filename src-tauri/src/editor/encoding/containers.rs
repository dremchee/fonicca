use super::{codec_error, sample};
use crate::{audio::AudioRead, error::Result};
use std::{
    fs::File,
    io::{BufWriter, Seek, SeekFrom, Write},
    path::Path,
};

const BLOCK: usize = 4096;

fn pcm24(value: f32) -> i32 {
    (sample(value) * 8_388_607.0).round() as i32
}

pub(super) fn flac(
    path: &Path,
    audio: &impl AudioRead,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    use flacenc::{
        bitsink::ByteSink,
        component::{BitRepr, Stream},
        error::Verify,
        source::{Fill, FrameBuf},
    };

    let channels = audio.channels();
    let config = flacenc::config::Encoder::default()
        .into_verified()
        .map_err(|(_, error)| codec_error(error))?;
    let mut stream =
        Stream::new(audio.sample_rate() as usize, channels, 24).map_err(codec_error)?;
    let mut block = FrameBuf::with_size(channels, BLOCK).map_err(codec_error)?;
    let mut input = audio.iter_samples();
    let mut pcm = Vec::with_capacity(BLOCK * channels);
    let mut sink = ByteSink::new();
    let mut file = BufWriter::new(File::create(path)?);

    // Write frames one at a time and backfill STREAMINFO with their actual sizes.
    stream.write(&mut sink).map_err(codec_error)?;
    file.write_all(sink.as_slice())?;
    for index in 0..audio.frames().div_ceil(BLOCK) {
        check()?;
        pcm.clear();
        pcm.extend(
            input
                .by_ref()
                .take(BLOCK * channels)
                .map(|&value| pcm24(value)),
        );
        block.fill_interleaved(&pcm).map_err(codec_error)?;
        let frame = flacenc::encode_fixed_size_frame(&config, &block, index, stream.stream_info())
            .map_err(codec_error)?;
        stream.stream_info_mut().update_frame_info(&frame);
        sink.clear();
        frame.write(&mut sink).map_err(codec_error)?;
        file.write_all(sink.as_slice())?;
    }
    // FLAC's fixed block size excludes the final, possibly short frame.
    stream
        .stream_info_mut()
        .set_block_sizes(BLOCK, BLOCK)
        .map_err(codec_error)?;
    sink.clear();
    stream.write(&mut sink).map_err(codec_error)?;
    file.seek(SeekFrom::Start(0))?;
    file.write_all(sink.as_slice())?;
    file.flush()?;
    Ok(())
}

pub(super) fn aiff(
    path: &Path,
    audio: &impl AudioRead,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    let info = aifc::AifcWriteInfo {
        file_format: aifc::FileFormat::Aiff,
        channels: audio.channels() as i16,
        sample_rate: f64::from(audio.sample_rate()),
        sample_format: aifc::SampleFormat::I24,
    };
    let mut file = BufWriter::new(File::create(path)?);
    let aiff_error = |error| codec_error(format!("{error:?}"));
    let mut writer = aifc::AifcWriter::new(&mut file, &info).map_err(aiff_error)?;
    let mut pcm = Vec::with_capacity(BLOCK);
    let mut input = audio.iter_samples();
    loop {
        check()?;
        pcm.clear();
        pcm.extend(input.by_ref().take(BLOCK).map(|&value| pcm24(value)));
        if pcm.is_empty() {
            break;
        }
        writer.write_samples_i24(&pcm).map_err(aiff_error)?;
    }
    writer.finalize().map_err(aiff_error)?;
    drop(writer);
    file.flush()?;
    Ok(())
}

pub(super) fn m4a(
    path: &Path,
    audio: &impl AudioRead,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    use alac_encoder::{AlacEncoder, FormatDescription};
    use oxideav_core::{CodecId, CodecParameters, Packet, SampleFormat, StreamInfo, TimeBase};
    use oxideav_mp4::options::{BrandPreset, Mp4MuxerOptions};

    let rate = audio.sample_rate();
    let channels = audio.channels();
    let input_format = FormatDescription::pcm::<i16>(f64::from(rate), channels as u32);
    let output_format = FormatDescription::alac(f64::from(rate), BLOCK as u32, channels as u32);
    let mut encoder = AlacEncoder::new(&output_format);
    let mut output = vec![0; output_format.max_packet_size()];
    let mut params = CodecParameters::audio(CodecId::new("alac"));
    params.channels = Some(channels as u16);
    params.sample_rate = Some(rate);
    params.sample_format = Some(SampleFormat::S16);
    params.extradata = encoder.magic_cookie();
    let time_base = TimeBase::new(1, i64::from(rate));
    let stream = StreamInfo {
        index: 0,
        time_base,
        duration: Some(audio.frames() as i64),
        start_time: Some(0),
        params,
    };
    let options = Mp4MuxerOptions {
        brand: BrandPreset::Custom {
            major: *b"M4A ",
            compatible: vec![*b"M4A ", *b"isom", *b"mp42"],
        },
        ..Default::default()
    };
    let mut muxer = oxideav_mp4::muxer::open_with_options(
        Box::new(BufWriter::new(File::create(path)?)),
        &[stream],
        options,
    )
    .map_err(codec_error)?;
    muxer.write_header().map_err(codec_error)?;
    let mut pcm = Vec::with_capacity(BLOCK * channels * 2);
    let mut input = audio.iter_samples();
    for start in (0..audio.frames()).step_by(BLOCK) {
        check()?;
        pcm.clear();
        for &value in input.by_ref().take(BLOCK * channels) {
            pcm.extend_from_slice(&((sample(value) * 32_767.0).round() as i16).to_le_bytes());
        }
        let size = encoder.encode(&input_format, &pcm, &mut output);
        let mut packet = Packet::new(0, time_base, output[..size].to_vec());
        packet.pts = Some(start as i64);
        packet.dts = packet.pts;
        packet.duration = Some((pcm.len() / (2 * channels)) as i64);
        packet.flags.keyframe = true;
        muxer.write_packet(&packet).map_err(codec_error)?;
    }
    muxer.write_trailer().map_err(codec_error)?;
    Ok(())
}

pub(super) fn caf(
    path: &Path,
    audio: &impl AudioRead,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    let mut file = BufWriter::new(File::create(path)?);
    let data_size = i64::try_from(audio.sample_count())
        .ok()
        .and_then(|samples| samples.checked_mul(4)?.checked_add(4))
        .ok_or_else(|| codec_error("CAF file is too large"))?;
    // CAF chunk headers are big endian. Flags 3 mean float, little endian PCM.
    file.write_all(b"caff\0\x01\0\0desc")?;
    file.write_all(&32i64.to_be_bytes())?;
    file.write_all(&f64::from(audio.sample_rate()).to_be_bytes())?;
    file.write_all(b"lpcm")?;
    for field in [
        3,
        audio.channels() as u32 * 4,
        1,
        audio.channels() as u32,
        32,
    ] {
        file.write_all(&field.to_be_bytes())?;
    }
    file.write_all(b"data")?;
    file.write_all(&data_size.to_be_bytes())?;
    file.write_all(&0u32.to_be_bytes())?; // Edit count.
    for (index, value) in audio.iter_samples().enumerate() {
        if index % BLOCK == 0 {
            check()?;
        }
        file.write_all(&value.to_le_bytes())?;
    }
    file.flush()?;
    Ok(())
}
