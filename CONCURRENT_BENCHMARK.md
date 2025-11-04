# RocksDB 读写并发测试

## 概述

这是一个用于测试 RocksDB 在读写并发场景下性能表现的工具。它实现了真实的并发读写场景，其中写线程持续写入数据，读线程同时读取已写入的数据。

## 核心特性

### 1. 并发读写设计

- **写线程**：根据共享的随机种子生成数据，使用 `db.put()` 直接写入（非批量写入）
- **读线程**：使用 RocksDB Iterator 接口扫描数据
- **进度同步**：通过 channel 在写线程和读线程之间传递写入进度
- **智能缓存**：读线程缓存上一批次的 key，在没有新数据时循环读取缓存数据

### 2. 测试场景

```
┌─────────────┐
│  写线程池   │  使用 db.put() 写入数据
│ (多线程)    │  ↓
└──────┬──────┘  完成批次后发送进度
       │
       │ Channel
       ↓
┌──────────────┐
│   读线程池   │  接收进度，使用 Iterator 读取
│  (多线程)    │  ↓
└──────────────┘  缓存上一批次，循环读取
```

### 3. 可重现性

所有数据生成都基于可配置的随机种子，确保测试结果可重现。

## 配置参数

```rust
pub struct ConcurrentBenchConfig {
    /// 随机种子 - 用于生成可重现的数据
    pub seed: u64,
    
    /// 每批次数据量
    pub batch_size: usize,
    
    /// 批次总数
    pub total_batches: usize,
    
    /// Key 大小（字节）
    pub key_size: usize,
    
    /// Value 大小（字节）
    pub value_size: usize,
    
    /// 写线程数
    pub write_threads: usize,
    
    /// 读线程数
    pub read_threads: usize,
    
    /// 存储路径
    pub storage_path: String,
}
```

## 使用方法

### 方法 1：运行示例程序

#### 小规模测试（快速验证）

```bash
cargo run --example concurrent_benchmark_small
```

这个测试使用较小的参数：
- 批次大小：1,000 条记录
- 批次总数：10
- 写线程：2
- 读线程：3

#### 完整测试

```bash
cargo run --example concurrent_benchmark
```

这个测试使用更大的参数：
- 批次大小：10,000 条记录
- 批次总数：50
- 写线程：2
- 读线程：4

### 方法 2：在代码中使用

```rust
use db_test::{ConcurrentBenchmarkRunner, ConcurrentBenchConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 配置测试参数
    let config = ConcurrentBenchConfig {
        seed: 42,
        batch_size: 10000,
        total_batches: 100,
        key_size: 64,
        value_size: 200,
        write_threads: 2,
        read_threads: 4,
        storage_path: "/tmp/rocksdb_test".to_string(),
    };
    
    // 创建运行器并执行测试
    let runner = ConcurrentBenchmarkRunner::new(config);
    let result = runner.run()?;
    
    // 打印结果
    result.print_summary();
    
    // 导出为 JSON
    let json = result.to_json()?;
    std::fs::write("result.json", json)?;
    
    Ok(())
}
```

## 测试结果解读

### 输出示例

```
=== 读写并发测试结果 ===
总运行时间: 161.336ms
总写入操作数: 10000
总读取操作数: 47000
写入吞吐量: 61982.35 ops/sec
读取吞吐量: 291317.05 ops/sec
平均写批次时间: 24.871ms
平均读批次时间: 7.111ms
========================
```

### 关键指标

1. **写入吞吐量**：每秒写入的 key-value 对数量
2. **读取吞吐量**：每秒读取的 key-value 对数量
3. **平均批次时间**：处理一个批次的平均耗时
4. **总操作数**：测试期间完成的总操作数

### 性能观察

- **读取通常比写入快**：因为读取是内存操作，而写入涉及磁盘同步
- **读线程会超过写线程**：读线程会循环读取缓存数据，所以读操作数会大于写操作数
- **并发度影响**：增加线程数可以提高吞吐量，但也会增加争用

## 实现细节

