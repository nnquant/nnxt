# 批量数据处理

本文档介绍如何高效处理大量市场数据。

## 场景描述

在量化交易中，经常需要处理大量历史数据进行分析或回测。

## 批量读取行情

```python
from nnxt import OrderBook, InstrumentId

def process_batch(books: list[OrderBook]) -> list[float]:
    """批量计算中间价"""
    results = []
    for book in books:
        mid = (book.bid_price[0] + book.ask_price[0]) / 2
        results.append(mid)
    return results
```

## 使用生成器节省内存

```python
def stream_orderbooks(file_path: str):
    """流式读取订单簿数据"""
    with open(file_path, 'rb') as f:
        while chunk := f.read(1024):
            yield parse_orderbook(chunk)

# 使用生成器处理
for book in stream_orderbooks("data.bin"):
    process_single(book)
```

## 并行处理

```python
from concurrent.futures import ProcessPoolExecutor

def parallel_process(data_files: list[str]):
    """并行处理多个数据文件"""
    with ProcessPoolExecutor(max_workers=4) as executor:
        results = list(executor.map(process_file, data_files))
    return results
```
