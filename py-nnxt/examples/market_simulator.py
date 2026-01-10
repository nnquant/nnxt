"""Python market simulator for nnxt."""

from __future__ import annotations

import argparse
import os
import threading
import time
from typing import Iterable, Sequence

import nnxt

ORDER_BOOK_DEPTH = 10


class MarketSimulator(nnxt.MarketGateway):
    """Simple market simulator that publishes synthetic order books."""

    def __init__(self, queue_path: str) -> None:
        super().__init__()
        self._queue_path = queue_path
        self._interval_ms = 200
        self._base_price = 100.0
        self._subscriptions: dict[str, nnxt.InstrumentId] = {}
        self._subscriptions_lock = threading.Lock()
        self._running = threading.Event()
        self._thread: threading.Thread | None = None
        self._seq = 0

    def configure(
        self,
        interval_ms: int,
        base_price: float,
        instruments: Sequence[str],
    ) -> None:
        self._interval_ms = interval_ms
        self._base_price = base_price
        subscriptions = {name: nnxt.InstrumentId(name) for name in instruments if name}
        with self._subscriptions_lock:
            self._subscriptions = subscriptions

    def on_start(self) -> None:
        nnxt.log_info("market simulator start event=[SIM_START]")
        self.init_writer(self._queue_path)
        self._running.set()
        self._thread = threading.Thread(target=self._run_loop, name="market-sim", daemon=True)
        self._thread.start()

    def on_subscribe(self, instrument: nnxt.InstrumentId) -> None:
        name = instrument.as_str()
        with self._subscriptions_lock:
            if name not in self._subscriptions:
                self._subscriptions[name] = instrument
        nnxt.log_info(f"market subscribe event=[SUBSCRIBE] instrument=[{name}]")

    def on_unsubscribe(self, instrument: nnxt.InstrumentId) -> None:
        name = instrument.as_str()
        with self._subscriptions_lock:
            self._subscriptions.pop(name, None)
        nnxt.log_info(f"market unsubscribe event=[UNSUBSCRIBE] instrument=[{name}]")

    def on_stop(self) -> None:
        nnxt.log_info("market simulator stop event=[SIM_STOP]")
        self._running.clear()
        if self._thread:
            self._thread.join(timeout=1.0)

    def _run_loop(self) -> None:
        nnxt.log_info(
            "market run loop started event=[RUN_LOOP_START] subscriptions=[{}]".format(
                list(self._subscriptions.keys())
            )
        )
        while self._running.is_set():
            with self._subscriptions_lock:
                instruments = list(self._subscriptions.values())
            if instruments:
                for instrument in instruments:
                    try:
                        self._publish_one(instrument)
                    except Exception as e:
                        nnxt.log_error(f"publish failed event=[PUBLISH_ERROR] error=[{e}]")
            else:
                nnxt.log_info("market run loop no subscriptions event=[NO_SUBS]")
            time.sleep(self._interval_ms / 1000.0)
        nnxt.log_info("market run loop stopped event=[RUN_LOOP_STOP]")

    def _publish_one(self, instrument: nnxt.InstrumentId) -> None:
        price = self._base_price + (self._seq % 20) * 0.1
        bid_prices = [price - 0.1 * i for i in range(ORDER_BOOK_DEPTH)]
        ask_prices = [price + 0.1 * i for i in range(ORDER_BOOK_DEPTH)]
        bid_volumes = [100] * ORDER_BOOK_DEPTH
        ask_volumes = [120] * ORDER_BOOK_DEPTH

        book = nnxt.OrderBook()
        book.instrument_id = instrument
        book.bid_price = bid_prices
        book.ask_price = ask_prices
        book.bid_volume = bid_volumes
        book.ask_volume = ask_volumes
        book.last_price = price
        book.timestamp = nnxt.monotonic_now_ns()
        self.publish_order_book(book)
        self._seq += 1
        if self._seq % 10 == 0:
            nnxt.log_info("market tick event=[MARKET_TICK] seq=[{}]".format(self._seq))


def parse_args(argv: Iterable[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Python market simulator")
    parser.add_argument("--master-addr", default="ipc:///tmp/nnxt/master")
    parser.add_argument("--queue-path", default="market/market-sim/public")
    parser.add_argument("--actor-id", default="market-sim")
    parser.add_argument("--actor-type", default="market-simulator")
    parser.add_argument("--interval-ms", type=int, default=200)
    parser.add_argument("--base-price", type=float, default=100.0)
    parser.add_argument("--instrument", action="append", default=None)
    parser.add_argument("--unique-queue", action="store_true")
    args = parser.parse_args(argv)
    if args.instrument is None:
        args.instrument = ["IF2409"]
    return args


def main() -> None:
    args = parse_args()
    nnxt.setup_log()
    queue_path = args.queue_path
    if args.unique_queue:
        queue_path = f"{queue_path}-{os.getpid()}"
    nnxt.log_info(
        "market simulator create event=[SIM_CREATE] master_addr=[{}] queue_path=[{}]".format(
            args.master_addr, queue_path
        )
    )
    gateway = MarketSimulator(queue_path)
    gateway.configure(args.interval_ms, args.base_price, args.instrument)
    runner = nnxt.MarketGatewayRunner(
        gateway,
        queue_path,
        master_addr=args.master_addr,
        actor_id=args.actor_id,
        actor_type=args.actor_type,
    )
    nnxt.log_info("market simulator run event=[SIM_RUN]")
    runner.run()


if __name__ == "__main__":
    main()
