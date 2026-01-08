//! Position ledger with trade dedup.

use std::collections::{HashMap, HashSet, VecDeque};

use nnxt_specs::{InstrumentId, Position, Side, TradeEvent};

const DEFAULT_FILL_WINDOW: usize = 10_000;

#[derive(Debug)]
pub struct Ledger {
    fill_ledger: FillLedger,
    positions: HashMap<InstrumentId, PositionState>,
}

#[derive(Debug)]
struct FillLedger {
    max_size: usize,
    deque: VecDeque<u64>,
    seen: HashSet<u64>,
}

impl FillLedger {
    fn new(max_size: usize) -> Self {
        Self {
            max_size,
            deque: VecDeque::new(),
            seen: HashSet::new(),
        }
    }

    fn check_and_insert(&mut self, trade_id: u64) -> bool {
        if self.seen.contains(&trade_id) {
            return false;
        }
        self.seen.insert(trade_id);
        self.deque.push_back(trade_id);
        if self.deque.len() > self.max_size && let Some(evicted) = self.deque.pop_front() {
            self.seen.remove(&evicted);
        }
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PositionState {
    quantity: i64,
    avg_price: f64,
    last_update_ns: u64,
}

impl Ledger {
    pub fn new() -> Self {
        Self {
            fill_ledger: FillLedger::new(DEFAULT_FILL_WINDOW),
            positions: HashMap::new(),
        }
    }

    pub fn apply_trade_event(&mut self, event: &TradeEvent) -> Option<Position> {
        if !self.fill_ledger.check_and_insert(event.trade_id) {
            return None;
        }

        let delta = match event.side {
            Side::Buy => event.quantity as i64,
            Side::Sell => -(event.quantity as i64),
        };

        let state = self.positions.entry(event.instrument_id).or_insert(PositionState {
            quantity: 0,
            avg_price: 0.0,
            last_update_ns: event.timestamp,
        });

        let prev_qty = state.quantity;
        let next_qty = prev_qty.saturating_add(delta);

        state.avg_price = next_avg_price(prev_qty, state.avg_price, delta, event.price, next_qty);
        state.quantity = next_qty;
        state.last_update_ns = event.timestamp;

        Some(Position {
            instrument_id: event.instrument_id,
            quantity: state.quantity,
            avg_price: state.avg_price,
            last_update_ns: state.last_update_ns,
        })
    }

    pub fn position(&self, instrument_id: &InstrumentId) -> Option<Position> {
        self.positions.get(instrument_id).map(|state| Position {
            instrument_id: *instrument_id,
            quantity: state.quantity,
            avg_price: state.avg_price,
            last_update_ns: state.last_update_ns,
        })
    }
}

impl Default for Ledger {
    fn default() -> Self {
        Self::new()
    }
}

fn next_avg_price(
    prev_qty: i64,
    prev_avg: f64,
    delta: i64,
    trade_price: f64,
    next_qty: i64,
) -> f64 {
    if next_qty == 0 {
        return 0.0;
    }
    let prev_sign = prev_qty.signum();
    let delta_sign = delta.signum();
    if prev_qty == 0 || prev_sign == delta_sign {
        let prev_abs = prev_qty.unsigned_abs() as f64;
        let delta_abs = delta.unsigned_abs() as f64;
        let total = prev_abs + delta_abs;
        if total == 0.0 {
            0.0
        } else {
            (prev_avg * prev_abs + trade_price * delta_abs) / total
        }
    } else if next_qty.signum() != prev_sign {
        trade_price
    } else {
        prev_avg
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nnxt_specs::market::InstrumentId;
    use std::str::FromStr;

    #[test]
    fn ledger_dedup_trade() {
        let instrument = InstrumentId::from_str("IF2409").expect("instrument");
        let event = TradeEvent {
            instrument_id: instrument,
            trade_id: 1,
            order_id: 1,
            side: Side::Buy,
            price: 10.0,
            quantity: 2,
            timestamp: 1,
        };
        let mut ledger = Ledger::new();
        assert!(ledger.apply_trade_event(&event).is_some());
        assert!(ledger.apply_trade_event(&event).is_none());
    }

    #[test]
    fn ledger_weighted_avg() {
        let instrument = InstrumentId::from_str("IF2409").expect("instrument");
        let mut ledger = Ledger::new();
        let buy1 = TradeEvent {
            instrument_id: instrument,
            trade_id: 1,
            order_id: 1,
            side: Side::Buy,
            price: 10.0,
            quantity: 2,
            timestamp: 1,
        };
        let buy2 = TradeEvent {
            instrument_id: instrument,
            trade_id: 2,
            order_id: 1,
            side: Side::Buy,
            price: 12.0,
            quantity: 2,
            timestamp: 2,
        };
        ledger.apply_trade_event(&buy1);
        let pos = ledger.apply_trade_event(&buy2).expect("position");
        assert_eq!(pos.quantity, 4);
        assert!((pos.avg_price - 11.0).abs() < 1e-9);
    }
}
