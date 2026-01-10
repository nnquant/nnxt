//! Gateway runners for driving Python gateways.

use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::time::Duration;

use nnxt_actors::{ActorContext, Event, Reactor};
use nnxt_master::protocol::QueueInfo;
use nnxt_specs::market::{InstrumentId, InstrumentIdError};
use nnxt_strategy::{Action, MasterClient, MasterClientError};
use nnxt_utils::clock::{Clock, InstantClock};
use nnxt_utils::signal::ShutdownSignal;
use nnxt_utils::setup_signal;
use nng::{Protocol, Socket};
use tracing::warn;

const DEFAULT_HEARTBEAT_INTERVAL_MS: u64 = 1000;
const DEFAULT_MARKET_QUEUE_PATH: &str = "market/market-gateway/public";
const DEFAULT_TRADE_ORDER_QUEUE: &str = "trade/order-event";
const DEFAULT_TRADE_TRADE_QUEUE: &str = "trade/trade-event";
const ACTION_QUEUE_RETRY_DELAY_MS: u64 = 200;
const ACTION_QUEUE_MAX_RETRIES: u32 = 10;

pub trait MarketGatewayCallbacks: Send {
    fn on_start(&mut self) -> Result<(), RunnerError>;
    fn on_subscribe(&mut self, instrument_id: InstrumentId) -> Result<(), RunnerError>;
    fn on_unsubscribe(&mut self, instrument_id: InstrumentId) -> Result<(), RunnerError>;
    fn on_stop(&mut self) -> Result<(), RunnerError>;
}

pub trait TradeGatewayCallbacks: Send {
    fn on_start(&mut self) -> Result<(), RunnerError>;
    fn on_action(&mut self, action: &Action) -> Result<(), RunnerError>;
    fn on_stop(&mut self) -> Result<(), RunnerError>;
}

#[derive(Debug)]
pub enum RunnerError {
    Address(nnxt_rapid::AddressError),
    Rapid(nnxt_rapid::Error),
    Master(MasterClientError),
    Actors(nnxt_actors::Error),
    Nng(nng::Error),
    Instrument(InstrumentIdError),
    Callback(String),
}

impl From<nnxt_rapid::AddressError> for RunnerError {
    fn from(value: nnxt_rapid::AddressError) -> Self {
        Self::Address(value)
    }
}

impl From<nnxt_rapid::Error> for RunnerError {
    fn from(value: nnxt_rapid::Error) -> Self {
        Self::Rapid(value)
    }
}

impl From<MasterClientError> for RunnerError {
    fn from(value: MasterClientError) -> Self {
        Self::Master(value)
    }
}

impl From<nnxt_actors::Error> for RunnerError {
    fn from(value: nnxt_actors::Error) -> Self {
        Self::Actors(value)
    }
}

impl From<nng::Error> for RunnerError {
    fn from(value: nng::Error) -> Self {
        Self::Nng(value)
    }
}

impl From<InstrumentIdError> for RunnerError {
    fn from(value: InstrumentIdError) -> Self {
        Self::Instrument(value)
    }
}

#[derive(Debug, Clone)]
pub struct MarketGatewayRunnerConfig {
    pub queue_path: String,
    pub master_addr: Option<String>,
    pub actor_id: String,
    pub actor_type: String,
    pub heartbeat_interval: Duration,
    pub control_addr: Option<String>,
}

impl Default for MarketGatewayRunnerConfig {
    fn default() -> Self {
        Self {
            queue_path: DEFAULT_MARKET_QUEUE_PATH.to_string(),
            master_addr: None,
            actor_id: "market-gateway".to_string(),
            actor_type: "market-gateway".to_string(),
            heartbeat_interval: Duration::from_millis(DEFAULT_HEARTBEAT_INTERVAL_MS),
            control_addr: None,
        }
    }
}

