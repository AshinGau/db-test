#!/bin/bash

# 数据库基准测试比较脚本
# 这个脚本展示了如何使用 db_bench 工具比较不同数据库的性能

echo "=== 数据库基准测试比较 ==="
echo

# 设置测试参数
BATCH_SIZE=10000
TARGET_BATCHES=20
STORAGE_DIR="./benchmark_data"

# 创建存储目录
mkdir -p "$STORAGE_DIR"

echo "测试参数:"
echo "  批次大小: $BATCH_SIZE"
echo "  目标批次数: $TARGET_BATCHES"
echo "  存储目录: $STORAGE_DIR"
echo

# 测试 RocksDB 写入性能
echo "1. 测试 RocksDB 写入性能..."
cargo run --release -- \
    --backend rocksdb \
    --test-mode write \
    --batch-size "$BATCH_SIZE" \
    --target-batches "$TARGET_BATCHES" \
    --storage-dir "$STORAGE_DIR/rocksdb"

echo

# 测试 Sled 写入性能
echo "2. 测试 Sled 写入性能..."
cargo run --release -- \
    --backend sled \
    --test-mode write \
    --batch-size "$BATCH_SIZE" \
    --target-batches "$TARGET_BATCHES" \
    --storage-dir "$STORAGE_DIR/sled"

echo

# 测试 RocksDB 读取性能
echo "3. 测试 RocksDB 读取性能..."
cargo run --release -- \
    --backend rocksdb \
    --test-mode read \
    --batch-size "$BATCH_SIZE" \
    --target-batches "$TARGET_BATCHES" \
    --storage-dir "$STORAGE_DIR/rocksdb"

echo

# 测试 Sled 读取性能
echo "4. 测试 Sled 读取性能..."
cargo run --release -- \
    --backend sled \
    --test-mode read \
    --batch-size "$BATCH_SIZE" \
    --target-batches "$TARGET_BATCHES" \
    --storage-dir "$STORAGE_DIR/sled"

echo
echo "=== 测试完成 ==="
echo "结果文件已保存到当前目录的 JSON 文件中"
echo "可以使用以下命令查看结果:"
echo "  ls -la benchmark_results_*.json"

