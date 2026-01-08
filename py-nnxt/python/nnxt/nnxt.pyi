"""Python stubs for nnxt bindings."""

from __future__ import annotations
from typing import Optional, Tuple

class InstantClock:
    """Instant clock anchored to object creation."""
    def __init__(self) -> None: ...
    def now_ns(self) -> int: ...

class MonotonicClock:
    """Monotonic clock for cross-process timestamps."""
    @classmethod
    def now_ns(cls) -> int: ...

class InstrumentId:
    """Instrument identifier wrapper.

    Args:
        value: Instrument identifier string (max 64 chars).
    """
    def __init__(self, value: str) -> None: ...
    def as_str(self) -> str: ...

class PriceType:
    """Price type enum values."""
    LIMIT: int
    MARKET: int
    OPPONENT_BEST: int
    OWN_BEST: int

class Side:
    """Side enum values."""
    BUY: int
    SELL: int

class OrderStatus:
    """Order status enum values."""
    PENDING: int
    PENDING_NEW: int
    ACTIVE: int
    PENDING_CANCEL: int
    FILLED: int
    CANCELLED: int
    REJECTED: int
    PARTIAL_FILLED: int

class OrderBook:
    """Order book snapshot."""
    def __init__(self) -> None: ...
    instrument_id: InstrumentId
    bid_price: list[float]
    ask_price: list[float]
    bid_volume: list[int]
    ask_volume: list[int]
    last_price: float
    timestamp: int

class OrderEvent:
    """Order event payload."""
    def __init__(self) -> None: ...
    order_id: int
    status: int
    timestamp: int

class TradeEvent:
    """Trade event payload."""
    def __init__(self) -> None: ...
    order_id: int
    trade_id: int
    price: float
    timestamp: int

class Action:
    """Execution action payload."""
    @classmethod
    def new_order(
        cls,
        order_id: int,
        instrument: InstrumentId,
        price: float,
        qty: int,
        side: int,
        price_type: int,
    ) -> "Action": ...

    @property
    def kind(self) -> int: ...

class Intent:
    """Strategy intent builder."""
    @classmethod
    def target_position(
        cls,
        instrument: InstrumentId,
        quantity: int,
        price_type: int,
        limit_price: float,
    ) -> "Intent": ...

    @classmethod
    def cancel_order(
        cls,
        instrument: InstrumentId,
        order_id: int,
    ) -> "Intent": ...

class StrategyContext:
    """Strategy execution context."""
    def subscribe_quote(self, source: str, instrument: InstrumentId) -> None: ...
    def connect_trade(self, target: str) -> None: ...
    def submit_intent(self, intent: Intent) -> None: ...
    def set_timer(self, interval_ns: int) -> int: ...
    def cancel_timer(self, timer_id: int) -> bool: ...
    def position(self, instrument: InstrumentId) -> Optional[Tuple[int, float, int]]: ...
    def log_debug(self, message: str) -> None: ...
    def log_info(self, message: str) -> None: ...
    def log_warn(self, message: str) -> None: ...
    def log_error(self, message: str) -> None: ...

class Strategy:
    """Base class for Python strategies."""
    def __init__(self, *args, **kwargs) -> None: ...
    def on_start(self, ctx: StrategyContext) -> None: ...
    def on_stop(self, ctx: StrategyContext) -> None: ...
    def on_order_book(self, book: OrderBook, ctx: StrategyContext) -> None: ...
    def on_order(self, event: OrderEvent, ctx: StrategyContext) -> None: ...
    def on_trade(self, event: TradeEvent, ctx: StrategyContext) -> None: ...

class StrategyRunner:
    """Runner for Python strategies."""
    def __init__(
        self,
        strategy: Strategy,
        master_addr: Optional[str] = None,
        actor_id: str = "strategy-1",
        actor_type: str = "strategy",
    ) -> None: ...
    def run(self) -> None: ...

class MarketGateway:
    """Market data gateway base class."""
    def __init__(self, queue_path: str, capacity: int = 1024) -> None: ...
    def run(self, poll_interval_ms: int = 10) -> None: ...
    def publish_order_book(self, book: OrderBook) -> None: ...
    def on_start(self) -> None: ...
    def on_stop(self) -> None: ...

class TradeGateway:
    """Trade gateway base class."""
    def __init__(
        self,
        order_event_queue: str,
        trade_event_queue: str,
        capacity: int = 1024,
    ) -> None: ...
    def run(self, poll_interval_ms: int = 10) -> None: ...
    def publish_order_event(self, event: OrderEvent) -> None: ...
    def publish_trade_event(self, event: TradeEvent) -> None: ...
    def on_start(self) -> None: ...
    def on_stop(self) -> None: ...

def setup_log() -> None: ...
def monotonic_now_ns() -> int: ...
def log_debug(message: str) -> None: ...
def log_info(message: str) -> None: ...
def log_warn(message: str) -> None: ...
def log_error(message: str) -> None: ...
