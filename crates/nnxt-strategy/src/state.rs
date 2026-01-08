//! State management for strategy runner.

use crate::ledger::Ledger;
use crate::order_manager::OrderManager;
use nnxt_specs::{InstrumentId, Order, OrderBook, OrderEvent, Position, TradeEvent};

#[derive(Debug, Default)]
pub struct RunnerState {
    order_manager: OrderManager,
    ledger: Ledger,
    market: Vec<OrderBook>,
}

impl RunnerState {
    pub fn position(&self, instrument_id: &InstrumentId) -> Option<Position> {
        self.ledger.position(instrument_id)
    }

    pub fn orders(&self, instrument_id: &InstrumentId) -> Vec<Order> {
        self.order_manager.orders(instrument_id)
    }

    pub fn market_view(&self, instrument_id: &InstrumentId) -> Option<OrderBook> {
        self.market
            .iter()
            .find(|book| &book.instrument_id == instrument_id)
            .copied()
    }

    pub fn update_market(&mut self, book: OrderBook) {
        if let Some(existing) = self
            .market
            .iter_mut()
            .find(|entry| entry.instrument_id == book.instrument_id)
        {
            *existing = book;
        } else {
            self.market.push(book);
        }
    }

    pub fn on_action_sent(&mut self, action: &crate::action::Action, now_ns: u64) {
        self.order_manager.on_action_sent(action, now_ns);
    }

    pub fn apply_order_event(&mut self, event: &OrderEvent) -> bool {
        self.order_manager.apply_order_event(event)
    }

    pub fn apply_trade_event(&mut self, event: &TradeEvent) -> bool {
        if self.ledger.apply_trade_event(event).is_some() {
            self.order_manager.apply_trade_event(event);
            return true;
        }
        false
    }

    pub fn pending_exposure(&self, instrument_id: &InstrumentId) -> (u64, u64) {
        self.order_manager.pending_exposure(instrument_id)
    }

    pub fn effective_position(&self, instrument_id: &InstrumentId) -> i64 {
        let position_qty = self
            .ledger
            .position(instrument_id)
            .map(|pos| pos.quantity)
            .unwrap_or(0);
        let (pending_buy, pending_sell) = self.pending_exposure(instrument_id);
        position_qty + pending_buy as i64 - pending_sell as i64
    }

    pub fn order_manager(&self) -> &OrderManager {
        &self.order_manager
    }

    pub fn ledger(&self) -> &Ledger {
        &self.ledger
    }
}
