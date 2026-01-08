//! Signal handling utilities.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, OnceLock,
};

#[derive(Clone)]
pub struct ShutdownSignal {
    flag: Arc<AtomicBool>,
}

impl ShutdownSignal {
    pub fn is_shutdown(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }

    pub fn flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.flag)
    }
}

pub fn setup_signal() -> ShutdownSignal {
    static FLAG: OnceLock<Arc<AtomicBool>> = OnceLock::new();
    let flag = FLAG.get_or_init(|| {
        let flag = Arc::new(AtomicBool::new(false));
        let handler_flag = Arc::clone(&flag);
        ctrlc::set_handler(move || {
            handler_flag.store(true, Ordering::SeqCst);
            tracing::warn!("system interrupted");
        })
        .expect("set ctrlc handler");
        flag
    });

    ShutdownSignal {
        flag: Arc::clone(flag),
    }
}
