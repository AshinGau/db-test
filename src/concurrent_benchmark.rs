use crate::error::{DbError, Result};
use rand::{rngs::StdRng, Rng, SeedableRng};
use rocksdb::{DB, Options, BlockBasedOptions};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{self, Sender, Receiver};
use std::thread;
use std::time::{Duration, Instant};
use serde::Serialize;

/// 读写并发测试配置
#[derive(Debug, Clone, Serialize)]
pub struct ConcurrentBenchConfig {
    /// 随机种子
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

impl Default for ConcurrentBenchConfig {
    fn default() -> Self {
        Self {
            seed: 42,
            batch_size: 10000,
            total_batches: 100,
            key_size: 64,
            value_size: 200,
            write_threads: 2,
            read_threads: 4,
            storage_path: std::env::temp_dir()
                .join("db_concurrent_bench")
                .to_str()
                .unwrap()
                .to_string(),
        }
    }
}

impl ConcurrentBenchConfig {
    pub fn print_summary(&self) {
        println!("=== 读写并发测试配置 ===");
        println!("  随机种子: {}", self.seed);
        println!("  批次大小: {}", self.batch_size);
        println!("  批次总数: {}", self.total_batches);
        println!("  Key 大小: {} bytes", self.key_size);
        println!("  Value 大小: {} bytes", self.value_size);
        println!("  写线程数: {}", self.write_threads);
        println!("  读线程数: {}", self.read_threads);
        println!("  存储路径: {}", self.storage_path);
        println!();
    }
}

/// 批次统计信息
#[derive(Debug, Clone, Serialize)]
pub struct BatchStats {
    pub batch_index: usize,
    pub duration: Duration,
    pub operations: usize,
}

/// 读写并发测试结果
#[derive(Debug, Clone, Serialize)]
pub struct ConcurrentBenchResult {
    pub config: ConcurrentBenchConfig,
    pub total_duration: Duration,
    pub write_stats: Vec<BatchStats>,
    pub read_stats: Vec<BatchStats>,
    pub total_writes: usize,
    pub total_reads: usize,
    pub write_ops_per_sec: f64,
    pub read_ops_per_sec: f64,
}

impl ConcurrentBenchResult {
    pub fn print_summary(&self) {
        println!("\n=== 读写并发测试结果 ===");
        println!("总运行时间: {:?}", self.total_duration);
        println!("总写入操作数: {}", self.total_writes);
        println!("总读取操作数: {}", self.total_reads);
        println!("写入吞吐量: {:.2} ops/sec", self.write_ops_per_sec);
        println!("读取吞吐量: {:.2} ops/sec", self.read_ops_per_sec);
        
        // 打印写入统计
        if !self.write_stats.is_empty() {
            let avg_write_time = self.write_stats.iter()
                .map(|s| s.duration.as_micros())
                .sum::<u128>() / self.write_stats.len() as u128;
            println!("平均写批次时间: {:?}", Duration::from_micros(avg_write_time as u64));
        }
        
        // 打印读取统计
        if !self.read_stats.is_empty() {
            let avg_read_time = self.read_stats.iter()
                .map(|s| s.duration.as_micros())
                .sum::<u128>() / self.read_stats.len() as u128;
            println!("平均读批次时间: {:?}", Duration::from_micros(avg_read_time as u64));
        }
        
        println!("========================\n");
    }
    
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string_pretty(self)
            .map_err(|e| DbError::Serialization(e))
    }
}

/// 写入进度消息
#[derive(Debug, Clone)]
struct WriteProgress {
    batch_index: usize,
    keys: Vec<Vec<u8>>,
    hot_keys: Vec<Vec<u8>>,
}

/// 读写并发测试运行器
pub struct ConcurrentBenchmarkRunner {
    config: ConcurrentBenchConfig,
}

impl ConcurrentBenchmarkRunner {
    pub fn new(config: ConcurrentBenchConfig) -> Self {
        Self { config }
    }
    
