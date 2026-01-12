# 数据模型

本文档详细介绍 nnxt 的核心数据结构。

## OrderBook - 订单簿快照

`OrderBook` 是市场行情的核心数据结构，包含多档买卖盘信息。

### 结构定义

| 字段 | 类型 | 说明 |
|------|------|------|
| `instrument_id` | `InstrumentId` | 合约标识 |
| `bid_price` | `list[float]` | 买盘价格（5档） |
| `ask_price` | `list[float]` | 卖盘价格（5档） |
| `bid_volume` | `list[int]` | 买盘数量（5档） |
| `ask_volume` | `list[int]` | 卖盘数量（5档） |
| `last_price` | `float` | 最新成交价 |
| `timestamp` | `int` | 时间戳（纳秒） |

### 使用示例

```python
from nnxt import OrderBook, InstrumentId

# 创建订单簿
book = OrderBook()
book.instrument_id = InstrumentId("IF2409")
book.bid_price = [3500.0, 3499.8, 3499.6, 3499.4, 3499.2]
book.ask_price = [3500.2, 3500.4, 3500.6, 3500.8, 3501.0]
book.bid_volume = [100, 200, 150, 300, 250]
book.ask_volume = [120, 180, 220, 160, 280]
book.last_price = 3500.0

# 计算买卖价差
spread = book.ask_price[0] - book.bid_price[0]
print(f"价差: {spread}")
```

### 性能说明

`OrderBook` 使用固定大小数组存储档位数据，内存布局紧凑，支持零拷贝传输。

## OrderEvent - 订单事件

`OrderEvent` 表示订单状态变化。

### 结构定义

| 字段 | 类型 | 说明 |
|------|------|------|
| `instrument` | `InstrumentId` | 合约标识 |
| `order_id` | `int` | 订单ID |
| `status` | `int` | 订单状态 |
| `filled_quantity` | `int` | 已成交数量 |
| `remaining_quantity` | `int` | 剩余数量 |
| `last_price` | `float` | 最新成交价 |
| `timestamp` | `int` | 时间戳（纳秒） |

### 订单状态枚举

```python
from nnxt import OrderStatus

# 订单状态值
OrderStatus.PENDING        # 待提交
OrderStatus.PENDING_NEW    # 待确认
OrderStatus.ACTIVE         # 活跃
OrderStatus.PENDING_CANCEL # 待撤销
OrderStatus.FILLED         # 全部成交
OrderStatus.CANCELLED      # 已撤销
OrderStatus.REJECTED       # 已拒绝
OrderStatus.PARTIAL_FILLED # 部分成交
```

## TradeEvent - 成交事件

`TradeEvent` 表示订单成交信息。

### 结构定义

| 字段 | 类型 | 说明 |
|------|------|------|
| `instrument` | `InstrumentId` | 合约标识 |
| `order_id` | `int` | 订单ID |
| `trade_id` | `int` | 成交ID |
| `side` | `int` | 买卖方向 |
| `price` | `float` | 成交价格 |
| `quantity` | `int` | 成交数量 |
| `timestamp` | `int` | 时间戳（纳秒） |

### 使用示例

```python
from nnxt import TradeEvent, Side

def on_trade(event: TradeEvent):
    direction = "买入" if event.side == Side.BUY else "卖出"
    print(f"{direction} {event.quantity}手 @ {event.price}")
```

## InstrumentId - 合约标识

`InstrumentId` 是合约的唯一标识符。

```python
from nnxt import InstrumentId

# 创建合约标识
instrument = InstrumentId("IF2409")

# 获取字符串表示
name = instrument.as_str()
print(f"合约: {name}")
```

!!! note "长度限制"
    合约标识最大支持 64 个字符。
