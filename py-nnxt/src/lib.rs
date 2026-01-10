//! Python bindings for nnxt.
//!
//! :module: nnxt

use std::str::FromStr;
use std::time::Duration;

use nnxt_gateway::{
    MarketGatewayCallbacks, MarketGatewayRunner, MarketGatewayRunnerConfig, RunnerError,
    TradeGatewayCallbacks, TradeGatewayRunner, TradeGatewayRunnerConfig,
};
use nnxt_rapid::{Address, Writer};
use nnxt_specs::market::{InstrumentId, ORDER_BOOK_DEPTH};
use nnxt_specs::{OrderBook, OrderEvent, OrderStatus, PriceType, Side, TradeEvent};
use nnxt_strategy::{Action, Intent, RunnerConfig, Strategy, StrategyContext, StrategyRunner};
use nnxt_utils::clock::{Clock, InstantClock, MonotonicClock};
use nnxt_utils::{setup_log as nnxt_setup_log, setup_signal};
use pyo3::exceptions::{PyKeyboardInterrupt, PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyModule, PyTuple, PyType};
use tracing::{debug, error, info, warn};

/// Python wrapper for InstrumentId.
///
/// :param value: instrument identifier string.
/// :type value: str
#[pyclass(name = "InstrumentId")]
pub struct PyInstrumentId {
    inner: InstrumentId,
}

/// Python wrapper for InstantClock.
///
/// :returns: instant clock instance.
/// :rtype: InstantClock
#[pyclass(name = "InstantClock")]
pub struct PyInstantClock {
    inner: InstantClock,
}

#[pymethods]
impl PyInstantClock {
    #[new]
    fn new() -> Self {
        Self {
            inner: InstantClock::new(),
        }
    }

    fn now_ns(&self) -> u64 {
        self.inner.now_ns()
    }
}

/// Python wrapper for MonotonicClock.
///
/// :returns: monotonic clock type.
/// :rtype: MonotonicClock
#[pyclass(name = "MonotonicClock")]
pub struct PyMonotonicClock;

#[pymethods]
impl PyMonotonicClock {
    #[classmethod]
    fn now_ns(_cls: &Bound<'_, PyType>) -> u64 {
        MonotonicClock::now_ns()
    }
}

/// Python enum-like wrapper for PriceType.
#[pyclass(name = "PriceType")]
pub struct PyPriceType;

#[pymethods]
impl PyPriceType {
    #[classattr]
    const LIMIT: u8 = 1;
    #[classattr]
    const MARKET: u8 = 2;
    #[classattr]
    const OPPONENT_BEST: u8 = 3;
    #[classattr]
    const OWN_BEST: u8 = 4;
}

/// Python enum-like wrapper for Side.
#[pyclass(name = "Side")]
pub struct PySide;

#[pymethods]
impl PySide {
    #[classattr]
    const BUY: u8 = 1;
    #[classattr]
    const SELL: u8 = 2;
}

/// Python enum-like wrapper for OrderStatus.
#[pyclass(name = "OrderStatus")]
pub struct PyOrderStatus;

#[pymethods]
impl PyOrderStatus {
    #[classattr]
    const PENDING: u8 = 1;
    #[classattr]
    const PENDING_NEW: u8 = 2;
    #[classattr]
    const ACTIVE: u8 = 3;
    #[classattr]
    const PENDING_CANCEL: u8 = 4;
    #[classattr]
    const FILLED: u8 = 5;
    #[classattr]
    const CANCELLED: u8 = 6;
    #[classattr]
    const REJECTED: u8 = 7;
    #[classattr]
    const PARTIAL_FILLED: u8 = 8;
}

#[pymethods]
impl PyInstrumentId {
    #[new]
    fn new(value: &str) -> PyResult<Self> {
        let inner = InstrumentId::from_str(value)
            .map_err(|err| PyValueError::new_err(err.to_string()))?;
        Ok(Self { inner })
    }

    fn as_str(&self) -> PyResult<String> {
        self.inner
            .as_str()
            .map(|value| value.to_string())
            .map_err(|err| PyValueError::new_err(err.to_string()))
    }

    fn __repr__(&self) -> PyResult<String> {
        Ok(format!("InstrumentId('{}')", self.as_str()?))
    }
}

/// Python wrapper for OrderBook.
///
/// :returns: order book instance with default values.
/// :rtype: OrderBook
#[pyclass(name = "OrderBook")]
pub struct PyOrderBook {
    inner: OrderBook,
}

#[pymethods]
impl PyOrderBook {
    #[new]
    fn new() -> Self {
        Self {
            inner: OrderBook::default(),
        }
    }

    #[getter]
    fn instrument_id(&self) -> PyResult<PyInstrumentId> {
        Ok(PyInstrumentId {
            inner: self.inner.instrument_id,
        })
    }

    #[setter]
    fn set_instrument_id(&mut self, id: &PyInstrumentId) {
        self.inner.instrument_id = id.inner;
    }

    #[getter]
    fn bid_price(&self) -> Vec<f64> {
        self.inner.bid_price.to_vec()
    }

