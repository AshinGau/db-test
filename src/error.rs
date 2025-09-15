use thiserror::Error;

#[derive(Error, Debug)]
pub enum DbError {
    #[error("Database operation failed: {0}")]
    Database(String),
    
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("Random number generation error: {0}")]
    Random(#[from] rand::Error),
}

pub type Result<T> = std::result::Result<T, DbError>;

