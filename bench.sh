#!/bin/bash

TARGET_DIR="../kaminari/kaminari-core/lib"
TARGET_DIR2="../kaminari/kaminari-core/lib/**/*.rb"
ITERATIONS=5

echo "--- Benchmark: Gemini-Fmt vs RFmt ---"
echo "目标目录: $TARGET_DIR"

# 1. 测试 Gemini-Fmt (你的工具)
echo "[1/2] 运行 Gemcut (Rust)..."
# 预热一下
./target/release/gemcut $TARGET_DIR --check > /dev/null 2>&1

TIME_GEMINI=0
for i in $(seq 1 $ITERATIONS); do
    START=$(date +%s%N)
    ./target/release/gemcut $TARGET_DIR --check > /dev/null 2>&1
    END=$(date +%s%N)
    DIFF=$(( (END - START) / 1000000 ))
    TIME_GEMINI=$(( TIME_GEMINI + DIFF ))
done
AVG_GEMINI=$(( TIME_GEMINI / ITERATIONS ))

# 2. 测试 RFmt (假设它已在 PATH 中)
echo "[2/2] 运行 RFmt..."
TIME_RFMT=0
for i in $(seq 1 $ITERATIONS); do
    START=$(date +%s%N)
    rfmt check $TARGET_DIR2 > /dev/null 2>&1
    END=$(date +%s%N)
    DIFF=$(( (END - START) / 1000000 ))
    TIME_RFMT=$(( TIME_RFMT + DIFF ))
done
AVG_RFMT=$(( TIME_RFMT / ITERATIONS ))

# 3. 输出结果
echo "-------------------------------------"
echo "Gemcut     平均耗时: ${AVG_GEMINI}ms"
echo "RFmt       平均耗时: ${AVG_RFMT}ms"
echo "-------------------------------------"

if [ $AVG_GEMINI -lt $AVG_RFMT ]; then
    RATIO=$(echo "scale=2; $AVG_RFMT / $AVG_GEMINI" | bc)
    echo "结论: Gemcut 比 RFmt 快了约 ${RATIO} 倍！"
else
    echo "结论: 还需要继续优化性能。"
fi