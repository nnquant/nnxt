//! Reactor event loop implementation.

use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread::JoinHandle;
use std::time::Duration;

use crossbeam_channel::{unbounded, Receiver, Sender};
use nng::options::{Options, RecvTimeout};
use nng::Socket;
use nnxt_rapid::Reader;
use nnxt_utils::clock::{Clock, InstantClock};
use nnxt_utils::setup_signal;

use crate::event::Event;

pub type RapidSourcesHandle = Rc<RefCell<Vec<RapidSource>>>;
pub type ControlHandle = Rc<RefCell<Option<Socket>>>;

const CONTROL_RECV_TIMEOUT_MS: u64 = 1;

pub trait EventSource {
    fn poll(&mut self) -> Option<Event>;
    fn source_id(&self) -> usize {
        0
    }
}

pub struct RapidSource {
    source_id: usize,
    reader: Box<dyn AnyReader>,
}

impl RapidSource {
    pub fn new<T: Copy + Send + 'static>(source_id: usize, reader: Reader<T>) -> Self {
        Self {
            source_id,
            reader: Box::new(TypedReader { inner: reader }),
        }
    }
}

impl EventSource for RapidSource {
    fn poll(&mut self) -> Option<Event> {
        self.reader.try_read_raw().map(|ptr| Event::Data {
            source_id: self.source_id,
            ptr,
        })
    }

    fn source_id(&self) -> usize {
        self.source_id
    }
}

pub struct ControlSource {
    handle: ControlHandle,
}

impl ControlSource {
    pub fn new(handle: ControlHandle) -> Self {
        Self { handle }
    }
}

impl EventSource for ControlSource {
    fn poll(&mut self) -> Option<Event> {
        let mut handle = self.handle.borrow_mut();
        let socket = handle.as_mut()?;
        match socket.recv() {
            Ok(msg) => Some(Event::Control {
                message: msg.as_slice().to_vec(),
            }),
            Err(nng::Error::TimedOut) => None,
            Err(_) => None,
        }
    }
}

pub struct ChannelSource {
    rx: Receiver<Box<dyn Any + Send>>,
}

impl ChannelSource {
    pub fn new(rx: Receiver<Box<dyn Any + Send>>) -> Self {
        Self { rx }
    }
}

impl EventSource for ChannelSource {
    fn poll(&mut self) -> Option<Event> {
        match self.rx.try_recv() {
            Ok(data) => Some(Event::External(data)),
            Err(_) => None,
        }
    }
}

struct ControlListener {
    stop: Arc<AtomicBool>,
    handle: JoinHandle<()>,
}

#[derive(Default)]
struct TimerEntry {
    timer_id: u64,
    due_ns: u64,
}

#[derive(Default)]
struct TimerManager {
    timers: Vec<TimerEntry>,
}

impl TimerManager {
    fn add_timer(&mut self, timer_id: u64, due_ns: u64) {
        self.timers.push(TimerEntry { timer_id, due_ns });
    }

    fn poll_due(&mut self, now_ns: u64) -> Option<u64> {
        let mut due_index = None;
        for (idx, timer) in self.timers.iter().enumerate() {
            if timer.due_ns <= now_ns {
                due_index = Some(idx);
                break;
            }
        }
        if let Some(idx) = due_index {
            let timer = self.timers.swap_remove(idx);
            return Some(timer.timer_id);
        }
        None
    }
}

pub struct Reactor {
    rapid_sources: RapidSourcesHandle,
    control_handle: ControlHandle,
    control_source: ControlSource,
    channel_source: ChannelSource,
    external_tx: Sender<Box<dyn Any + Send>>,
    control_rx: Option<Receiver<Vec<u8>>>,
    control_listener: Option<ControlListener>,
    shutdown: nnxt_utils::signal::ShutdownSignal,
    timers: TimerManager,
    poll_count: u64,
    control_poll_interval: u64,
    clock: Box<dyn Clock + Send + Sync>,
}

impl Default for Reactor {
    fn default() -> Self {
        Self::new()
    }
}

impl Reactor {
    pub fn new() -> Self {
        let rapid_sources = Rc::new(RefCell::new(Vec::new()));
        let control_handle = Rc::new(RefCell::new(None));
        let (tx, rx) = unbounded::<Box<dyn Any + Send>>();
        Self {
            rapid_sources,
            control_handle: Rc::clone(&control_handle),
            control_source: ControlSource::new(control_handle),
            channel_source: ChannelSource::new(rx),
            external_tx: tx,
            control_rx: None,
            control_listener: None,
            shutdown: setup_signal(),
            timers: TimerManager::default(),
            poll_count: 0,
            control_poll_interval: 1000,
            clock: Box::new(InstantClock::new()),
        }
    }