    /// 运行并发测试
    pub fn run(&self) -> Result<ConcurrentBenchResult> {
        self.config.print_summary();
        
        // 确保存储目录存在
        let storage_path = Path::new(&self.config.storage_path);
        std::fs::create_dir_all(storage_path)?;
        
        // 打开数据库
        let db = Self::open_db(storage_path)?;
        let db = Arc::new(db);
        
        // 生成热点数据的 keys（20% 的 batch_size）
        let hot_keys_count = (self.config.batch_size as f64 * 0.2) as usize;
        let hot_keys = Self::generate_hot_keys(self.config.seed, self.config.key_size, hot_keys_count);
        let hot_keys = Arc::new(hot_keys);
        
        println!("生成热点数据: {} 个 keys", hot_keys_count);
        
        // 创建进度通道
        let (progress_tx, progress_rx) = mpsc::channel::<WriteProgress>();
        let progress_rx = Arc::new(Mutex::new(progress_rx));
        
        // 用于收集统计信息
        let write_stats = Arc::new(Mutex::new(Vec::new()));
        let read_stats = Arc::new(Mutex::new(Vec::new()));
        
        let start_time = Instant::now();
        
        // 启动写线程
        let mut write_handles = Vec::new();
        for thread_id in 0..self.config.write_threads {
            let db_clone = Arc::clone(&db);
            let progress_tx_clone = progress_tx.clone();
            let write_stats_clone = Arc::clone(&write_stats);
            let hot_keys_clone = Arc::clone(&hot_keys);
            let config = self.config.clone();
            
            let handle = thread::spawn(move || {
                Self::write_thread(
                    thread_id,
                    db_clone,
                    progress_tx_clone,
                    write_stats_clone,
                    hot_keys_clone,
                    config,
                )
            });
            write_handles.push(handle);
        }
        
        // 启动读线程
        let mut read_handles = Vec::new();
        for thread_id in 0..self.config.read_threads {
            let db_clone = Arc::clone(&db);
            let progress_rx_clone = Arc::clone(&progress_rx);
            let read_stats_clone = Arc::clone(&read_stats);
            let config = self.config.clone();
            
            let handle = thread::spawn(move || {
                Self::read_thread(
                    thread_id,
                    db_clone,
                    progress_rx_clone,
                    read_stats_clone,
                    config,
                )
            });
            read_handles.push(handle);
        }
        
        // 等待所有写线程完成
        println!("等待写线程完成...");
        for handle in write_handles {
            handle.join().unwrap()?;
        }
        
        // 关闭进度通道，通知读线程写入已完成
        drop(progress_tx);
        
        // 等待所有读线程完成
        println!("等待读线程完成...");
        for handle in read_handles {
            handle.join().unwrap()?;
        }
        
        let total_duration = start_time.elapsed();
        
        // 收集统计信息
        let write_stats = Arc::try_unwrap(write_stats)
            .unwrap()
            .into_inner()
            .unwrap();
        let read_stats = Arc::try_unwrap(read_stats)
            .unwrap()
            .into_inner()
            .unwrap();
        
        let total_writes: usize = write_stats.iter().map(|s| s.operations).sum();
        let total_reads: usize = read_stats.iter().map(|s| s.operations).sum();
        
        let write_ops_per_sec = total_writes as f64 / total_duration.as_secs_f64();
        let read_ops_per_sec = total_reads as f64 / total_duration.as_secs_f64();
        
        // 释放数据库，确保所有文件句柄被关闭
        drop(db);
        
        // 如果是临时目录，清理数据
        let storage_path = Path::new(&self.config.storage_path);
        let temp_dir = std::env::temp_dir();
        if storage_path.starts_with(&temp_dir) {
            if let Err(e) = std::fs::remove_dir_all(storage_path) {
                eprintln!("警告: 清理临时目录失败: {}", e);
            } else {
                println!("已清理临时目录: {:?}", storage_path);
            }
        }
        
        Ok(ConcurrentBenchResult {
            config: self.config.clone(),
            total_duration,
            write_stats,
            read_stats,
            total_writes,
            total_reads,
            write_ops_per_sec,
            read_ops_per_sec,
        })
    }
    
    /// 生成热点数据的 keys
    fn generate_hot_keys(seed: u64, key_size: usize, count: usize) -> Vec<Vec<u8>> {
        let mut rng = StdRng::seed_from_u64(seed.wrapping_add(999999)); // 使用不同的种子偏移
        let mut hot_keys = Vec::with_capacity(count);
        
        for _ in 0..count {
            let mut key = vec![0u8; key_size];
            rng.fill(&mut key[..]);
            hot_keys.push(key);
        }
        
        hot_keys
    }
    
    /// 打开 RocksDB 数据库
    fn open_db(path: &Path) -> Result<DB> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        
        // 性能优化配置
        opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
        opts.set_write_buffer_size(128 * 1024 * 1024); // 128MB
        opts.set_max_write_buffer_number(4);
        opts.set_max_background_jobs(8); // 增加后台任务数
        opts.increase_parallelism(4); // 增加并行度
        
