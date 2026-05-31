#!/usr/bin/env bash
# Copyright 2026 秋毫mem Contributors
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

# 秋毫mem 本地交叉编译脚本
# 在 Linux 上交叉编译 macOS 和 Linux 二进制
set -euo pipefail

echo "=== 秋毫mem 交叉编译 ==="

# 安装 target（如果还没有）
echo ">>> 安装 Rust targets..."
rustup target add aarch64-apple-darwin x86_64-apple-darwin x86_64-unknown-linux-musl 2>/dev/null || true

# Linux musl（静态链接）
echo ""
echo ">>> 🐧 Linux musl (static)"
cargo build --release --target x86_64-unknown-linux-musl
BIN="target/x86_64-unknown-linux-musl/release/hawk-eye-mem"
ls -lh "$BIN"
echo "  文件类型: $(file "$BIN" | cut -d: -f2)"

# macOS Apple Silicon（需要 osxcross）
if command -v o64-clang &>/dev/null; then
    echo ""
    echo ">>> 🍎 macOS Apple Silicon (aarch64)"
    export CC=aarch64-apple-darwin14-clang
    export CXX=aarch64-apple-darwin14-clang++
    cargo build --release --target aarch64-apple-darwin
    BIN="target/aarch64-apple-darwin/release/hawk-eye-mem"
    ls -lh "$BIN"
    echo "  文件类型: $(file "$BIN" | cut -d: -f2)"
    
    echo ""
    echo ">>> 🍎 macOS Intel (x86_64)"
    export CC=x86_64-apple-darwin14-clang
    export CXX=x86_64-apple-darwin14-clang++
    cargo build --release --target x86_64-apple-darwin
    BIN="target/x86_64-apple-darwin/release/hawk-eye-mem"
    ls -lh "$BIN"
    echo "  文件类型: $(file "$BIN" | cut -d: -f2)"
else
    echo ""
    echo "⚠️  osxcross 未安装，跳过 macOS 交叉编译"
    echo "   安装方式: https://github.com/messense/homebrew-macos-cross-toolchains"
fi

echo ""
echo "=== ✅ 编译完成 ==="
ls -lh target/*/release/hawk-eye-mem 2>/dev/null
