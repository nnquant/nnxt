//! Intent definitions from strategies.

use nnxt_specs::{InstrumentId, PriceType, Side};

#[derive(Debug, Clone, PartialEq)]
pub enum Intent {
    TargetPosition(TargetPositionIntent),
    TargetOrders(TargetOrdersIntent),
    CancelOrder(CancelOrderIntent),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TargetPositionIntent {
    pub instrument_id: InstrumentId,
    pub target_quantity: i64,
    pub price_type: PriceType,
    pub limit_price: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TargetOrdersIntent {
    pub instrument_id: InstrumentId,
    pub orders: Vec<TargetOrder>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TargetOrder {
    pub side: Side,
    pub price_type: PriceType,
    pub limit_price: f64,
    pub quantity: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CancelOrderIntent {
    pub instrument_id: InstrumentId,
    pub order_id: u64,
}

impl Intent {
    pub fn target_position(
        instrument_id: InstrumentId,
        target_quantity: i64,
        price_type: PriceType,
        limit_price: f64,
    ) -> Self {
        Self::TargetPosition(TargetPositionIntent {
            instrument_id,
            target_quantity,
            price_type,
            limit_price,
        })
    }

    pub fn cancel_order(instrument_id: InstrumentId, order_id: u64) -> Self {
        Self::CancelOrder(CancelOrderIntent {
            instrument_id,
            order_id,
        })
    }
}
