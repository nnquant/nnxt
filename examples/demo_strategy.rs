use nnxt_specs::market::InstrumentId;
use std::str::FromStr;
use nnxt_strategy::{Intent, RunnerConfig, Strategy, StrategyContext, StrategyRunner};
use tracing::info;
use nnxt_utils::clock::MonotonicClock;
use nnxt_utils::setup_log;

const MASTER_ADDR: &str = "ipc:///tmp/nnxt/master";

fn main() {
    let _ = setup_log();
    info!("starting demo strategy master_addr=[{}]", MASTER_ADDR);

    let instrument = InstrumentId::from_str("IF2409").expect("instrument");
    let strategy = DemoStrategy::new(instrument);
    let config = RunnerConfig {
        master_addr: Some(MASTER_ADDR.to_string()),
        actor_id: "strategy-1".to_string(),
        actor_type: "strategy".to_string(),
        ..RunnerConfig::default()
    };

    let mut runner = StrategyRunner::new(strategy, config).expect("runner");
    runner.run().expect("runner run failed");
}

struct DemoStrategy {
    instrument: InstrumentId,
    last_order_ts: Option<u64>,
    market2strategy: LatencyStats,
    order2ack: LatencyStats,
    trade2ack: LatencyStats,
    timer_id: Option<u64>,
}

impl DemoStrategy {
    fn new(instrument: InstrumentId) -> Self {
        Self {
            instrument,
            last_order_ts: None,
            market2strategy: LatencyStats::default(),
            order2ack: LatencyStats::default(),
            trade2ack: LatencyStats::default(),
            timer_id: None,
        }
    }
}

impl Strategy for DemoStrategy {
    fn on_start(&mut self, ctx: &mut StrategyContext) {
        ctx.subscribe_quote::<nnxt_specs::OrderBook>("market-simulator", &self.instrument);
        ctx.connect_trade("trade-simulator");
        let interval_ns = 10_000_000_000u64;
        self.timer_id = Some(ctx.set_timer(interval_ns));
    }

    fn on_order_book(&mut self, book: &nnxt_specs::OrderBook, _ctx: &mut StrategyContext) {
        let now_ns = MonotonicClock::now_ns();
        self.market2strategy.record(now_ns.saturating_sub(book.timestamp));
    }

    fn on_timer(&mut self, event: &nnxt_strategy::TimerEvent, ctx: &mut StrategyContext) {
        if Some(event.timer_id) != self.timer_id {
            return;
        }
        let current_pos = ctx
            .position(&self.instrument)
            .map(|pos| pos.quantity)
            .unwrap_or(0);
        let target_position = if current_pos == 0 { 1 } else { 0 };
        self.last_order_ts = Some(MonotonicClock::now_ns());
        ctx.submit_intent(Intent::target_position(
            self.instrument,
            target_position,
            nnxt_specs::PriceType::Limit,
            10.0,
        ));
    }

    fn on_order(&mut self, event: &nnxt_specs::OrderEvent, _ctx: &mut StrategyContext) {
        if event.status != nnxt_specs::OrderStatus::Active {
            return;
        }
        let now_ns = MonotonicClock::now_ns();
        if let Some(order_ts) = self.last_order_ts {
            self.order2ack.record(now_ns.saturating_sub(order_ts));
        }
    }

    fn on_trade(&mut self, _event: &nnxt_specs::TradeEvent, _ctx: &mut StrategyContext) {
        let now_ns = MonotonicClock::now_ns();
        if let Some(order_ts) = self.last_order_ts {
            self.trade2ack.record(now_ns.saturating_sub(order_ts));
        }
    }

    fn on_stop(&mut self, _ctx: &mut StrategyContext) {
        self.market2strategy.print("market2strategy");
        self.order2ack.print("order2ack");
        self.trade2ack.print("trade2ack");
    }
}

#[derive(Default)]
struct LatencyStats {
    samples: Vec<u64>,
}

impl LatencyStats {
    fn record(&mut self, value: u64) {
        self.samples.push(value);
    }

    fn print(&self, label: &str) {
        let Some(summary) = self.summary() else {
            return;
        };
        info!(
            "latency summary event=[LATENCY_SUMMARY] metric=[{}] count=[{}] avg_ns=[{}] stddev_ns=[{}] min_ns=[{}] max_ns=[{}] p50_ns=[{}] p90_ns=[{}] p99_ns=[{}]",
            label,
            summary.count,
            summary.avg,
            summary.stddev,
            summary.min,
            summary.max,
            summary.p50,
            summary.p90,
            summary.p99
        );
    }

    fn summary(&self) -> Option<LatencySummary> {
        if self.samples.is_empty() {
            return None;
        }
        let mut data = self.samples.clone();
        data.sort_unstable();
        let trim = ((data.len() as f64) * 0.001).ceil() as usize;
        if trim > 0 && data.len() > trim {
            data.truncate(data.len().saturating_sub(trim));
        }
        if data.is_empty() {
            return None;
        }
        let count = data.len();
        let sum: u128 = data.iter().map(|v| *v as u128).sum();
        let avg = (sum as f64) / (count as f64);
        let mut var = 0.0;
        for value in &data {
            let diff = (*value as f64) - avg;
            var += diff * diff;
        }
        let stddev = (var / (count as f64)).sqrt();
        let min = data[0];
        let max = data[count - 1];
        let p50 = percentile(&data, 0.50);
        let p90 = percentile(&data, 0.90);
        let p99 = percentile(&data, 0.99);
        Some(LatencySummary {
            count,
            avg,
            stddev,
            min,
            max,
            p50,
            p90,
            p99,
        })
    }
}

struct LatencySummary {
    count: usize,
    avg: f64,
    stddev: f64,
    min: u64,
    max: u64,
    p50: u64,
    p90: u64,
    p99: u64,
}

fn percentile(sorted: &[u64], p: f64) -> u64 {
    let count = sorted.len();
    if count == 0 {
        return 0;
    }
    let rank = (p * (count as f64 - 1.0)).round() as usize;
    sorted[rank]
}
