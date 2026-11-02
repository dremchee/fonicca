//! Request identity is separate from Session revision: closing an empty editor
//! must invalidate an in-flight open even though both session states are None.
use std::{collections::HashMap, sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}}};
use crate::{editor::Session, error::{Error, Result}};
use super::transfer::Transfers;

#[derive(Clone, Default)]
pub(super) struct Cancellation(Arc<AtomicBool>);
impl Cancellation {
    pub fn cancelled(&self) -> bool { self.0.load(Ordering::Relaxed) }
    fn cancel(&self) { self.0.store(true, Ordering::Relaxed); }
}

#[derive(Default)]
pub(super) struct Requests(HashMap<String, Cancellation>);
impl Requests {
    fn begin(&mut self, label: &str) -> Cancellation {
        self.cancel(label);
        let request = Cancellation::default();
        self.0.insert(label.into(), request.clone());
        request
    }
    pub fn cancel(&mut self, label: &str) {
        if let Some(request) = self.0.remove(label) { request.cancel(); }
    }
    pub fn active(&self, label: &str) -> bool { self.0.contains_key(label) }
    fn current(&self, label: &str, request: &Cancellation) -> bool {
        !request.cancelled() && self.0.get(label).is_some_and(|r| Arc::ptr_eq(&r.0, &request.0))
    }
    fn finish(&mut self, label: &str, request: &Cancellation) {
        if self.0.get(label).is_some_and(|r| Arc::ptr_eq(&r.0, &request.0)) { self.0.remove(label); }
        request.cancel();
    }
}

pub(super) struct Lease<'a> {
    registry: &'a Mutex<Transfers>,
    label: String,
    pub expected_revision: Option<u64>,
    pub cancellation: Cancellation,
}
impl<'a> Lease<'a> {
    pub fn begin(registry: &'a Mutex<Transfers>, sessions: &Mutex<HashMap<String, Session>>, label: &str, discard: bool) -> Result<Self> {
        let mut transfers = registry.lock().unwrap();
        transfers.guard(label)?;
        let sessions = sessions.lock().unwrap();
        if !discard && sessions.get(label).is_some_and(|s| s.state().dirty) {
            return Err(Error::Other("editor has unsaved changes".into()));
        }
        let expected_revision = sessions.get(label).map(Session::revision);
        let cancellation = transfers.openings.begin(label);
        Ok(Self { registry, label: label.into(), expected_revision, cancellation })
    }
    pub fn check(&self, registry: &Transfers) -> Result<()> {
        if !registry.openings.current(&self.label, &self.cancellation) {
            return Err(Error::Other("editor opening cancelled".into()));
        }
        Ok(())
    }
}
impl Drop for Lease<'_> {
    fn drop(&mut self) {
        self.cancellation.cancel();
        if let Ok(mut registry) = self.registry.lock() { registry.openings.finish(&self.label, &self.cancellation); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn closing_an_empty_editor_invalidates_its_pending_open() {
        let registry = Mutex::new(Transfers::default());
        let sessions = Mutex::new(HashMap::new());
        let request = Lease::begin(&registry, &sessions, "main", false).unwrap();
        registry.lock().unwrap().close(&mut sessions.lock().unwrap(), "main", true).unwrap();
        assert!(sessions.lock().unwrap().is_empty());
        assert!(request.check(&registry.lock().unwrap()).is_err(), "None -> None must still cancel the old open");
        assert!(request.cancellation.cancelled());
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    #[test]
    fn newer_open_supersedes_old_and_old_drop_does_not_cancel_new() {
        let registry = Mutex::new(Transfers::default());
        let sessions = Mutex::new(HashMap::new());
        let first = Lease::begin(&registry, &sessions, "main", false).unwrap();
        let second = Lease::begin(&registry, &sessions, "main", false).unwrap();
        assert!(first.cancellation.cancelled());
        assert!(first.check(&registry.lock().unwrap()).is_err());
        drop(first);
        assert!(second.check(&registry.lock().unwrap()).is_ok());
        drop(second);
        assert!(!registry.lock().unwrap().openings.active("main"));
    }
    #[test]
    fn release_and_failed_scope_cleanup_are_per_window_and_remove_requests() {
        let registry = Mutex::new(Transfers::default());
        let sessions = Mutex::new(HashMap::new());
        let first = Lease::begin(&registry, &sessions, "main", false).unwrap();
        let other = Lease::begin(&registry, &sessions, "editor-other", false).unwrap();
        registry.lock().unwrap().release("main");
        assert!(first.cancellation.cancelled());
        assert!(first.check(&registry.lock().unwrap()).is_err());
        assert!(other.check(&registry.lock().unwrap()).is_ok());
        drop(first);
        drop(other); // Metadata/decode errors also leave scope through Drop.
        assert!(!registry.lock().unwrap().openings.active("editor-other"));
    }
    #[test]
    fn closing_while_a_real_decoder_is_paused_cancels_it_before_session_creation() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("large.wav");
        let pcm = crate::audio::Pcm { samples: vec![0.25; 200000], channels: 2, sample_rate: 44100 };
        crate::editor::write_wav(&path, &pcm, Some(32), None).unwrap();
        let registry = Mutex::new(Transfers::default());
        let sessions = Mutex::new(HashMap::new());
        let request = Lease::begin(&registry, &sessions, "main", false).unwrap();
        let cancellation = request.cancellation.clone();
        let (reached, progress) = std::sync::mpsc::channel();
        let (resume, resumed) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            let checks = std::cell::Cell::new(0);
            Session::open_cancellable(1, &path, Some(32), || {
                checks.set(checks.get()+1);
                if checks.get() == 2 {
                    reached.send(()).unwrap();
                    resumed.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
                }
                cancellation.cancelled()
            })
        });
        progress.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
        registry.lock().unwrap().close(&mut sessions.lock().unwrap(), "main", false).unwrap();
        resume.send(()).unwrap();
        let result = worker.join().unwrap();
        assert!(result.err().unwrap().to_string().contains("opening cancelled"));
        assert!(request.check(&registry.lock().unwrap()).is_err());
        assert!(sessions.lock().unwrap().is_empty());
        drop(request);
        assert!(!registry.lock().unwrap().openings.active("main"));
    }

    #[test]
    fn loading_target_prevents_transfer_and_failed_open_preserves_dirty_source() {
        let dir = tempfile::tempdir().unwrap();
        let mut source = super::super::handoff_tests::session(dir.path());
        source.try_apply(crate::editor::Op::Gain { start: 0, end: 10, db: -6.0 }).unwrap();
        let registry = Mutex::new(Transfers::default());
        let sessions = Mutex::new(HashMap::from([("editor-source".into(), source)]));
        assert!(Lease::begin(&registry, &sessions, "editor-source", false).is_err());
        assert!(!registry.lock().unwrap().openings.active("editor-source"));
        let target = Lease::begin(&registry, &sessions, "main", false).unwrap();
        assert!(registry.lock().unwrap().begin(&sessions.lock().unwrap(), "editor-source", "main").is_err());
        drop(target);
        let replacement = Lease::begin(&registry, &sessions, "editor-source", true).unwrap();
        drop(replacement);
        assert!(sessions.lock().unwrap()["editor-source"].state().dirty);
        assert!(!registry.lock().unwrap().openings.active("editor-source"));
    }
}
