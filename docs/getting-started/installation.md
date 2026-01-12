# 安装指南

本文档介绍如何安装 nnxt 及其依赖。

## 系统要求

| 组件 | 最低版本 | 推荐版本 |
|------|----------|----------|
| Python | 3.10 | 3.11+ |
| Rust | 1.70 | 1.75+ |
| 操作系统 | Linux / macOS | Linux (Ubuntu 22.04+) |

## 安装方式

### 使用 pip 安装（推荐）

```bash
pip install nnxt
```

### 使用 uv 安装

```bash
uv pip install nnxt
```

### 从源码安装

如果你需要修改源码或使用最新开发版本：

```bash
# 克隆仓库
git clone https://github.com/nnquant/nnxt.git
cd nnxt

# 创建虚拟环境
python -m venv .venv
source .venv/bin/activate

# 安装 maturin（用于构建 Rust 扩展）
pip install maturin

# 开发模式安装
cd py-nnxt
maturin develop --release
```

## 验证安装

安装完成后，运行以下代码验证：

```python
import nnxt

# 检查版本
print(f"nnxt 已安装")

# 测试基础功能
from nnxt import InstrumentId, OrderBook, MonotonicClock

# 创建合约标识
instrument = InstrumentId("IF2409")
print(f"合约: {instrument.as_str()}")

# 获取时间戳
ts = MonotonicClock.now_ns()
print(f"时间戳: {ts} ns")
```

预期输出：

```
nnxt 已安装
合约: IF2409
时间戳: 1234567890123456789 ns
```

## 平台说明

### Linux

Linux 是 nnxt 的主要支持平台，所有功能均经过完整测试。

```bash
# Ubuntu/Debian 依赖
sudo apt-get install build-essential pkg-config
```

### macOS

macOS 支持开发和测试，但生产环境建议使用 Linux。

```bash
# 安装 Xcode 命令行工具
xcode-select --install
```

### Windows

!!! warning "Windows 支持"
    Windows 目前处于实验性支持阶段，建议使用 WSL2。

## 常见问题

### 编译错误：找不到 Rust 编译器

确保已安装 Rust：

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

### ImportError: 无法加载动态库

这通常是因为 Rust 扩展未正确编译。尝试重新安装：

```bash
pip uninstall nnxt
pip install nnxt --no-cache-dir
```
