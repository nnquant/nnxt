//! Order lifecycle management.

use std::collections::{HashMap, HashSet};

use crate::action::{Action, ActionKind};
use nnxt_specs::{InstrumentId, Order, OrderEvent, OrderStatus, PriceType, Side, TradeEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct OrderEventKey {
    status: OrderStatus,
    filled_quantity: u64,
    remaining_quantity: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrderState {
    pub order: Order,
}

#[derive(Debug, Default)]
pub struct OrderManager {
    orders: HashMap<u64, OrderState>,
    order_event_seen: HashMap<u64, HashSet<OrderEventKey>>,
}

impl OrderManager {
    pub fn on_action_sent(&mut self, action: &Action, now_ns: u64) {
        match action.kind {
            ActionKind::NewOrder => {
                let order = Order {
                    instrument_id: action.new_order.instrument_id,
                    order_id: action.new_order.order_id,
                    client_order_id: action.new_order.client_order_id,
                    side: action.new_order.side,
                    price_type: action.new_order.price_type,
                    limit_price: action.new_order.limit_price,
                    quantity: action.new_order.quantity,
                    filled_quantity: 0,
                    status: OrderStatus::PendingNew,
                    timestamp: now_ns,
                };
                self.orders.insert(order.order_id, OrderState { order });
            }
            ActionKind::CancelOrder => {
                if let Some(state) = self.orders.get_mut(&action.cancel_order.order_id)
                    && !is_terminal(state.order.status)
                {
                    state.order.status = OrderStatus::PendingCancel;
                    state.order.timestamp = now_ns;
                }
            }
        }
    }

    pub fn apply_order_event(&mut self, event: &OrderEvent) -> bool {
        let key = OrderEventKey {
            status: event.status,
            filled_quantity: event.filled_quantity,
            remaining_quantity: event.remaining_quantity,
        };
        let seen = self.order_event_seen.entry(event.order_id).or_default();
        if !seen.insert(key) {
            return false;
        }

        let quantity = event.filled_quantity.saturating_add(event.remaining_quantity);
        let state = self.orders.entry(event.order_id).or_insert_with(|| OrderState {
            order: Order {
                instrument_id: event.instrument_id,
                order_id: event.order_id,
                client_order_id: 0,
                side: Side::Buy,
                price_type: PriceType::Limit,
                limit_price: event.last_price,
                quantity,
                filled_quantity: event.filled_quantity,
                status: event.status,
                timestamp: event.timestamp,
            },
        });

        state.order.status = event.status;
        state.order.filled_quantity = event.filled_quantity;
        state.order.timestamp = event.timestamp;
        if quantity > state.order.quantity {
            state.order.quantity = quantity;
        }
        true
    }

    pub fn apply_trade_event(&mut self, event: &TradeEvent) {
        if let Some(state) = self.orders.get_mut(&event.order_id) {
            state.order.filled_quantity = state
                .order
                .filled_quantity
                .saturating_add(event.quantity);
            state.order.timestamp = event.timestamp;
            if state.order.filled_quantity >= state.order.quantity {
                state.order.status = OrderStatus::Filled;
            } else if state.order.status == OrderStatus::PendingNew {
                state.order.status = OrderStatus::Active;
            }
        }
    }

    pub fn pending_exposure(&self, instrument_id: &InstrumentId) -> (u64, u64) {
        let mut pending_buy = 0u64;
        let mut pending_sell = 0u64;
        for state in self.orders.values() {
            if &state.order.instrument_id != instrument_id {
                continue;
            }
            if is_terminal(state.order.status) {
                continue;
            }
            let remaining = state
                .order
                .quantity
                .saturating_sub(state.order.filled_quantity);
            if remaining == 0 {
                continue;
            }
            match state.order.side {
                Side::Buy => pending_buy = pending_buy.saturating_add(remaining),
                Side::Sell => pending_sell = pending_sell.saturating_add(remaining),
            }
        }
        (pending_buy, pending_sell)
    }

    pub fn orders(&self, instrument_id: &InstrumentId) -> Vec<Order> {
        self.orders
            .values()
            .filter(|state| &state.order.instrument_id == instrument_id)
            .map(|state| state.order)
            .collect()
    }
}

fn is_terminal(status: OrderStatus) -> bool {
    matches!(
        status,
        OrderStatus::Filled | OrderStatus::Cancelled | OrderStatus::Rejected
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use nnxt_specs::market::InstrumentId;
    use std::str::FromStr;

    #[test]
    fn action_sent_creates_pending_new() {
        let instrument = InstrumentId::from_str("IF2409").expect("instrument");
        let action = Action::new_order(crate::action::NewOrder {
            instrument_id: instrument,
            order_id: 1,
            client_order_id: 1,
            side: Side::Buy,
            price_type: PriceType::Limit,
            limit_price: 10.0,
            quantity: 5,
            timestamp: 1,
        });
        let mut manager = OrderManager::default();
        manager.on_action_sent(&action, 1);
        let orders = manager.orders(&instrument);
        assert_eq!(orders.len(), 1);
        assert_eq!(orders[0].status, OrderStatus::PendingNew);
    }

    #[test]
    fn order_event_dedup() {
        let instrument = InstrumentId::from_str("IF2409").expect("instrument");
        let event = OrderEvent {
            instrument_id: instrument,
            order_id: 1,
            status: OrderStatus::Active,
            filled_quantity: 0,
            remaining_quantity: 10,
            last_price: 10.0,
            timestamp: 1,
        };
        let mut manager = OrderManager::default();
        assert!(manager.apply_order_event(&event));
        assert!(!manager.apply_order_event(&event));
    }
}
