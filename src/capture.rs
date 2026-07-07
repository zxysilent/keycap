use crossbeam::channel::{Receiver, bounded};
use log::{error, info};
use rdev::{EventType, Key, listen};
use std::thread;

/// A simplified key event sent from the capture thread to the main app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEvent {
    Press(Key),
    Release(Key),
}

/// Spawn a background thread that listens for global keyboard events
/// and sends them through the returned channel.
///
/// `rdev::listen` blocks forever, so we run it on a dedicated thread.
pub fn start_capture() -> Receiver<KeyEvent> {
    let (tx, rx) = bounded::<KeyEvent>(256);

    thread::Builder::new()
        .name("keycap-capture".into())
        .spawn(move || {
            info!("capture thread started");
            if let Err(e) = listen(move |event| {
                let ev = match event.event_type {
                    EventType::KeyPress(key) => Some(KeyEvent::Press(key)),
                    EventType::KeyRelease(key) => Some(KeyEvent::Release(key)),
                    _ => None,
                };
                if let Some(ke) = ev {
                    if tx.send(ke).is_err() {
                        // channel closed, stop listening
                        // (rdev listen takes a closure that returns Option<bool>)
                    }
                }
            }) {
                error!("capture thread error: {:?}", e);
            }
            info!("capture thread exited");
        })
        .expect("failed to spawn capture thread");

    rx
}
