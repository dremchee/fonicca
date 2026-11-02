//! A window offer keeps audio at its source until the destination explicitly accepts.
//! Always lock this registry before the session map when both are needed.
use std::{collections::HashMap, sync::mpsc};
use crate::{editor::Session, error::{Error, Result}, AppState};
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder, Window};
use super::{Opened, WINDOW_PREFIX};

struct Offer {
    token: u64,
    source: String,
    target: String,
    revision: u64,
    reply: mpsc::Sender<std::result::Result<(), String>>,
}

#[derive(Default)]
pub struct Transfers { next: u64, offer: Option<Offer>, pub(super) openings: super::opening::Requests }

impl Transfers {
    pub fn guard(&self, label: &str) -> Result<()> {
        if self.offer.as_ref().is_some_and(|o| label == o.target || label == o.source) {
            return Err(Error::Other("editor window transfer is pending".into()));
        }
        Ok(())
    }
    pub fn begin(&mut self, sessions: &HashMap<String, Session>, source: &str, target: &str) -> Result<(u64, mpsc::Receiver<std::result::Result<(), String>>)> {
        if self.offer.is_some() || source == target || sessions.contains_key(target) {
            return Err(Error::Other("the destination editor is occupied".into()));
        }
        if self.openings.active(source) || self.openings.active(target) {
            return Err(Error::Other("wait for the editor file to finish loading".into()));
        }
        let s = sessions.get(source).ok_or_else(|| Error::Other("no editor session to transfer".into()))?;
        if s.save_pending() { return Err(Error::Other("wait for the current save to finish".into())); }
        self.next += 1;
        let (reply, receiver) = mpsc::channel();
        self.offer = Some(Offer { token: self.next, source: source.into(), target: target.into(), revision: s.revision(), reply });
        Ok((self.next, receiver))
    }
    pub fn source(&self, token: u64, target: &str) -> Result<&str> {
        self.offer.as_ref().filter(|o| o.token == token && o.target == target).map(|o| o.source.as_str())
            .ok_or_else(|| Error::Other("editor transfer expired".into()))
    }
    pub fn accept(&mut self, sessions: &mut HashMap<String, Session>, token: u64, target: &str) -> Result<()> {
        self.source(token, target)?;
        let offer = self.offer.take().unwrap();
        let result = if sessions.get(&offer.source).is_none_or(|s| s.revision() != offer.revision) {
            Err(Error::Other("source editor changed during transfer".into()))
        } else { super::move_session(sessions, &offer.source, &offer.target) };
        match result {
            Ok(()) => {
                if offer.reply.send(Ok(())).is_err() {
                    super::move_session(sessions, &offer.target, &offer.source)?;
                    return Err(Error::Other("source editor stopped waiting".into()));
                }
                Ok(())
            }
            Err(error) => { let _ = offer.reply.send(Err(error.to_string())); Err(error) }
        }
    }
    pub fn cancel(&mut self, token: u64, reason: &str) {
        if self.offer.as_ref().is_some_and(|o| o.token == token) {
            let offer = self.offer.take().unwrap();
            let _ = offer.reply.send(Err(reason.into()));
        }
    }
    /// Explicit UI reload/discard cancels its offer before clearing this
    /// window. A target reset never discards the source's unsaved session.
    pub fn close(&mut self, sessions: &mut HashMap<String, Session>, label: &str, discard: bool) -> Result<()> {
        if discard {
            self.release(label);
        } else {
            self.guard(label)?;
            if sessions.get(label).is_some_and(|s| s.state().dirty) {
                return Err(Error::Other("editor has unsaved changes".into()));
            }
        }
        self.openings.cancel(label);
        sessions.remove(label);
        Ok(())
    }
    pub fn release(&mut self, label: &str) {
        self.openings.cancel(label);
        if let Some(token) = self.offer.as_ref().filter(|o| label == o.target || label == o.source).map(|o| o.token) {
            self.cancel(token, "editor window closed during transfer");
        }
    }
}

