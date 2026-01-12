#!/bin/bash
# 构建 Rust API 文档并输出到 docs 目录

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
OUTPUT_DIR="$PROJECT_ROOT/docs/api/rustdoc"

echo "构建 Rust 文档..."

cd "$PROJECT_ROOT"

# 生成文档
cargo doc --no-deps --document-private-items

# 创建输出目录
mkdir -p "$OUTPUT_DIR"

# 复制文档
cp -r target/doc/* "$OUTPUT_DIR/"

echo "Rust 文档已生成: $OUTPUT_DIR"
