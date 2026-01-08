//! Event abstractions for actor reactors.

use std::any::Any;

/// Unified event type for actor reactors.
#[derive(Debug)]
pub enum Event {
    /// rapid queue data event.
    Data { source_id: usize, ptr: *const u8 },
    /// nanomsg control message event.
    Control { message: Vec<u8> },
    /// External thread pushed event.
    External(Box<dyn Any + Send>),
    /// Timer fired event.
    Timer { timer_id: u64 },
    /// Shutdown signal.
    Shutdown,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_variants_construct() {
        let _ = Event::Data {
            source_id: 0,
            ptr: std::ptr::null(),
        };
        let _ = Event::Control {
            message: b"ping".to_vec(),
        };
        let _ = Event::External(Box::new(123u64));
        let _ = Event::Timer { timer_id: 42 };
        let _ = Event::Shutdown;
    }
}
