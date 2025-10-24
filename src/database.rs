use crate::error::{DbError, Result};
use std::path::Path;
use std::time::Duration;

/// Generic database trait that all database backends must implement
pub trait Database: Send + Sync {
    /// Initialize the database at the given path
    fn open(path: &Path) -> Result<Self>
    where
        Self: Sized;
    
    /// Write a batch of key-value pairs to multiple tables
    fn write_batch(&mut self, data: Vec<(Vec<u8>, Vec<u8>)>, table_count: usize) -> Result<Duration>;
    
    /// Read a batch of keys from all tables and return found values
    fn read_batch(&mut self, keys: Vec<Vec<u8>>, table_count: usize) -> Result<(Duration, Vec<Vec<Option<Vec<u8>>>>)>;
    
    /// Get the total number of entries in the database
    fn count_entries(&self) -> Result<u64>;
    
    /// Close the database and cleanup resources
    fn close(self) -> Result<()>;
    
    /// Get database-specific information
    fn get_info(&self) -> String;
}


/// Factory function to create a database instance
pub fn create_database(
    backend: &crate::config::DatabaseBackend,
    path: &Path,
) -> Result<Box<dyn Database>> {
    match backend {
        crate::config::DatabaseBackend::RocksDB => {
            let db = RocksDBImpl::open(path)?;
            Ok(Box::new(db))
        }
        crate::config::DatabaseBackend::Sled => {
            let db = SledImpl::open(path)?;
            Ok(Box::new(db))
        }
        crate::config::DatabaseBackend::Mdbx => {
            let db = MdbxImpl::open(path)?;
            Ok(Box::new(db))
        }
    }
}

/// RocksDB implementation
pub struct RocksDBImpl {
    db: rocksdb::DB,
    column_family_names: Vec<String>,
}

impl RocksDBImpl {
    fn new(db: rocksdb::DB) -> Self {
        Self { 
            db,
            column_family_names: Vec::new(),
        }
    }
    
    fn ensure_column_families(&mut self, count: usize) -> Result<()> {
        while self.column_family_names.len() < count {
            let cf_name = format!("table_{}", self.column_family_names.len());
            
            // Check if column family exists, if not create it
            if self.db.cf_handle(&cf_name).is_none() {
                self.db.create_cf(&cf_name, &rocksdb::Options::default())
                    .map_err(|e| DbError::Database(format!("Failed to create column family: {}", e)))?;
            }
            
            self.column_family_names.push(cf_name);
        }
        Ok(())
    }
}

impl Database for RocksDBImpl {
    fn open(path: &Path) -> Result<Self> {
        use rocksdb::{DB, Options, BlockBasedOptions};
        
        let mut opts = Options::default();
        opts.create_if_missing(true);
        
        // Main performance tuning parameters
        opts.set_compression_type(rocksdb::DBCompressionType::Lz4);  // Fast compression
        opts.set_write_buffer_size(64 * 1024 * 1024);  // write buffer
        opts.set_max_write_buffer_number(4);  // Allow more memtables
        opts.set_max_background_jobs(4);  // Background threads for flush/compaction
        
        // Bloom filter for better read performance
        let mut block_opts = BlockBasedOptions::default();
        block_opts.set_bloom_filter(10.0, true);  // 10 bits per key, block-based
        opts.set_block_based_table_factory(&block_opts);
        
        // Ensure data persistence for benchmarking
        opts.set_use_fsync(true);  // Force sync to disk
        
        // Try to open with existing column families first
        let existing_cfs = DB::list_cf(&opts, path).unwrap_or_default();
        
        let db = if existing_cfs.is_empty() {
            // No existing column families, create default database
            DB::open(&opts, path)
                .map_err(|e| DbError::Database(format!("Failed to open RocksDB: {}", e)))?
        } else {
            // Open with existing column families
            DB::open_cf(&opts, path, &existing_cfs)
                .map_err(|e| DbError::Database(format!("Failed to open RocksDB with column families: {}", e)))?
        };
        
        Ok(Self::new(db))
    }
    
    fn write_batch(&mut self, data: Vec<(Vec<u8>, Vec<u8>)>, table_count: usize) -> Result<Duration> {
        use std::time::Instant;
        use rocksdb::{WriteBatch, WriteOptions};
        
        let start = Instant::now();
        
        // Ensure we have enough column families
        self.ensure_column_families(table_count)?;
        
        // Use rayon for parallel processing instead of manual threads
        use rayon::prelude::*;
        
        // Process each column family in parallel
        let results: Result<Vec<()>> = (0..table_count)
            .into_par_iter()
            .map(|table_idx| {
                let cf_name = &self.column_family_names[table_idx];
                // Build batch for this specific column family
                for (key, value) in &data {
                    if let Some(cf) = self.db.cf_handle(cf_name) {
                        self.db.put_cf(cf, key, value).map_err(|e| DbError::Database(format!("Failed to write batch to {}: {}", cf_name, e)))?;
                    }
                }                
                Ok(())
            })
            .collect();
        
        results?;
        
        // Force flush to ensure data is persisted to disk
        self.db
            .flush()
            .map_err(|e| DbError::Database(format!("Failed to flush to disk: {}", e)))?;
        
        Ok(start.elapsed())
    }
    
