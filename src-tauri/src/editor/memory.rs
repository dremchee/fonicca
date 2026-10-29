//! Conservative PCM budget, leaving room for the original, working buffer,
//! playback, processing and wave caches. Source PCM can spill to disk.
use crate::error::{Error, Result};

pub const MAX_PCM_BYTES: usize = 2 * 1024 * 1024 * 1024;

pub fn budget_for_available(available: Option<u64>) -> usize {
    available.map(|bytes| (bytes / 6).min(MAX_PCM_BYTES as u64) as usize)
        .unwrap_or(256 * 1024 * 1024)
}

pub fn pcm_budget() -> usize {
    let mut system = sysinfo::System::new();
    system.refresh_memory_specifics(sysinfo::MemoryRefreshKind::nothing().with_ram());
    budget_for_available((system.total_memory() > 0).then(|| system.available_memory()))
}

pub fn check_samples(samples: usize, budget: usize) -> Result<()> {
    if samples.checked_mul(size_of::<f32>()).is_none_or(|bytes| bytes > budget) {
        return Err(Error::Other(format!("Not enough memory to edit this audio in full. The decoded audio exceeds the current memory budget ({} MiB). Close other applications or edit a shorter excerpt.", budget / (1024 * 1024))));
    }
    Ok(())
}

pub fn check_growth(frames: usize, result_frames: usize, channels: usize) -> Result<()> {
    let samples = result_frames.checked_mul(channels.max(1)).ok_or_else(|| Error::Other("Audio size is too large".into()))?;
    check_samples(samples, isize::MAX as usize)?;
    let extra = result_frames.saturating_sub(frames).checked_mul(channels.max(1)).ok_or_else(|| Error::Other("Audio size is too large".into()))?;
    if extra > 0 { check_samples(extra, pcm_budget())?; }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn budgets_account_for_multiple_buffers_and_do_not_exceed_the_cap() {
        assert_eq!(budget_for_available(Some(6 * 1024)), 1024);
        assert_eq!(budget_for_available(Some(0)), 0);
        assert_eq!(budget_for_available(Some(u64::MAX)), MAX_PCM_BYTES);
        assert_eq!(budget_for_available(None), 256 * 1024 * 1024);
        assert!(check_samples(4, 16).is_ok());
        assert!(check_samples(5, 16).is_err());
        assert!(check_samples(usize::MAX, usize::MAX).is_err());
        // Same duration, different channel counts => different memory needs.
        assert!(check_samples(44100 * 60 * 30, 512 * 1024 * 1024).is_ok());
        assert!(check_samples(44100 * 60 * 30 * 2, 512 * 1024 * 1024).is_err());
    }
}
