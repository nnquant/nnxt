# 发布策略

本文档介绍版本发布流程。

## 版本号规范

遵循语义化版本：`MAJOR.MINOR.PATCH`

- MAJOR: 不兼容的 API 变更
- MINOR: 向后兼容的功能新增
- PATCH: 向后兼容的问题修复

## 发布流程

1. 更新 CHANGELOG
2. 更新版本号
3. 创建 Git 标签
4. 构建并发布

```bash
# 创建标签
git tag v0.1.0
git push origin v0.1.0
```
