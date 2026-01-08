use std::str::FromStr;

use nnxt_specs::market::InstrumentId;
use nnxt_specs::{OrderEvent, OrderStatus, Side, TradeEvent};
use nnxt_specs::PriceType;
use nnxt_strategy::{Action, NewOrder, RunnerState};

#[test]
fn order_flow_updates_state_and_dedup() {
    let instrument = InstrumentId::from_str("IF2409").expect("instrument");
    let mut state = RunnerState::default();

    let action = Action::new_order(NewOrder {
        instrument_id: instrument,
        order_id: 1,
        client_order_id: 1,
        side: Side::Buy,
        price_type: PriceType::Limit,
        limit_price: 10.0,
        quantity: 5,
        timestamp: 1,
    });
    state.on_action_sent(&action, 1);

    let event = OrderEvent {
        instrument_id: instrument,
        order_id: 1,
        status: OrderStatus::Active,
        filled_quantity: 0,
        remaining_quantity: 5,
        last_price: 10.0,
        timestamp: 2,
    };
    assert!(state.apply_order_event(&event));

    let trade = TradeEvent {
        instrument_id: instrument,
        trade_id: 100,
        order_id: 1,
        side: Side::Buy,
        price: 10.0,
        quantity: 5,
        timestamp: 3,
    };
    assert!(state.apply_trade_event(&trade));
    assert!(!state.apply_trade_event(&trade));

    let orders = state.orders(&instrument);
    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0].status, OrderStatus::Filled);
    assert_eq!(orders[0].filled_quantity, 5);
}
