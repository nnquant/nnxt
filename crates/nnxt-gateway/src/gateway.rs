//! Gateway core for broadcasting OrderBook snapshots.

use std::time::Duration;

use nnxt_master::protocol::{ActorRegistration, QueueInfo, Request, Response};
use nng::{Protocol, Socket};
use nnxt_rapid::Address;
use nnxt_rapid::Writer;
use nnxt_specs::market::InstrumentId;
use nnxt_specs::OrderBook;
use nnxt_utils::clock::{Clock, InstantClock, MonotonicClock};
use nnxt_utils::setup_signal;

use crate::simulator::{SimulatedSource, SourceConfig};
use crate::stats::LatencyStats;
use crate::subscription::SubscriptionManager;

const DEFAULT_QUEUE_PATH: &str = "market/market-gateway/public";
const DEFAULT_QUEUE_CAPACITY: usize = 1024;
const DEFAULT_HEARTBEAT_INTERVAL_MS: u64 = 1000;

#[derive(Debug, Clone)]
pub struct MarketSimulatorConfig {
    pub queue_path: String,
    pub queue_capacity: usize,
    pub heartbeat_interval: Duration,
    pub master_addr: Option<String>,
    pub actor_id: String,
    pub actor_type: String,
    pub source: SourceConfig,
}

impl Default for MarketSimulatorConfig {
    fn default() -> Self {
        Self {
            queue_path: DEFAULT_QUEUE_PATH.to_string(),
            queue_capacity: DEFAULT_QUEUE_CAPACITY,
            heartbeat_interval: Duration::from_millis(DEFAULT_HEARTBEAT_INTERVAL_MS),
            master_addr: None,
            actor_id: "market-gateway".to_string(),
            actor_type: "market-gateway".to_string(),
            source: SourceConfig::default(),
        }
    }
}

#[derive(Debug)]
pub enum GatewayError {
    Address(nnxt_rapid::AddressError),
    Rapid(nnxt_rapid::Error),
    MasterConnect(nng::Error),
    MasterProtocol(String),
}

pub trait MarketGateway {
    fn subscribe(&mut self, instrument_id: InstrumentId) -> bool;
    fn unsubscribe(&mut self, instrument_id: &InstrumentId) -> bool;
    fn run(&mut self) -> Result<(), GatewayError>;
}

pub struct MarketSimulator {
    writer: Writer<OrderBook>,
    subscriptions: SubscriptionManager,
    source: SimulatedSource,
    stats: LatencyStats,
    clock: Box<dyn Clock + Send + Sync>,
    heartbeat_interval_ns: u64,
    last_heartbeat_ns: u64,
    master: Option<MasterClient>,
    actor_id: String,
    queue_path: String,
}

impl MarketGateway for MarketSimulator {
    fn subscribe(&mut self, instrument_id: InstrumentId) -> bool {
        self.subscribe(instrument_id)
    }

    fn unsubscribe(&mut self, instrument_id: &InstrumentId) -> bool {
        self.unsubscribe(instrument_id)
    }

    fn run(&mut self) -> Result<(), GatewayError> {
        MarketSimulator::run(self)
    }
}

impl MarketSimulator {
    pub fn new(config: MarketSimulatorConfig) -> Result<Self, GatewayError> {
        let addr = Address::new(&config.queue_path).map_err(GatewayError::Address)?;
        let writer = Writer::create(&addr, config.queue_capacity).map_err(GatewayError::Rapid)?;
        let clock = Box::new(InstantClock::new());
        let heartbeat_interval_ns = config.heartbeat_interval.as_nanos() as u64;
        let master = match config.master_addr.as_ref() {
            Some(addr) => Some(MasterClient::new(addr).map_err(GatewayError::MasterConnect)?),
            None => None,
        };

        let mut gateway = Self {
            writer,
            subscriptions: SubscriptionManager::new(),
            source: SimulatedSource::new(config.source),
            stats: LatencyStats::default(),
            clock,
            heartbeat_interval_ns,
            last_heartbeat_ns: 0,
            master,
            actor_id: config.actor_id,
            queue_path: config.queue_path,
        };

        gateway.register_with_master()?;
        Ok(gateway)
    }

