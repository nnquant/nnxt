# 快速上手

本教程将带你在 10 分钟内构建第一个量化策略。

## 概述

我们将创建一个简单的策略，它会：

1. 订阅市场行情
2. 根据行情生成交易意图
3. 接收订单和成交回报

## 第一步：创建策略类

```python
from nnxt import (
    Strategy,
    StrategyContext,
    OrderBook,
    OrderEvent,
    TradeEvent,
    Intent,
    InstrumentId,
    PriceType,
)


class SimpleStrategy(Strategy):
    """简单的示例策略"""

    def __init__(self, instrument: str):
        super().__init__()
        self.instrument = InstrumentId(instrument)
        self.target_position = 0

    def on_start(self, ctx: StrategyContext):
        """策略启动时调用"""
        ctx.log_info(f"策略启动: {self.instrument.as_str()}")
        # 订阅行情
        ctx.subscribe_quote("market-gateway", self.instrument)
        # 连接交易网关
        ctx.connect_trade("trade-gateway")
```

## 第二步：处理行情事件

```python
    def on_order_book(self, book: OrderBook, ctx: StrategyContext):
        """收到行情快照时调用"""
        # 获取最优买卖价
        best_bid = book.bid_price[0]
        best_ask = book.ask_price[0]
        mid_price = (best_bid + best_ask) / 2

        ctx.log_debug(f"行情更新: bid={best_bid}, ask={best_ask}")

        # 简单策略逻辑：维持目标仓位
        if self.target_position > 0:
            ctx.submit_intent(Intent.target_position(
                self.instrument,
                self.target_position,
                PriceType.LIMIT,
                best_bid
            ))
```

## 第三步：处理订单回报

```python
    def on_order(self, event: OrderEvent, ctx: StrategyContext):
        """收到订单状态更新时调用"""
        ctx.log_info(
            f"订单更新: order_id={event.order_id}, "
            f"status={event.status}, "
            f"filled={event.filled_quantity}"
        )

    def on_trade(self, event: TradeEvent, ctx: StrategyContext):
        """收到成交回报时调用"""
        ctx.log_info(
            f"成交回报: trade_id={event.trade_id}, "
            f"price={event.price}, qty={event.quantity}"
        )
```

## 第四步：运行策略

```python
from nnxt import StrategyRunner, setup_log


def main():
    # 初始化日志
    setup_log()

    # 创建策略实例
    strategy = SimpleStrategy("IF2409")
    strategy.target_position = 10

    # 创建运行器并启动
    runner = StrategyRunner(
        strategy,
        master_addr="ipc:///tmp/nnxt-master",
        actor_id="my-strategy",
    )

    print("策略启动中...")
    runner.run()


if __name__ == "__main__":
    main()
```

## 完整示例

将上述代码保存为 `my_strategy.py`：

```python
"""完整的示例策略"""
from nnxt import (
    Strategy, StrategyContext, StrategyRunner,
    OrderBook, OrderEvent, TradeEvent,
    Intent, InstrumentId, PriceType, setup_log,
)


class SimpleStrategy(Strategy):
    def __init__(self, instrument: str):
        super().__init__()
        self.instrument = InstrumentId(instrument)
        self.target_position = 0

    def on_start(self, ctx: StrategyContext):
        ctx.log_info(f"策略启动: {self.instrument.as_str()}")
        ctx.subscribe_quote("market-gateway", self.instrument)
        ctx.connect_trade("trade-gateway")

    def on_order_book(self, book: OrderBook, ctx: StrategyContext):
        if self.target_position > 0:
            ctx.submit_intent(Intent.target_position(
                self.instrument,
                self.target_position,
                PriceType.LIMIT,
                book.bid_price[0]
            ))

    def on_order(self, event: OrderEvent, ctx: StrategyContext):
        ctx.log_info(f"订单: {event.order_id}, 状态: {event.status}")

    def on_trade(self, event: TradeEvent, ctx: StrategyContext):
        ctx.log_info(f"成交: {event.trade_id}, 价格: {event.price}")


if __name__ == "__main__":
    setup_log()
    strategy = SimpleStrategy("IF2409")
    strategy.target_position = 10
    runner = StrategyRunner(strategy, actor_id="demo")
    runner.run()
```

## 下一步

- [数据模型](../user-guide/data-model.md) - 了解 OrderBook、OrderEvent 等核心数据结构
- [策略开发](../user-guide/strategy-development.md) - 深入学习策略开发技巧
- [网关集成](../user-guide/gateway-integration.md) - 了解如何对接真实交易所