pub struct MarketGatewayRunner<G: MarketGatewayCallbacks> {
    gateway: G,
    config: MarketGatewayRunnerConfig,
    master: Option<MasterClient>,
    heartbeat_interval_ns: u64,
    last_heartbeat_ns: u64,
    clock: Box<dyn Clock + Send + Sync>,
    control_addr: Option<String>,
    shutdown: ShutdownSignal,
}

impl<G: MarketGatewayCallbacks> MarketGatewayRunner<G> {
    pub fn new(gateway: G, config: MarketGatewayRunnerConfig) -> Result<Self, RunnerError> {
        let master = match config.master_addr.as_ref() {
            Some(addr) => Some(MasterClient::new(addr).map_err(RunnerError::Nng)?),
            None => None,
        };
        let control_addr = resolve_control_addr(&config, master.is_some());
        Ok(Self {
            gateway,
            heartbeat_interval_ns: config.heartbeat_interval.as_nanos() as u64,
            last_heartbeat_ns: 0,
            clock: Box::new(InstantClock::new()),
            config,
            master,
            control_addr,
            shutdown: setup_signal(),
        })
    }

    pub fn run(&mut self) -> Result<(), RunnerError> {
        let mut reactor = Reactor::new();
        let external_tx = reactor.external_sender();
        let mut ctx = ActorContext::new(
            reactor.rapid_sources_handle(),
            reactor.control_handle(),
            external_tx,
        );

        self.start_with_ctx(&mut ctx)?;

        loop {
            if self.shutdown.is_shutdown() {
                self.gateway.on_stop()?;
                break;
            }
            if let Some(event) = reactor.poll() {
                if self.handle_event(event, &mut ctx)? {
                    break;
                }
            } else {
                self.maybe_send_heartbeat();
                std::thread::sleep(Duration::from_millis(1));
            }
        }

        Ok(())
    }

    fn start_with_ctx(&mut self, ctx: &mut ActorContext) -> Result<(), RunnerError> {
        self.setup_control_socket(ctx)?;
        self.register_with_master()?;
        self.gateway.on_start()?;
        Ok(())
    }

    fn handle_event(&mut self, event: Event, ctx: &mut ActorContext) -> Result<bool, RunnerError> {
        match event {
            Event::Control { message } => {
                self.handle_control_message(&message, ctx);
                self.maybe_send_heartbeat();
                Ok(false)
            }
            Event::Shutdown => {
                self.gateway.on_stop()?;
                Ok(true)
            }
            _ => {
                self.maybe_send_heartbeat();
                Ok(false)
            }
        }
    }

    fn handle_control_message(&mut self, message: &[u8], ctx: &mut ActorContext) {
        let command = match std::str::from_utf8(message) {
            Ok(value) => value.trim(),
            Err(_) => {
                let _ = reply_control(ctx, b"error: invalid utf8");
                return;
            }
        };

        let result = match parse_market_command(command) {
            Ok(MarketControlCommand::Subscribe { instrument }) => {
                self.gateway.on_subscribe(instrument)
            }
            Ok(MarketControlCommand::Unsubscribe { instrument }) => {
                self.gateway.on_unsubscribe(instrument)
            }
            Err(err) => Err(err),
        };

        let reply: &[u8] = if result.is_ok() { b"ok" } else { b"error" };
        let _ = reply_control(ctx, reply);
    }

    fn setup_control_socket(&mut self, ctx: &mut ActorContext) -> Result<(), RunnerError> {
        let Some(control_addr) = self.control_addr.as_ref() else {
            return Ok(());
        };
        let socket = Socket::new(Protocol::Rep0)?;
        socket.listen(control_addr)?;
        ctx.set_control_socket(socket)?;
        Ok(())
    }