    pub fn subscribe(&mut self, instrument_id: InstrumentId) -> bool {
        self.subscriptions.subscribe(instrument_id)
    }

    pub fn unsubscribe(&mut self, instrument_id: &InstrumentId) -> bool {
        self.subscriptions.unsubscribe(instrument_id)
    }

    pub fn stats(&self) -> &LatencyStats {
        &self.stats
    }

    pub fn broadcast_once(&mut self) -> Result<(), GatewayError> {
        self.maybe_send_heartbeat()?;
        for instrument_id in self.subscriptions.iter().copied() {
            let start = self.clock.now_ns();
            let mut order_book = self.source.generate(instrument_id, 0);
            // Set timestamp right before write to measure true queue latency
            order_book.timestamp = MonotonicClock::now_ns();
            self.writer.write(order_book);
            let end = self.clock.now_ns();
            self.stats.record(end.saturating_sub(start));
        }
        Ok(())
    }

    pub fn run(&mut self) -> Result<(), GatewayError> {
        let shutdown = setup_signal();
        let interval = self.source.interval();
        loop {
            if shutdown.is_shutdown() {
                break;
            }
            self.broadcast_once()?;
            std::thread::sleep(interval);
        }
        Ok(())
    }

    fn register_with_master(&mut self) -> Result<(), GatewayError> {
        let Some(master) = self.master.as_mut() else {
            return Ok(());
        };

        let registration = ActorRegistration {
            actor_id: self.actor_id.clone(),
            actor_type: "market-gateway".to_string(),
            queues: vec![QueueInfo {
                addr: self.queue_path.clone(),
                queue_type: "market".to_string(),
            }],
        };

        master.send(Request::Register { actor: registration })?;
        self.last_heartbeat_ns = self.clock.now_ns();
        Ok(())
    }

    fn maybe_send_heartbeat(&mut self) -> Result<(), GatewayError> {
        let Some(master) = self.master.as_mut() else {
            return Ok(());
        };

        let now_ns = self.clock.now_ns();
        if now_ns.saturating_sub(self.last_heartbeat_ns) < self.heartbeat_interval_ns {
            return Ok(());
        }

        master.send(Request::Heartbeat {
            actor_id: self.actor_id.clone(),
        })?;
        self.last_heartbeat_ns = now_ns;
        Ok(())
    }
}

struct MasterClient {
    socket: Socket,
}

impl MasterClient {
    fn new(addr: &str) -> Result<Self, nng::Error> {
        let socket = Socket::new(Protocol::Req0)?;
        socket.dial(addr)?;
        Ok(Self { socket })
    }

    fn send(&mut self, request: Request) -> Result<Response, GatewayError> {
        let payload = serde_json::to_vec(&request)
            .map_err(|err| GatewayError::MasterProtocol(err.to_string()))?;
        self.socket
            .send(payload.as_slice())
            .map_err(|(_, err)| GatewayError::MasterConnect(err))?;

        let msg = self.socket.recv().map_err(GatewayError::MasterConnect)?;
        let response: Response = serde_json::from_slice(msg.as_slice())
            .map_err(|err| GatewayError::MasterProtocol(err.to_string()))?;

        match response {
            Response::Error { message } => Err(GatewayError::MasterProtocol(message)),
            _ => Ok(response),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nnxt_rapid::cleanup;
    use std::str::FromStr;

    #[test]
    fn gateway_subscription_flow() {
        let queue_path = "test/gateway_sub_flow";
        let addr = Address::new(queue_path).expect("addr");
        let _ = cleanup(&addr);

        let config = MarketSimulatorConfig {
            queue_path: queue_path.to_string(),
            ..Default::default()
        };
        let mut gateway = MarketSimulator::new(config).expect("gateway");
        let instrument = InstrumentId::from_str("IF2409").expect("instrument");
        assert!(gateway.subscribe(instrument));
        assert!(!gateway.subscribe(instrument));
        assert!(gateway.unsubscribe(&instrument));

        let _ = cleanup(&addr);
    }
}