    #[setter]
    fn set_bid_price(&mut self, values: Vec<f64>) -> PyResult<()> {
        fill_levels(&mut self.inner.bid_price, values)
    }

    #[getter]
    fn ask_price(&self) -> Vec<f64> {
        self.inner.ask_price.to_vec()
    }

    #[setter]
    fn set_ask_price(&mut self, values: Vec<f64>) -> PyResult<()> {
        fill_levels(&mut self.inner.ask_price, values)
    }

    #[getter]
    fn bid_volume(&self) -> Vec<u64> {
        self.inner.bid_volume.to_vec()
    }

    #[setter]
    fn set_bid_volume(&mut self, values: Vec<u64>) -> PyResult<()> {
        fill_levels(&mut self.inner.bid_volume, values)
    }

    #[getter]
    fn ask_volume(&self) -> Vec<u64> {
        self.inner.ask_volume.to_vec()
    }

    #[setter]
    fn set_ask_volume(&mut self, values: Vec<u64>) -> PyResult<()> {
        fill_levels(&mut self.inner.ask_volume, values)
    }

    #[getter]
    fn last_price(&self) -> f64 {
        self.inner.last_price
    }

    #[setter]
    fn set_last_price(&mut self, value: f64) {
        self.inner.last_price = value;
    }

    #[getter]
    fn timestamp(&self) -> u64 {
        self.inner.timestamp
    }

    #[setter]
    fn set_timestamp(&mut self, value: u64) {
        self.inner.timestamp = value;
    }
}

/// Python wrapper for OrderEvent.
///
/// :returns: order event instance with default values.
/// :rtype: OrderEvent
#[pyclass(name = "OrderEvent")]
pub struct PyOrderEvent {
    inner: OrderEvent,
}

#[pymethods]
impl PyOrderEvent {
    #[new]
    fn new() -> Self {
        Self {
            inner: OrderEvent {
                instrument_id: InstrumentId::default(),
                order_id: 0,
                status: OrderStatus::Pending,
                filled_quantity: 0,
                remaining_quantity: 0,
                last_price: 0.0,
                timestamp: 0,
            },
        }
    }

    #[getter]
    fn order_id(&self) -> u64 {
        self.inner.order_id
    }

    #[setter]
    fn set_order_id(&mut self, value: u64) {
        self.inner.order_id = value;
    }

    #[getter]
    fn instrument(&self) -> PyResult<PyInstrumentId> {
        Ok(PyInstrumentId {
            inner: self.inner.instrument_id,
        })
    }

    #[setter]
    fn set_instrument(&mut self, value: &PyInstrumentId) {
        self.inner.instrument_id = value.inner;
    }

    #[getter]
    fn status(&self) -> u8 {
        self.inner.status as u8
    }

    #[setter]
    fn set_status(&mut self, value: u8) -> PyResult<()> {
        self.inner.status = parse_order_status(value)?;
        Ok(())
    }

    #[getter]
    fn filled_quantity(&self) -> u64 {
        self.inner.filled_quantity
    }

    #[setter]
    fn set_filled_quantity(&mut self, value: u64) {
        self.inner.filled_quantity = value;
    }

    #[getter]
    fn remaining_quantity(&self) -> u64 {
        self.inner.remaining_quantity
    }

    #[setter]
    fn set_remaining_quantity(&mut self, value: u64) {
        self.inner.remaining_quantity = value;
    }

    #[getter]
    fn last_price(&self) -> f64 {
        self.inner.last_price
    }

    #[setter]
    fn set_last_price(&mut self, value: f64) {
        self.inner.last_price = value;
    }

    #[getter]
    fn timestamp(&self) -> u64 {
        self.inner.timestamp
    }

    #[setter]
    fn set_timestamp(&mut self, value: u64) {
        self.inner.timestamp = value;
    }
}

/// Python wrapper for TradeEvent.
///
/// :returns: trade event instance with default values.
/// :rtype: TradeEvent
#[pyclass(name = "TradeEvent")]
pub struct PyTradeEvent {
    inner: TradeEvent,
}

#[pymethods]
impl PyTradeEvent {
    #[new]
    fn new() -> Self {
        Self {
            inner: TradeEvent {
                instrument_id: InstrumentId::default(),
                trade_id: 0,
                order_id: 0,
                side: Side::Buy,
                price: 0.0,
                quantity: 0,
                timestamp: 0,
            },
        }
    }

    #[getter]
    fn order_id(&self) -> u64 {
        self.inner.order_id
    }

    #[setter]
    fn set_order_id(&mut self, value: u64) {
        self.inner.order_id = value;
    }

    #[getter]
    fn instrument(&self) -> PyResult<PyInstrumentId> {
        Ok(PyInstrumentId {
            inner: self.inner.instrument_id,
        })
    }

    #[setter]
    fn set_instrument(&mut self, value: &PyInstrumentId) {
        self.inner.instrument_id = value.inner;
    }

    #[getter]
    fn side(&self) -> u8 {
        self.inner.side as u8
    }