    fn register_with_master(&mut self) -> Result<(), RunnerError> {
        let Some(master) = self.master.as_mut() else {
            return Ok(());
        };
        let mut queues = Vec::new();
        queues.push(QueueInfo {
            addr: self.config.queue_path.clone(),
            queue_type: "market".to_string(),
        });
        if let Some(control_addr) = self.control_addr.as_ref() {
            queues.push(QueueInfo {
                addr: control_addr.clone(),
                queue_type: "control".to_string(),
            });
        }
        master.register_actor(&self.config.actor_id, &self.config.actor_type, queues)?;
        self.last_heartbeat_ns = self.clock.now_ns();
        Ok(())
    }

    fn maybe_send_heartbeat(&mut self) {
        let Some(master) = self.master.as_mut() else {
            return;
        };
        let now_ns = self.clock.now_ns();
        if now_ns.saturating_sub(self.last_heartbeat_ns) < self.heartbeat_interval_ns {
            return;
        }
        // Ignore heartbeat errors - master may be unavailable
        let _ = master.heartbeat(&self.config.actor_id);
        self.last_heartbeat_ns = now_ns;
    }
}

#[derive(Debug, Clone)]
pub struct TradeGatewayRunnerConfig {
    pub order_event_queue: String,
    pub trade_event_queue: String,
    pub action_queues: Vec<String>,
    pub master_addr: Option<String>,
    pub actor_id: String,
    pub actor_type: String,
    pub heartbeat_interval: Duration,
    pub control_addr: Option<String>,
}

impl Default for TradeGatewayRunnerConfig {
    fn default() -> Self {
        Self {
            order_event_queue: DEFAULT_TRADE_ORDER_QUEUE.to_string(),
            trade_event_queue: DEFAULT_TRADE_TRADE_QUEUE.to_string(),
            action_queues: Vec::new(),
            master_addr: None,
            actor_id: "trade-gateway".to_string(),
            actor_type: "trade-gateway".to_string(),
            heartbeat_interval: Duration::from_millis(DEFAULT_HEARTBEAT_INTERVAL_MS),
            control_addr: None,
        }
    }
}

pub struct TradeGatewayRunner<G: TradeGatewayCallbacks> {
    gateway: G,
    config: TradeGatewayRunnerConfig,
    master: Option<MasterClient>,
    heartbeat_interval_ns: u64,
    last_heartbeat_ns: u64,
    clock: Box<dyn Clock + Send + Sync>,
    control_addr: Option<String>,
    action_sources: HashMap<usize, String>,
    action_queue_paths: HashSet<String>,
    shutdown: ShutdownSignal,
}

impl<G: TradeGatewayCallbacks> TradeGatewayRunner<G> {
    pub fn new(gateway: G, config: TradeGatewayRunnerConfig) -> Result<Self, RunnerError> {
        let master = match config.master_addr.as_ref() {
            Some(addr) => Some(MasterClient::new(addr).map_err(RunnerError::Nng)?),
            None => None,
        };
        let control_addr = resolve_control_addr(&config, master.is_some());
        Ok(Self {
            gateway,
            heartbeat_interval_ns: config.heartbeat_interval.as_nanos() as u64,
            last_heartbeat_ns: 0,
            clock: Box::new(InstantClock::new()),
            config,
            master,
            control_addr,
            action_sources: HashMap::new(),
            action_queue_paths: HashSet::new(),
            shutdown: setup_signal(),
        })
    }

    pub fn run(&mut self) -> Result<(), RunnerError> {
        let mut reactor = Reactor::new();
        let external_tx = reactor.external_sender();
        let mut ctx = ActorContext::new(
            reactor.rapid_sources_handle(),
            reactor.control_handle(),
            external_tx,
        );

        self.start_with_ctx(&mut ctx)?;

        loop {
            if self.shutdown.is_shutdown() {
                self.gateway.on_stop()?;
                break;
            }
            if let Some(event) = reactor.poll() {
                if self.handle_event(event, &mut ctx)? {
                    break;
                }
            } else {
                self.maybe_send_heartbeat();
                std::thread::sleep(Duration::from_millis(1));
            }
        }

        Ok(())
    }

