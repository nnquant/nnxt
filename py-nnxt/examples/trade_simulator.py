"""Python trade simulator for nnxt."""

from __future__ import annotations

import argparse
import os
from typing import Iterable

import nnxt

NEW_ORDER_KIND = 1
CANCEL_ORDER_KIND = 2


class TradeSimulator(nnxt.TradeGateway):
    """Simple trade simulator that echoes actions into events."""

    def __init__(self, order_event_queue: str, trade_event_queue: str) -> None:
        super().__init__()
        self._order_event_queue = order_event_queue
        self._trade_event_queue = trade_event_queue
        self._next_order_id = 1
        self._next_trade_id = 1
        self._last_order_id: int | None = None

    def on_start(self) -> None:
        nnxt.log_info("trade simulator start event=[SIM_START]")
        self.init_writers(self._order_event_queue, self._trade_event_queue)

    def on_action(self, action: nnxt.Action) -> None:
        now_ns = nnxt.monotonic_now_ns()
        if action.kind == NEW_ORDER_KIND:
            order_id = action.new_order_order_id
            if order_id == 0:
                order_id = self._next_order_id
            self._next_order_id = max(self._next_order_id, order_id + 1)
            self._last_order_id = order_id
            instrument = action.new_order_instrument

            order_event = nnxt.OrderEvent()
            order_event.instrument = instrument
            order_event.order_id = order_id
            order_event.status = nnxt.OrderStatus.ACTIVE
            order_event.filled_quantity = 0
            order_event.remaining_quantity = action.new_order_quantity
            order_event.last_price = action.new_order_limit_price
            order_event.timestamp = now_ns
            self.publish_order_event(order_event)
            nnxt.log_info(
                "trade event event=[ORDER_EVENT] order_id=[{}] status=[{}]".format(
                    order_event.order_id, order_event.status
                )
            )

            trade_event = nnxt.TradeEvent()
            trade_event.instrument = instrument
            trade_event.order_id = order_id
            trade_event.trade_id = self._next_trade_id
            self._next_trade_id += 1
            trade_event.side = action.new_order_side
            trade_event.price = action.new_order_limit_price
            trade_event.quantity = action.new_order_quantity
            trade_event.timestamp = now_ns
            self.publish_trade_event(trade_event)
            nnxt.log_info(
                "trade event event=[TRADE_EVENT] order_id=[{}] trade_id=[{}] qty=[{}] price=[{}]".format(
                    trade_event.order_id,
                    trade_event.trade_id,
                    trade_event.quantity,
                    trade_event.price,
                )
            )

            order_event = nnxt.OrderEvent()
            order_event.instrument = instrument
            order_event.order_id = order_id
            order_event.status = nnxt.OrderStatus.FILLED
            order_event.filled_quantity = action.new_order_quantity
            order_event.remaining_quantity = 0
            order_event.last_price = action.new_order_limit_price
            order_event.timestamp = now_ns
            self.publish_order_event(order_event)
            nnxt.log_info(
                "trade event event=[ORDER_EVENT] order_id=[{}] status=[{}]".format(
                    order_event.order_id, order_event.status
                )
            )
        elif action.kind == CANCEL_ORDER_KIND:
            order_id = action.cancel_order_order_id or self._last_order_id
            if order_id is None:
                nnxt.log_warn("trade cancel ignored event=[CANCEL_SKIP] reason=[no_order]")
                return
            instrument = action.cancel_order_instrument
            order_event = nnxt.OrderEvent()
            order_event.instrument = instrument
            order_event.order_id = order_id
            order_event.status = nnxt.OrderStatus.CANCELLED
            order_event.filled_quantity = 0
            order_event.remaining_quantity = 0
            order_event.last_price = 0.0
            order_event.timestamp = now_ns
            self.publish_order_event(order_event)
            nnxt.log_info(
                "trade event event=[ORDER_EVENT] order_id=[{}] status=[{}]".format(
                    order_event.order_id, order_event.status
                )
            )
        else:
            nnxt.log_warn("trade action ignored event=[ACTION_SKIP] kind=[{}]".format(action.kind))

        nnxt.log_info(
            "trade action processed event=[ACTION_OK] kind=[{}] order_id=[{}]".format(
                action.kind, self._last_order_id
            )
        )

    def on_stop(self) -> None:
        nnxt.log_info("trade simulator stop event=[SIM_STOP]")


def parse_args(argv: Iterable[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Python trade simulator")
    parser.add_argument("--master-addr", default="ipc:///tmp/nnxt/master")
    parser.add_argument("--order-event-queue", default="order-event/trade-sim/public")
    parser.add_argument("--trade-event-queue", default="trade-event/trade-sim/public")
    parser.add_argument("--actor-id", default="trade-sim")
    parser.add_argument("--actor-type", default="trade-simulator")
    parser.add_argument("--action-queue", action="append", default=[])
    parser.add_argument("--unique-queue", action="store_true")
    return parser.parse_args(argv)


def main() -> None:
    args = parse_args()
    nnxt.setup_log()
    order_event_queue = args.order_event_queue
    trade_event_queue = args.trade_event_queue
    if args.unique_queue:
        suffix = f"-{os.getpid()}"
        order_event_queue = f"{order_event_queue}{suffix}"
        trade_event_queue = f"{trade_event_queue}{suffix}"
    nnxt.log_info(
        "trade simulator create event=[SIM_CREATE] master_addr=[{}] order_event_queue=[{}] trade_event_queue=[{}]".format(
            args.master_addr, order_event_queue, trade_event_queue
        )
    )
    gateway = TradeSimulator(order_event_queue, trade_event_queue)
    runner = nnxt.TradeGatewayRunner(
        gateway,
        order_event_queue,
        trade_event_queue,
        action_queues=args.action_queue or None,
        master_addr=args.master_addr,
        actor_id=args.actor_id,
        actor_type=args.actor_type,
    )
    nnxt.log_info("trade simulator run event=[SIM_RUN]")
    runner.run()


if __name__ == "__main__":
    main()