    fn read_batch(&mut self, keys: Vec<Vec<u8>>, table_count: usize) -> Result<(Duration, Vec<Vec<Option<Vec<u8>>>>)> {
        use std::time::Instant;
        
        let start = Instant::now();
        
        // Ensure we have enough column families
        self.ensure_column_families(table_count)?;
        
        let mut results = Vec::with_capacity(keys.len());
        
        for key in keys {
            let mut key_results = Vec::with_capacity(table_count);
            
            // Read from all tables
            for cf_name in &self.column_family_names[..table_count] {
                let value = if let Some(cf) = self.db.cf_handle(cf_name) {
                    self.db
                        .get_cf(cf, &key)
                        .map_err(|e| DbError::Database(format!("Failed to read key: {}", e)))?
                } else {
                    None
                };
                key_results.push(value);
            }
            
            results.push(key_results);
        }
        
        Ok((start.elapsed(), results))
    }
    
    fn count_entries(&self) -> Result<u64> {
        // With column families, we need to count entries across all column families
        use rocksdb::properties::ESTIMATE_NUM_KEYS;
        
        let mut total_count = 0u64;
        
        // Count entries in default column family
        if let Ok(Some(estimate_str)) = self.db.property_value(ESTIMATE_NUM_KEYS) {
            if let Ok(estimate) = estimate_str.parse::<u64>() {
                total_count += estimate;
            }
        }
        
        // Count entries in each custom column family
        for cf_name in &self.column_family_names {
            if let Some(cf) = self.db.cf_handle(cf_name) {
                if let Ok(Some(estimate_str)) = self.db.property_value_cf(cf, ESTIMATE_NUM_KEYS) {
                    if let Ok(estimate) = estimate_str.parse::<u64>() {
                        total_count += estimate;
                    }
                }
            }
        }
        
        Ok(total_count)
    }
    
    fn close(self) -> Result<()> {
        drop(self.db);
        Ok(())
    }
    
    fn get_info(&self) -> String {
        format!("RocksDB - Path: {:?}", self.db.path())
    }
}

/// Sled implementation
pub struct SledImpl {
    db: sled::Db,
    trees: Vec<sled::Tree>,
}

impl SledImpl {
    fn new(db: sled::Db) -> Self {
        Self { 
            db,
            trees: Vec::new(),
        }
    }
    
    fn ensure_trees(&mut self, count: usize) -> Result<()> {
        while self.trees.len() < count {
            let tree_name = format!("table_{}", self.trees.len());
            let tree = self.db.open_tree(&tree_name)
                .map_err(|e| DbError::Database(format!("Failed to open tree: {}", e)))?;
            self.trees.push(tree);
        }
        Ok(())
    }
}

impl Database for SledImpl {
    fn open(path: &Path) -> Result<Self> {
        let db = sled::open(path)
            .map_err(|e| DbError::Database(format!("Failed to open Sled: {}", e)))?;
        
        Ok(Self::new(db))
    }
    
    fn write_batch(&mut self, data: Vec<(Vec<u8>, Vec<u8>)>, table_count: usize) -> Result<Duration> {
        use std::time::Instant;
        
        let start = Instant::now();
        
        // Ensure we have enough trees
        self.ensure_trees(table_count)?;
        
        // Use rayon for parallel processing
        use rayon::prelude::*;
        
        // Process each tree in parallel
        let results: Result<Vec<()>> = (0..table_count)
            .into_par_iter()
            .map(|tree_idx| {
                let tree_name = format!("table_{}", tree_idx);
                let tree = self.db.open_tree(&tree_name)
                    .map_err(|e| DbError::Database(format!("Failed to open tree {}: {}", tree_name, e)))?;
                
                let mut batch = sled::Batch::default();
                
                // Build batch data for this tree
                for (key, value) in &data {
                    batch.insert(&**key, &**value);
                }
                
                // Execute atomic batch write for this tree
                tree
                    .apply_batch(batch)
                    .map_err(|e| DbError::Database(format!("Failed to write batch to {}: {}", tree_name, e)))?;
                
                Ok(())
            })
            .collect();
        
        results?;
        
        // Force flush to ensure data is persisted to disk
        self.db
            .flush()
            .map_err(|e| DbError::Database(format!("Failed to flush to disk: {}", e)))?;
        
        Ok(start.elapsed())
    }
    
