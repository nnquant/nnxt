//! Market gateway for broadcasting OrderBook snapshots.

pub mod gateway;
pub mod runner;
pub mod simulator;
pub mod stats;
pub mod subscription;
pub mod trade;

pub use gateway::{GatewayError, MarketGateway, MarketSimulator, MarketSimulatorConfig};
pub use runner::{
    MarketGatewayCallbacks, MarketGatewayRunner, MarketGatewayRunnerConfig, RunnerError,
    TradeGatewayCallbacks, TradeGatewayRunner, TradeGatewayRunnerConfig,
};
pub use simulator::{SimulatedSource, SourceConfig};
pub use stats::{LatencySnapshot, LatencyStats};
pub use subscription::SubscriptionManager;
pub use trade::{TradeGateway, TradeGatewayEvent, TradeSimulator};
