use crate::config::BenchConfig;
use crate::database::create_database;
use crate::error::Result;
use rand::{rngs::StdRng, Rng, SeedableRng};
use std::path::Path;
use std::time::{Duration, Instant};

/// Benchmark result for a single batch operation
#[derive(Debug, Clone, serde::Serialize)]
pub struct BatchResult {
    pub batch_index: usize,
    pub duration: Duration,
    pub entries_processed: usize,
    pub total_entries: u64,
}

/// Complete benchmark results
#[derive(Debug, Clone, serde::Serialize)]
pub struct BenchmarkResult {
    pub test_mode: String,
    pub backend: String,
    pub total_batches: usize,
    pub total_duration: Duration,
    pub total_entries: u64,
    pub average_batch_time: Duration,
    pub operations_per_second: f64,
    pub batch_results: Vec<BatchResult>,
}

/// Benchmark runner that handles the execution of database benchmarks
pub struct BenchmarkRunner {
    config: BenchConfig,
    rng: StdRng,
}

impl BenchmarkRunner {
    pub fn new(config: BenchConfig) -> Self {
        let rng = StdRng::seed_from_u64(config.seed);
        Self { config, rng }
    }
    
    /// Run the complete benchmark suite
    pub fn run(&mut self) -> Result<Vec<BenchmarkResult>> {
        let mut results = Vec::new();
        
        // Ensure storage directory exists
        let storage_path = self.config.get_storage_path();
        std::fs::create_dir_all(&storage_path)?;
        
        match self.config.test_mode {
            crate::config::TestMode::Write => {
                let result = self.run_write_benchmark(&storage_path)?;
                results.push(result);
            }
            crate::config::TestMode::Read => {
                let result = self.run_read_benchmark(&storage_path)?;
                results.push(result);
            }
            crate::config::TestMode::Both => {
                let write_result = self.run_write_benchmark(&storage_path)?;
                results.push(write_result);
                
                let read_result = self.run_read_benchmark(&storage_path)?;
                results.push(read_result);
            }
        }
        
        Ok(results)
    }
    
    /// Run write benchmark
    fn run_write_benchmark(&mut self, storage_path: &Path) -> Result<BenchmarkResult> {
        println!("Starting write benchmark with {:?} backend...", self.config.backend);
        
        let mut db = create_database(&self.config.backend, storage_path)?;
        let mut batch_results = Vec::new();
        let start_time = Instant::now();
        
        for batch_idx in 0..self.config.target_batches {
            let current_entries = db.count_entries()?;
            let data = self.generate_batch_data(current_entries as usize, self.config.batch_size);
            
            let duration = db.write_batch(data)?;
            let new_entries = db.count_entries()?;
            
            let result = BatchResult {
                batch_index: batch_idx,
                duration,
                entries_processed: self.config.batch_size,
                total_entries: new_entries,
            };
            
            batch_results.push(result.clone());
            
            println!(
                "Write Batch {}: {:?}, DB entries: {}, per entry: {:?}",
                batch_idx,
                duration,
                new_entries,
                duration / self.config.batch_size as u32
            );
        }
        
        let total_duration = start_time.elapsed();
        let total_entries = db.count_entries()?;
        let operations_per_second = (self.config.target_batches * self.config.batch_size) as f64 / total_duration.as_secs_f64();
        
        drop(db);
        
        Ok(BenchmarkResult {
            test_mode: "Write".to_string(),
            backend: format!("{:?}", self.config.backend),
            total_batches: self.config.target_batches,
            total_duration,
            total_entries,
            average_batch_time: total_duration / self.config.target_batches as u32,
            operations_per_second,
            batch_results,
        })
    }
    
