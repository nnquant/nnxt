"""Minimal Python strategy example for nnxt.

Run after starting master/market-sim/trade-sim.
"""

import sys

import nnxt


class DemoStrategy(nnxt.Strategy):
    """Simple strategy that targets position every tick."""

    def __init__(self, instrument: nnxt.InstrumentId):
        super().__init__()
        self.instrument = instrument

    def on_start(self, ctx: nnxt.StrategyContext) -> None:
        nnxt.log_info(
            f"strategy start event=[STRATEGY_START] instrument=[{self.instrument.as_str()}]"
        )
        ctx.subscribe_quote("market-simulator", self.instrument)
        ctx.connect_trade("trade-simulator")  # 暂时注释，需要启动 trade-sim
        nnxt.log_info("strategy ready event=[STRATEGY_READY]")

    def on_order_book(self, book: nnxt.OrderBook, ctx: nnxt.StrategyContext) -> None:
        now_ns = nnxt.monotonic_now_ns()
        latency_ns = now_ns - book.timestamp
        nnxt.log_info(
            "market latency event=[MARKET_LATENCY] latency_ns=[{}] bid=[{}] ask=[{}]".format(
                latency_ns, book.bid_price[0], book.ask_price[0]
            )
        )
        ctx.submit_intent(
            nnxt.Intent.target_position(
                self.instrument, 1, nnxt.PriceType.LIMIT, 10.0
            )
        )


def main() -> None:
    nnxt.setup_log()
    master_addr = sys.argv[1] if len(sys.argv) > 1 else None
    nnxt.log_info("creating strategy event=[STRATEGY_CREATE]")
    instrument = nnxt.InstrumentId("IF2409")
    strategy = DemoStrategy(instrument)
    nnxt.log_info(f"creating runner event=[RUNNER_CREATE] master_addr=[{master_addr}]")
    runner = nnxt.StrategyRunner(strategy, master_addr=master_addr)
    nnxt.log_info("starting runner event=[RUNNER_START]")
    runner.run()
    nnxt.log_info("runner finished event=[RUNNER_STOP]")


if __name__ == "__main__":
    main()
