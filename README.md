# 数据库基准测试工具

这是一个通用的数据库基准测试工具，支持多种数据库后端（RocksDB、Sled），提供统一的接口来测试不同数据库的读写性能。

## 功能特性

- **多后端支持**: 支持 RocksDB 和 Sled 数据库
- **灵活配置**: 可配置批次大小、键值大小、测试模式等
- **性能统计**: 详细的性能指标和统计信息
- **结果导出**: 支持 JSON 格式的结果导出
- **可扩展设计**: 易于添加新的数据库后端

## 安装和编译

```bash
# 克隆或下载项目
cd db-test

# 编译项目
cargo build --release

# 或者直接运行
cargo run --release
```

## 使用方法

### 基本用法

```bash
# 使用默认配置运行 RocksDB 写入测试
cargo run --release

# 使用 Sled 后端运行写入测试
cargo run --release -- --backend sled

# 运行读取测试
cargo run --release -- --test-mode read

# 运行写入和读取测试
cargo run --release -- --test-mode both
```

### 命令行参数

```bash
cargo run --release -- --help
```

主要参数：

- `--backend {rocksdb|sled}`: 选择数据库后端（默认: rocksdb）
- `--test-mode {write|read|both}`: 测试模式（默认: write）
- `--batch-size SIZE`: 每批次的数据条目数（默认: 50000）
- `--key-size SIZE`: 键的大小（字节）（默认: 64）
- `--value-size SIZE`: 值的大小（字节）（默认: 200）
- `--target-batches COUNT`: 目标批次数（默认: 100）
- `--storage-dir PATH`: 存储目录（默认: 临时目录）
- `--sort-keys`: 写入前排序键
- `--seed SEED`: 随机种子（默认: 42）

### 示例

```bash
# 测试 RocksDB 的小批量写入性能
cargo run --release -- \
    --backend rocksdb \
    --test-mode write \
    --batch-size 10000 \
    --target-batches 50

# 测试 Sled 的读取性能
cargo run --release -- \
    --backend sled \
    --test-mode read \
    --batch-size 20000 \
    --storage-dir ./sled_test_db

# 比较不同后端的性能
cargo run --release -- --backend rocksdb --test-mode both --target-batches 20
cargo run --release -- --backend sled --test-mode both --target-batches 20
```

## 输出说明

程序会输出以下信息：

1. **配置摘要**: 显示当前测试配置
2. **批次进度**: 每个批次的执行时间和统计信息
3. **结果摘要**: 完整的性能统计
4. **JSON 文件**: 结果保存为 JSON 格式文件

### 示例输出

```
Database Benchmark Configuration:
  Backend: RocksDB
  Test Mode: Write
  Batch Size: 50000
  Key Size: 64 bytes
  Value Size: 200 bytes
  Target Batches: 100
  Storage Path: "/tmp/db_bench"
  Sort Keys: false
  Random Seed: 42

Starting write benchmark with RocksDB backend...
Write Batch 0: 234.5ms, DB entries: 50000, per entry: 4.69µs
Write Batch 1: 267.8ms, DB entries: 100000, per entry: 5.36µs
...

=== Benchmark Results ===
Backend: RocksDB
Test Mode: Write
Total Batches: 100
Total Duration: 28.4s
Total Entries: 5000000
Average Batch Time: 284ms
Operations per Second: 17606.34
========================
```

## 架构设计

### 核心组件

1. **Database Trait**: 定义统一的数据库接口
2. **BenchmarkRunner**: 执行基准测试的核心逻辑
3. **Config**: 配置管理和命令行参数解析
4. **Results**: 结果统计和导出

### 扩展新后端

要添加新的数据库后端，只需：

1. 实现 `Database` trait
2. 在 `create_database` 函数中添加新的后端分支
3. 更新 `DatabaseBackend` 枚举

示例：

```rust
impl Database for MyDatabaseImpl {
    fn open(path: &Path) -> Result<Self> { /* ... */ }
    fn write_batch(&mut self, data: Vec<(Vec<u8>, Vec<u8>)>) -> Result<Duration> { /* ... */ }
    fn read_batch(&mut self, keys: Vec<Vec<u8>>) -> Result<(Duration, Vec<Option<Vec<u8>>>)> { /* ... */ }
    fn count_entries(&self) -> Result<u64> { /* ... */ }
    fn close(self) -> Result<()> { /* ... */ }
    fn get_info(&self) -> String { /* ... */ }
}
```

## 性能建议

- 对于大数据库测试，建议使用 SSD 存储
- 调整批次大小以平衡内存使用和性能
- 使用 `--sort-keys` 可以提高某些数据库的写入性能
- 设置合适的随机种子以确保结果可重现

## 故障排除

1. **编译错误**: 确保安装了 Rust 工具链和依赖
2. **权限错误**: 确保对存储目录有写入权限
3. **内存不足**: 减小批次大小或目标批次数
4. **磁盘空间**: 确保有足够的磁盘空间存储测试数据

