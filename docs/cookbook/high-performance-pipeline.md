# 高性能管线

本文档介绍如何构建低延迟数据处理管线。

## 零拷贝数据流

```python
from nnxt import Strategy, OrderBook, StrategyContext

class LowLatencyStrategy(Strategy):
    def on_order_book(self, book: OrderBook, ctx: StrategyContext):
        # 直接访问共享内存，无拷贝
        price = book.bid_price[0]
        self.fast_process(price)
```

## 避免内存分配

```python
class PreallocStrategy(Strategy):
    def __init__(self):
        # 预分配缓冲区
        self.buffer = [0.0] * 100

    def on_order_book(self, book: OrderBook, ctx: StrategyContext):
        # 复用缓冲区
        for i, p in enumerate(book.bid_price[:5]):
            self.buffer[i] = p
```
