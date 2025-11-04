use db_test::{ConcurrentBenchmarkRunner, ConcurrentBenchConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== RocksDB 读写并发测试 ===\n");
    
    // 配置并发测试参数
    let config = ConcurrentBenchConfig {
        seed: 42,                    // 随机种子
        batch_size: 50000,          // 每批次数据量
        total_batches: 2000,          // 总批次数
        key_size: 64,               // Key 大小
        value_size: 200,            // Value 大小
        write_threads: 1,           // 写线程数
        read_threads: 4,            // 读线程数
        storage_path: "/Users/gx/data/db_bench/rocksdb/concurrent".to_string(),
    };
    
    // 创建测试运行器
    let runner = ConcurrentBenchmarkRunner::new(config);
    
    // 运行并发测试
    println!("开始运行并发测试...\n");
    let result = runner.run()?;
    
    // 打印测试结果
    result.print_summary();
    
    // 保存 JSON 结果
    if let Ok(json) = result.to_json() {
        let output_path = "concurrent_benchmark_result.json";
        std::fs::write(output_path, json)?;
        println!("测试结果已保存到: {}", output_path);
    }
    
    println!("\n测试完成！");
    
    Ok(())
}

