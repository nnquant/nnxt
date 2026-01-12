# 类型系统

本文档介绍 Python 与 Rust 之间的类型映射。

## 基础类型映射

| Python 类型 | Rust 类型 | 说明 |
|-------------|-----------|------|
| `int` | `u64` / `i64` | 整数 |
| `float` | `f64` | 浮点数 |
| `str` | `String` | 字符串 |
| `bool` | `bool` | 布尔值 |
| `list[T]` | `Vec<T>` | 列表 |

## 枚举类型

```python
from nnxt import Side, PriceType, OrderStatus

# Side 枚举
Side.BUY   # 买入
Side.SELL  # 卖出

# PriceType 枚举
PriceType.LIMIT         # 限价
PriceType.MARKET        # 市价
PriceType.OPPONENT_BEST # 对手价
PriceType.OWN_BEST      # 己方最优
```

## 时间戳

nnxt 统一使用纳秒时间戳：

```python
from nnxt import MonotonicClock

# 获取当前纳秒时间戳
ts = MonotonicClock.now_ns()
print(f"时间戳: {ts}")
```
