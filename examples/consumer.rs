use nnxt_specs::market::InstrumentId;
use nnxt_specs::OrderBook;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};
use nnxt_strategy::{RunnerConfig, Strategy, StrategyContext, StrategyRunner};
use tracing::info;
use nnxt_utils::setup_log;

const MASTER_ADDR: &str = "ipc:///tmp/nnxt/master";
const QUOTE_SOURCE: &str = "market/market-sim/public";

fn main() {
    let _ = setup_log();
    info!("starting consumer master_addr=[{}]", MASTER_ADDR);

    let instrument = InstrumentId::from_str("IF2409").expect("instrument");
    let strategy = QuoteConsumer {
        instrument,
        count: 0,
    };

    let config = RunnerConfig {
        master_addr: Some(MASTER_ADDR.to_string()),
        actor_id: "consumer".to_string(),
        ..RunnerConfig::default()
    };

    let mut runner = StrategyRunner::new(strategy, config).expect("runner create failed");
    runner.run().expect("runner failed");
}

struct QuoteConsumer {
    instrument: InstrumentId,
    count: u64,
}

impl Strategy for QuoteConsumer {
    fn on_start(&mut self, ctx: &mut StrategyContext) {
        ctx.subscribe_quote::<OrderBook>(QUOTE_SOURCE, &self.instrument);
    }

    fn on_order_book(&mut self, book: &OrderBook, _ctx: &mut StrategyContext) {
        self.count = self.count.saturating_add(1);
        if self.count % 100 != 0 {
            return;
        }
        let now_ns = unix_time_ns();
        let latency_ns = now_ns.saturating_sub(book.timestamp);
        let instrument = book.instrument_id.as_str().unwrap_or("unknown");
        info!(
            "received order_book instrument_id=[{}] latency_ns=[{}]",
            instrument,
            latency_ns
        );
    }
}

fn unix_time_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}