    fn start_with_ctx(&mut self, ctx: &mut ActorContext) -> Result<(), RunnerError> {
        self.setup_control_socket(ctx)?;
        self.register_with_master()?;
        self.gateway.on_start()?;
        let queues = self.config.action_queues.clone();
        for addr in queues {
            self.add_action_queue_with_ctx(ctx, &addr)?;
        }
        Ok(())
    }

    fn handle_event(&mut self, event: Event, ctx: &mut ActorContext) -> Result<bool, RunnerError> {
        match event {
            Event::Data { source_id, ptr } => {
                if self.action_sources.contains_key(&source_id) {
                    let action = unsafe { *(ptr as *const Action) };
                    self.gateway.on_action(&action)?;
                }
                self.maybe_send_heartbeat();
                Ok(false)
            }
            Event::Control { message } => {
                self.handle_control_message(&message, ctx);
                self.maybe_send_heartbeat();
                Ok(false)
            }
            Event::Shutdown => {
                self.gateway.on_stop()?;
                Ok(true)
            }
            _ => {
                self.maybe_send_heartbeat();
                Ok(false)
            }
        }
    }

    fn handle_control_message(&mut self, message: &[u8], ctx: &mut ActorContext) {
        let command = match std::str::from_utf8(message) {
            Ok(value) => value.trim(),
            Err(_) => {
                let _ = reply_control(ctx, b"error: invalid utf8");
                return;
            }
        };

        let result = match parse_trade_command(command) {
            Ok(TradeControlCommand::AddActionQueue { addr }) => {
                self.add_action_queue_with_ctx(ctx, &addr)
            }
            Err(err) => Err(err),
        };

        let reply: &[u8] = if result.is_ok() { b"ok" } else { b"error" };
        let _ = reply_control(ctx, reply);
    }

    fn add_action_queue_with_ctx(
        &mut self,
        ctx: &mut ActorContext,
        addr: &str,
    ) -> Result<(), RunnerError> {
        if self.action_queue_paths.contains(addr) {
            return Ok(());
        }
        for attempt in 1..=ACTION_QUEUE_MAX_RETRIES {
            match ctx.read_from::<Action>(addr) {
                Ok(source_id) => {
                    self.action_sources.insert(source_id, addr.to_string());
                    self.action_queue_paths.insert(addr.to_string());
                    return Ok(());
                }
                Err(nnxt_actors::Error::RapidError(nnxt_rapid::Error::NotFound)) => {
                    if attempt == ACTION_QUEUE_MAX_RETRIES {
                        return Err(
                            nnxt_actors::Error::RapidError(nnxt_rapid::Error::NotFound).into(),
                        );
                    }
                    warn!(
                        "action queue not ready event=[ACTION_QUEUE_WAIT] addr=[{}] attempt=[{}/{}]",
                        addr,
                        attempt,
                        ACTION_QUEUE_MAX_RETRIES
                    );
                    std::thread::sleep(Duration::from_millis(ACTION_QUEUE_RETRY_DELAY_MS));
                }
                Err(err) => return Err(err.into()),
            }
        }
        Ok(())
    }

    fn setup_control_socket(&mut self, ctx: &mut ActorContext) -> Result<(), RunnerError> {
        let Some(control_addr) = self.control_addr.as_ref() else {
            return Ok(());
        };
        let socket = Socket::new(Protocol::Rep0)?;
        socket.listen(control_addr)?;
        ctx.set_control_socket(socket)?;
        Ok(())
    }

