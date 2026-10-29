//! On-demand diagnostics. No sampler thread runs while the monitor is closed.
use crate::{
    AppState,
    editor::diagnostics::EditorDiagnostics,
    error::{Error, Result},
};
use serde::Serialize;
use std::sync::{Mutex, OnceLock};
use sysinfo::{MemoryRefreshKind, ProcessRefreshKind, ProcessesToUpdate, System};
use tauri::{State, Window};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostics {
    process_bytes: Option<u64>,
    cpu_percent: Option<f32>,
    available_bytes: u64,
    editor_busy: bool,
    editor: Option<EditorDiagnostics>,
}
#[derive(Default)]
struct Sampler {
    system: System,
    primed: bool,
}
static SAMPLER: OnceLock<Mutex<Sampler>> = OnceLock::new();

#[tauri::command]
pub async fn debug_snapshot(state: State<'_, AppState>, window: Window) -> Result<Diagnostics> {
    // Edits and peak generation may hold this lock for a while. Skip session
    // counters during that time; process RSS must remain observable throughout.
    let (editor, editor_busy) = match state.editor.try_lock() {
        Ok(sessions) => (sessions.get(window.label()).map(|s| s.diagnostics()), false),
        Err(_) => (None, true),
    };
    tauri::async_runtime::spawn_blocking(move || {
        let (process_bytes, cpu_percent, available_bytes) = process_sample()?;
        Ok(Diagnostics {
            process_bytes,
            cpu_percent,
            available_bytes,
            editor_busy,
            editor,
        })
    })
    .await
    .map_err(|e| Error::Other(e.to_string()))?
}

fn process_sample() -> Result<(Option<u64>, Option<f32>, u64)> {
    let mut sampler = SAMPLER
        .get_or_init(|| Mutex::new(Sampler::default()))
        .lock()
        .unwrap();
    let pid = sysinfo::get_current_pid().map_err(|e| Error::Other(e.to_string()))?;
    sampler
        .system
        .refresh_memory_specifics(MemoryRefreshKind::nothing().with_ram());
    sampler.system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[pid]),
        true,
        ProcessRefreshKind::nothing().with_memory().with_cpu(),
    );
    let process = sampler.system.process(pid);
    let process_bytes = process.map(|p| p.memory());
    let cpu_percent = if sampler.primed {
        process.map(|p| p.cpu_usage())
    } else {
        None
    };
    sampler.primed = true;
    Ok((
        process_bytes,
        cpu_percent,
        sampler.system.available_memory(),
    ))
}

#[cfg(test)]
mod tests {
    #[test]
    fn samples_the_current_process_without_an_editor_or_audio_device() {
        let (rss, _, available) = super::process_sample().unwrap();
        assert!(rss.is_some_and(|bytes| bytes > 0));
        assert!(available > 0);
        let (_, cpu, _) = super::process_sample().unwrap();
        assert!(cpu.is_some_and(|value| value.is_finite() && value >= 0.0));
    }
}
