//! Trading order and position data types.

use crate::{InstrumentId, OrderStatus, PriceType, Side};

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Order {
    pub instrument_id: InstrumentId,
    pub order_id: u64,
    pub client_order_id: u64,
    pub side: Side,
    pub price_type: PriceType,
    pub limit_price: f64,
    pub quantity: u64,
    pub filled_quantity: u64,
    pub status: OrderStatus,
    pub timestamp: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Position {
    pub instrument_id: InstrumentId,
    pub quantity: i64,
    pub avg_price: f64,
    pub last_update_ns: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OrderEvent {
    pub instrument_id: InstrumentId,
    pub order_id: u64,
    pub status: OrderStatus,
    pub filled_quantity: u64,
    pub remaining_quantity: u64,
    pub last_price: f64,
    pub timestamp: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TradeEvent {
    pub instrument_id: InstrumentId,
    pub trade_id: u64,
    pub order_id: u64,
    pub side: Side,
    pub price: f64,
    pub quantity: u64,
    pub timestamp: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_types_are_copy() {
        fn assert_copy<T: Copy>() {}
        assert_copy::<Order>();
        assert_copy::<Position>();
        assert_copy::<OrderEvent>();
        assert_copy::<TradeEvent>();
    }
}