    fn register_with_master(&mut self) -> Result<(), RunnerError> {
        let Some(master) = self.master.as_mut() else {
            return Ok(());
        };
        let mut queues = Vec::new();
        queues.push(QueueInfo {
            addr: self.config.order_event_queue.clone(),
            queue_type: "order-event".to_string(),
        });
        queues.push(QueueInfo {
            addr: self.config.trade_event_queue.clone(),
            queue_type: "trade-event".to_string(),
        });
        if let Some(control_addr) = self.control_addr.as_ref() {
            queues.push(QueueInfo {
                addr: control_addr.clone(),
                queue_type: "control".to_string(),
            });
        }
        master.register_actor(&self.config.actor_id, &self.config.actor_type, queues)?;
        self.last_heartbeat_ns = self.clock.now_ns();
        Ok(())
    }

    fn maybe_send_heartbeat(&mut self) {
        let Some(master) = self.master.as_mut() else {
            return;
        };
        let now_ns = self.clock.now_ns();
        if now_ns.saturating_sub(self.last_heartbeat_ns) < self.heartbeat_interval_ns {
            return;
        }
        // Ignore heartbeat errors - master may be unavailable
        let _ = master.heartbeat(&self.config.actor_id);
        self.last_heartbeat_ns = now_ns;
    }
}

fn resolve_control_addr<C>(config: &C, needs_control: bool) -> Option<String>
where
    C: HasControlConfig,
{
    if let Some(addr) = config.control_addr() {
        return Some(addr.to_string());
    }
    if needs_control {
        return Some(default_control_addr(config.actor_id()));
    }
    None
}

fn default_control_addr(actor_id: &str) -> String {
    format!("ipc:///tmp/{}", actor_id)
}

fn reply_control(ctx: &mut ActorContext, payload: &[u8]) -> Result<(), RunnerError> {
    match ctx.reply_control(payload) {
        Ok(()) => Ok(()),
        Err(nnxt_actors::Error::ControlNotAvailable) => Ok(()),
        Err(err) => Err(err.into()),
    }
}

trait HasControlConfig {
    fn actor_id(&self) -> &str;
    fn control_addr(&self) -> Option<&str>;
}

impl HasControlConfig for MarketGatewayRunnerConfig {
    fn actor_id(&self) -> &str {
        &self.actor_id
    }

    fn control_addr(&self) -> Option<&str> {
        self.control_addr.as_deref()
    }
}

impl HasControlConfig for TradeGatewayRunnerConfig {
    fn actor_id(&self) -> &str {
        &self.actor_id
    }

    fn control_addr(&self) -> Option<&str> {
        self.control_addr.as_deref()
    }
}

enum MarketControlCommand {
    Subscribe { instrument: InstrumentId },
    Unsubscribe { instrument: InstrumentId },
}

fn parse_market_command(command: &str) -> Result<MarketControlCommand, RunnerError> {
    if let Some(value) = command.strip_prefix("subscribe:") {
        let instrument = InstrumentId::from_str(value.trim())?;
        return Ok(MarketControlCommand::Subscribe { instrument });
    }
    if let Some(value) = command.strip_prefix("unsubscribe:") {
        let instrument = InstrumentId::from_str(value.trim())?;
        return Ok(MarketControlCommand::Unsubscribe { instrument });
    }
    Err(RunnerError::Callback(format!(
        "unknown control command: {}",
        command
    )))
}

enum TradeControlCommand {
    AddActionQueue { addr: String },
}