    /// Run read benchmark
    fn run_read_benchmark(&mut self, storage_path: &Path) -> Result<BenchmarkResult> {
        println!("Starting read benchmark with {:?} backend...", self.config.backend);
        
        // Check if database exists and has data
        {
            let db = create_database(&self.config.backend, storage_path)?;
            let db_size = db.count_entries()?;
            drop(db);
            
            if db_size == 0 {
                println!("Warning: Database is empty. Running write benchmark first...");
                let _write_result = self.run_write_benchmark(storage_path)?;
            }
        }
        
        // Reopen database for read test
        let mut db = create_database(&self.config.backend, storage_path)?;
        let mut batch_results = Vec::new();
        let start_time = Instant::now();
        
        for batch_idx in 0..self.config.target_batches {
            let keys = self.generate_read_keys(batch_idx);
            let (duration, found_values) = db.read_batch(keys.clone())?;
            
            let found_count = found_values.iter().filter(|v| v.is_some()).count();
            let total_entries = db.count_entries()?;
            
            let result = BatchResult {
                batch_index: batch_idx,
                duration,
                entries_processed: keys.len(),
                total_entries,
            };
            
            batch_results.push(result.clone());
            
            println!(
                "Read Batch {}: {:?}, found {}/{} entries, DB entries: {}, per entry: {:?}",
                batch_idx,
                duration,
                found_count,
                keys.len(),
                total_entries,
                duration / keys.len() as u32
            );
        }
        
        let total_duration = start_time.elapsed();
        let total_entries = db.count_entries()?;
        let operations_per_second = (self.config.target_batches * self.config.batch_size) as f64 / total_duration.as_secs_f64();
        
        drop(db);
        
        Ok(BenchmarkResult {
            test_mode: "Read".to_string(),
            backend: format!("{:?}", self.config.backend),
            total_batches: self.config.target_batches,
            total_duration,
            total_entries,
            average_batch_time: total_duration / self.config.target_batches as u32,
            operations_per_second,
            batch_results,
        })
    }
    
    /// Generate batch data for writing
    fn generate_batch_data(&mut self, start_index: usize, count: usize) -> Vec<(Vec<u8>, Vec<u8>)> {
        let mut data = Vec::with_capacity(count);

        let mut value_rng = StdRng::seed_from_u64(start_index as u64);
        for _ in 0..count {
            let key = self.generate_key(self.config.key_size);
            let value = self.generate_value(&mut value_rng, self.config.value_size);
            data.push((key, value));
        }
        
        if self.config.sort_keys {
            data.sort_by(|a, b| a.0.cmp(&b.0));
        }
        
        data
    }
    
    /// Generate random key
    fn generate_key(&mut self, size: usize) -> Vec<u8> {
        let mut key = vec![0u8; size];
        self.rng.fill(&mut key[..]);
        key
    }
    
    /// Generate value
    fn generate_value(&mut self, rng: &mut StdRng, size: usize) -> Vec<u8> {
        let mut value = vec![0u8; size];
        rng.fill(&mut value[..]);
        value
    }
    
    /// Generate keys for read testing
    fn generate_read_keys(&mut self, _batch_idx: usize) -> Vec<Vec<u8>> {
        let mut keys = Vec::with_capacity(self.config.batch_size);
        
        for _ in 0..self.config.batch_size {
            let key = self.generate_key(self.config.key_size);
            keys.push(key);
        }
        
        keys
    }
}

impl BenchmarkResult {
    /// Print a summary of the benchmark results
    pub fn print_summary(&self) {
        println!("\n=== Benchmark Results ===");
        println!("Backend: {}", self.backend);
        println!("Test Mode: {}", self.test_mode);
        println!("Total Batches: {}", self.total_batches);
        println!("Total Duration: {:?}", self.total_duration);
        println!("Total Entries: {}", self.total_entries);
        println!("Average Batch Time: {:?}", self.average_batch_time);
        println!("Operations per Second: {:.2}", self.operations_per_second);
        println!("========================\n");
    }
    
    /// Export results to JSON
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string_pretty(self)
            .map_err(|e| crate::error::DbError::Serialization(e))
    }
}
