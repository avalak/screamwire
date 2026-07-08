//! Lightweight, lock‑free bridge for waking up the network sender when
//! new audio data is available.
//!
//! Based on Linux's [`eventfd`]
use nix::sys::eventfd::{EfdFlags, EventFd};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone)]
pub struct StreamEventBridge {
    inner: Arc<EventFd>,
    flush_requested: Arc<AtomicBool>,
}

impl StreamEventBridge {
    pub fn new() -> Self {
        let event_fd = EventFd::from_value_and_flags(0, EfdFlags::EFD_CLOEXEC)
            .expect("Failed to create eventfd");

        Self {
            inner: Arc::new(event_fd),
            flush_requested: Arc::new(AtomicBool::new(false)),
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

    #[inline]
    pub fn notify_flush(&self) {
        self.flush_requested.store(true, Ordering::Release);
        let _ = self.inner.write(1);
    }

    #[inline]
    pub fn swap_flush_requested(&self) -> bool {
        self.flush_requested.swap(false, Ordering::Acquire)
    }
}

impl Default for StreamEventBridge {
    fn default() -> Self {
        Self::new()
    }
}
