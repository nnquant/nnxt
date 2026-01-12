# 开发流程

本文档介绍本地开发环境搭建和工作流程。

## 环境准备

```bash
# 克隆仓库
git clone https://github.com/nnquant/nnxt.git
cd nnxt

# 创建虚拟环境
python -m venv .venv
source .venv/bin/activate

# 安装依赖
pip install -e ".[dev]"
```

## 本地预览文档

```bash
# 安装文档依赖
pip install mkdocs-material mkdocstrings[python]

# 启动预览服务
mkdocs serve

# 访问 http://127.0.0.1:8000
```

## 运行测试

```bash
# Python 测试
pytest

# Rust 测试
cargo test
```
