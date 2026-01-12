# 错误处理

本文档介绍异常处理的最佳实践。

## 策略异常处理

```python
from nnxt import Strategy, StrategyContext, OrderBook

class SafeStrategy(Strategy):
    def on_order_book(self, book: OrderBook, ctx: StrategyContext):
        try:
            self.process_book(book, ctx)
        except ValueError as e:
            ctx.log_error(f"数据错误: {e}")
        except Exception as e:
            ctx.log_error(f"未知错误: {e}")
```

## 网关异常处理

```python
from nnxt import TradeGateway, Action

class SafeTradeGateway(TradeGateway):
    def on_action(self, action: Action):
        try:
            self.send_order(action)
        except ConnectionError:
            self.reconnect()
```
