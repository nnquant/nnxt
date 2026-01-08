use nnxt_gateway::{MarketSimulator, MarketSimulatorConfig};
use nnxt_rapid::{cleanup, Address};
use nnxt_specs::market::InstrumentId;
use std::str::FromStr;
use tracing::info;
use nnxt_utils::setup_log;

const MASTER_ADDR: &str = "ipc:///tmp/nnxt/master";

fn main() {
    let _ = setup_log();
    info!("starting gateway master_addr=[{}]", MASTER_ADDR);

    let config = MarketSimulatorConfig {
        master_addr: Some(MASTER_ADDR.to_string()),
        ..Default::default()
    };

    let addr = Address::new(&config.queue_path).expect("queue address");
    let _ = cleanup(&addr);

    let mut gateway = MarketSimulator::new(config).expect("gateway create failed");
    let instrument = InstrumentId::from_str("IF2409").expect("instrument parse");
    gateway.subscribe(instrument);

    gateway.run().expect("gateway run failed");
}
