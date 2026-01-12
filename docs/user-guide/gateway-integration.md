# 网关集成

本文档介绍如何开发和集成市场数据网关与交易网关。

## 市场数据网关

### MarketGateway 基类

```python
from nnxt import MarketGateway, OrderBook, InstrumentId

class MyMarketGateway(MarketGateway):
    def on_start(self):
        """网关启动"""
        print("市场网关启动")

    def on_subscribe(self, instrument: InstrumentId):
        """处理订阅请求"""
        print(f"订阅: {instrument.as_str()}")

    def on_stop(self):
        """网关停止"""
        print("市场网关停止")
```

### 发布行情数据

```python
def publish_market_data(self):
    book = OrderBook()
    book.instrument_id = self.instrument
    book.bid_price = [100.0, 99.9]
    book.ask_price = [100.1, 100.2]
    self.publish_order_book(book)
```

## 交易网关

### TradeGateway 基类

```python
from nnxt import TradeGateway, Action, OrderEvent

class MyTradeGateway(TradeGateway):
    def on_action(self, action: Action):
        """处理交易指令"""
        if action.kind == Action.NEW_ORDER:
            self.handle_new_order(action)
```