    #[setter]
    fn set_side(&mut self, value: u8) -> PyResult<()> {
        self.inner.side = parse_side(value)?;
        Ok(())
    }

    #[getter]
    fn quantity(&self) -> u64 {
        self.inner.quantity
    }

    #[setter]
    fn set_quantity(&mut self, value: u64) {
        self.inner.quantity = value;
    }

    #[getter]
    fn trade_id(&self) -> u64 {
        self.inner.trade_id
    }

    #[setter]
    fn set_trade_id(&mut self, value: u64) {
        self.inner.trade_id = value;
    }

    #[getter]
    fn price(&self) -> f64 {
        self.inner.price
    }

    #[setter]
    fn set_price(&mut self, value: f64) {
        self.inner.price = value;
    }

    #[getter]
    fn timestamp(&self) -> u64 {
        self.inner.timestamp
    }

    #[setter]
    fn set_timestamp(&mut self, value: u64) {
        self.inner.timestamp = value;
    }
}

/// Python wrapper for Action.
///
/// :returns: action instance.
/// :rtype: Action
#[pyclass(name = "Action")]
pub struct PyAction {
    inner: Action,
}

#[pymethods]
impl PyAction {
    #[classmethod]
    #[pyo3(signature = (order_id, instrument, price, qty, side, price_type, client_order_id = 0, timestamp = 0))]
    fn new_order(
        _cls: &Bound<'_, PyType>,
        order_id: u64,
        instrument: &PyInstrumentId,
        price: f64,
        qty: u64,
        side: u8,
        price_type: u8,
        client_order_id: u64,
        timestamp: u64,
    ) -> PyResult<Self> {
        let order = nnxt_strategy::NewOrder {
            instrument_id: instrument.inner,
            order_id,
            client_order_id,
            side: parse_side(side)?,
            price_type: parse_price_type(price_type)?,
            limit_price: price,
            quantity: qty,
            timestamp,
        };
        Ok(Self {
            inner: Action::new_order(order),
        })
    }

    #[classmethod]
    #[pyo3(signature = (order_id, instrument, timestamp = 0))]
    fn cancel_order(
        _cls: &Bound<'_, PyType>,
        order_id: u64,
        instrument: &PyInstrumentId,
        timestamp: u64,
    ) -> PyResult<Self> {
        let cancel = nnxt_strategy::CancelOrder {
            instrument_id: instrument.inner,
            order_id,
            timestamp,
        };
        Ok(Self {
            inner: Action::cancel_order(cancel),
        })
    }

    #[classattr]
    const NEW_ORDER: u8 = nnxt_strategy::ActionKind::NewOrder as u8;
    #[classattr]
    const CANCEL_ORDER: u8 = nnxt_strategy::ActionKind::CancelOrder as u8;

    #[getter]
    fn kind(&self) -> u8 {
        self.inner.kind as u8
    }

    #[getter]
    fn new_order_order_id(&self) -> u64 {
        self.inner.new_order.order_id
    }

    #[setter]
    fn set_new_order_order_id(&mut self, value: u64) {
        self.inner.new_order.order_id = value;
    }

    #[getter]
    fn new_order_client_order_id(&self) -> u64 {
        self.inner.new_order.client_order_id
    }

    #[setter]
    fn set_new_order_client_order_id(&mut self, value: u64) {
        self.inner.new_order.client_order_id = value;
    }

    #[getter]
    fn new_order_instrument(&self) -> PyResult<PyInstrumentId> {
        Ok(PyInstrumentId {
            inner: self.inner.new_order.instrument_id,
        })
    }

    #[setter]
    fn set_new_order_instrument(&mut self, value: &PyInstrumentId) {
        self.inner.new_order.instrument_id = value.inner;
    }

    #[getter]
    fn new_order_side(&self) -> u8 {
        self.inner.new_order.side as u8
    }

    #[setter]
    fn set_new_order_side(&mut self, value: u8) -> PyResult<()> {
        self.inner.new_order.side = parse_side(value)?;
        Ok(())
    }

    #[getter]
    fn new_order_price_type(&self) -> u8 {
        self.inner.new_order.price_type as u8
    }

    #[setter]
    fn set_new_order_price_type(&mut self, value: u8) -> PyResult<()> {
        self.inner.new_order.price_type = parse_price_type(value)?;
        Ok(())
    }

    #[getter]
    fn new_order_limit_price(&self) -> f64 {
        self.inner.new_order.limit_price
    }

    #[setter]
    fn set_new_order_limit_price(&mut self, value: f64) {
        self.inner.new_order.limit_price = value;
    }

    #[getter]
    fn new_order_quantity(&self) -> u64 {
        self.inner.new_order.quantity
    }

    #[setter]
    fn set_new_order_quantity(&mut self, value: u64) {
        self.inner.new_order.quantity = value;
    }

    #[getter]
    fn new_order_timestamp(&self) -> u64 {
        self.inner.new_order.timestamp
    }

    #[setter]
    fn set_new_order_timestamp(&mut self, value: u64) {
        self.inner.new_order.timestamp = value;
    }