fn parse_trade_command(command: &str) -> Result<TradeControlCommand, RunnerError> {
    if let Some(value) = command.strip_prefix("add_action_queue:") {
        return Ok(TradeControlCommand::AddActionQueue {
            addr: value.trim().to_string(),
        });
    }
    Err(RunnerError::Callback(format!(
        "unknown control command: {}",
        command
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use nnxt_rapid::{cleanup, Address, Writer};
    use nnxt_specs::Side;
    use nnxt_strategy::{ActionKind, CancelOrder, NewOrder};
    use std::str::FromStr;

    #[derive(Default)]
    struct TestMarketGateway {
        subscribed: Vec<InstrumentId>,
        unsubscribed: Vec<InstrumentId>,
        started: bool,
        stopped: bool,
    }

    impl MarketGatewayCallbacks for TestMarketGateway {
        fn on_start(&mut self) -> Result<(), RunnerError> {
            self.started = true;
            Ok(())
        }

        fn on_subscribe(&mut self, instrument_id: InstrumentId) -> Result<(), RunnerError> {
            self.subscribed.push(instrument_id);
            Ok(())
        }

        fn on_unsubscribe(&mut self, instrument_id: InstrumentId) -> Result<(), RunnerError> {
            self.unsubscribed.push(instrument_id);
            Ok(())
        }

        fn on_stop(&mut self) -> Result<(), RunnerError> {
            self.stopped = true;
            Ok(())
        }
    }

    #[derive(Default)]
    struct TestTradeGateway {
        actions: Vec<Action>,
        started: bool,
        stopped: bool,
    }

    impl TradeGatewayCallbacks for TestTradeGateway {
        fn on_start(&mut self) -> Result<(), RunnerError> {
            self.started = true;
            Ok(())
        }

        fn on_action(&mut self, action: &Action) -> Result<(), RunnerError> {
            self.actions.push(*action);
            Ok(())
        }

        fn on_stop(&mut self) -> Result<(), RunnerError> {
            self.stopped = true;
            Ok(())
        }
    }

    #[test]
    fn market_runner_handles_subscriptions() {
        let config = MarketGatewayRunnerConfig::default();
        let mut runner = MarketGatewayRunner::new(TestMarketGateway::default(), config)
            .expect("runner");
        let reactor = Reactor::new();
        let external_tx = reactor.external_sender();
        let mut ctx = ActorContext::new(
            reactor.rapid_sources_handle(),
            reactor.control_handle(),
            external_tx,
        );

        runner.handle_control_message(b"subscribe:IF2409", &mut ctx);
        runner.handle_control_message(b"unsubscribe:IF2409", &mut ctx);

        assert_eq!(runner.gateway.subscribed.len(), 1);
        assert_eq!(runner.gateway.unsubscribed.len(), 1);
        assert_eq!(
            runner.gateway.subscribed[0].as_str().expect("instrument"),
            "IF2409"
        );
    }

    #[test]
    fn trade_runner_receives_actions() {
        let queue_path = "test/trade_runner_actions";
        let addr = Address::new(queue_path).expect("addr");
        let _ = cleanup(&addr);
        let mut writer = Writer::create(&addr, 16).expect("writer");

        let config = TradeGatewayRunnerConfig::default();
        let mut runner = TradeGatewayRunner::new(TestTradeGateway::default(), config)
            .expect("runner");
        let mut reactor = Reactor::new();
        let external_tx = reactor.external_sender();
        let mut ctx = ActorContext::new(
            reactor.rapid_sources_handle(),
            reactor.control_handle(),
            external_tx,
        );

        runner
            .add_action_queue_with_ctx(&mut ctx, queue_path)
            .expect("add queue");

        let instrument = InstrumentId::from_str("IF2409").expect("instrument");
        let action = Action {
            kind: ActionKind::CancelOrder,
            new_order: NewOrder {
                instrument_id: instrument,
                order_id: 1,
                client_order_id: 1,
                side: Side::Buy,
                price_type: nnxt_specs::PriceType::Limit,
                limit_price: 1.0,
                quantity: 1,
                timestamp: 1,
            },
            cancel_order: CancelOrder {
                instrument_id: instrument,
                order_id: 1,
                timestamp: 1,
            },
        };

        writer.write(action);

        let mut handled = false;
        for _ in 0..50 {
            if let Some(event) = reactor.poll() {
                runner.handle_event(event, &mut ctx).expect("event");
                handled = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }

        assert!(handled);
        assert_eq!(runner.gateway.actions.len(), 1);

        let _ = cleanup(&addr);
    }
}
