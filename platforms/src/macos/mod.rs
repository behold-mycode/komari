use std::{
    sync::{
        Arc, Barrier,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};

mod bitblt;
mod error;
mod handle;
mod keys;
pub mod screenshot;

pub use {bitblt::*, error::*, handle::*, keys::*, screenshot::*};
pub use keys::{client_to_monitor_or_frame, KeyInputKind, KeysManager as Keys, KeyReceiver};

#[derive(Clone, Debug)]
pub struct Frame {
    pub width: i32,
    pub height: i32,
    pub data: Vec<u8>,
}

pub fn init() {
    static INITIALIZED: AtomicBool = AtomicBool::new(false);

    if INITIALIZED
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::Acquire)
        .is_ok()
    {
        let barrier = Arc::new(Barrier::new(2));
        let keys_barrier = barrier.clone();
        thread::spawn(move || {
            let _hook = keys::init();
            keys_barrier.wait();
            // macOS input handling runs until shutdown
            keys::run_event_loop();
        });
        barrier.wait();
    }
}