    #[getter]
    fn cancel_order_order_id(&self) -> u64 {
        self.inner.cancel_order.order_id
    }

    #[setter]
    fn set_cancel_order_order_id(&mut self, value: u64) {
        self.inner.cancel_order.order_id = value;
    }

    #[getter]
    fn cancel_order_instrument(&self) -> PyResult<PyInstrumentId> {
        Ok(PyInstrumentId {
            inner: self.inner.cancel_order.instrument_id,
        })
    }

    #[setter]
    fn set_cancel_order_instrument(&mut self, value: &PyInstrumentId) {
        self.inner.cancel_order.instrument_id = value.inner;
    }

    #[getter]
    fn cancel_order_timestamp(&self) -> u64 {
        self.inner.cancel_order.timestamp
    }

    #[setter]
    fn set_cancel_order_timestamp(&mut self, value: u64) {
        self.inner.cancel_order.timestamp = value;
    }
}

/// Python wrapper for Intent.
///
/// :returns: intent instance.
/// :rtype: Intent
#[pyclass(name = "Intent")]
pub struct PyIntent {
    inner: Intent,
}

#[pymethods]
impl PyIntent {
    #[classmethod]
    fn target_position(
        _cls: &Bound<'_, PyType>,
        instrument: &PyInstrumentId,
        quantity: i64,
        price_type: u8,
        limit_price: f64,
    ) -> PyResult<Self> {
        let price_type = parse_price_type(price_type)?;
        let intent = Intent::target_position(instrument.inner, quantity, price_type, limit_price);
        Ok(Self { inner: intent })
    }

    #[classmethod]
    fn cancel_order(_cls: &Bound<'_, PyType>, instrument: &PyInstrumentId, order_id: u64) -> Self {
        let intent = Intent::cancel_order(instrument.inner, order_id);
        Self { inner: intent }
    }
}

/// Python wrapper for StrategyContext.
///
/// :param ctx: internal context pointer for the current callback.
/// :type ctx: StrategyContext
#[pyclass(name = "StrategyContext", unsendable)]
pub struct PyStrategyContext {
    ctx: *mut StrategyContext<'static>,
    valid: bool,
}

impl PyStrategyContext {
    fn check_valid(&self) -> PyResult<()> {
        if !self.valid {
            return Err(PyRuntimeError::new_err(
                "StrategyContext is only valid during callback"
            ));
        }
        Ok(())
    }

    fn invalidate(&mut self) {
        self.valid = false;
    }
}

#[pymethods]
impl PyStrategyContext {
    fn subscribe_quote(&mut self, source: &str, instrument: &PyInstrumentId) -> PyResult<()> {
        self.check_valid()?;
        unsafe { (*self.ctx).subscribe_quote::<OrderBook>(source, &instrument.inner); }
        Ok(())
    }

    fn connect_trade(&mut self, target: &str) -> PyResult<()> {
        self.check_valid()?;
        unsafe { (*self.ctx).connect_trade(target); }
        Ok(())
    }

    fn submit_intent(&mut self, intent: &PyIntent) -> PyResult<()> {
        self.check_valid()?;
        unsafe { (*self.ctx).submit_intent(intent.inner.clone()); }
        Ok(())
    }

    fn set_timer(&mut self, interval_ns: u64) -> PyResult<u64> {
        self.check_valid()?;
        Ok(unsafe { (*self.ctx).set_timer(interval_ns) })
    }

    fn cancel_timer(&mut self, timer_id: u64) -> PyResult<bool> {
        self.check_valid()?;
        Ok(unsafe { (*self.ctx).cancel_timer(timer_id) })
    }

    fn position(&mut self, instrument: &PyInstrumentId) -> PyResult<Option<(i64, f64, u64)>> {
        self.check_valid()?;
        Ok(unsafe {
            (*self.ctx)
                .position(&instrument.inner)
                .map(|pos| (pos.quantity, pos.avg_price, pos.last_update_ns))
        })
    }

    fn log_debug(&mut self, message: &str) -> PyResult<()> {
        self.check_valid()?;
        debug!("{}", message);
        Ok(())
    }

    fn log_info(&mut self, message: &str) -> PyResult<()> {
        self.check_valid()?;
        info!("{}", message);
        Ok(())
    }

    fn log_warn(&mut self, message: &str) -> PyResult<()> {
        self.check_valid()?;
        warn!("{}", message);
        Ok(())
    }

    fn log_error(&mut self, message: &str) -> PyResult<()> {
        self.check_valid()?;
        error!("{}", message);
        Ok(())
    }
}

/// Base Python Strategy class.
///
/// :returns: strategy base instance.
/// :rtype: Strategy
#[pyclass(name = "Strategy", subclass)]
pub struct PyStrategy {}

#[pymethods]
impl PyStrategy {
    #[new]
    #[pyo3(signature = (*_args, **_kwargs))]
    fn new(_args: &Bound<'_, PyAny>, _kwargs: Option<&Bound<'_, PyAny>>) -> Self {
        Self {}
    }

