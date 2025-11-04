# 并发测试快速开始指南

## 1分钟快速测试

最快速的方式验证功能：

```bash
cargo run --example concurrent_benchmark_small
```

这将运行一个小规模测试（10批次，每批1000条记录），大约耗时几秒钟。

## 5分钟完整测试

运行一个更完整的测试：

```bash
cargo run --release --example concurrent_benchmark
```

这将运行一个中等规模测试（50批次，每批10000条记录），大约耗时1-2分钟。

## 测试多种场景

运行预设的5种不同场景：

```bash
cargo run --release --example concurrent_scenarios
```

这将依次测试以下场景：
1. 高并发写入 + 低延迟读取
2. 均衡读写
3. 大 Value 场景
4. 小批次高频次
5. 读密集型

## 自定义测试

创建你自己的测试配置：

```rust
use db_test::{ConcurrentBenchmarkRunner, ConcurrentBenchConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = ConcurrentBenchConfig {
        seed: 42,                // 随机种子
        batch_size: 10000,      // 每批数据量
        total_batches: 100,     // 总批次数
        key_size: 64,           // Key 大小（字节）
        value_size: 200,        // Value 大小（字节）
        write_threads: 2,       // 写线程数
        read_threads: 4,        // 读线程数
        storage_path: "/tmp/my_test".to_string(),
    };
    
    let runner = ConcurrentBenchmarkRunner::new(config);
    let result = runner.run()?;
    result.print_summary();
    
    Ok(())
}
```

## 理解输出

### 运行时输出

```
写线程 0 - 批次 0: 19.01ms, 1000 条记录, 52611.04 ops/sec
读线程 0 - 批次 0: 2.37ms, 查找 1000 条记录, 找到 1000 条, 421111.68 ops/sec
```

- **写线程输出**：显示每批次写入耗时和吞吐量
- **读线程输出**：显示每批次读取耗时、找到的记录数和吞吐量

### 最终结果

```
=== 读写并发测试结果 ===
总运行时间: 67.79ms
总写入操作数: 10000
总读取操作数: 55000
写入吞吐量: 147506.31 ops/sec
读取吞吐量: 811284.73 ops/sec
平均写批次时间: 12.38ms
平均读批次时间: 2.194ms
========================
已清理临时目录: "/tmp/rocksdb_concurrent_bench_small"
```

关键指标解读：

- **总写入操作数**：实际写入的 key-value 对总数
- **总读取操作数**：实际读取尝试的总数（通常大于写入数，因为读线程会循环读取）
- **写入吞吐量**：每秒写入的记录数
- **读取吞吐量**：每秒读取的记录数
- **平均批次时间**：处理一个批次的平均时间

## 参数调优建议

### 提高写入性能

```rust
ConcurrentBenchConfig {
    write_threads: 4,        // 增加写线程
    batch_size: 5000,        // 减小批次大小（减少争用）
    ..Default::default()
}
```

### 提高读取性能

```rust
ConcurrentBenchConfig {
    read_threads: 8,         // 增加读线程
    batch_size: 10000,       // 增大批次大小（提高缓存命中）
    ..Default::default()
}
```

### 测试大数据量

```rust
ConcurrentBenchConfig {
    batch_size: 50000,
    total_batches: 1000,
    value_size: 1024,        // 1KB value
    ..Default::default()
}
```

### 测试高并发

```rust
ConcurrentBenchConfig {
    write_threads: 8,
    read_threads: 16,
    batch_size: 1000,        // 小批次，高频次
    total_batches: 500,
    ..Default::default()
}
```

## 常见问题

### Q: 读取操作数为什么比写入多？

A: 因为读线程通常比写线程快，当没有新数据时会循环读取缓存的批次数据。这是设计行为，用于持续测试读取性能。

### Q: 如何确保测试结果可重现？

A: 使用相同的 `seed` 参数。相同的种子会生成相同的数据序列。

### Q: 线程数应该设置多少？

A: 建议：
- 写线程：1-4（写操作涉及磁盘I/O，过多线程会增加争用）
- 读线程：2-8（读操作主要在内存，可以适当增加）
- 总线程数不要超过 CPU 核心数的 2 倍

### Q: 如何导出测试结果？

A: 调用 `result.to_json()` 并保存到文件：

```rust
let json = result.to_json()?;
std::fs::write("result.json", json)?;
```

### Q: 可以测试其他数据库吗？

A: 当前实现专门针对 RocksDB 优化。如需测试其他数据库，可以修改 `concurrent_benchmark.rs` 中的 `open_db` 函数。

## 下一步

- 📖 阅读完整文档：[CONCURRENT_BENCHMARK.md](CONCURRENT_BENCHMARK.md)
- 🔧 查看源代码：`src/concurrent_benchmark.rs`
- 📊 运行多场景测试：`cargo run --example concurrent_scenarios`
- 🚀 创建自定义测试场景

## 技术支持

如有问题或建议，请查看：
- 完整文档：`CONCURRENT_BENCHMARK.md`
- 主 README：`README.md`
- 示例代码：`examples/` 目录