### 写线程行为

1. 根据批次索引生成确定性的随机种子
2. 生成指定数量的 key-value 对
3. 使用 `db.put()` 逐个写入（非批量）
4. 调用 `db.flush()` 确保数据持久化
5. 通过 channel 发送写入进度给读线程

### 读线程行为

1. 尝试从 channel 接收新的写入进度
2. 如果收到新进度：
   - 缓存新批次的 keys
   - 使用 Iterator 扫描并查找这些 keys
3. 如果没有新进度：
   - 使用缓存的 keys 继续读取
   - 短暂休眠避免过度消耗 CPU
4. 当 channel 关闭时（写线程完成）：
   - 执行最后一轮读取后退出

### Iterator 使用

每个读取批次都会创建一个新的 RawIterator，并使用 `seek()` 方法直接定位到目标 key：

```rust
let mut iter = db.raw_iterator();

for key in keys {
    iter.seek(key);
    
    if iter.valid() {
        if let Some(found_key) = iter.key() {
            if found_key == key.as_slice() {
                // 找到匹配的 key
                found_count += 1;
            }
        }
    }
}
```

这种方法比遍历整个数据库更高效，直接利用 RocksDB 的索引定位数据。

## RocksDB 配置优化

测试中使用了以下 RocksDB 配置以获得最佳并发性能：

```rust
// 性能优化
opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
opts.set_write_buffer_size(128 * 1024 * 1024); // 128MB
opts.set_max_write_buffer_number(4);
opts.set_max_background_jobs(8);
opts.increase_parallelism(4);

// Bloom 过滤器优化读性能
let mut block_opts = BlockBasedOptions::default();
block_opts.set_bloom_filter(10.0, true);
opts.set_block_based_table_factory(&block_opts);

// 并发优化
opts.set_allow_concurrent_memtable_write(true);
opts.set_enable_write_thread_adaptive_yield(true);
```

## 测试场景建议

### 场景 1：高并发写入 + 低延迟读取

```rust
let config = ConcurrentBenchConfig {
    write_threads: 4,
    read_threads: 8,
    batch_size: 5000,
    total_batches: 200,
    ..Default::default()
};
```

### 场景 2：均衡读写

```rust
let config = ConcurrentBenchConfig {
    write_threads: 2,
    read_threads: 2,
    batch_size: 10000,
    total_batches: 100,
    ..Default::default()
};
```

### 场景 3：大数据量持久化测试

```rust
let config = ConcurrentBenchConfig {
    write_threads: 2,
    read_threads: 4,
    batch_size: 50000,
    total_batches: 1000,
    key_size: 128,
    value_size: 1024,
    ..Default::default()
};
```

## 注意事项

1. **磁盘空间**：确保有足够的磁盘空间存储测试数据
2. **内存使用**：大批次会占用更多内存
3. **线程数**：线程数应该根据 CPU 核心数合理配置
4. **数据目录**：测试会在指定路径创建数据库
   - 如果使用临时目录（默认行为），测试完成后会**自动清理**
   - 如果使用自定义路径，需要手动清理

## 与其他测试的区别

| 特性 | 传统测试 (benchmark.rs) | 并发测试 (concurrent_benchmark.rs) |
|------|------------------------|-----------------------------------|
| 写入方式 | WriteBatch | db.put() |
| 读取方式 | db.get() | Iterator |
| 执行方式 | 顺序执行 | 并发执行 |
| 进度同步 | 不需要 | Channel |
| 数据缓存 | 不需要 | 读线程缓存 keys |

## 故障排查

### 问题：读线程读不到数据

**原因**：写线程还未写入数据

**解决**：读线程会自动等待写线程的第一批数据

### 问题：性能低于预期

**可能原因**：
- 磁盘 I/O 瓶颈
- 线程数过多导致争用
- 批次太小导致开销过高

**解决**：
- 调整 `write_buffer_size` 和 `max_background_jobs`
- 减少线程数
- 增加 `batch_size`

## 许可证

与项目保持一致

