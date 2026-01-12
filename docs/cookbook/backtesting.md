# 回测系统

本文档介绍如何构建策略回测框架。

## 基础回测框架

```python
from nnxt import OrderBook, InstrumentId

class Backtester:
    def __init__(self, strategy):
        self.strategy = strategy
        self.pnl = 0.0

    def run(self, data: list[OrderBook]):
        for book in data:
            self.strategy.on_order_book(book)
```

## 模拟成交

```python
class SimulatedExchange:
    def __init__(self):
        self.orders = {}

    def match(self, book: OrderBook):
        for oid, order in self.orders.items():
            if self.can_fill(order, book):
                self.fill_order(oid)
```
