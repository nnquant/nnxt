//! Trading constants shared across the system.

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Side {
    Buy = 1,
    Sell = 2,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PriceType {
    Limit = 1,
    Market = 2,
    OpponentBest = 3,
    OwnBest = 4,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum OrderStatus {
    Pending = 1,
    PendingNew = 2,
    Active = 3,
    PendingCancel = 4,
    Filled = 5,
    Cancelled = 6,
    Rejected = 7,
    PartialFilled = 8,
}
