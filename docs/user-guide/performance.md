# 性能优化

本文档介绍 nnxt 的性能优化技巧。

## 延迟优化

### 零拷贝数据传输

nnxt 使用共享内存实现零拷贝：

```python
# 数据直接在共享内存中读写，无需序列化
def on_order_book(self, book: OrderBook, ctx: StrategyContext):
    # book 直接指向共享内存，访问延迟 < 100ns
    price = book.bid_price[0]
```

### CPU 亲和性

建议将关键组件绑定到独立 CPU 核心：

```bash
# 绑定策略进程到 CPU 2
taskset -c 2 python my_strategy.py
```

## 吞吐量优化

### 批量处理

```python
def process_batch(self, books: list[OrderBook]):
    for book in books:
        self.update_signal(book)
    # 批量提交意图
    self.flush_intents()
```

## 内存优化

### 预分配缓冲区

```python
class OptimizedStrategy(Strategy):
    def __init__(self):
        # 预分配数据缓冲区
        self.price_buffer = [0.0] * 1000
        self.volume_buffer = [0] * 1000
```
