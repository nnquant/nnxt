//! Execution engine that converts intents to actions.

use std::sync::atomic::{AtomicU64, Ordering};

use crate::action::{Action, CancelOrder, NewOrder};
use crate::intent::{Intent, TargetOrder, TargetOrdersIntent, TargetPositionIntent};
use crate::state::RunnerState;
use nnxt_specs::{InstrumentId, Side};

#[derive(Debug)]
pub struct PortfolioView<'a> {
    state: &'a RunnerState,
}

impl<'a> PortfolioView<'a> {
    pub fn new(state: &'a RunnerState) -> Self {
        Self { state }
    }

    pub fn effective_position(&self, instrument_id: &InstrumentId) -> i64 {
        self.state.effective_position(instrument_id)
    }

    pub fn pending_exposure(&self, instrument_id: &InstrumentId) -> (u64, u64) {
        self.state.pending_exposure(instrument_id)
    }
}

#[derive(Debug, Default)]
pub struct ExecutionEngine {
    counter: AtomicU64,
}

impl ExecutionEngine {
    pub fn new() -> Self {
        Self {
            counter: AtomicU64::new(1),
        }
    }

    pub fn execute(&self, intents: &[Intent], state: &RunnerState, now_ns: u64) -> Vec<Action> {
        let view = PortfolioView::new(state);
        let mut actions = Vec::new();
        for intent in intents {
            match intent {
                Intent::TargetPosition(intent) => {
                    actions.extend(self.target_position_actions(intent, &view, now_ns));
                }
                Intent::TargetOrders(intent) => {
                    actions.extend(self.target_orders_actions(intent, now_ns));
                }
                Intent::CancelOrder(intent) => {
                    let action = CancelOrder {
                        instrument_id: intent.instrument_id,
                        order_id: intent.order_id,
                        timestamp: now_ns,
                    };
                    actions.push(Action::cancel_order(action));
                }
            }
        }
        actions
    }

    fn next_id(&self) -> u64 {
        self.counter.fetch_add(1, Ordering::Relaxed)
    }

    fn target_position_actions(
        &self,
        intent: &TargetPositionIntent,
        view: &PortfolioView<'_>,
        now_ns: u64,
    ) -> Vec<Action> {
        let effective_position = view.effective_position(&intent.instrument_id);
        let delta = intent.target_quantity.saturating_sub(effective_position);
        if delta == 0 {
            return Vec::new();
        }
        let side = if delta > 0 { Side::Buy } else { Side::Sell };
        let quantity = delta.unsigned_abs();
        let order = NewOrder {
            instrument_id: intent.instrument_id,
            order_id: self.next_id(),
            client_order_id: 0,
            side,
            price_type: intent.price_type,
            limit_price: intent.limit_price,
            quantity,
            timestamp: now_ns,
        };
        vec![Action::new_order(order)]
    }

    fn target_orders_actions(&self, intent: &TargetOrdersIntent, now_ns: u64) -> Vec<Action> {
        intent
            .orders
            .iter()
            .map(|order| self.build_order(intent.instrument_id, order, now_ns))
            .collect()
    }

    fn build_order(
        &self,
        instrument_id: InstrumentId,
        order: &TargetOrder,
        now_ns: u64,
    ) -> Action {
        let new_order = NewOrder {
            instrument_id,
            order_id: self.next_id(),
            client_order_id: 0,
            side: order.side,
            price_type: order.price_type,
            limit_price: order.limit_price,
            quantity: order.quantity,
            timestamp: now_ns,
        };
        Action::new_order(new_order)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nnxt_specs::market::InstrumentId;
    use nnxt_specs::PriceType;
    use std::str::FromStr;

    #[test]
    fn target_position_diff_generates_action() {
        let engine = ExecutionEngine::new();
        let state = RunnerState::default();
        let instrument = InstrumentId::from_str("IF2409").expect("instrument");
        let intent = Intent::TargetPosition(TargetPositionIntent {
            instrument_id: instrument,
            target_quantity: 10,
            price_type: PriceType::Limit,
            limit_price: 3500.0,
        });

        let actions = engine.execute(&[intent], &state, 1);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].kind, crate::action::ActionKind::NewOrder);
        assert_eq!(actions[0].new_order.quantity, 10);
    }

    #[test]
    fn pending_exposure_reduces_delta() {
        let engine = ExecutionEngine::new();
        let mut state = RunnerState::default();
        let instrument = InstrumentId::from_str("IF2409").expect("instrument");

        let action = crate::action::Action::new_order(crate::action::NewOrder {
            instrument_id: instrument,
            order_id: 1,
            client_order_id: 1,
            side: nnxt_specs::Side::Buy,
            price_type: PriceType::Limit,
            limit_price: 10.0,
            quantity: 5,
            timestamp: 1,
        });
        state.on_action_sent(&action, 1);

        let intent = Intent::TargetPosition(TargetPositionIntent {
            instrument_id: instrument,
            target_quantity: 5,
            price_type: PriceType::Limit,
            limit_price: 10.0,
        });

        let actions = engine.execute(&[intent], &state, 2);
        assert!(actions.is_empty());
    }
}
