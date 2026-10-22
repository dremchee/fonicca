use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::{mpsc, Mutex};
use std::time::Duration;

use rodio::{Decoder, DeviceSinkBuilder, Player as RodioPlayer};
use rodio::mixer::Mixer;

use crate::error::{Error, Result};

/// Preview player. The output device lives on its own thread (cpal streams are not
/// `Send` on every platform); only the mixer handle is shared.
pub struct Player {
    mixer: Option<Mixer>,
    current: Mutex<Option<RodioPlayer>>,
}

impl Player {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("audio-output".into())
            .spawn(move || match DeviceSinkBuilder::open_default_sink() {
                Ok(mut sink) => {
                    sink.log_on_drop(false);
                    let _ = tx.send(Some(sink.mixer().clone()));
                    // Keep the stream alive for the lifetime of the app.
                    loop {
                        std::thread::park();
                    }
                }
                Err(e) => {
                    eprintln!("failed to open audio output: {e}");
                    let _ = tx.send(None);
                }
            })
            .expect("spawn audio thread");
        Self { mixer: rx.recv().ok().flatten(), current: Mutex::new(None) }
    }

    pub fn play(&self, path: &Path, start: f64) -> Result<()> {
        let mixer = self.mixer.as_ref().ok_or_else(|| Error::Audio("no output device".into()))?;
        let file = File::open(path)?;
        let len = file.metadata()?.len();
        let mut builder = Decoder::builder()
            .with_data(BufReader::new(file))
            .with_byte_len(len)
            .with_seekable(true);
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            builder = builder.with_hint(ext);
        }
        let source = builder.build().map_err(|e| Error::Audio(e.to_string()))?;

        let player = RodioPlayer::connect_new(mixer);
        player.append(source);
        if start > 0.0 {
            let _ = player.try_seek(Duration::from_secs_f64(start));
        }
        // Replacing the previous player drops it, which stops its sound.
        *self.current.lock().unwrap() = Some(player);
        Ok(())
    }

    pub fn stop(&self) {
        self.current.lock().unwrap().take();
    }

    pub fn seek(&self, pos: f64) -> Result<()> {
        if let Some(p) = self.current.lock().unwrap().as_ref() {
            p.try_seek(Duration::from_secs_f64(pos)).map_err(|e| Error::Audio(e.to_string()))?;
        }
        Ok(())
    }

    /// Current position in seconds, or `None` when nothing is playing.
    pub fn position(&self) -> Option<f64> {
        let mut guard = self.current.lock().unwrap();
        match guard.as_ref() {
            Some(p) if !p.empty() => Some(p.get_pos().as_secs_f64()),
            Some(_) => {
                guard.take();
                None
            }
            None => None,
        }
    }
}
