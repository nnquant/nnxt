//! Strategy trait definition.

use crate::context::{StrategyContext, TimerEvent};
use nnxt_specs::{OrderBook, OrderEvent, TradeEvent};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StrategyError {
    Rejected { message: String },
    Risk { message: String },
    Unknown { message: String },
}

pub trait Strategy {
    fn on_start(&mut self, _ctx: &mut StrategyContext) {}

    fn on_stop(&mut self, _ctx: &mut StrategyContext) {}

    fn on_order_book(&mut self, _book: &OrderBook, _ctx: &mut StrategyContext) {}

    fn on_timer(&mut self, _event: &TimerEvent, _ctx: &mut StrategyContext) {}

    fn on_order(&mut self, _event: &OrderEvent, _ctx: &mut StrategyContext) {}

    fn on_trade(&mut self, _event: &TradeEvent, _ctx: &mut StrategyContext) {}

    fn on_error(&mut self, _error: &StrategyError, _ctx: &mut StrategyContext) {}
}
