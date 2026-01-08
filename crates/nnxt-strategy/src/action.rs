//! Actions generated for execution.

use nnxt_specs::{InstrumentId, PriceType, Side};

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionKind {
    NewOrder = 1,
    CancelOrder = 2,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NewOrder {
    pub instrument_id: InstrumentId,
    pub order_id: u64,
    pub client_order_id: u64,
    pub side: Side,
    pub price_type: PriceType,
    pub limit_price: f64,
    pub quantity: u64,
    pub timestamp: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CancelOrder {
    pub instrument_id: InstrumentId,
    pub order_id: u64,
    pub timestamp: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Action {
    pub kind: ActionKind,
    pub new_order: NewOrder,
    pub cancel_order: CancelOrder,
}

impl Action {
    pub fn new_order(action: NewOrder) -> Self {
        Self {
            kind: ActionKind::NewOrder,
            new_order: action,
            cancel_order: CancelOrder {
                instrument_id: action.instrument_id,
                order_id: 0,
                timestamp: action.timestamp,
            },
        }
    }

    pub fn cancel_order(action: CancelOrder) -> Self {
        Self {
            kind: ActionKind::CancelOrder,
            new_order: NewOrder {
                instrument_id: action.instrument_id,
                order_id: 0,
                client_order_id: 0,
                side: Side::Buy,
                price_type: PriceType::Limit,
                limit_price: 0.0,
                quantity: 0,
                timestamp: action.timestamp,
            },
            cancel_order: action,
        }
    }
}