        // Bloom 过滤器优化读性能
        let mut block_opts = BlockBasedOptions::default();
        block_opts.set_bloom_filter(10.0, true);
        block_opts.set_block_size(16 * 1024); // 16KB
        opts.set_block_based_table_factory(&block_opts);
        
        // 并发优化
        opts.set_allow_concurrent_memtable_write(true);
        opts.set_enable_write_thread_adaptive_yield(true);
        
        DB::open(&opts, path)
            .map_err(|e| DbError::Database(format!("Failed to open RocksDB: {}", e)))
    }
    
    /// 写线程：根据随机种子生成数据并写入
    fn write_thread(
        thread_id: usize,
        db: Arc<DB>,
        progress_tx: Sender<WriteProgress>,
        write_stats: Arc<Mutex<Vec<BatchStats>>>,
        hot_keys: Arc<Vec<Vec<u8>>>,
        config: ConcurrentBenchConfig,
    ) -> Result<()> {
        println!("写线程 {} 启动", thread_id);
        
        // 每个写线程处理总批次的一部分
        let batches_per_thread = (config.total_batches + config.write_threads - 1) / config.write_threads;
        let start_batch = thread_id * batches_per_thread;
        let end_batch = ((thread_id + 1) * batches_per_thread).min(config.total_batches);
        
        for batch_idx in start_batch..end_batch {
            // 基于批次索引生成种子，保证可重现性
            let batch_seed = config.seed.wrapping_add(batch_idx as u64);
            let mut rng = StdRng::seed_from_u64(batch_seed);
            
            let mut keys = Vec::with_capacity(config.batch_size);
            let batch_start = Instant::now();
            
            // 生成并写入数据
            for _ in 0..config.batch_size {
                let mut key = vec![0u8; config.key_size];
                let mut value = vec![0u8; config.value_size];
                
                rng.fill(&mut key[..]);
                rng.fill(&mut value[..]);
                
                // 使用 db.put 直接写入
                db.put(&key, &value)
                    .map_err(|e| DbError::Database(format!("Failed to put: {}", e)))?;
                
                keys.push(key);
            }
            
            // 更新热点数据（使用随机值）
            for hot_key in hot_keys.iter() {
                let mut hot_value = vec![0u8; config.value_size];
                rng.fill(&mut hot_value[..]);
                
                db.put(hot_key, &hot_value)
                    .map_err(|e| DbError::Database(format!("Failed to put hot key: {}", e)))?;
            }
            
            // 强制 flush 确保数据写入磁盘
            db.flush()
                .map_err(|e| DbError::Database(format!("Failed to flush: {}", e)))?;
            
            let duration = batch_start.elapsed();
            
            // 记录统计信息
            {
                let mut stats = write_stats.lock().unwrap();
                stats.push(BatchStats {
                    batch_index: batch_idx,
                    duration,
                    operations: config.batch_size,
                });
            }
            
            println!(
                "写线程 {} - 批次 {}: {:?}, {} 条记录, {:.2} ops/sec",
                thread_id,
                batch_idx,
                duration,
                config.batch_size,
                config.batch_size as f64 / duration.as_secs_f64()
            );
            
            // 发送进度给读线程
            if progress_tx.send(WriteProgress {
                batch_index: batch_idx,
                keys,
                hot_keys: hot_keys.to_vec(),
            }).is_err() {
                // 读线程可能已经结束
                break;
            }
        }
        
        println!("写线程 {} 完成", thread_id);
        Ok(())
    }
    
    /// 读线程：根据进度生成 key 并使用 Iterator 读取
    fn read_thread(
        thread_id: usize,
        db: Arc<DB>,
        progress_rx: Arc<Mutex<Receiver<WriteProgress>>>,
        read_stats: Arc<Mutex<Vec<BatchStats>>>,
        config: ConcurrentBenchConfig,
    ) -> Result<()> {
        println!("读线程 {} 启动", thread_id);
        
        let mut cached_keys: Option<Vec<Vec<u8>>> = None;
        let mut cached_hot_keys: Option<Vec<Vec<u8>>> = None;
        let mut batch_counter = 0;
        
        loop {
            // 尝试接收新的写入进度
            let progress = {
                let rx = progress_rx.lock().unwrap();
                match rx.try_recv() {
                    Ok(progress) => Some(progress),
                    Err(mpsc::TryRecvError::Empty) => None,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        // 写线程已完成，执行最后一轮读取后退出
                        if cached_keys.is_some() && cached_hot_keys.is_some() {
                            Self::read_batch_with_iterator(
                                thread_id,
                                &db,
                                cached_keys.as_ref().unwrap(),
                                cached_hot_keys.as_ref().unwrap(),
                                batch_counter,
                                &read_stats,
                                &config,
                            )?;
                        }
                        break;
                    }
                }
            };
            
            if let Some(progress) = progress {
                // 收到新进度，读取新批次
                cached_keys = Some(progress.keys.clone());
                cached_hot_keys = Some(progress.hot_keys.clone());
                Self::read_batch_with_iterator(
                    thread_id,
                    &db,
                    &progress.keys,
                    &progress.hot_keys,
                    progress.batch_index,
                    &read_stats,
                    &config,
                )?;
                batch_counter += 1;
            } else if let Some(ref keys) = cached_keys {
                if let Some(ref hot_keys) = cached_hot_keys {
                    // 没有新进度，循环读取上一批次
                    Self::read_batch_with_iterator(
                        thread_id,
                        &db,
                        keys,
                        hot_keys,
                        batch_counter,
                        &read_stats,
                        &config,
                    )?;
                    batch_counter += 1;
                    
                    // 短暂休眠，避免过度消耗CPU
                    thread::sleep(Duration::from_micros(100));
                }
            } else {
                // 还没有收到任何数据，等待
                thread::sleep(Duration::from_millis(1));
            }
        }
        
        println!("读线程 {} 完成，共读取 {} 个批次", thread_id, batch_counter);
        Ok(())
    }
    
    /// 使用 Iterator 读取一批数据
    fn read_batch_with_iterator(
        thread_id: usize,
        db: &Arc<DB>,
        keys: &[Vec<u8>],
        hot_keys: &[Vec<u8>],
        batch_index: usize,
        read_stats: &Arc<Mutex<Vec<BatchStats>>>,
        _config: &ConcurrentBenchConfig,
    ) -> Result<()> {
        let batch_start = Instant::now();
        let mut found_count = 0;
        
        // 为每个批次创建新的 Raw Iterator
        let mut iter = db.raw_iterator();

        let cnt = keys.len() / 2;
        if let Some(first_key) = keys.first() {
            iter.seek(first_key);
            for _ in 0..cnt {
                if iter.valid() {
                    // 读取当前位置的数据
                    let _key = iter.key();
                    let _value = iter.value();
                    // 移动到下一条记录
                    iter.next();
                } else {
                    break;
                }
            }
        }
        
        // 使用 seek() 查找每个 key
        let mut i = 0;
        for key in keys {
            if i > cnt {
                break;
            }
            i += 1;
            iter.seek(key);
            
            // 检查是否找到了精确匹配的 key
            if iter.valid() {
                if let Some(found_key) = iter.key() {
                    if found_key == key.as_slice() {
                        // 可以获取 value: iter.value()
                        found_count += 1;
                    }
                }
            }
        }
        
        // 读取热点数据
        for hot_key in hot_keys {
            iter.seek(hot_key);
            
            if iter.valid() {
                if let Some(found_key) = iter.key() {
                    if found_key == hot_key.as_slice() {
                        let _value = iter.value();
                        found_count += 1;
                    }
                }
            }
        }
        
        let duration = batch_start.elapsed();
        
        // 记录统计信息
        {
            let mut stats = read_stats.lock().unwrap();
            stats.push(BatchStats {
                batch_index,
                duration,
                operations: keys.len(),
            });
        }
        
        if batch_index % 10 == 0 {
            println!(
                "读线程 {} - 批次 {}: {:?}, 查找 {} 条记录, 找到 {} 条, {:.2} ops/sec",
                thread_id,
                batch_index,
                duration,
                keys.len(),
                found_count,
                keys.len() as f64 / duration.as_secs_f64()
            );
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_concurrent_benchmark() {
        let config = ConcurrentBenchConfig {
            seed: 42,
            batch_size: 100,
            total_batches: 10,
            key_size: 32,
            value_size: 64,
            write_threads: 2,
            read_threads: 2,
            storage_path: std::env::temp_dir()
                .join("test_concurrent_bench")
                .to_str()
                .unwrap()
                .to_string(),
        };
        
        let runner = ConcurrentBenchmarkRunner::new(config);
        let result = runner.run().unwrap();
        
        result.print_summary();
        assert!(result.total_writes > 0);
        assert!(result.total_reads > 0);
    }
}