    fn on_start(&mut self, _ctx: &PyStrategyContext) {}
    fn on_stop(&mut self, _ctx: &PyStrategyContext) {}
    fn on_order_book(&mut self, _book: &PyOrderBook, _ctx: &PyStrategyContext) {}
    fn on_order(&mut self, _event: &PyOrderEvent, _ctx: &PyStrategyContext) {}
    fn on_trade(&mut self, _event: &PyTradeEvent, _ctx: &PyStrategyContext) {}
}

fn call_python_method(obj: &Bound<'_, PyAny>, name: &str) -> PyResult<()> {
    obj.call_method0(name)?;
    Ok(())
}

fn py_err_to_runner(err: PyErr) -> RunnerError {
    RunnerError::Callback(format!("{:?}", err))
}

fn should_stop(py: Python<'_>) -> PyResult<bool> {
    match py.check_signals() {
        Ok(()) => Ok(false),
        Err(err) => {
            if err.is_instance_of::<PyKeyboardInterrupt>(py) {
                return Ok(true);
            }
            Err(err)
        }
    }
}

fn run_gateway_loop(obj: &Bound<'_, PyAny>, poll_interval_ms: u64) -> PyResult<()> {
    let shutdown = setup_signal();
    call_python_method(obj, "on_start")?;
    loop {
        let stop = Python::with_gil(|py| should_stop(py))?;
        if stop || shutdown.is_shutdown() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(poll_interval_ms));
    }
    call_python_method(obj, "on_stop")?;
    Ok(())
}

/// StrategyRunner for Python strategies.
///
/// :param strategy: python strategy instance.
/// :type strategy: Strategy
/// :param master_addr: master address.
/// :type master_addr: str or None
#[pyclass(name = "StrategyRunner")]
pub struct PyStrategyRunner {
    inner: StrategyRunner<PyStrategyAdapter>,
}

#[pymethods]
impl PyStrategyRunner {
    #[new]
    #[pyo3(signature = (strategy, master_addr = None, actor_id = "strategy-1", actor_type = "strategy"))]
    fn new(strategy: Py<PyAny>, master_addr: Option<String>, actor_id: &str, actor_type: &str) -> PyResult<Self> {
        let config = RunnerConfig {
            master_addr,
            actor_id: actor_id.to_string(),
            actor_type: actor_type.to_string(),
            ..RunnerConfig::default()
        };
        let adapter = PyStrategyAdapter { py_strategy: strategy };
        let inner = StrategyRunner::new(adapter, config)
            .map_err(|err| PyRuntimeError::new_err(format!("runner create failed: {:?}", err)))?;
        Ok(Self { inner })
    }

    fn run(&mut self, py: Python<'_>) -> PyResult<()> {
        // Release the GIL so Python threads (e.g., market simulator loop) can run.
        let result = py.allow_threads(|| self.inner.run());
        result.map_err(|err| PyRuntimeError::new_err(format!("runner run failed: {:?}", err)))
    }
}

/// MarketGatewayRunner for Python gateways.
///
/// :param gateway: python market gateway instance.
/// :type gateway: MarketGateway
/// :param queue_path: rapid queue path.
/// :type queue_path: str
/// :param master_addr: master address.
/// :type master_addr: str or None
#[pyclass(name = "MarketGatewayRunner")]
pub struct PyMarketGatewayRunner {
    inner: MarketGatewayRunner<PyMarketGatewayAdapter>,
}

#[pymethods]
impl PyMarketGatewayRunner {
    #[new]
    #[pyo3(
        signature = (
            gateway,
            queue_path,
            master_addr = None,
            actor_id = "market-gateway",
            actor_type = "market-gateway",
            heartbeat_interval_ms = 1000,
            control_addr = None
        )
    )]
    fn new(
        gateway: Py<PyAny>,
        queue_path: &str,
        master_addr: Option<String>,
        actor_id: &str,
        actor_type: &str,
        heartbeat_interval_ms: u64,
        control_addr: Option<String>,
    ) -> PyResult<Self> {
        let config = MarketGatewayRunnerConfig {
            queue_path: queue_path.to_string(),
            master_addr,
            actor_id: actor_id.to_string(),
            actor_type: actor_type.to_string(),
            heartbeat_interval: Duration::from_millis(heartbeat_interval_ms),
            control_addr,
        };
        let adapter = PyMarketGatewayAdapter { gateway };
        let inner = MarketGatewayRunner::new(adapter, config)
            .map_err(|err| PyRuntimeError::new_err(format!("runner create failed: {:?}", err)))?;
        Ok(Self { inner })
    }

    fn run(&mut self, py: Python<'_>) -> PyResult<()> {
        // Release the GIL so Python callbacks/threads can run.
        let result = py.allow_threads(|| self.inner.run());
        result.map_err(|err| PyRuntimeError::new_err(format!("runner run failed: {:?}", err)))
    }
}

