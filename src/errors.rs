//! # Error Handling Module
//!
//! ## Purpose
//! Centralized error handling for the legal search engine, providing comprehensive error types
//! and conversion utilities for all system components.
//!
//! ## Input/Output Specification
//! - **Input**: Error conditions from various system components
//! - **Output**: Structured error types with context and error chains
//! - **Error Categories**: Ingestion, Processing, Storage, Search, API, Configuration
//!
//! ## Key Features
//! - Hierarchical error types with detailed context
//! - Automatic error conversion and chaining
//! - User-friendly error messages for API responses
//! - Structured logging integration
//! - Recovery suggestions where applicable
//!
//! ## Usage
//! ```rust
//! use crate::errors::{Result, SearchError};
//!
//! fn search_operation() -> Result<Vec<String>> {
//!     // Operation that might fail
//!     Err(SearchError::IndexCorrupted {
//!         index_type: "trie".to_string(),
//!         details: "Checksum mismatch".to_string(),
//!     })
//! }
//! ```

// Serde traits not needed for error types
use std::fmt;
use thiserror::Error;

/// Result type used throughout the application
pub type Result<T> = std::result::Result<T, SearchError>;

/// Comprehensive error types for the legal search engine
#[derive(Debug, thiserror::Error)]
pub enum SearchError {
    /// Generic I/O errors
    #[error("I/O error: {0}")]
    Io(std::io::Error),

    /// Network-related errors
    #[error("Network error: {details}")]
    NetworkError { details: String },

    /// Rate limiting errors
    #[error("Rate limit exceeded for {source}")]
    RateLimitExceeded {
        source: String,
        retry_after_seconds: Option<u64>,
    },

    /// Data source unavailable
    #[error("Data source '{source}' is unavailable: {details}")]
    DataSourceUnavailable { source: String, details: String },

    /// Data parsing errors
    #[error("Failed to parse data from {source}: {details}")]
    DataParsing { source: String, details: String },

    /// Configuration errors
    #[error("Configuration error: {message}")]
    Config { message: String },

    /// Internal system errors
    #[error("Internal error: {message}")]
    Internal { message: String },

    /// Serialization/deserialization errors
    #[error("Serialization failed: {message}")]
    SerializationFailed { message: String },

    /// Validation errors
    #[error("Validation failed for field '{field}': {reason}")]
    ValidationFailed { field: String, reason: String },

    /// Not supported operation
    #[error("Operation '{operation}' is not supported")]
    NotSupported { operation: String },

    /// Database errors
    #[error("Database error: {0}")]
    Database(sled::Error),

    /// Serialization errors
    #[error("Serialization error: {0}")]
    Serialization(bincode::Error),

    /// HTTP client errors
    #[error("HTTP error: {0}")]
    Http(reqwest::Error),

    /// JSON parsing errors
    #[error("JSON error: {0}")]
    Json(serde_json::Error),

    /// TOML parsing errors
    #[error("TOML error: {0}")]
    Toml(toml::de::Error),

    /// Generic error with message
    #[error("{message}")]
    Generic { message: String },

    // Data ingestion errors
    #[error("Invalid case format in {file}: {details}")]
    InvalidCaseFormat { file: String, details: String },

    #[error("Network error during ingestion: {details}")]
    NetworkErrorDuringIngestion { details: String },

    // Text processing errors
    #[error("Tokenization failed: {text_preview} - {reason}")]
    TokenizationFailed {
        text_preview: String,
        reason: String,
    },

    #[error("Text normalization error: {details}")]
    TextNormalizationError { details: String },

    #[error("Unsupported text encoding: {encoding}")]
    UnsupportedEncoding { encoding: String },

    // Trie index errors
    #[error("Trie construction failed: {reason}")]
    TrieConstructionFailed { reason: String },

    #[error("Trie lookup error: {query} - {details}")]
    TrieLookupError { query: String, details: String },

    #[error("FST compilation failed: {reason}")]
    FstCompilationFailed { reason: String },

    // Vector search errors
    #[error("Embedding model not found: {model_path}")]
    EmbeddingModelNotFound { model_path: String },

    #[error("Embedding generation failed: {text_preview} - {reason}")]
    EmbeddingGenerationFailed {
        text_preview: String,
        reason: String,
    },

    #[error("Vector index construction failed: {reason}")]
    VectorIndexFailed { reason: String },

    #[error("HNSW search error: {details}")]
    HnswSearchError { details: String },

    #[error("ONNX runtime error: {details}")]
    OnnxRuntimeError { details: String },

    // Storage errors
    #[error("Database connection failed: {db_path} - {reason}")]
    DatabaseConnectionFailed { db_path: String, reason: String },

    #[error("Storage corruption detected: {location} - {details}")]
    StorageCorrupted { location: String, details: String },

    #[error("Insufficient disk space: required {required_gb}GB, available {available_gb}GB")]
    InsufficientDiskSpace {
        required_gb: u64,
        available_gb: u64,
    },

    // Search engine errors
    #[error("Index not found: {index_name}")]
    IndexNotFound { index_name: String },

    #[error("Index corrupted: {index_type} - {details}")]
    IndexCorrupted {
        index_type: String,
        details: String,
    },

    #[error("Search timeout: query took longer than {timeout_ms}ms")]
    SearchTimeout { timeout_ms: u64 },

    #[error("Invalid search query: {query} - {reason}")]
    InvalidSearchQuery { query: String, reason: String },

    #[error("Search capacity exceeded: {current_load}% - {details}")]
    SearchCapacityExceeded {
        current_load: u8,
        details: String,
    },