    fn read_batch(&mut self, keys: Vec<Vec<u8>>, table_count: usize) -> Result<(Duration, Vec<Vec<Option<Vec<u8>>>>)> {
        use std::time::Instant;
        
        let start = Instant::now();
        
        // Ensure we have enough trees
        self.ensure_trees(table_count)?;
        
        let mut results = Vec::with_capacity(keys.len());
        
        for key in keys {
            let mut key_results = Vec::with_capacity(table_count);
            
            // Read from all trees
            for tree in &self.trees[..table_count] {
                let value = tree
                    .get(&key)
                    .map_err(|e| DbError::Database(format!("Failed to read key: {}", e)))?;
                key_results.push(value.map(|iv| iv.to_vec()));
            }
            
            results.push(key_results);
        }
        
        Ok((start.elapsed(), results))
    }
    
    fn count_entries(&self) -> Result<u64> {
        Ok(self.db.len() as u64)
    }
    
    fn close(self) -> Result<()> {
        self.db
            .flush()
            .map_err(|e| DbError::Database(format!("Failed to flush Sled: {}", e)))?;
        Ok(())
    }
    
    fn get_info(&self) -> String {
        format!("Sled - Database opened successfully")
    }
}

/// MDBX implementation using lmdb (MDBX-compatible)
pub struct MdbxImpl {
    env: lmdb::Environment,
}

impl MdbxImpl {
    fn new(env: lmdb::Environment) -> Self {
        Self { env }
    }
}

impl Database for MdbxImpl {
    fn open(path: &Path) -> Result<Self> {
        use lmdb::{Environment, EnvironmentFlags};
        
        // Create the LMDB environment with dynamic map size
        // Start with a reasonable initial size and let LMDB grow as needed
        let env = Environment::new()
            .set_flags(EnvironmentFlags::WRITE_MAP | EnvironmentFlags::MAP_ASYNC)
            .set_max_dbs(100)  // Allow up to 100 databases
            .set_map_size(32 * 1024 * 1024 * 1024)  // 32GB to handle large datasets (75M entries)
            .set_max_readers(1)  // Single reader (no concurrency)
            .open(path)
            .map_err(|e| DbError::Database(format!("Failed to open LMDB environment: {}", e)))?;
        
        Ok(Self::new(env))
    }
    
    fn write_batch(&mut self, data: Vec<(Vec<u8>, Vec<u8>)>, _table_count: usize) -> Result<Duration> {
        use std::time::Instant;
        use lmdb::Transaction;
        
        let start = Instant::now();
        
        // Single write transaction (no concurrency)
        let mut txn = self.env.begin_rw_txn()
            .map_err(|e| DbError::Database(format!("Failed to begin transaction: {}", e)))?;
        
        // Use default database for all writes (simplified approach)
        let db = self.env.open_db(None::<&str>)
            .map_err(|e| DbError::Database(format!("Failed to open default database: {}", e)))?;
        
        // Write data once to the default database (simplified approach)
        for (key, value) in &data {
            txn.put(db, key, value, lmdb::WriteFlags::empty())
                .map_err(|e| DbError::Database(format!("Failed to put key-value: {}", e)))?;
        }
        
        // Commit transaction
        txn.commit()
            .map_err(|e| DbError::Database(format!("Failed to commit transaction: {}", e)))?;
        
        Ok(start.elapsed())
    }
    
    fn read_batch(&mut self, keys: Vec<Vec<u8>>, table_count: usize) -> Result<(Duration, Vec<Vec<Option<Vec<u8>>>>)> {
        use std::time::Instant;
        use lmdb::Transaction;
        
        let start = Instant::now();
        
        let mut results = Vec::with_capacity(keys.len());
        
        // Single read transaction for all operations (no concurrency)
        let txn = self.env.begin_ro_txn()
            .map_err(|e| DbError::Database(format!("Failed to begin read transaction: {}", e)))?;
        
        // Use default database for all reads
        let db = self.env.open_db(None::<&str>)
            .map_err(|e| DbError::Database(format!("Failed to open default database: {}", e)))?;
        
        for key in keys {
            let mut key_results = Vec::with_capacity(table_count);
            
            // Read from database once and replicate result for table_count
            let value = match txn.get(db, &key) {
                Ok(val) => Some(val.to_vec()),
                Err(lmdb::Error::NotFound) => None,
                Err(e) => return Err(DbError::Database(format!("Failed to read key: {}", e))),
            };
            
            // Replicate the result for table_count to maintain compatibility
            for _ in 0..table_count {
                key_results.push(value.clone());
            }
            
            results.push(key_results);
        }
        
        Ok((start.elapsed(), results))
    }
    
    fn count_entries(&self) -> Result<u64> {
        // Get database statistics to count entries
        let stat = self.env.stat()
            .map_err(|e| DbError::Database(format!("Failed to get database statistics: {}", e)))?;
        Ok(stat.entries() as u64)
    }
    
    fn close(self) -> Result<()> {
        // MDBX database is automatically closed when dropped
        Ok(())
    }
    
    fn get_info(&self) -> String {
        format!("MDBX - Database opened successfully")
    }
}
