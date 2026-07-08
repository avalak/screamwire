//! Lightweight, lock‑free bridge for waking up the network sender when
//! new audio data is available.
//!
//! Based on Linux's [`eventfd`]
use nix::sys::eventfd::{EfdFlags, EventFd};
use std::sync::Arc;
#[derive(Clone)]
pub struct StreamEventBridge {
    inner: Arc<EventFd>,
}

impl StreamEventBridge {
    pub fn new() -> Self {
        let event_fd = EventFd::from_value_and_flags(0, EfdFlags::EFD_CLOEXEC)
            .expect("Failed to create eventfd");

        Self {
            inner: Arc::new(event_fd),
        }
    }

    #[inline]
    pub fn notify_data_ready(&self) {
        let _ = self.inner.write(1);
    }

    pub fn wait_for_data(&self) {
        // TODO: gracefull shutdown?
        let _ = self.inner.read();
    }
}

impl Default for StreamEventBridge {
    fn default() -> Self {
        Self::new()
    }
}