    // API errors
    #[error("Invalid API request: {details}")]
    InvalidApiRequest { details: String },

    #[error("Authentication failed: {reason}")]
    AuthenticationFailed { reason: String },

    #[error("Rate limit exceeded: {requests_per_minute} RPM exceeded")]
    ApiRateLimitExceeded { requests_per_minute: u32 },

    #[error("Request payload too large: {size_mb}MB exceeds limit of {limit_mb}MB")]
    PayloadTooLarge { size_mb: u32, limit_mb: u32 },

    // System errors
    #[error("Memory allocation failed: {requested_mb}MB")]
    MemoryAllocationFailed { requested_mb: u64 },

    #[error("Thread pool exhausted: {active_threads}/{max_threads}")]
    ThreadPoolExhausted {
        active_threads: usize,
        max_threads: usize,
    },

    #[error("System resource unavailable: {resource} - {reason}")]
    SystemResourceUnavailable { resource: String, reason: String },
}

impl SearchError {
    /// Check if the error is recoverable (can be retried)
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            SearchError::NetworkError { .. }
                | SearchError::RateLimitExceeded { .. }
                | SearchError::DatabaseConnectionFailed { .. }
                | SearchError::SearchTimeout { .. }
                | SearchError::ThreadPoolExhausted { .. }
                | SearchError::SearchCapacityExceeded { .. }
        )
    }

    /// Get error category for metrics and logging
    pub fn category(&self) -> &'static str {
        match self {
            SearchError::Config { .. } => "configuration",
            SearchError::DataSourceUnavailable { .. }
            | SearchError::InvalidCaseFormat { .. }
            | SearchError::NetworkError { .. }
            | SearchError::RateLimitExceeded { .. } => "ingestion",
            SearchError::TokenizationFailed { .. }
            | SearchError::TextNormalizationError { .. }
            | SearchError::UnsupportedEncoding { .. } => "text_processing",
            SearchError::TrieConstructionFailed { .. }
            | SearchError::TrieLookupError { .. }
            | SearchError::FstCompilationFailed { .. } => "trie",
            SearchError::EmbeddingModelNotFound { .. }
            | SearchError::EmbeddingGenerationFailed { .. }
            | SearchError::VectorIndexFailed { .. }
            | SearchError::HnswSearchError { .. }
            | SearchError::OnnxRuntimeError { .. } => "vector",
            SearchError::DatabaseConnectionFailed { .. }
            | SearchError::StorageCorrupted { .. }
            | SearchError::InsufficientDiskSpace { .. }
            | SearchError::SerializationFailed { .. } => "storage",
            SearchError::IndexNotFound { .. }
            | SearchError::IndexCorrupted { .. }
            | SearchError::SearchTimeout { .. }
            | SearchError::InvalidSearchQuery { .. }
            | SearchError::SearchCapacityExceeded { .. } => "search",
            SearchError::InvalidApiRequest { .. }
            | SearchError::AuthenticationFailed { .. }
            | SearchError::ApiRateLimitExceeded { .. }
            | SearchError::PayloadTooLarge { .. } => "api",
            SearchError::MemoryAllocationFailed { .. }
            | SearchError::ThreadPoolExhausted { .. }
            | SearchError::SystemResourceUnavailable { .. } => "system",
            SearchError::Internal { .. }
            | SearchError::NotSupported { .. }
            | SearchError::ValidationFailed { .. } => "generic",
        }
    }

    /// Get suggested recovery action
    pub fn recovery_suggestion(&self) -> Option<&'static str> {
        match self {
            SearchError::RateLimitExceeded { .. } => Some("Wait and retry after the specified time"),
            SearchError::NetworkError { .. } => Some("Check network connectivity and retry"),
            SearchError::SearchTimeout { .. } => Some("Simplify query or increase timeout"),
            SearchError::SearchCapacityExceeded { .. } => Some("Reduce query complexity or try again later"),
            SearchError::InsufficientDiskSpace { .. } => Some("Free up disk space or increase storage"),
            SearchError::ThreadPoolExhausted { .. } => Some("Reduce concurrent operations"),
            _ => None,
        }
    }
}

// Conversion from common error types
impl From<std::io::Error> for SearchError {
    fn from(err: std::io::Error) -> Self {
        SearchError::Internal {
            message: format!("IO error: {}", err),
        }
    }
}

impl From<serde_json::Error> for SearchError {
    fn from(err: serde_json::Error) -> Self {
        SearchError::SerializationFailed {
            message: format!("JSON serialization error: {}", err),
        }
    }
}

impl From<reqwest::Error> for SearchError {
    fn from(err: reqwest::Error) -> Self {
        SearchError::NetworkError {
            details: err.to_string(),
        }
    }
}

impl From<bincode::Error> for SearchError {
    fn from(err: bincode::Error) -> Self {
        SearchError::SerializationFailed {
            message: format!("Binary serialization error: {}", err),
        }
    }
}

// Helper macros for common error patterns
#[macro_export]
macro_rules! internal_error {
    ($msg:expr) => {
        $crate::errors::SearchError::Internal {
            message: $msg.to_string(),
        }
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::errors::SearchError::Internal {
            message: format!($fmt, $($arg)*),
        }
    };
}

#[macro_export]
macro_rules! validation_error {
    ($field:expr, $reason:expr) => {
        $crate::errors::SearchError::ValidationFailed {
            field: $field.to_string(),
            reason: $reason.to_string(),
        }
    };
} 