/// Create the target WebView, retaining audio/history in main until it is ready.
#[tauri::command]
pub async fn editor_detach_window(app: AppHandle, state: State<'_, AppState>, window: Window, view: serde_json::Value) -> Result<()> {
    if window.label() != "main" { return Err(Error::Other("only the embedded editor can be detached".into())); }
    let view = serde_json::to_string(&view).map_err(|e| Error::Other(e.to_string()))?;
    if view.len() > 4096 { return Err(Error::Other("editor view state is too large".into())); }
    static NEXT_WINDOW: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let label = format!("{WINDOW_PREFIX}detached-{}", NEXT_WINDOW.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
    let (token, receiver, url, title) = {
        let mut transfers = state.editor_transfers.lock().unwrap();
        let sessions = state.editor.lock().unwrap();
        let session = sessions.get("main").ok_or_else(|| Error::Other("no editor session to transfer".into()))?;
        let mut url = tauri::Url::parse("http://localhost/index.html").unwrap();
        let title = session.path.file_name().unwrap_or_default().to_string_lossy().into_owned();
        let id = session.sample_id;
        let (token, receiver) = transfers.begin(&sessions, "main", &label)?;
        url.query_pairs_mut().append_pair("editor", &id.to_string()).append_pair("transfer", &token.to_string())
            .append_pair("detached", "1").append_pair("view", &view);
        (token, receiver, format!("index.html?{}", url.query().unwrap()), title)
    };
    let builder = WebviewWindowBuilder::new(&app, &label, WebviewUrl::App(url.into()))
        .title(title).inner_size(1100.0, 560.0).min_inner_size(640.0, 360.0).visible(false);
    #[cfg(target_os = "macos")]
    let builder = builder.title_bar_style(tauri::TitleBarStyle::Overlay).hidden_title(true);
    if let Err(error) = builder.build() {
        state.editor_transfers.lock().unwrap().cancel(token, &error.to_string());
        return Err(Error::Other(format!("cannot open editor window: {error}")));
    }
    crate::show_window_fallback(&app, &label);
    let result = wait_for_accept(&state, token, receiver).await;
    if result.is_err() {
        // A failed/unready target owns no session. Close its error surface,
        // leaving the source editor and its unsaved history available.
        if let Some(target) = app.get_webview_window(&label) { let _ = target.destroy(); }
    }
    result
}

/// Offer the session; queued event delivery alone never transfers ownership.
#[tauri::command]
pub async fn editor_attach_window(app: AppHandle, state: State<'_, AppState>, window: Window, view: serde_json::Value) -> Result<()> {
    let source = window.label();
    if !source.starts_with(WINDOW_PREFIX) { return Err(Error::Other("only a standalone editor can return to the main window".into())); }
    if serde_json::to_string(&view).map_err(|e| Error::Other(e.to_string()))?.len() > 4096 {
        return Err(Error::Other("editor view state is too large".into()));
    }
    let main = app.get_webview_window("main").ok_or_else(|| Error::Other("the main window is closed".into()))?;
    let (token, receiver) = {
        let mut transfers = state.editor_transfers.lock().unwrap();
        transfers.begin(&state.editor.lock().unwrap(), source, "main")?
    };
    if let Err(error) = app.emit_to("main", "editor-return-offered", serde_json::json!({ "token": token, "view": view })) {
        state.editor_transfers.lock().unwrap().cancel(token, &error.to_string());
        return Err(Error::Other(format!("cannot return the editor: {error}")));
    }
    let _ = main.show();
    let _ = main.unminimize();
    let _ = main.set_focus();
    wait_for_accept(&state, token, receiver).await
}

async fn wait_for_accept(state: &AppState, token: u64, receiver: mpsc::Receiver<std::result::Result<(), String>>) -> Result<()> {
    let (receiver, received) = tauri::async_runtime::spawn_blocking(move || {
        let received = receiver.recv_timeout(std::time::Duration::from_secs(15));
        (receiver, received)
    }).await.map_err(|e| Error::Other(e.to_string()))?;
    let result = match received {
        Ok(result) => result,
        Err(_) => {
            // Serialize timeout with accept. If accept won at the deadline its
            // reply is already queued; never report failure after moving audio.
            state.editor_transfers.lock().unwrap().cancel(token, "destination editor did not accept the session");
            receiver.try_recv().unwrap_or_else(|_| Err("editor transfer interrupted".into()))
        }
    };
    result.map_err(Error::Other)
}

/// Inspect without moving: the target builds its UI before committing the offer.
#[tauri::command]
pub fn editor_prepare_transfer(state: State<'_, AppState>, window: Window, token: u64) -> Result<Opened> {
    let transfers = state.editor_transfers.lock().unwrap();
    let sessions = state.editor.lock().unwrap();
    let source = transfers.source(token, window.label())?;
    let session = sessions.get(source).ok_or_else(|| Error::Other("source editor is closed".into()))?;
    let path = session.path.clone();
    let current = session.state();
    let id = session.sample_id;
    drop(sessions);
    drop(transfers);
    let bpm = state.db.lock().unwrap().query_row("SELECT bpm FROM samples WHERE id = ?1", [id], |r| r.get(0)).unwrap_or(None);
    Ok(Opened { state: current, name: path.file_stem().unwrap_or_default().to_string_lossy().into_owned(),
        ext: path.extension().unwrap_or_default().to_string_lossy().into_owned(), path: path.to_string_lossy().into_owned(), bpm })
}

#[tauri::command]
pub fn editor_accept_transfer(state: State<'_, AppState>, window: Window, token: u64) -> Result<()> {
    state.editor_transfers.lock().unwrap().accept(&mut state.editor.lock().unwrap(), token, window.label())
}

#[tauri::command]
pub fn editor_reject_transfer(state: State<'_, AppState>, window: Window, token: u64) -> Result<()> {
    let mut transfers = state.editor_transfers.lock().unwrap();
    transfers.source(token, window.label())?;
    transfers.cancel(token, "destination editor is not ready to accept the session");
    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::Op;
    fn setup() -> (tempfile::TempDir, HashMap<String, Session>, Transfers) {
        let dir = tempfile::tempdir().unwrap();
        let mut session = super::super::handoff_tests::session(dir.path());
        session.try_apply(Op::Gain { start: 0, end: 50, db: -6.0 }).unwrap();
        (dir, HashMap::from([("editor-source".into(), session)]), Transfers::default())
    }
    #[test]
    fn offer_retains_history_at_source_until_explicit_acceptance() {
        let (_dir, mut sessions, mut transfers) = setup();
        let expected = serde_json::to_value(sessions["editor-source"].state()).unwrap();
        let (token, reply) = transfers.begin(&sessions, "editor-source", "main").unwrap();
        assert!(sessions.contains_key("editor-source") && !sessions.contains_key("main"));
        assert!(transfers.guard("main").is_err());
        assert!(transfers.guard("editor-source").is_err());
        assert!(transfers.guard("editor-other").is_ok());
        assert!(transfers.begin(&sessions, "editor-source", "main").is_err());
        assert!(reply.try_recv().is_err());
        transfers.accept(&mut sessions, token, "main").unwrap();
        assert_eq!(reply.recv().unwrap(), Ok(()));
        assert!(!sessions.contains_key("editor-source"));
        assert_eq!(serde_json::to_value(sessions["main"].state()).unwrap(), expected);
        assert!(transfers.guard("main").is_ok());
        transfers.cancel(token, "late timeout");
        assert!(sessions.contains_key("main"));
        assert!(transfers.accept(&mut sessions, token, "main").is_err());
    }
    #[test]
    fn timeout_rejection_and_closed_target_leave_source_and_reject_stale_acceptance() {
        let (_dir, mut sessions, mut transfers) = setup();
        for close in [false, true] {
            let (token, reply) = transfers.begin(&sessions, "editor-source", "main").unwrap();
            if close { transfers.release("main"); } else { transfers.cancel(token, "timeout"); }
            assert!(reply.recv().unwrap().is_err());
            assert!(sessions["editor-source"].state().dirty);
            assert!(transfers.accept(&mut sessions, token, "main").is_err());
        }
        let (token, reply) = transfers.begin(&sessions, "editor-source", "main").unwrap();
        drop(reply);
        assert!(transfers.accept(&mut sessions, token, "main").is_err());
        assert!(sessions.contains_key("editor-source") && !sessions.contains_key("main"));
    }
    #[test]
    fn changed_source_or_occupied_target_cannot_be_overwritten() {
        let (dir, mut sessions, mut transfers) = setup();
        let (token, reply) = transfers.begin(&sessions, "editor-source", "main").unwrap();
        sessions.get_mut("editor-source").unwrap().undo();
        assert!(transfers.accept(&mut sessions, token, "main").is_err());
        assert!(reply.recv().unwrap().is_err());
        let (token, reply) = transfers.begin(&sessions, "editor-source", "main").unwrap();
        sessions.insert("main".into(), super::super::handoff_tests::session(dir.path()));
        assert!(transfers.accept(&mut sessions, token, "main").is_err());
        assert!(reply.recv().unwrap().is_err());
        assert_eq!(sessions.len(), 2);
        assert!(transfers.guard("main").is_ok());
    }
}

#[cfg(test)]
mod detach_tests {
    use super::*;
    use crate::editor::Op;

    #[test]
    fn detach_waits_for_its_exact_target_and_preserves_session_history() {
        let dir = tempfile::tempdir().unwrap();
        let mut session = super::super::handoff_tests::session(dir.path());
        session.try_apply(Op::SetLoop { start: 10, end: 80 }).unwrap();
        session.try_apply(Op::Gain { start: 10, end: 50, db: -6.0 }).unwrap();
        session.undo();
        let expected = serde_json::to_value(session.state()).unwrap();
        let revision = session.revision();
        let mut sessions = HashMap::from([("main".into(), session)]);
        let mut transfers = Transfers::default();
        let (token, reply) = transfers.begin(&sessions, "main", "editor-target").unwrap();
        assert!(sessions.contains_key("main") && !sessions.contains_key("editor-target"));
        assert!(transfers.guard("main").is_err());
        assert!(transfers.guard("editor-target").is_err());
        assert!(transfers.source(token, "editor-impostor").is_err());
        assert!(transfers.accept(&mut sessions, token, "editor-impostor").is_err());
        assert_eq!(transfers.source(token, "editor-target").unwrap(), "main");
        assert!(reply.try_recv().is_err());
        transfers.accept(&mut sessions, token, "editor-target").unwrap();
        assert_eq!(reply.recv().unwrap(), Ok(()));
        assert!(!sessions.contains_key("main"));
        assert_eq!(sessions["editor-target"].revision(), revision);
        assert_eq!(serde_json::to_value(sessions["editor-target"].state()).unwrap(), expected);
        assert!(sessions["editor-target"].state().dirty && sessions["editor-target"].state().can_redo);
    }

    #[test]
    fn explicit_source_reset_cancels_offer_before_discard_but_normal_close_does_not() {
        let dir = tempfile::tempdir().unwrap();
        let mut source = super::super::handoff_tests::session(dir.path());
        source.try_apply(Op::Gain { start: 0, end: 10, db: -6.0 }).unwrap();
        let mut sessions = HashMap::from([("main".into(), source)]);
        let mut transfers = Transfers::default();
        let (token, reply) = transfers.begin(&sessions, "main", "editor-target").unwrap();
        assert!(transfers.close(&mut sessions, "main", false).is_err());
        assert!(sessions["main"].state().dirty && reply.try_recv().is_err());
        transfers.close(&mut sessions, "main", true).unwrap();
        assert!(reply.recv().unwrap().is_err());
        assert!(sessions.is_empty());
        assert!(transfers.accept(&mut sessions, token, "editor-target").is_err());
    }

    #[test]
    fn resetting_destination_cancels_offer_without_discarding_source_history() {
        let dir = tempfile::tempdir().unwrap();
        let mut source = super::super::handoff_tests::session(dir.path());
        source.try_apply(Op::SetLoop { start: 10, end: 80 }).unwrap();
        let expected = serde_json::to_value(source.state()).unwrap();
        let mut sessions = HashMap::from([("editor-source".into(), source)]);
        let mut transfers = Transfers::default();
        let (token, reply) = transfers.begin(&sessions, "editor-source", "main").unwrap();
        transfers.close(&mut sessions, "main", true).unwrap();
        assert!(reply.recv().unwrap().is_err());
        assert!(transfers.accept(&mut sessions, token, "main").is_err());
        assert_eq!(serde_json::to_value(sessions["editor-source"].state()).unwrap(), expected);
        assert!(transfers.guard("editor-source").is_ok());
        assert!(transfers.close(&mut sessions, "editor-source", false).is_err());
    }

    #[test]
    fn save_in_progress_prevents_an_offer_and_can_be_retried_afterward() {
        let dir = tempfile::tempdir().unwrap();
        let source = super::super::handoff_tests::session(dir.path());
        let save = crate::editor::saving::Snapshot::new(&source, crate::editor::saving::SaveMode::Save, None, None).unwrap();
        let sessions = HashMap::from([("main".into(), source)]);
        let mut transfers = Transfers::default();
        assert!(transfers.begin(&sessions, "main", "editor-target").is_err());
        assert!(transfers.guard("main").is_ok());
        drop(save);
        let (token, reply) = transfers.begin(&sessions, "main", "editor-target").unwrap();
        transfers.release("unrelated-window");
        assert!(reply.try_recv().is_err());
        assert_eq!(transfers.source(token, "editor-target").unwrap(), "main");
        transfers.cancel(token, "test complete");
    }

    #[test]
    fn unready_or_closed_target_keeps_main_and_old_token_cannot_claim_retry() {
        let dir = tempfile::tempdir().unwrap();
        let mut sessions = HashMap::from([("main".into(), super::super::handoff_tests::session(dir.path()))]);
        let mut transfers = Transfers::default();
        for closed in [false, true] {
            let (token, reply) = transfers.begin(&sessions, "main", "editor-failed").unwrap();
            if closed { transfers.release("editor-failed"); } else { transfers.cancel(token, "builder/timeout failure"); }
            assert!(reply.recv().unwrap().is_err());
            let (retry, retry_reply) = transfers.begin(&sessions, "main", "editor-retry").unwrap();
            assert_ne!(token, retry);
            transfers.cancel(token, "late reject");
            assert!(transfers.accept(&mut sessions, token, "editor-retry").is_err());
            assert!(retry_reply.try_recv().is_err());
            assert!(sessions.contains_key("main") && sessions.len() == 1);
            transfers.cancel(retry, "test complete");
        }
    }
}
