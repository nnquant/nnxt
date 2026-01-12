# 调试技巧

本文档介绍问题排查方法。

## 日志系统

```python
from nnxt import setup_log, log_info, log_debug

setup_log()
log_info("策略启动")
log_debug("调试信息")
```

## 策略内日志

```python
def on_order_book(self, book, ctx):
    ctx.log_debug(f"价格: {book.bid_price[0]}")
```

## 性能分析

```bash
# 使用 perf 分析
perf record -g python my_strategy.py
perf report
```
