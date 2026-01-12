# 文档规范

本文档介绍 nnxt 文档的写作标准。

## 文档结构

每个文档应包含：

1. 标题和简介
2. 核心内容
3. 代码示例
4. 相关链接

## 代码示例要求

- 所有示例必须可运行
- 使用 pytest 验证正确性
- 包含必要的 import 语句

```python
# 正确示例
from nnxt import InstrumentId

instrument = InstrumentId("IF2409")
assert instrument.as_str() == "IF2409"
```

## Markdown 规范

- 使用 ATX 风格标题（`#`）
- 代码块标注语言类型
- 表格对齐整齐
