//! Strategy context for data access and intent submission.

use crate::intent::{CancelOrderIntent, Intent};
use crate::state::RunnerState;
use nnxt_specs::{InstrumentId, Order, OrderBook, Position};

pub type TimerId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketDataType {
    OrderBook,
}

pub trait MarketData {
    const DATA_TYPE: MarketDataType;
}

impl MarketData for OrderBook {
    const DATA_TYPE: MarketDataType = MarketDataType::OrderBook;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuoteSubscription {
    pub source: String,
    pub instrument_id: InstrumentId,
    pub data_type: MarketDataType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TradeConnection {
    pub source: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PendingRequests {
    pub intents: Vec<Intent>,
    pub subscriptions: Vec<QuoteSubscription>,
    pub trade_connection: Option<TradeConnection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimerEvent {
    pub timer_id: TimerId,
    pub timestamp: u64,
}

#[derive(Debug, Default)]
pub struct TimerManager {
    next_id: TimerId,
    timers: Vec<TimerState>,
}

#[derive(Debug, Clone, Copy)]
struct TimerState {
    id: TimerId,
    interval_ns: u64,
    next_fire_ns: u64,
}

impl TimerManager {
    pub fn set_timer(&mut self, interval_ns: u64, now_ns: u64) -> TimerId {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        self.timers.push(TimerState {
            id,
            interval_ns,
            next_fire_ns: now_ns.saturating_add(interval_ns),
        });
        id
    }

    pub fn cancel_timer(&mut self, timer_id: TimerId) -> bool {
        if let Some(index) = self.timers.iter().position(|timer| timer.id == timer_id) {
            self.timers.swap_remove(index);
            return true;
        }
        false
    }

    pub fn due_timers(&mut self, now_ns: u64) -> Vec<TimerEvent> {
        let mut events = Vec::new();
        for timer in &mut self.timers {
            if now_ns >= timer.next_fire_ns {
                events.push(TimerEvent {
                    timer_id: timer.id,
                    timestamp: now_ns,
                });
                timer.next_fire_ns = now_ns.saturating_add(timer.interval_ns);
            }
        }
        events
    }
}

pub struct StrategyContext<'a> {
    now_ns: u64,
    state: &'a RunnerState,
    timers: &'a mut TimerManager,
    intents: Vec<Intent>,
    subscriptions: Vec<QuoteSubscription>,
    trade_connection: Option<TradeConnection>,
}

impl<'a> StrategyContext<'a> {
    pub fn new(now_ns: u64, state: &'a RunnerState, timers: &'a mut TimerManager) -> Self {
        Self {
            now_ns,
            state,
            timers,
            intents: Vec::new(),
            subscriptions: Vec::new(),
            trade_connection: None,
        }
    }

    pub fn now_ns(&self) -> u64 {
        self.now_ns
    }

    pub fn position(&self, instrument_id: &InstrumentId) -> Option<Position> {
        self.state.position(instrument_id)
    }

    pub fn orders(&self, instrument_id: &InstrumentId) -> Vec<Order> {
        self.state.orders(instrument_id)
    }

    pub fn market_view(&self, instrument_id: &InstrumentId) -> Option<OrderBook> {
        self.state.market_view(instrument_id)
    }

    pub fn set_timer(&mut self, interval_ns: u64) -> TimerId {
        self.timers.set_timer(interval_ns, self.now_ns)
    }

    pub fn cancel_timer(&mut self, timer_id: TimerId) -> bool {
        self.timers.cancel_timer(timer_id)
    }

    pub fn submit_intent(&mut self, intent: Intent) {
        self.intents.push(intent);
    }

    pub fn subscribe_quote<T: MarketData>(&mut self, source: &str, instrument_id: &InstrumentId) {
        self.subscriptions.push(QuoteSubscription {
            source: source.to_string(),
            instrument_id: *instrument_id,
            data_type: T::DATA_TYPE,
        });
    }

    pub fn connect_trade(&mut self, source: &str) {
        self.trade_connection = Some(TradeConnection {
            source: source.to_string(),
        });
    }

    pub fn cancel_all(&mut self, instrument_id: &InstrumentId) {
        for order in self.state.orders(instrument_id) {
            self.intents.push(Intent::CancelOrder(CancelOrderIntent {
                instrument_id: order.instrument_id,
                order_id: order.order_id,
            }));
        }
    }

    pub fn into_intents(self) -> Vec<Intent> {
        self.intents
    }

    pub fn into_pending(self) -> PendingRequests {
        PendingRequests {
            intents: self.intents,
            subscriptions: self.subscriptions,
            trade_connection: self.trade_connection,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nnxt_specs::market::InstrumentId;
    use std::str::FromStr;

    #[test]
    fn timer_manager_registers_and_cancels() {
        let mut manager = TimerManager::default();
        let timer_id = manager.set_timer(100, 0);
        assert!(manager.cancel_timer(timer_id));
        assert!(!manager.cancel_timer(timer_id));
    }

    #[test]
    fn context_submit_intent() {
        let mut timers = TimerManager::default();
        let state = RunnerState::default();
        let mut ctx = StrategyContext::new(1, &state, &mut timers);
        let instrument = InstrumentId::from_str("IF2409").expect("instrument");
        ctx.submit_intent(Intent::cancel_order(instrument, 1));
        assert_eq!(ctx.into_intents().len(), 1);
    }

    #[test]
    fn context_subscribe_quote() {
        let mut timers = TimerManager::default();
        let state = RunnerState::default();
        let mut ctx = StrategyContext::new(1, &state, &mut timers);
        let instrument = InstrumentId::from_str("IF2409").expect("instrument");
        ctx.subscribe_quote::<OrderBook>("market/market-gateway/public", &instrument);
        let pending = ctx.into_pending();
        assert_eq!(pending.subscriptions.len(), 1);
    }
}