/// TradeGatewayRunner for Python gateways.
///
/// :param gateway: python trade gateway instance.
/// :type gateway: TradeGateway
/// :param order_event_queue: order event queue path.
/// :type order_event_queue: str
/// :param trade_event_queue: trade event queue path.
/// :type trade_event_queue: str
#[pyclass(name = "TradeGatewayRunner")]
pub struct PyTradeGatewayRunner {
    inner: TradeGatewayRunner<PyTradeGatewayAdapter>,
}

#[pymethods]
impl PyTradeGatewayRunner {
    #[new]
    #[pyo3(
        signature = (
            gateway,
            order_event_queue,
            trade_event_queue,
            action_queues = None,
            master_addr = None,
            actor_id = "trade-gateway",
            actor_type = "trade-gateway",
            heartbeat_interval_ms = 1000,
            control_addr = None
        )
    )]
    fn new(
        gateway: Py<PyAny>,
        order_event_queue: &str,
        trade_event_queue: &str,
        action_queues: Option<Vec<String>>,
        master_addr: Option<String>,
        actor_id: &str,
        actor_type: &str,
        heartbeat_interval_ms: u64,
        control_addr: Option<String>,
    ) -> PyResult<Self> {
        let config = TradeGatewayRunnerConfig {
            order_event_queue: order_event_queue.to_string(),
            trade_event_queue: trade_event_queue.to_string(),
            action_queues: action_queues.unwrap_or_default(),
            master_addr,
            actor_id: actor_id.to_string(),
            actor_type: actor_type.to_string(),
            heartbeat_interval: Duration::from_millis(heartbeat_interval_ms),
            control_addr,
        };
        let adapter = PyTradeGatewayAdapter { gateway };
        let inner = TradeGatewayRunner::new(adapter, config)
            .map_err(|err| PyRuntimeError::new_err(format!("runner create failed: {:?}", err)))?;
        Ok(Self { inner })
    }

    fn run(&mut self, py: Python<'_>) -> PyResult<()> {
        // Release the GIL so Python callbacks/threads can run.
        let result = py.allow_threads(|| self.inner.run());
        result.map_err(|err| PyRuntimeError::new_err(format!("runner run failed: {:?}", err)))
    }
}

/// Base class for MarketGateway callbacks (used with MarketGatewayRunner).
///
/// This class does not create a Writer in __init__ - call init_writer() in on_start().
#[pyclass(name = "MarketGateway", subclass)]
pub struct PyMarketGateway {
    writer: Option<Writer<OrderBook>>,
}

#[pymethods]
impl PyMarketGateway {
    #[new]
    #[pyo3(signature = (*_args, **_kwargs))]
    fn new(_args: &Bound<'_, PyTuple>, _kwargs: Option<&Bound<'_, PyDict>>) -> Self {
        Self { writer: None }
    }

    /// Initialize the writer. Call this in on_start().
    #[pyo3(signature = (queue_path, capacity = 1024))]
    fn init_writer(&mut self, queue_path: &str, capacity: usize) -> PyResult<()> {
        let addr = Address::new(queue_path).map_err(|err| {
            PyValueError::new_err(format!("invalid queue path: {:?}", err))
        })?;
        let writer = Writer::create(&addr, capacity)
            .map_err(|err| PyRuntimeError::new_err(format!("writer create failed: {:?}", err)))?;
        self.writer = Some(writer);
        Ok(())
    }

    fn publish_order_book(&mut self, book: &PyOrderBook) -> PyResult<()> {
        match &mut self.writer {
            Some(w) => {
                w.write(book.inner);
                Ok(())
            }
            None => Err(PyRuntimeError::new_err("writer not initialized, call init_writer() first")),
        }
    }

    fn on_start(&mut self) {}
    fn on_subscribe(&mut self, _instrument: &PyInstrumentId) {}
    fn on_unsubscribe(&mut self, _instrument: &PyInstrumentId) {}
    fn on_stop(&mut self) {}
}

/// Base class for TradeGateway callbacks (used with TradeGatewayRunner).
///
/// This class does not create Writers in __init__ - call init_writers() in on_start().
#[pyclass(name = "TradeGateway", subclass)]
pub struct PyTradeGateway {
    order_writer: Option<Writer<OrderEvent>>,
    trade_writer: Option<Writer<TradeEvent>>,
}

#[pymethods]
impl PyTradeGateway {
    #[new]
    #[pyo3(signature = (*_args, **_kwargs))]
    fn new(_args: &Bound<'_, PyTuple>, _kwargs: Option<&Bound<'_, PyDict>>) -> Self {
        Self {
            order_writer: None,
            trade_writer: None,
        }
    }

    /// Initialize the writers. Call this in on_start().
    #[pyo3(signature = (order_event_queue, trade_event_queue, capacity = 1024))]
    fn init_writers(
        &mut self,
        order_event_queue: &str,
        trade_event_queue: &str,
        capacity: usize,
    ) -> PyResult<()> {
        let order_addr = Address::new(order_event_queue).map_err(|err| {
            PyValueError::new_err(format!("invalid order queue: {:?}", err))
        })?;
        let trade_addr = Address::new(trade_event_queue).map_err(|err| {
            PyValueError::new_err(format!("invalid trade queue: {:?}", err))
        })?;
        let order_writer = Writer::create(&order_addr, capacity)
            .map_err(|err| PyRuntimeError::new_err(format!("order writer failed: {:?}", err)))?;
        let trade_writer = Writer::create(&trade_addr, capacity)
            .map_err(|err| PyRuntimeError::new_err(format!("trade writer failed: {:?}", err)))?;
        self.order_writer = Some(order_writer);
        self.trade_writer = Some(trade_writer);
        Ok(())
    }

