# 策略开发

本文档介绍如何开发高质量的量化策略。

## Strategy 基类

所有策略都需要继承 `Strategy` 基类：

```python
from nnxt import Strategy, StrategyContext

class MyStrategy(Strategy):
    def on_start(self, ctx: StrategyContext):
        """策略启动"""
        pass

    def on_stop(self, ctx: StrategyContext):
        """策略停止"""
        pass
```

## 生命周期回调

### on_start

策略启动时调用，用于初始化订阅和连接：

```python
def on_start(self, ctx: StrategyContext):
    # 订阅行情
    ctx.subscribe_quote("market-gateway", self.instrument)
    # 连接交易
    ctx.connect_trade("trade-gateway")
    # 设置定时器（每秒触发）
    ctx.set_timer(1_000_000_000)
```

### on_order_book

收到行情快照时调用：

```python
def on_order_book(self, book: OrderBook, ctx: StrategyContext):
    mid = (book.bid_price[0] + book.ask_price[0]) / 2
    ctx.log_debug(f"中间价: {mid}")
```

## Intent 意图系统

策略通过提交 Intent 表达交易意图：

```python
from nnxt import Intent, PriceType

# 目标仓位意图
intent = Intent.target_position(
    instrument,
    quantity=10,
    price_type=PriceType.LIMIT,
    limit_price=3500.0
)
ctx.submit_intent(intent)

# 撤单意图
cancel = Intent.cancel_order(instrument, order_id=123)
ctx.submit_intent(cancel)
```
