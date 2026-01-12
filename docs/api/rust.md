# Rust API 文档

Rust API 文档由 `cargo doc` 生成。

## 在线文档

Rust 文档发布在：[docs.rs/nnxt](https://docs.rs/nnxt)

## 本地生成

```bash
# 生成 Rust 文档
./scripts/build_rustdoc.sh

# 文档输出位置
# docs/api/rustdoc/index.html
```

## Crate 列表

| Crate | 说明 |
|-------|------|
| `nnxt-actors` | Actor 抽象层 |
| `nnxt-gateway` | 网关实现 |
| `nnxt-master` | 控制平面 |
| `nnxt-rapid` | 高性能队列 |
| `nnxt-specs` | 数据规范 |
| `nnxt-strategy` | 策略引擎 |
| `nnxt-utils` | 工具函数 |

## 核心模块

### nnxt-rapid

高性能共享内存队列：

- `Address` - 队列地址
- `Writer<T>` - 写入器
- `Reader<T>` - 读取器

### nnxt-specs

数据结构定义：

- `OrderBook` - 订单簿
- `OrderEvent` - 订单事件
- `TradeEvent` - 成交事件