    fn publish_order_event(&mut self, event: &PyOrderEvent) -> PyResult<()> {
        match &mut self.order_writer {
            Some(w) => {
                w.write(event.inner);
                Ok(())
            }
            None => Err(PyRuntimeError::new_err("writers not initialized")),
        }
    }

    fn publish_trade_event(&mut self, event: &PyTradeEvent) -> PyResult<()> {
        match &mut self.trade_writer {
            Some(w) => {
                w.write(event.inner);
                Ok(())
            }
            None => Err(PyRuntimeError::new_err("writers not initialized")),
        }
    }

    fn on_start(&mut self) {}
    fn on_action(&mut self, _action: &PyAction) {}
    fn on_stop(&mut self) {}
}

struct PyMarketGatewayAdapter {
    gateway: Py<PyAny>,
}

impl PyMarketGatewayAdapter {
    fn call_method0(&self, name: &str) -> Result<(), RunnerError> {
        Python::with_gil(|py| {
            let obj = self.gateway.bind(py);
            obj.call_method0(name).map_err(py_err_to_runner)?;
            Ok(())
        })
    }

    fn call_method_instrument(
        &self,
        name: &str,
        instrument_id: InstrumentId,
    ) -> Result<(), RunnerError> {
        Python::with_gil(|py| {
            let obj = self.gateway.bind(py);
            let instrument = Py::new(py, PyInstrumentId { inner: instrument_id })
                .map_err(py_err_to_runner)?;
            obj.call_method1(name, (instrument,)).map_err(py_err_to_runner)?;
            Ok(())
        })
    }
}

impl MarketGatewayCallbacks for PyMarketGatewayAdapter {
    fn on_start(&mut self) -> Result<(), RunnerError> {
        self.call_method0("on_start")
    }

    fn on_subscribe(&mut self, instrument_id: InstrumentId) -> Result<(), RunnerError> {
        self.call_method_instrument("on_subscribe", instrument_id)
    }

    fn on_unsubscribe(&mut self, instrument_id: InstrumentId) -> Result<(), RunnerError> {
        self.call_method_instrument("on_unsubscribe", instrument_id)
    }

    fn on_stop(&mut self) -> Result<(), RunnerError> {
        self.call_method0("on_stop")
    }
}

struct PyTradeGatewayAdapter {
    gateway: Py<PyAny>,
}

impl PyTradeGatewayAdapter {
    fn call_method0(&self, name: &str) -> Result<(), RunnerError> {
        Python::with_gil(|py| {
            let obj = self.gateway.bind(py);
            obj.call_method0(name).map_err(py_err_to_runner)?;
            Ok(())
        })
    }

    fn call_method_action(&self, name: &str, action: &Action) -> Result<(), RunnerError> {
        Python::with_gil(|py| {
            let obj = self.gateway.bind(py);
            let action = Py::new(py, PyAction { inner: *action }).map_err(py_err_to_runner)?;
            obj.call_method1(name, (action,)).map_err(py_err_to_runner)?;
            Ok(())
        })
    }
}

impl TradeGatewayCallbacks for PyTradeGatewayAdapter {
    fn on_start(&mut self) -> Result<(), RunnerError> {
        self.call_method0("on_start")
    }

    fn on_action(&mut self, action: &Action) -> Result<(), RunnerError> {
        self.call_method_action("on_action", action)
    }

    fn on_stop(&mut self) -> Result<(), RunnerError> {
        self.call_method0("on_stop")
    }
}

struct PyStrategyAdapter {
    py_strategy: Py<PyAny>,
}

impl PyStrategyAdapter {
    fn call_with_ctx(
        &self,
        name: &str,
        ctx: &mut StrategyContext,
        extra_args: impl FnOnce(Python<'_>) -> PyResult<Vec<PyObject>>,
    ) {
        Python::with_gil(|py| {
            let obj = self.py_strategy.bind(py);
            let py_ctx = Self::wrap_ctx(ctx, py)?;
            let mut args = extra_args(py)?;
            args.push(py_ctx.clone_ref(py).into_py(py));
            let args = PyTuple::new_bound(py, args);
            let _ = obj.call_method1(name, args);
            // 回调结束，使 ctx 失效
            py_ctx.borrow_mut(py).invalidate();
            Ok::<(), PyErr>(())
        })
        .unwrap_or(())
    }

    fn wrap_ctx(ctx: &mut StrategyContext, py: Python<'_>) -> PyResult<Py<PyStrategyContext>> {
        let ptr = ctx as *mut StrategyContext as *mut StrategyContext<'static>;
        Py::new(py, PyStrategyContext { ctx: ptr, valid: true })
    }
}

impl Strategy for PyStrategyAdapter {
    fn on_start(&mut self, ctx: &mut StrategyContext) {
        self.call_with_ctx("on_start", ctx, |_py| Ok(vec![]));
    }

