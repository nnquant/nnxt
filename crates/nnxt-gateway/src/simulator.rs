//! Simulated market data source.

use std::time::Duration;

use nnxt_specs::market::InstrumentId;
use nnxt_specs::OrderBook;

#[derive(Debug, Clone)]
pub struct SourceConfig {
    pub interval: Duration,
    pub seed: u64,
}

impl Default for SourceConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_millis(50),
            seed: 7,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SimulatedSource {
    interval: Duration,
    state: u64,
}

impl SimulatedSource {
    pub fn new(config: SourceConfig) -> Self {
        Self {
            interval: config.interval,
            state: config.seed,
        }
    }

    pub fn interval(&self) -> Duration {
        self.interval
    }

    pub fn generate(&mut self, instrument_id: InstrumentId, timestamp: u64) -> OrderBook {
        let base_price = self.next_price(100.0, 2.0);
        let tick = 0.2;
        let mut book = OrderBook {
            instrument_id,
            bid_price: [0.0; 10],
            bid_volume: [0; 10],
            ask_price: [0.0; 10],
            ask_volume: [0; 10],
            last_price: base_price,
            volume: 0,
            turnover: 0.0,
            upper_limit_price: base_price * 1.1,
            lower_limit_price: base_price * 0.9,
            pre_close_price: base_price * 0.98,
            trade_count: 0,
            timestamp,
        };

        for level in 0..10 {
            let offset = tick * level as f64;
            book.bid_price[level] = base_price - offset;
            book.ask_price[level] = base_price + offset;
            book.bid_volume[level] = self.next_u64(10, 900);
            book.ask_volume[level] = self.next_u64(10, 900);
        }

        let volume = self.next_u64(100, 5_000);
        book.volume = volume;
        book.turnover = base_price * volume as f64;
        book.trade_count = self.next_u64(1, 1000);

        book
    }

    fn next_price(&mut self, min: f64, span: f64) -> f64 {
        let value = (self.next_u64(0, 10_000) as f64) / 10_000.0;
        min + value * span
    }

    fn next_u64(&mut self, min: u64, span: u64) -> u64 {
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1);
        min + (self.state % span)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nnxt_specs::market::InstrumentId;
    use std::str::FromStr;

    #[test]
    fn simulated_source_is_deterministic() {
        let instrument = InstrumentId::from_str("IF2409").expect("instrument");
        let mut source = SimulatedSource::new(SourceConfig::default());
        let first = source.generate(instrument, 1);
        let second = source.generate(instrument, 2);
        assert_ne!(first.last_price, second.last_price);
        assert_eq!(first.instrument_id, instrument);
    }
}
