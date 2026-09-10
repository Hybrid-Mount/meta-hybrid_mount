#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-only
# 自动修复 Clippy 警告的脚本

set -e

ROOT_DIR=$(CDPATH='' cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$ROOT_DIR"

echo "🔧 开始修复 Clippy 警告..."
echo

# 1. 自动修复可修复的警告
echo "📝 步骤 1: 运行 clippy --fix"
cargo clippy --workspace --all-targets --fix --allow-dirty --allow-staged

echo
echo "📝 步骤 2: 格式化代码"
cargo fmt --all

echo
echo "📝 步骤 3: 检查剩余警告"
cargo clippy --workspace --all-targets -- \
    -W clippy::missing_errors_doc \
    -W clippy::doc_markdown \
    -W clippy::map_unwrap_or \
    -A clippy::module_name_repetitions \
    -A clippy::too_many_lines

echo
echo "✅ Clippy 修复完成"
echo "💡 请检查 git diff 确认修改正确"
