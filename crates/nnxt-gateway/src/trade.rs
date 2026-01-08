//! Trade gateway trait and simulator.

use std::collections::VecDeque;

use nnxt_specs::{OrderEvent, OrderStatus, Side, TradeEvent};
use nnxt_strategy::Action;
use nnxt_utils::clock::MonotonicClock;

#[derive(Debug)]
pub enum TradeGatewayEvent {
    Order(OrderEvent),
    Trade(TradeEvent),
}

pub trait TradeGateway {
    fn send_order(&mut self, action: &Action) -> Result<(), TradeGatewayError>;
    fn poll_events(&mut self) -> Vec<TradeGatewayEvent>;
}

#[derive(Debug)]
pub enum TradeGatewayError {
    UnsupportedAction,
}

pub struct TradeSimulator {
    next_trade_id: u64,
    events: VecDeque<TradeGatewayEvent>,
}

impl TradeSimulator {
    pub fn new() -> Self {
        Self {
            next_trade_id: 1,
            events: VecDeque::new(),
        }
    }

    fn push_order_event(
        &mut self,
        order_id: u64,
        instrument_id: nnxt_specs::InstrumentId,
        status: OrderStatus,
        filled_quantity: u64,
        remaining_quantity: u64,
        last_price: f64,
    ) {
        let now_ns = MonotonicClock::now_ns();
        self.events.push_back(TradeGatewayEvent::Order(OrderEvent {
            instrument_id,
            order_id,
            status,
            filled_quantity,
            remaining_quantity,
            last_price,
            timestamp: now_ns,
        }));
    }

    fn push_trade_event(
        &mut self,
        order_id: u64,
        instrument_id: nnxt_specs::InstrumentId,
        side: Side,
        price: f64,
        quantity: u64,
    ) {
        let now_ns = MonotonicClock::now_ns();
        let trade_id = self.next_trade_id;
        self.next_trade_id = self.next_trade_id.wrapping_add(1);
        self.events.push_back(TradeGatewayEvent::Trade(TradeEvent {
            instrument_id,
            trade_id,
            order_id,
            side,
            price,
            quantity,
            timestamp: now_ns,
        }));
    }
}

impl Default for TradeSimulator {
    fn default() -> Self {
        Self::new()
    }
}

impl TradeGateway for TradeSimulator {
    fn send_order(&mut self, action: &Action) -> Result<(), TradeGatewayError> {
        match action.kind {
            nnxt_strategy::ActionKind::NewOrder => {
                let order = action.new_order;
                self.push_order_event(
                    order.order_id,
                    order.instrument_id,
                    OrderStatus::Active,
                    0,
                    order.quantity,
                    order.limit_price,
                );
                self.push_trade_event(
                    order.order_id,
                    order.instrument_id,
                    order.side,
                    order.limit_price,
                    order.quantity,
                );
                self.push_order_event(
                    order.order_id,
                    order.instrument_id,
                    OrderStatus::Filled,
                    order.quantity,
                    0,
                    order.limit_price,
                );
                Ok(())
            }
            nnxt_strategy::ActionKind::CancelOrder => {
                let cancel = action.cancel_order;
                self.push_order_event(
                    cancel.order_id,
                    cancel.instrument_id,
                    OrderStatus::Cancelled,
                    0,
                    0,
                    0.0,
                );
                Ok(())
            }
        }
    }

    fn poll_events(&mut self) -> Vec<TradeGatewayEvent> {
        self.events.drain(..).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nnxt_specs::market::InstrumentId;
    use std::str::FromStr;

    #[test]
    fn trade_simulator_generates_events() {
        let instrument = InstrumentId::from_str("IF2409").expect("instrument");
        let action = Action::new_order(nnxt_strategy::NewOrder {
            instrument_id: instrument,
            order_id: 1,
            client_order_id: 1,
            side: Side::Buy,
            price_type: nnxt_specs::PriceType::Limit,
            limit_price: 10.0,
            quantity: 2,
            timestamp: 1,
        });
        let mut simulator = TradeSimulator::new();
        simulator.send_order(&action).expect("send");
        let events = simulator.poll_events();
        assert_eq!(events.len(), 3);
    }
}
