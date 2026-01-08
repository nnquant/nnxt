//! Strategy runner for intent-based execution.

pub mod action;
pub mod context;
pub mod execution;
pub mod intent;
pub mod ledger;
pub mod master_client;
pub mod order_manager;
pub mod runner;
pub mod state;
pub mod strategy;

pub use action::{Action, ActionKind, CancelOrder, NewOrder};
pub use context::{
    MarketDataType, PendingRequests, QuoteSubscription, StrategyContext, TimerEvent, TimerId,
    TradeConnection,
};
pub use execution::{ExecutionEngine, PortfolioView};
pub use intent::{CancelOrderIntent, Intent, TargetOrder, TargetOrdersIntent, TargetPositionIntent};
pub use ledger::Ledger;
pub use master_client::{MasterClient, MasterClientError};
pub use order_manager::{OrderManager, OrderState};
pub use runner::{RunnerConfig, RunnerError, StrategyRunner};
pub use state::RunnerState;
pub use strategy::{Strategy, StrategyError};
