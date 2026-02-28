#!/bin/bash
# 1. 编译 release 版本以获得真实性能数据
cargo build --release

# 2. 针对 kaminari 源码运行
# 假设你的二进制叫 gemini-fmt
TARGET_DIR="../kaminari/kaminari-core/lib"

echo "开始格式化项目: $TARGET_DIR"
time ./target/release/gemcut $TARGET_DIR --check
