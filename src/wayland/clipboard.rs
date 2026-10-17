use std::fs::File;
use std::io::Read;
use std::os::fd::OwnedFd;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use super::WaylandState;

// the names programs offer text under, the best first
const TEXT_TYPES: [&str; 5] = ["text/plain;charset=utf-8", "UTF8_STRING", "text/plain", "TEXT", "STRING"];

// how long the program holding the clipboard gets to hand it over
const WAIT: Duration = Duration::from_millis(500);

impl WaylandState {
    /*
     * the clipboard's text, none when it holds no text or its owner doesn't
     * answer in time; the compositor only shows the clipboard to a window
     * with keyboard focus, which a window being typed into always has
     */
    pub(super) fn clipboard_text(&self) -> Option<String> {
        let offer = self.data_device.as_ref()?.data().selection_offer()?;

        let kind = offer.with_mime_types(|types| {
            TEXT_TYPES
                .iter()
                .find(|wanted| types.iter().any(|offered| offered == *wanted))
                .map(|wanted| String::from(*wanted))
        })?;

        let pipe = offer.receive(kind).ok()?;

        // the receive request has to reach the other program before it can write anything
        let _ = self.connection.flush();

        let mut file = File::from(OwnedFd::from(pipe));

        let (send, receive) = mpsc::channel();

        // read on a thread, so a program that never writes can't freeze every window
        thread::spawn(move || {
            let mut bytes = Vec::new();

            if file.read_to_end(&mut bytes).is_ok() {
                let _ = send.send(bytes);
            }
        });

        let bytes = receive.recv_timeout(WAIT).ok()?;

        Some(String::from_utf8_lossy(&bytes).into_owned())
    }
}
