# Database Benchmark Tool

A comprehensive database benchmarking tool that supports multiple database backends (RocksDB, Sled, MDBX), providing a unified interface to test and compare the read/write performance of different database engines.

## Features

- **Multi-Backend Support**: RocksDB, Sled, and MDBX (LMDB-compatible) databases
- **Flexible Configuration**: Configurable batch sizes, key/value sizes, test modes, and more
- **Performance Metrics**: Detailed performance statistics and timing information
- **Result Export**: JSON format result export for analysis
- **Extensible Design**: Easy to add new database backends
- **Concurrent Testing**: Support for parallel table operations
- **Memory-Mapped Files**: Optimized for large datasets with efficient memory usage

## Installation and Compilation

```bash
# Clone or download the project
cd db-test

# Compile the project
cargo build --release

# Or run directly
cargo run --release
```

## Usage

### Basic Usage

```bash
# Run RocksDB write test with default configuration
cargo run --release

# Run write test with Sled backend
cargo run --release -- --backend sled

# Run read test
cargo run --release -- --test-mode read

# Run both write and read tests
cargo run --release -- --test-mode both

# Run MDBX write test
cargo run --release -- --backend mdbx --test-mode write
```

### Command Line Arguments

```bash
cargo run --release -- --help
```

Main parameters:

- `--backend {rocksdb|sled|mdbx}`: Select database backend (default: rocksdb)
- `--test-mode {write|read|both}`: Test mode (default: write)
- `--batch-size SIZE`: Number of entries per batch (default: 50000)
- `--key-size SIZE`: Key size in bytes (default: 64)
- `--value-size SIZE`: Value size in bytes (default: 200)
- `--target-batches COUNT`: Target number of batches (default: 100)
- `--storage-dir PATH`: Storage directory (default: temporary directory)
- `--sort-keys`: Sort keys before writing
- `--parallel-tables COUNT`: Number of parallel tables (default: 5)
- `--seed SEED`: Random seed (default: 42)

### Examples

```bash
# Test RocksDB small batch write performance
cargo run --release -- \
    --backend rocksdb \
    --test-mode write \
    --batch-size 10000 \
    --target-batches 50

# Test Sled read performance
cargo run --release -- \
    --backend sled \
    --test-mode read \
    --batch-size 20000 \
    --storage-dir ./sled_test_db

# Test MDBX with large dataset
cargo run --release -- \
    --backend mdbx \
    --test-mode write \
    --batch-size 50000 \
    --target-batches 1500 \
    --storage-dir ./mdbx_test_db

# Compare different backends
cargo run --release -- --backend rocksdb --test-mode both --target-batches 20
cargo run --release -- --backend sled --test-mode both --target-batches 20
cargo run --release -- --backend mdbx --test-mode both --target-batches 20
```

## Output Description

The program outputs the following information:

1. **Configuration Summary**: Shows current test configuration
2. **Batch Progress**: Execution time and statistics for each batch
3. **Result Summary**: Complete performance statistics
4. **JSON Files**: Results saved in JSON format for analysis

### Example Output

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
  Parallel Tables: 5

Starting write benchmark with RocksDB backend...
Write Batch 0: 234.5ms, DB entries: 250000, per entry: 4.69µs, total writes: 50000
Write Batch 1: 267.8ms, DB entries: 500000, per entry: 5.36µs, total writes: 50000
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

## Architecture Design

### Core Components

1. **Database Trait**: Defines unified database interface
2. **BenchmarkRunner**: Core logic for executing benchmarks
3. **Config**: Configuration management and command-line argument parsing
4. **Results**: Result statistics and export functionality

### Database Backends

#### RocksDB
- **Type**: LSM-Tree based key-value store
- **Features**: Column families, compression, bloom filters
- **Use Case**: High write throughput, range queries
- **Performance**: ~200,000+ ops/sec write, ~500,000+ ops/sec read

#### Sled
- **Type**: B+ tree based embedded database
- **Features**: ACID transactions, concurrent access
- **Use Case**: Embedded applications, high concurrency
- **Performance**: ~300,000+ ops/sec write, ~800,000+ ops/sec read

#### MDBX (LMDB-compatible)
- **Type**: Memory-mapped B+ tree database
- **Features**: Memory-mapped files, copy-on-write
- **Use Case**: High read performance, memory efficiency
- **Performance**: ~200,000+ ops/sec write, ~500,000+ ops/sec read
- **Note**: Optimized map_size to prevent sparse files

### Adding New Backends

To add a new database backend:

1. Implement the `Database` trait
2. Add new backend branch in `create_database` function
3. Update `DatabaseBackend` enum

Example:

```rust
impl Database for MyDatabaseImpl {
    fn open(path: &Path) -> Result<Self> { /* ... */ }
    fn write_batch(&mut self, data: Vec<(Vec<u8>, Vec<u8>)>, table_count: usize) -> Result<Duration> { /* ... */ }
    fn read_batch(&mut self, keys: Vec<Vec<u8>>, table_count: usize) -> Result<(Duration, Vec<Vec<Option<Vec<u8>>>>)> { /* ... */ }
    fn count_entries(&self) -> Result<u64> { /* ... */ }
    fn close(self) -> Result<()> { /* ... */ }
    fn get_info(&self) -> String { /* ... */ }
}
```

## Performance Recommendations

- For large database tests, use SSD storage
- Adjust batch size to balance memory usage and performance
- Use `--sort-keys` to improve write performance for certain databases
- Set appropriate random seed for reproducible results
- For MDBX, the tool automatically configures optimal map_size to prevent sparse files

## Benchmark Comparison

Use the provided comparison script to test all backends:

```bash
# Run comprehensive comparison
./examples/benchmark_comparison.sh
```

This script will:
1. Test RocksDB write performance
2. Test Sled write performance  
3. Test MDBX write performance
4. Test RocksDB read performance
5. Test Sled read performance
6. Test MDBX read performance

## Troubleshooting

1. **Compilation Errors**: Ensure Rust toolchain and dependencies are installed
2. **Permission Errors**: Ensure write permissions for storage directory
3. **Memory Issues**: Reduce batch size or target batches
4. **Disk Space**: Ensure sufficient disk space for test data
5. **MDBX Map Size**: If you get "MDB_MAP_FULL" error, the tool automatically handles this with appropriate map_size configuration

## File Structure

```
db-test/
├── src/
│   ├── main.rs           # Entry point
│   ├── config.rs         # Configuration and CLI
│   ├── database.rs       # Database trait and implementations
│   └── benchmark.rs      # Benchmark execution logic
├── examples/
│   └── benchmark_comparison.sh  # Comparison script
├── Cargo.toml            # Dependencies
└── README.md             # This file
```

## Dependencies

- **RocksDB**: High-performance LSM-tree database
- **Sled**: Embedded B+ tree database
- **LMDB**: Memory-mapped database (MDBX-compatible)
- **Clap**: Command-line argument parsing
- **Serde**: Serialization framework
- **Rayon**: Parallel processing
- **Anyhow/Thiserror**: Error handling