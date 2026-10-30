//! Export-rate conversion with bounded FFT buffers and private disk-backed output.
//! The live session and its undo history always remain at their original rate.
use super::{blocks::BlockAudio, storage::Builder};
use crate::{
    audio::AudioRead,
    error::{Error, Result},
};
use rubato::{Fft, FixedSync, Resampler, audioadapter_buffers::direct::InterleavedSlice};
use std::sync::Arc;

pub(super) fn convert(audio: &impl AudioRead, rate: u32) -> Result<BlockAudio> {
    let err = |e: &dyn std::fmt::Display| Error::Audio(format!("export resampling: {e}"));
    let ch = audio.channels();
    let count = ((audio.frames() as u128 * rate as u128 + audio.sample_rate() as u128 / 2)
        / audio.sample_rate() as u128)
        .max(u128::from(audio.frames() > 0));
    let frames = usize::try_from(count)
        .ok()
        .filter(|n| {
            n.checked_mul(ch)
                .is_some_and(|n| n <= isize::MAX as usize / 4)
        })
        .ok_or_else(|| Error::Audio("resampled audio is too large".into()))?;
    let mut resampler = Fft::<f32>::new(
        audio.sample_rate() as usize,
        rate as usize,
        1024,
        ch,
        FixedSync::Both,
    )
    .map_err(|e| err(&e))?;
    let mut input = vec![0.0; resampler.input_frames_max() * ch];
    let mut output = vec![0.0; resampler.output_frames_max() * ch];
    let mut skip = resampler.output_delay();
    let mut read = 0;
    let mut written = 0;
    // Export staging must not allocate another full-length PCM buffer in RAM.
    let mut builder = Builder::new(0);
    while written < frames {
        let needed = resampler.input_frames_next();
        let valid = needed.min(audio.frames() - read);
        input.fill(0.0);
        input[..valid * ch].copy_from_slice(&audio.range(read * ch..(read + valid) * ch));
        read += valid;
        let input = InterleavedSlice::new(&input, ch, needed).map_err(|e| err(&e))?;
        let output_frames = output.len() / ch;
        let mut adapter =
            InterleavedSlice::new_mut(&mut output, ch, output_frames).map_err(|e| err(&e))?;
        let (_, produced) = resampler
            .process_into_buffer(&input, &mut adapter, None)
            .map_err(|e| err(&e))?;
        let start = skip.min(produced);
        skip -= start;
        let take = (produced - start).min(frames - written);
        builder.append(&output[start * ch..(start + take) * ch])?;
        written += take;
    }
    Ok(BlockAudio::from_storage(
        Arc::new(builder.finish()?),
        ch,
        rate,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::Pcm;
    #[test]
    fn disk_backed_blocks_match_full_fft_resampling_without_delay_or_lost_tail() {
        for (from, to) in [
            (44100, 48000),
            (48000, 22050),
            (96000, 44100),
            (8000, 96000),
        ] {
            let frames = 12345;
            let mut audio = Pcm {
                channels: 2,
                sample_rate: from,
                samples: (0..frames)
                    .flat_map(|i| [(i as f32 * 0.1).sin() * 0.4, (i as f32 * 0.17).sin() * 0.3])
                    .collect(),
            };
            audio.samples[0] = 0.8;
            audio.samples[frames * 2 - 1] = -0.7;
            let original = audio.samples.clone();
            let converted = convert(&audio, to).unwrap();
            let mut reference =
                Fft::<f32>::new(from as usize, to as usize, 1024, 2, FixedSync::Both).unwrap();
            let adapter = InterleavedSlice::new(&audio.samples, 2, frames).unwrap();
            let expected = reference
                .process_all(&adapter, frames, None)
                .unwrap()
                .take_data();
            assert_eq!(
                converted.frames(),
                ((frames as f64 * to as f64 / from as f64).round() as usize).max(1)
            );
            assert!(
                converted
                    .samples
                    .iter()
                    .zip(expected)
                    .all(|(a, b)| (a - b).abs() < 1e-5),
                "{from} -> {to}"
            );
            assert_eq!(audio.samples, original);
        }
    }
    #[test]
    fn downsampling_filters_frequencies_above_the_new_nyquist_limit() {
        let audio = Pcm {
            channels: 1,
            sample_rate: 48000,
            samples: (0..48000)
                .map(|i| (i as f32 * std::f32::consts::TAU * 18000.0 / 48000.0).sin() * 0.4)
                .collect(),
        };
        let converted = convert(&audio, 22050).unwrap();
        let middle = converted.samples.range(1024..converted.frames() - 1024);
        let rms = (middle.iter().map(|s| s * s).sum::<f32>() / middle.len() as f32).sqrt();
        assert!(rms < 0.01, "aliasing level: {rms}");
    }
}