    fn on_stop(&mut self, ctx: &mut StrategyContext) {
        self.call_with_ctx("on_stop", ctx, |_py| Ok(vec![]));
    }

    fn on_order_book(&mut self, book: &OrderBook, ctx: &mut StrategyContext) {
        self.call_with_ctx("on_order_book", ctx, |py| {
            let book = Py::new(py, PyOrderBook { inner: *book })?;
            Ok(vec![book.into_py(py)])
        });
    }

    fn on_order(&mut self, event: &OrderEvent, ctx: &mut StrategyContext) {
        self.call_with_ctx("on_order", ctx, |py| {
            let event = Py::new(py, PyOrderEvent { inner: *event })?;
            Ok(vec![event.into_py(py)])
        });
    }

    fn on_trade(&mut self, event: &TradeEvent, ctx: &mut StrategyContext) {
        self.call_with_ctx("on_trade", ctx, |py| {
            let event = Py::new(py, PyTradeEvent { inner: *event })?;
            Ok(vec![event.into_py(py)])
        });
    }
}

#[pymodule(name = "nnxt")]
fn py_nnxt(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyInstantClock>()?;
    m.add_class::<PyMonotonicClock>()?;
    m.add_class::<PyInstrumentId>()?;
    m.add_class::<PyPriceType>()?;
    m.add_class::<PySide>()?;
    m.add_class::<PyOrderStatus>()?;
    m.add_class::<PyOrderBook>()?;
    m.add_class::<PyOrderEvent>()?;
    m.add_class::<PyTradeEvent>()?;
    m.add_class::<PyAction>()?;
    m.add_class::<PyIntent>()?;
    m.add_class::<PyStrategyContext>()?;
    m.add_class::<PyStrategy>()?;
    m.add_class::<PyStrategyRunner>()?;
    m.add_class::<PyMarketGatewayRunner>()?;
    m.add_class::<PyTradeGatewayRunner>()?;
    m.add_class::<PyMarketGateway>()?;
    m.add_class::<PyTradeGateway>()?;
    m.add_function(wrap_pyfunction!(setup_log, m)?)?;
    m.add_function(wrap_pyfunction!(monotonic_now_ns, m)?)?;
    m.add_function(wrap_pyfunction!(log_debug, m)?)?;
    m.add_function(wrap_pyfunction!(log_info, m)?)?;
    m.add_function(wrap_pyfunction!(log_warn, m)?)?;
    m.add_function(wrap_pyfunction!(log_error, m)?)?;
    Ok(())
}

#[pyfunction]
fn setup_log() -> PyResult<()> {
    nnxt_setup_log()
        .map_err(|err| PyRuntimeError::new_err(format!("setup log failed: {:?}", err)))
}

#[pyfunction]
fn monotonic_now_ns() -> u64 {
    MonotonicClock::now_ns()
}

#[pyfunction]
fn log_debug(message: &str) {
    debug!("{}", message);
}

#[pyfunction]
fn log_info(message: &str) {
    info!("{}", message);
}

#[pyfunction]
fn log_warn(message: &str) {
    warn!("{}", message);
}

#[pyfunction]
fn log_error(message: &str) {
    error!("{}", message);
}

fn fill_levels<T: Copy>(target: &mut [T; ORDER_BOOK_DEPTH], values: Vec<T>) -> PyResult<()> {
    if values.len() != ORDER_BOOK_DEPTH {
        return Err(PyValueError::new_err(format!(
            "expected {} levels, got {}",
            ORDER_BOOK_DEPTH,
            values.len()
        )));
    }
    for (idx, value) in values.into_iter().enumerate() {
        target[idx] = value;
    }
    Ok(())
}

fn parse_price_type(value: u8) -> PyResult<PriceType> {
    match value {
        1 => Ok(PriceType::Limit),
        2 => Ok(PriceType::Market),
        3 => Ok(PriceType::OpponentBest),
        4 => Ok(PriceType::OwnBest),
        _ => Err(PyValueError::new_err("invalid price_type")),
    }
}

fn parse_side(value: u8) -> PyResult<Side> {
    match value {
        1 => Ok(Side::Buy),
        2 => Ok(Side::Sell),
        _ => Err(PyValueError::new_err("invalid side")),
    }
}

fn parse_order_status(value: u8) -> PyResult<OrderStatus> {
    match value {
        1 => Ok(OrderStatus::Pending),
        2 => Ok(OrderStatus::PendingNew),
        3 => Ok(OrderStatus::Active),
        4 => Ok(OrderStatus::PendingCancel),
        5 => Ok(OrderStatus::Filled),
        6 => Ok(OrderStatus::Cancelled),
        7 => Ok(OrderStatus::Rejected),
        8 => Ok(OrderStatus::PartialFilled),
        _ => Err(PyValueError::new_err("invalid order_status")),
    }
}
