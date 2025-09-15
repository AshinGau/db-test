use clap::Parser;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Test mode for benchmarking
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, clap::ValueEnum)]
pub enum TestMode {
    Write,
    Read,
    Both,
}

/// Database backend selection
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, clap::ValueEnum)]
pub enum DatabaseBackend {
    #[value(name = "rocksdb")]
    RocksDB,
    #[value(name = "sled")]
    Sled,
}

/// Configuration parameters for database benchmarking
#[derive(Debug, Clone, Serialize, Deserialize, Parser)]
#[command(author, version, about = "Database Benchmark Tool")]
pub struct BenchConfig {
    /// Number of data entries per batch
    #[arg(long, default_value = "10000")]
    pub batch_size: usize,
    
    /// Size of key in bytes
    #[arg(long, default_value = "64")]
    pub key_size: usize,
    
    /// Size of value in bytes
    #[arg(long, default_value = "200")]
    pub value_size: usize,
    
    /// Target number of batches to process
    #[arg(long, default_value = "100")]
    pub target_batches: usize,
    
    /// Storage directory (default: use temporary directory)
    #[arg(long)]
    pub storage_dir: Option<PathBuf>,
    
    /// Whether to sort keys before writing
    #[arg(long)]
    pub sort_keys: bool,
    
    /// Test mode: write, read, or both
    #[arg(long, default_value = "write")]
    pub test_mode: TestMode,
    
    /// Database backend to use
    #[arg(long, default_value = "rocksdb")]
    pub backend: DatabaseBackend,
    
    /// Random seed for reproducible tests
    #[arg(long, default_value = "42")]
    pub seed: u64,
    
    /// Number of parallel tables/column families to use
    #[arg(long, default_value = "5")]
    pub parallel_tables: usize,
}

impl Default for BenchConfig {
    fn default() -> Self {
        Self {
            batch_size: 10000,
            key_size: 64,
            value_size: 200,
            target_batches: 100,
            storage_dir: None,
            sort_keys: false,
            test_mode: TestMode::Write,
            backend: DatabaseBackend::RocksDB,
            seed: 42,
            parallel_tables: 5,
        }
    }
}

impl BenchConfig {
    /// Get the storage path for the database
    pub fn get_storage_path(&self) -> PathBuf {
        self.storage_dir
            .clone()
            .unwrap_or_else(|| std::env::temp_dir().join("db_bench"))
    }
    
    /// Print configuration summary
    pub fn print_summary(&self) {
        println!("Database Benchmark Configuration:");
        println!("  Backend: {:?}", self.backend);
        println!("  Test Mode: {:?}", self.test_mode);
        println!("  Batch Size: {}", self.batch_size);
        println!("  Key Size: {} bytes", self.key_size);
        println!("  Value Size: {} bytes", self.value_size);
        println!("  Target Batches: {}", self.target_batches);
        println!("  Storage Path: {:?}", self.get_storage_path());
        println!("  Sort Keys: {}", self.sort_keys);
        println!("  Random Seed: {}", self.seed);
        println!("  Parallel Tables: {}", self.parallel_tables);
        println!();
    }
}
