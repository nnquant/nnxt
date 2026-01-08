//! Strategy runner loop and wiring.

use nnxt_actors::{Actor, ActorContext, Event, Reactor};
use crate::action::{Action, ActionKind};
use crate::context::{
    MarketDataType, PendingRequests, QuoteSubscription, StrategyContext, TimerManager,
    TradeConnection,
};
use crate::execution::ExecutionEngine;
use crate::master_client::{MasterClient, MasterClientError};
use crate::state::RunnerState;
use crate::strategy::Strategy;
use nnxt_master::protocol::QueueInfo;
use nnxt_specs::{InstrumentId, OrderBook, OrderEvent, TradeEvent};
use std::time::Duration;
use tracing::{info, warn};
use nnxt_utils::action_queue;
use nnxt_utils::clock::{Clock, InstantClock};
use nnxt_utils::setup_signal;

#[derive(Debug, Clone)]
pub struct RunnerConfig {
    pub action_queue_path: Option<String>,
    pub action_queue_capacity: usize,
    pub master_addr: Option<String>,
    pub actor_id: String,
    pub actor_type: String,
}

impl Default for RunnerConfig {
    fn default() -> Self {
        Self {
            action_queue_path: None,
            action_queue_capacity: 1024,
            master_addr: None,
            actor_id: "strategy-runner".to_string(),
            actor_type: "strategy".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
struct QuoteSource {
    instrument_id: InstrumentId,
    source_id: usize,
}

pub struct StrategyRunner<S: Strategy> {
    strategy: S,
    state: RunnerState,
    engine: ExecutionEngine,
    timers: TimerManager,
    clock: Box<dyn Clock + Send + Sync>,
    master: Option<MasterClient>,
    actor_id: String,
    actor_type: String,
    action_queue_path: Option<String>,
    action_queue_capacity: usize,
    action_queue_addr: Option<String>,
    quote_sources: Vec<QuoteSource>,
    order_event_source_id: Option<usize>,
    trade_event_source_id: Option<usize>,
    pending_subscriptions: Vec<QuoteSubscription>,
    pending_trade: Option<TradeConnection>,
}

#[derive(Debug)]
pub enum RunnerError {
    Address(nnxt_rapid::AddressError),
    Rapid(nnxt_rapid::Error),
    Master(MasterClientError),
    Actors(nnxt_actors::Error),
    Shutdown,
    UnsupportedDataType { data_type: MarketDataType },
    QueueNotFound { queue_type: String },
}

impl From<nnxt_rapid::Error> for RunnerError {
    fn from(value: nnxt_rapid::Error) -> Self {
        Self::Rapid(value)
    }
}

impl From<nnxt_rapid::AddressError> for RunnerError {
    fn from(value: nnxt_rapid::AddressError) -> Self {
        Self::Address(value)
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

impl<S: Strategy> StrategyRunner<S> {
    pub fn new(strategy: S, config: RunnerConfig) -> Result<Self, RunnerError> {
        let master = match config.master_addr.as_ref() {
            Some(addr) => Some(
                MasterClient::new(addr)
                    .map_err(|err| RunnerError::Master(MasterClientError::Transport(err)))?,
            ),
            None => None,
        };

        Ok(Self {
            strategy,
            state: RunnerState::default(),
            engine: ExecutionEngine::new(),
            timers: TimerManager::default(),
            clock: Box::new(InstantClock::new()),
            master,
            actor_id: config.actor_id,
            actor_type: config.actor_type,
            action_queue_path: config.action_queue_path,
            action_queue_capacity: config.action_queue_capacity,
            action_queue_addr: None,
            quote_sources: Vec::new(),
            order_event_source_id: None,
            trade_event_source_id: None,
            pending_subscriptions: Vec::new(),
            pending_trade: None,
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
            if let Some(event) = reactor.poll() {
                let should_stop = self.handle_event_with_ctx(event, &mut ctx)?;
                if should_stop {
                    break;
                }
            } else {
                std::hint::spin_loop();
            }
        }

        Ok(())
    }

    fn start_with_ctx(&mut self, ctx: &mut ActorContext) -> Result<(), RunnerError> {
        self.register_with_master()?;
        let now_ns = self.clock.now_ns();
        let mut ctx_strategy = StrategyContext::new(now_ns, &self.state, &mut self.timers);
        self.strategy.on_start(&mut ctx_strategy);
        let pending = ctx_strategy.into_pending();
        self.handle_pending(pending, ctx)?;
        Ok(())
    }

    fn handle_event_with_ctx(
        &mut self,
        event: Event,
        ctx: &mut ActorContext,
    ) -> Result<bool, RunnerError> {
        match event {
            Event::Data { source_id, ptr } => {
                self.handle_data_event(source_id, ptr, ctx)?;
                self.tick_timers(ctx);
                Ok(false)
            }
            Event::Control { .. } => {
                self.tick_timers(ctx);
                Ok(false)
            }
            Event::External(_) => {
                self.tick_timers(ctx);
                Ok(false)
            }
            Event::Timer { .. } => {
                self.tick_timers(ctx);
                Ok(false)
            }
            Event::Shutdown => {
                self.on_shutdown(ctx)?;
                Ok(true)
            }
        }
    }

    fn on_shutdown(&mut self, ctx: &mut ActorContext) -> Result<(), RunnerError> {
        let now_ns = self.clock.now_ns();
        let mut ctx_strategy = StrategyContext::new(now_ns, &self.state, &mut self.timers);
        self.strategy.on_stop(&mut ctx_strategy);
        let pending = ctx_strategy.into_pending();
        self.handle_pending(pending, ctx)?;
        Ok(())
    }

    fn handle_data_event(
        &mut self,
        source_id: usize,
        ptr: *const u8,
        ctx: &mut ActorContext,
    ) -> Result<(), RunnerError> {
        if let Some(source) = self
            .quote_sources
            .iter()
            .find(|source| source.source_id == source_id)
        {
            // SAFETY: 对应 source_id 只注册 OrderBook
            let book = unsafe { *(ptr as *const OrderBook) };
            self.on_order_book(book, source.instrument_id, ctx);
            return Ok(());
        }

        if Some(source_id) == self.order_event_source_id {
            let event = unsafe { *(ptr as *const OrderEvent) };
            self.on_order_event(event, ctx);
            return Ok(());
        }

        if Some(source_id) == self.trade_event_source_id {
            let event = unsafe { *(ptr as *const TradeEvent) };
            self.on_trade_event(event, ctx);
        }

        Ok(())
    }

    fn handle_pending(
        &mut self,
        pending: PendingRequests,
        ctx: &mut ActorContext,
    ) -> Result<(), RunnerError> {
        let now_ns = self.clock.now_ns();
        let actions = self.engine.execute(&pending.intents, &self.state, now_ns);
        self.apply_actions(now_ns, &actions);
        self.write_actions(&actions, ctx);

        for subscription in pending.subscriptions {
            self.pending_subscriptions.push(subscription);
        }
        self.pending_trade = pending.trade_connection;

        self.connect_subscriptions(ctx)?;
        self.connect_trade(ctx)?;
        Ok(())
    }

    fn connect_subscriptions(&mut self, ctx: &mut ActorContext) -> Result<(), RunnerError> {
        let subscriptions = std::mem::take(&mut self.pending_subscriptions);
        for subscription in subscriptions {
            let addr = if let Some(master) = self.master.as_mut() {
                info!("queue request event=[QUEUE_REQUEST] queue_type=[market]");
                wait_for_queue(master, "market")?
            } else {
                subscription.source
            };
            info!(
                "queue resolved event=[QUEUE_READY] queue_type=[market] addr=[{}]",
                addr
            );
            match subscription.data_type {
                MarketDataType::OrderBook => {
                    let source_id = read_from_with_retry::<OrderBook>(ctx, &addr)?;
                    info!(
                        "queue connected event=[QUEUE_CONNECTED] queue_type=[market] addr=[{}] source_id=[{}]",
                        addr, source_id
                    );
                    self.quote_sources.push(QuoteSource {
                        instrument_id: subscription.instrument_id,
                        source_id,
                    });
                }
            }
        }
        Ok(())
    }

    fn connect_trade(&mut self, ctx: &mut ActorContext) -> Result<(), RunnerError> {
        let Some(trade) = self.pending_trade.take() else {
            return Ok(());
        };

        if let Some(master) = self.master.as_mut() {
            info!("queue request event=[QUEUE_REQUEST] queue_type=[trade]");
            let (trade_gateway_id, order_event_queue, trade_event_queue) =
                master.connect_trade(&trade.source, &self.actor_id)?;

            let action_path = action_queue(&self.actor_id, &trade_gateway_id);
            info!(
                "queue resolved event=[QUEUE_READY] queue_type=[action] addr=[{}]",
                action_path
            );
            ctx.write_to::<Action>(&action_path, self.action_queue_capacity)?;
            self.action_queue_addr = Some(action_path.clone());
            info!(
                "queue connected event=[QUEUE_CONNECTED] queue_type=[action] addr=[{}]",
                action_path
            );
            let action_queue = QueueInfo {
                addr: action_path.clone(),
                queue_type: "action".to_string(),
            };
            master.register_queue(
                &self.actor_id,
                action_queue,
                Some(trade_gateway_id.clone()),
            )?;

            info!(
                "queue resolved event=[QUEUE_READY] queue_type=[order-event] addr=[{}]",
                order_event_queue
            );
            let order_source_id = read_from_with_retry::<OrderEvent>(ctx, &order_event_queue)?;
            self.order_event_source_id = Some(order_source_id);
            info!(
                "queue connected event=[QUEUE_CONNECTED] queue_type=[order-event] addr=[{}] source_id=[{}]",
                order_event_queue, order_source_id
            );

            info!(
                "queue resolved event=[QUEUE_READY] queue_type=[trade-event] addr=[{}]",
                trade_event_queue
            );
            let trade_source_id = read_from_with_retry::<TradeEvent>(ctx, &trade_event_queue)?;
            self.trade_event_source_id = Some(trade_source_id);
            info!(
                "queue connected event=[QUEUE_CONNECTED] queue_type=[trade-event] addr=[{}] source_id=[{}]",
                trade_event_queue, trade_source_id
            );
        } else {
            let action_addr = self
                .action_queue_path
                .clone()
                .unwrap_or(trade.source.clone());
            info!(
                "queue resolved event=[QUEUE_READY] queue_type=[trade] addr=[{}]",
                action_addr
            );
            ctx.write_to::<Action>(&action_addr, self.action_queue_capacity)?;
            self.action_queue_addr = Some(action_addr.clone());
            info!(
                "queue connected event=[QUEUE_CONNECTED] queue_type=[trade] addr=[{}]",
                action_addr
            );
        }

        Ok(())
    }

    fn on_order_book(&mut self, book: OrderBook, instrument_id: InstrumentId, ctx: &mut ActorContext) {
        if book.instrument_id != instrument_id {
            return;
        }
        self.state.update_market(book);
        let now_ns = self.clock.now_ns();
        let mut ctx_strategy = StrategyContext::new(now_ns, &self.state, &mut self.timers);
        self.strategy.on_order_book(&book, &mut ctx_strategy);
        let intents = ctx_strategy.into_intents();
        let actions = self.engine.execute(&intents, &self.state, now_ns);
        self.apply_actions(now_ns, &actions);
        self.write_actions(&actions, ctx);
    }

    fn on_order_event(&mut self, event: OrderEvent, ctx: &mut ActorContext) {
        tracing::debug!(
            "order event received event=[ORDER_EVENT] order_id=[{}] status=[{:?}] filled_quantity=[{}] remaining_quantity=[{}]",
            event.order_id,
            event.status,
            event.filled_quantity,
            event.remaining_quantity
        );
        let now_ns = self.clock.now_ns();
        if !self.state.apply_order_event(&event) {
            return;
        }
        let mut ctx_strategy = StrategyContext::new(now_ns, &self.state, &mut self.timers);
        self.strategy.on_order(&event, &mut ctx_strategy);
        let actions = self.engine.execute(&ctx_strategy.into_intents(), &self.state, now_ns);
        self.apply_actions(now_ns, &actions);
        self.write_actions(&actions, ctx);
    }

    fn on_trade_event(&mut self, event: TradeEvent, ctx: &mut ActorContext) {
        tracing::debug!(
            "trade event received event=[TRADE_EVENT] order_id=[{}] trade_id=[{}] price=[{}] quantity=[{}]",
            event.order_id,
            event.trade_id,
            event.price,
            event.quantity
        );
        let now_ns = self.clock.now_ns();
        if !self.state.apply_trade_event(&event) {
            return;
        }
        let mut ctx_strategy = StrategyContext::new(now_ns, &self.state, &mut self.timers);
        self.strategy.on_trade(&event, &mut ctx_strategy);
        let actions = self.engine.execute(&ctx_strategy.into_intents(), &self.state, now_ns);
        self.apply_actions(now_ns, &actions);
        self.write_actions(&actions, ctx);
    }

    fn tick_timers(&mut self, ctx: &mut ActorContext) {
        let now_ns = self.clock.now_ns();
        let events = self.timers.due_timers(now_ns);
        for event in events {
            let mut ctx_strategy = StrategyContext::new(now_ns, &self.state, &mut self.timers);
            self.strategy.on_timer(&event, &mut ctx_strategy);
            let actions = self.engine.execute(&ctx_strategy.into_intents(), &self.state, now_ns);
            self.apply_actions(now_ns, &actions);
            self.write_actions(&actions, ctx);
        }
    }

    fn apply_actions(&mut self, now_ns: u64, actions: &[Action]) {
        for action in actions {
            match action.kind {
                ActionKind::NewOrder => {
                    self.state.on_action_sent(action, now_ns);
                }
                ActionKind::CancelOrder => {
                    self.state.on_action_sent(action, now_ns);
                }
            }
        }
    }

    fn write_actions(&mut self, actions: &[Action], ctx: &mut ActorContext) {
        let Some(addr) = self.action_queue_addr.as_ref() else {
            return;
        };
        for action in actions {
            let _ = ctx.publish(addr, action);
            tracing::debug!(
                "action sent event=[ACTION_SENT] kind=[{:?}] order_id=[{}] timestamp=[{}]",
                action.kind,
                match action.kind {
                    ActionKind::NewOrder => action.new_order.order_id,
                    ActionKind::CancelOrder => action.cancel_order.order_id,
                },
                match action.kind {
                    ActionKind::NewOrder => action.new_order.timestamp,
                    ActionKind::CancelOrder => action.cancel_order.timestamp,
                }
            );
        }
    }

    pub fn state(&self) -> &RunnerState {
        &self.state
    }

    fn register_with_master(&mut self) -> Result<(), RunnerError> {
        let Some(master) = self.master.as_mut() else {
            return Ok(());
        };
        info!(
            "register actor event=[ACTOR_REGISTER] actor_id=[{}] actor_type=[{}]",
            self.actor_id, self.actor_type
        );
        master.register_actor(&self.actor_id, &self.actor_type, Vec::new())?;
        Ok(())
    }
}

impl<S: Strategy + Send + 'static> Actor for StrategyRunner<S> {
    fn on_start(&mut self, ctx: &mut ActorContext) {
        let _ = self.start_with_ctx(ctx);
    }

    fn on_event(&mut self, event: Event, ctx: &mut ActorContext) {
        let _ = self.handle_event_with_ctx(event, ctx);
    }

    fn on_stop(&mut self) {}
}

const MAX_QUEUE_RETRIES: u32 = 10;
const INITIAL_RETRY_DELAY_MS: u64 = 100;
const MAX_RETRY_DELAY_MS: u64 = 5000;

fn wait_for_queue(master: &mut MasterClient, queue_type: &str) -> Result<String, RunnerError> {
    let shutdown = setup_signal();
    let mut delay_ms = INITIAL_RETRY_DELAY_MS;

    for attempt in 1..=MAX_QUEUE_RETRIES {
        if shutdown.is_shutdown() {
            return Err(RunnerError::Shutdown);
        }
        let queues = master.find_queues(queue_type)?;
        if let Some(queue) = queues.first() {
            return Ok(queue.addr.clone());
        }
        warn!(
            "queue not found event=[QUEUE_NOT_FOUND] queue_type=[{}] attempt=[{}/{}] retry_ms=[{}]",
            queue_type, attempt, MAX_QUEUE_RETRIES, delay_ms
        );
        std::thread::sleep(Duration::from_millis(delay_ms));
        delay_ms = (delay_ms * 2).min(MAX_RETRY_DELAY_MS);
    }

    Err(RunnerError::QueueNotFound {
        queue_type: queue_type.to_string(),
    })
}

fn read_from_with_retry<T: Copy + Send + 'static>(
    ctx: &mut ActorContext,
    addr: &str,
) -> Result<usize, RunnerError> {
    let shutdown = setup_signal();
    loop {
        if shutdown.is_shutdown() {
            return Err(RunnerError::Shutdown);
        }
        match ctx.read_from::<T>(addr) {
            Ok(source_id) => return Ok(source_id),
            Err(nnxt_actors::Error::RapidError(nnxt_rapid::Error::NotFound)) => {
                std::thread::sleep(Duration::from_millis(200));
            }
            Err(err) => return Err(err.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intent::Intent;
    use crate::strategy::Strategy;
    use nnxt_specs::market::InstrumentId;
    use nnxt_specs::{OrderBook, PriceType};
    use std::str::FromStr;

    struct TestStrategy {
        instrument: InstrumentId,
    }

    impl Strategy for TestStrategy {
        fn on_order_book(&mut self, _book: &OrderBook, ctx: &mut StrategyContext) {
            ctx.submit_intent(Intent::target_position(
                self.instrument,
                5,
                PriceType::Limit,
                100.0,
            ));
        }
    }

    #[test]
    fn runner_generates_actions() {
        let instrument = InstrumentId::from_str("TEST").expect("instrument");
        let mut runner = StrategyRunner::new(
            TestStrategy { instrument },
            RunnerConfig::default(),
        )
        .expect("runner");
        let mut book = OrderBook::default();
        book.instrument_id = instrument;
        let mut reactor = Reactor::new();
        let external_tx = reactor.external_sender();
        let mut ctx = ActorContext::new(
            reactor.rapid_sources_handle(),
            reactor.control_handle(),
            external_tx,
        );
        runner.start_with_ctx(&mut ctx).expect("start");
        runner.on_order_book(book, instrument, &mut ctx);
        assert_eq!(runner.state().orders(&instrument).len(), 1);
    }
}