    pub fn add_rapid_reader<T: Copy + Send + 'static>(&mut self, reader: Reader<T>) -> usize {
        let mut sources = self.rapid_sources.borrow_mut();
        let source_id = sources.len();
        sources.push(RapidSource::new(source_id, reader));
        source_id
    }

    pub fn set_control_socket(&mut self, socket: Socket) -> Result<(), nng::Error> {
        socket.set_opt::<RecvTimeout>(Some(Duration::from_millis(0)))?;
        *self.control_handle.borrow_mut() = Some(socket);
        Ok(())
    }

    pub fn external_sender(&self) -> Sender<Box<dyn Any + Send>> {
        self.external_tx.clone()
    }

    pub fn poll(&mut self) -> Option<Event> {
        if self.shutdown.is_shutdown() {
            return Some(Event::Shutdown);
        }

        self.ensure_control_listener();
        for source in self.rapid_sources.borrow_mut().iter_mut() {
            if let Some(event) = source.poll() {
                return Some(event);
            }
        }

        if let Some(event) = self.channel_source.poll() {
            return Some(event);
        }

        if let Some(rx) = self.control_rx.as_ref() {
            if let Ok(message) = rx.try_recv() {
                return Some(Event::Control { message });
            }
        }

        self.poll_count = self.poll_count.wrapping_add(1);
        if self.poll_count % self.control_poll_interval == 0 {
            if self.control_listener.is_none() {
                if let Some(event) = self.control_source.poll() {
                    return Some(event);
                }
            }
            if let Some(timer_id) = self.timers.poll_due(self.clock.now_ns()) {
                return Some(Event::Timer { timer_id });
            }
        }

        None
    }

    pub fn rapid_sources_handle(&self) -> RapidSourcesHandle {
        Rc::clone(&self.rapid_sources)
    }

    pub fn control_handle(&self) -> ControlHandle {
        Rc::clone(&self.control_handle)
    }

    pub fn add_timer(&mut self, timer_id: u64, due_ns: u64) {
        self.timers.add_timer(timer_id, due_ns);
    }

    fn ensure_control_listener(&mut self) {
        if self.control_listener.is_some() {
            return;
        }
        let socket = self.control_handle.borrow().as_ref().cloned();
        let Some(socket) = socket else {
            return;
        };

        let (tx, rx) = unbounded::<Vec<u8>>();
        let stop = Arc::new(AtomicBool::new(false));
        let stop_flag = Arc::clone(&stop);
        let shutdown = self.shutdown.clone();
        let thread_socket = socket.clone();
        let _ = thread_socket
            .set_opt::<RecvTimeout>(Some(Duration::from_millis(CONTROL_RECV_TIMEOUT_MS)));

        let handle = std::thread::Builder::new()
            .name("nnxt-control-listener".to_string())
            .spawn(move || loop {
                if stop_flag.load(Ordering::SeqCst) || shutdown.is_shutdown() {
                    break;
                }
                match thread_socket.recv() {
                    Ok(msg) => {
                        if tx.send(msg.as_slice().to_vec()).is_err() {
                            break;
                        }
                    }
                    Err(nng::Error::TimedOut) => continue,
                    Err(_) => break,
                }
            });

        if let Ok(handle) = handle {
            self.control_rx = Some(rx);
            self.control_listener = Some(ControlListener { stop, handle });
        }
    }
}

impl Drop for Reactor {
    fn drop(&mut self) {
        if let Some(listener) = self.control_listener.take() {
            listener.stop.store(true, Ordering::SeqCst);
            let _ = listener.handle.join();
        }
    }
}

pub trait AnyReader: Send {
    fn try_read_raw(&mut self) -> Option<*const u8>;
}

struct TypedReader<T: Copy + Send + 'static> {
    inner: Reader<T>,
}

impl<T: Copy + Send + 'static> AnyReader for TypedReader<T> {
    fn try_read_raw(&mut self) -> Option<*const u8> {
        self.inner.try_read().map(|v| v as *const T as *const u8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nnxt_rapid::{cleanup, Address, Writer};
    use nng::{Protocol, Socket};
    use std::time::Duration;

    #[test]
    fn reactor_polls_channel_event() {
        let mut reactor = Reactor::new();
        let sender = reactor.external_sender();
        sender.send(Box::new(123u64)).expect("send");
        match reactor.poll() {
            Some(Event::External(_)) => {}
            other => panic!("expected external event, got {other:?}"),
        }
    }

    #[test]
    fn reactor_polls_rapid_event() {
        let addr = Address::new("test/reactor").expect("addr");
        let _ = cleanup(&addr);
        let mut writer = Writer::<u64>::create(&addr, 8).expect("writer");
        let reader = Reader::<u64>::connect(&addr).expect("reader");

        let mut reactor = Reactor::new();
        reactor.add_rapid_reader(reader);

        writer.write(42u64);
        match reactor.poll() {
            Some(Event::Data { ptr, .. }) => {
                let value = unsafe { *(ptr as *const u64) };
                assert_eq!(value, 42u64);
            }
            other => panic!("expected data event, got {other:?}"),
        }
    }

    #[test]
    fn reactor_polls_control_event_with_listener() {
        let addr = format!("ipc:///tmp/nnxt-reactor-control-{}", std::process::id());
        let server = Socket::new(Protocol::Rep0).expect("control socket");
        server.listen(&addr).expect("listen");

        let mut reactor = Reactor::new();
        reactor.set_control_socket(server).expect("set control");

        let client = Socket::new(Protocol::Req0).expect("client socket");
        client.dial(&addr).expect("dial");
        client.send(b"ping".as_slice()).expect("send");

        let mut handled = false;
        for _ in 0..50 {
            if let Some(Event::Control { message }) = reactor.poll() {
                assert_eq!(message, b"ping".to_vec());
                handled = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(handled);
    }
}
