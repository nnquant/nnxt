//! Market data structures.

use core::fmt;
use core::str::FromStr;

pub const ORDER_BOOK_DEPTH: usize = 10;

#[repr(transparent)]
#[derive(Clone, Copy, Eq, PartialEq, Hash)]
pub struct InstrumentId(pub [u8; 64]);

impl Default for InstrumentId {
    fn default() -> Self {
        Self([0u8; 64])
    }
}

impl fmt::Debug for InstrumentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.as_str() {
            Ok(s) => write!(f, "InstrumentId({:?})", s),
            Err(_) => write!(f, "InstrumentId(<invalid utf8>)"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstrumentIdError {
    TooLong { length: usize },
}

impl fmt::Display for InstrumentIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InstrumentIdError::TooLong { length } => {
                write!(f, "instrument id too long length=[{}]", length)
            }
        }
    }
}

impl InstrumentId {
    pub const LEN: usize = 64;

    pub fn as_str(&self) -> Result<&str, core::str::Utf8Error> {
        let end = self
            .0
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(Self::LEN);
        core::str::from_utf8(&self.0[..end])
    }
}

impl FromStr for InstrumentId {
    type Err = InstrumentIdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.len() > Self::LEN {
            return Err(InstrumentIdError::TooLong { length: value.len() });
        }

        let mut bytes = [0u8; Self::LEN];
        bytes[..value.len()].copy_from_slice(value.as_bytes());
        Ok(Self(bytes))
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct OrderBook {
    pub instrument_id: InstrumentId,
    pub bid_price: [f64; ORDER_BOOK_DEPTH],
    pub bid_volume: [u64; ORDER_BOOK_DEPTH],
    pub ask_price: [f64; ORDER_BOOK_DEPTH],
    pub ask_volume: [u64; ORDER_BOOK_DEPTH],
    pub last_price: f64,
    pub volume: u64,
    pub turnover: f64,
    pub upper_limit_price: f64,
    pub lower_limit_price: f64,
    pub pre_close_price: f64,
    pub trade_count: u64,
    pub timestamp: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instrument_id_round_trip() {
        let id = InstrumentId::from_str("IF2409").expect("valid instrument id");
        assert_eq!(id.as_str().expect("utf8"), "IF2409");
    }

    #[test]
    fn instrument_id_rejects_long_input() {
        let long = "A".repeat(InstrumentId::LEN + 1);
        let err = InstrumentId::from_str(&long).expect_err("should reject");
        assert_eq!(err, InstrumentIdError::TooLong { length: long.len() });
    }

    #[test]
    fn order_book_is_copy() {
        fn assert_copy<T: Copy>() {}
        assert_copy::<OrderBook>();
    }

    #[test]
    fn order_book_layout_is_stable() {
        let expected_size = 448usize;
        assert_eq!(core::mem::size_of::<OrderBook>(), expected_size);
        assert_eq!(core::mem::align_of::<OrderBook>(), 8);
    }
}
