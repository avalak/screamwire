//! Lightweight, lock‑free bridge for waking up the network sender when
//! new audio data is available.
//!
//! Based on Linux's [`eventfd`](https://man7.org/linux/man-pages/man2/eventfd.2.html).
use nix::libc;
use nix::sys::eventfd::{EfdFlags, EventFd};
use std::io;
use std::os::fd::AsRawFd;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone)]
pub struct StreamEventBridge {
    inner: Arc<EventFd>,
    flush_requested: Arc<AtomicBool>,
}

impl StreamEventBridge {
    pub fn new() -> Self {
        let flags = EfdFlags::EFD_CLOEXEC | EfdFlags::EFD_NONBLOCK;
        let event_fd = EventFd::from_value_and_flags(0, flags).expect("Failed to create eventfd");

        Self {
            inner: Arc::new(event_fd),
            flush_requested: Arc::new(AtomicBool::new(false)),
        }
    }

    #[inline(always)]
    pub fn notify_data_ready(&self) {
        let _ = self.inner.write(1);
    }

    pub fn wait_for_data(&self) {
        let fd = self.inner.as_raw_fd();

        let mut fds = [libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        }];

        let ret = unsafe { libc::poll(fds.as_mut_ptr(), 1, -1) };

        match ret {
            // Success
            n if n > 0 && (fds[0].revents & libc::POLLIN) != 0 => {
                let mut value: u64 = 0;
                let _ = unsafe {
                    libc::read(
                        fd,
                        &mut value as *mut u64 as *mut libc::c_void,
                        std::mem::size_of::<u64>(),
                    )
                };
            }

            // Signal interrupted
            n if n < 0 => {
                let err = io::Error::last_os_error();
                if err.kind() != io::ErrorKind::Interrupted {
                    log::error!("StreamEventBridge: poll failed: {}", err);
                }
            }

            // No action
            _ => {}
        }
    }

    #[inline(always)]
    pub fn notify_flush(&self) {
        self.flush_requested.store(true, Ordering::Release);
        let _ = self.inner.write(1);
    }

    #[inline(always)]
    pub fn swap_flush_requested(&self) -> bool {
        self.flush_requested.swap(false, Ordering::Acquire)
    }
}

impl Default for StreamEventBridge {
    fn default() -> Self {
        Self::new()
    }
}
