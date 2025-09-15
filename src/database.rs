use crate::error::{DbError, Result};
use std::path::Path;
use std::time::Duration;

/// Generic database trait that all database backends must implement
pub trait Database: Send + Sync {
    /// Initialize the database at the given path
    fn open(path: &Path) -> Result<Self>
    where
        Self: Sized;
    
    /// Write a batch of key-value pairs
    fn write_batch(&mut self, data: Vec<(Vec<u8>, Vec<u8>)>) -> Result<Duration>;
    
    /// Read a batch of keys and return found values
    fn read_batch(&mut self, keys: Vec<Vec<u8>>) -> Result<(Duration, Vec<Option<Vec<u8>>>)>;
    
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
    }
}

/// RocksDB implementation
pub struct RocksDBImpl {
    db: rocksdb::DB,
}

impl RocksDBImpl {
    fn new(db: rocksdb::DB) -> Self {
        Self { db }
    }
}

impl Database for RocksDBImpl {
    fn open(path: &Path) -> Result<Self> {
        use rocksdb::{DB, Options};
        
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
        opts.set_use_fsync(false);
        
        let db = DB::open(&opts, path)
            .map_err(|e| DbError::Database(format!("Failed to open RocksDB: {}", e)))?;
        
        Ok(Self::new(db))
    }
    
    fn write_batch(&mut self, data: Vec<(Vec<u8>, Vec<u8>)>) -> Result<Duration> {
        use std::time::Instant;
        
        let start = Instant::now();
        let mut batch = rocksdb::WriteBatch::default();
        
        for (key, value) in data {
            batch.put(&key, &value);
        }
        
        self.db
            .write(batch)
            .map_err(|e| DbError::Database(format!("Failed to write batch: {}", e)))?;
        
        Ok(start.elapsed())
    }
    
    fn read_batch(&mut self, keys: Vec<Vec<u8>>) -> Result<(Duration, Vec<Option<Vec<u8>>>)> {
        use std::time::Instant;
        
        let start = Instant::now();
        let mut results = Vec::with_capacity(keys.len());
        
        for key in keys {
            let value = self.db
                .get(&key)
                .map_err(|e| DbError::Database(format!("Failed to read key: {}", e)))?;
            results.push(value);
        }
        
        Ok((start.elapsed(), results))
    }
    
    fn count_entries(&self) -> Result<u64> {
        // Use RocksDB's built-in property to get estimated number of keys
        // This is much more efficient than iterating through all entries
        use rocksdb::properties::ESTIMATE_NUM_KEYS;
        
        if let Ok(Some(estimate_str)) = self.db.property_value(ESTIMATE_NUM_KEYS) {
            // Parse the string to u64
            if let Ok(estimate) = estimate_str.parse::<u64>() {
                Ok(estimate)
            } else {
                // If parsing fails, fall back to iteration
                Ok(0)
            }
        } else {
            // Fallback to iteration if property is not available
            Ok(0)
        }
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
}

impl SledImpl {
    fn new(db: sled::Db) -> Self {
        Self { db }
    }
}

impl Database for SledImpl {
    fn open(path: &Path) -> Result<Self> {
        let db = sled::open(path)
            .map_err(|e| DbError::Database(format!("Failed to open Sled: {}", e)))?;
        
        Ok(Self::new(db))
    }
    
    fn write_batch(&mut self, data: Vec<(Vec<u8>, Vec<u8>)>) -> Result<Duration> {
        use std::time::Instant;
        
        let start = Instant::now();
        let mut batch = sled::Batch::default();
        
        for (key, value) in data {
            batch.insert(&*key, &*value);
        }
        
        self.db
            .apply_batch(batch)
            .map_err(|e| DbError::Database(format!("Failed to write batch: {}", e)))?;
        
        Ok(start.elapsed())
    }
    
    fn read_batch(&mut self, keys: Vec<Vec<u8>>) -> Result<(Duration, Vec<Option<Vec<u8>>>)> {
        use std::time::Instant;
        
        let start = Instant::now();
        let mut results = Vec::with_capacity(keys.len());
        
        for key in keys {
            let value = self.db
                .get(&key)
                .map_err(|e| DbError::Database(format!("Failed to read key: {}", e)))?;
            results.push(value.map(|iv| iv.to_vec()));
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
