use std::sync::{Arc, Condvar, Mutex};

#[derive(Clone)]
pub struct StreamEventBridge {
    state: Arc<(Mutex<bool>, Condvar)>,
}

impl StreamEventBridge {
    pub fn new() -> Self {
        Self {
            state: Arc::new((Mutex::new(false), Condvar::new())),
        }
    }

    #[inline]
    pub fn notify_data_ready(&self) {
        let (lock, cvar) = &*self.state;
        let mut ready = lock.lock().unwrap();
        *ready = true;
        cvar.notify_one();
    }

    pub fn wait_for_data(&self) {
        let (lock, cvar) = &*self.state;
        let mut ready = lock.lock().unwrap();

        while !*ready {
            ready = cvar.wait(ready).unwrap();
        }
        *ready = false;
    }
}
