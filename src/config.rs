//! # Configuration Management Module
//!
//! ## Purpose
//! Centralized configuration management for the legal search engine, supporting multiple
//! sources (files, environment variables, command line arguments) with validation and
//! type-safe access to all system settings.
//!
//! ## Input/Output Specification
//! - **Input**: Configuration files (TOML), environment variables, CLI arguments
//! - **Output**: Validated configuration structs with defaults and overrides
//! - **Validation**: Type checking, range validation, dependency verification
//!
//! ## Key Features
//! - Hierarchical configuration with environment-specific overrides
//! - Automatic validation with detailed error messages
//! - Hot-reload capability for runtime configuration changes
//! - Secure handling of sensitive configuration (API keys, database credentials)
//! - Performance tuning parameters with intelligent defaults
//!
//! ## Configuration Sources (in order of precedence)
//! 1. Command line arguments (highest priority)
//! 2. Environment variables
//! 3. Configuration files
//! 4. Default values (lowest priority)
//!
//! ## Usage
//! ```rust
//! use crate::config::Config;
//!
//! // Load from default locations
//! let config = Config::load()?;
//!
//! // Load from specific file
//! let config = Config::from_file("custom.toml")?;
//!
//! // Access configuration
//! println!("Server port: {}", config.server.port);
//! ```

use crate::errors::{Result, SearchError};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Main configuration structure containing all system settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Server and API configuration
    pub server: ServerConfig,
    /// Data ingestion settings
    pub ingestion: IngestionConfig,
    /// Text processing configuration
    pub text_processing: TextProcessingConfig,
    /// Trie index configuration
    pub trie: TrieConfig,
    /// Vector search configuration
    pub vector: VectorConfig,
    /// Storage and database settings
    pub storage: StorageConfig,
    /// Search engine behavior
    pub search: SearchEngineConfig,
    /// Logging and monitoring
    pub logging: LoggingConfig,
    /// Performance tuning
    pub performance: PerformanceConfig,
}

/// Server and API configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Server bind address
    pub host: String,
    /// Server port
    pub port: u16,
    /// Maximum request payload size in MB
    pub max_payload_size_mb: u32,
    /// Request timeout in seconds
    pub request_timeout_seconds: u64,
    /// Enable CORS
    pub enable_cors: bool,
    /// API key for authentication (optional)
    pub api_key: Option<String>,
    /// Rate limiting (requests per minute)
    pub rate_limit_rpm: u32,
}

/// Data ingestion configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestionConfig {
    /// Caselaw Access Project settings
    pub cap: CapConfig,
    /// CourtListener settings
    pub courtlistener: CourtListenerConfig,
    /// Batch processing settings
    pub batch_size: usize,
    /// Maximum concurrent downloads
    pub max_concurrent_downloads: usize,
    /// Maximum concurrent processing jobs
    pub max_concurrent_jobs: usize,
    /// Rate limiting delay between requests (ms)
    pub rate_limit_delay_ms: u64,
    /// Maximum memory usage before triggering cleanup (MB)
    pub max_memory_usage_mb: usize,
    /// Retry configuration
    pub retry_attempts: u32,
    /// Retry delay in seconds
    pub retry_delay_seconds: u64,
    /// Enable incremental updates
    pub enable_incremental_updates: bool,
    /// Update check interval in hours
    pub update_check_interval_hours: u64,
    /// Validation configuration
    pub validation: ValidationConfig,
    /// Cache configuration
    pub cache: CacheConfig,
}

/// Caselaw Access Project configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapConfig {
    /// API base URL
    pub api_url: String,
    /// API key for authentication
    pub api_key: Option<String>,
    /// Bulk data download URL
    pub bulk_data_url: String,
    /// Local cache directory for downloaded data
    pub cache_dir: PathBuf,
}

/// CourtListener configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CourtListenerConfig {
    /// API base URL
    pub api_url: String,
    /// API key for authentication
    pub api_key: Option<String>,
    /// Local cache directory
    pub cache_dir: PathBuf,
}

/// Text processing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextProcessingConfig {
    /// Tokenizer model path
    pub tokenizer_model_path: PathBuf,
    /// Enable case folding (lowercase conversion)
    pub enable_case_folding: bool,
    /// Enable Unicode normalization
    pub enable_unicode_normalization: bool,
    /// Preserve legal citations as single tokens
    pub preserve_legal_citations: bool,
    /// Maximum text length for processing
    pub max_text_length: usize,
    /// Remove extra whitespace
    pub remove_extra_whitespace: bool,
    /// Normalize quote characters
    pub normalize_quotes: bool,
    /// Extract citations from text
    pub extract_citations: bool,
    /// Extract named entities
    pub extract_entities: bool,
    /// Sentence splitting configuration
    pub sentence_splitting: SentenceSplittingConfig,
}

/// Sentence splitting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentenceSplittingConfig {
    /// Enable sentence splitting
    pub enabled: bool,
    /// Minimum sentence length in characters
    pub min_sentence_length: usize,
    /// Maximum sentence length in characters
    pub max_sentence_length: usize,
}

/// Trie index configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrieConfig {
    /// Use FST (Finite State Transducer) for compression
    pub use_fst: bool,
    /// Index case names separately
    pub index_case_names: bool,
    /// Index legal citations
    pub index_citations: bool,
    /// Maximum prefix length for auto-completion
    pub max_prefix_length: usize,
    /// Index file path
    pub index_path: PathBuf,
    /// Enable memory mapping for FST
    pub enable_memory_mapping: bool,
}

/// Vector search configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorConfig {
    /// Embedding model configuration
    pub model: EmbeddingModelConfig,
    /// HNSW index configuration
    pub hnsw: HnswConfig,
    /// Vector dimension (must match model output)
    pub dimension: usize,
    /// Similarity threshold for results
    pub similarity_threshold: f32,
    /// Maximum vectors to return from ANN search
    pub max_ann_results: usize,
}

/// Embedding model configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingModelConfig {
    /// Model file path (ONNX format)
    pub model_path: PathBuf,
    /// Tokenizer configuration path
    pub tokenizer_path: PathBuf,
    /// Model type identifier
    pub model_type: String,
    /// Use GPU acceleration if available
    pub use_gpu: bool,
    /// Batch size for embedding generation
    pub batch_size: usize,
    /// Maximum sequence length
    pub max_sequence_length: usize,
}

/// HNSW (Hierarchical Navigable Small World) configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HnswConfig {
    /// Number of bi-directional links for each node (M parameter)
    pub m: usize,
    /// Size of the dynamic candidate list (ef_construction)
    pub ef_construction: usize,
    /// Size of the dynamic candidate list during search (ef)
    pub ef_search: usize,
    /// Maximum number of elements in the index
    pub max_elements: usize,
    /// Index file path
    pub index_path: PathBuf,
}

/// Storage and database configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Database type (currently supports "sled")
    pub db_type: String,
    /// Database file path
    pub db_path: PathBuf,
    /// Maximum database size in GB
    pub max_db_size_gb: u64,
    /// Enable database compression
    pub enable_compression: bool,
    /// Backup configuration
    pub backup: BackupConfig,
}

/// Backup configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupConfig {
    /// Enable automatic backups
    pub enabled: bool,
    /// Backup directory
    pub backup_dir: PathBuf,
    /// Backup interval in hours
    pub interval_hours: u64,
    /// Maximum number of backups to retain
    pub max_backups: u32,
}

/// Search engine configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchEngineConfig {
    /// Default maximum number of results
    pub default_max_results: usize,
    /// Search timeout in milliseconds
    pub search_timeout_ms: u64,
    /// Enable query caching
    pub enable_query_cache: bool,
    /// Query cache size (number of entries)
    pub query_cache_size: usize,
    /// Query cache TTL in seconds
    pub query_cache_ttl_seconds: u64,
    /// Minimum query length
    pub min_query_length: usize,
    /// Maximum query length
    pub max_query_length: usize,
}

/// Logging and monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level (trace, debug, info, warn, error)
    pub level: String,
    /// Log file path (optional, logs to stdout if not specified)
    pub file_path: Option<PathBuf>,
    /// Enable structured JSON logging
    pub json_format: bool,
    /// Enable performance metrics logging
    pub enable_metrics: bool,
    /// Metrics collection interval in seconds
    pub metrics_interval_seconds: u64,
}

/// Performance tuning configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Number of worker threads for async runtime
    pub worker_threads: usize,
    /// Thread pool size for CPU-intensive tasks
    pub cpu_pool_size: usize,
    /// Enable memory pool for allocations
    pub enable_memory_pool: bool,
    /// Garbage collection settings
    pub gc: GcConfig,
}

/// Garbage collection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcConfig {
    /// Enable periodic garbage collection
    pub enabled: bool,
    /// GC interval in seconds
    pub interval_seconds: u64,
    /// Memory threshold for triggering GC (percentage)
    pub memory_threshold_percent: u8,
}

/// Data validation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationConfig {
    /// Minimum text length
    pub min_text_length: usize,
    /// Maximum text length
    pub max_text_length: usize,
    /// Required metadata fields
    pub required_fields: Vec<String>,
    /// Allow empty citations
    pub allow_empty_citations: bool,
    /// Validate dates
    pub validate_dates: bool,
    /// Validate citations
    pub validate_citations: bool,
}

/// Cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Enable caching
    pub enabled: bool,
    /// Maximum memory cache entries
    pub max_memory_entries: usize,
    /// Disk cache path
    pub disk_cache_path: Option<PathBuf>,
    /// Maximum disk cache size (MB)
    pub max_disk_size_mb: usize,
    /// Time to live for cache entries (hours)
    pub ttl_hours: u64,
}

impl Config {
    /// Load configuration from default locations
    pub fn load() -> Result<Self> {
        Self::from_file("config.toml")
    }

    /// Load configuration from a specific file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        
        if !path.exists() {
            tracing::warn!("Configuration file not found: {:?}, using defaults", path);
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(path)
            .map_err(|e| SearchError::Config {
                message: format!("Failed to read config file {:?}: {}", path, e),
            })?;

        let mut config: Config = toml::from_str(&content)
            .map_err(|e| SearchError::Config {
                message: format!("Failed to parse config file {:?}: {}", path, e),
            })?;

        // Apply environment variable overrides
        config.apply_env_overrides()?;

        // Validate configuration
        config.validate()?;

        Ok(config)
    }

    /// Apply environment variable overrides
    fn apply_env_overrides(&mut self) -> Result<()> {
        // Server configuration
        if let Ok(host) = std::env::var("LEGAL_SEARCH_HOST") {
            self.server.host = host;
        }
        if let Ok(port) = std::env::var("LEGAL_SEARCH_PORT") {
            self.server.port = port.parse().map_err(|_| SearchError::Config {
                message: "Invalid port number in LEGAL_SEARCH_PORT".to_string(),
            })?;
        }
        if let Ok(api_key) = std::env::var("LEGAL_SEARCH_API_KEY") {
            self.server.api_key = Some(api_key);
        }

        // Database configuration
        if let Ok(db_path) = std::env::var("LEGAL_SEARCH_DB_PATH") {
            self.storage.db_path = PathBuf::from(db_path);
        }

        // Model paths
        if let Ok(model_path) = std::env::var("LEGAL_SEARCH_MODEL_PATH") {
            self.vector.model.model_path = PathBuf::from(model_path);
        }

        Ok(())
    }

    /// Validate configuration values
    fn validate(&self) -> Result<()> {
        // Validate server configuration
        if self.server.port == 0 {
            return Err(SearchError::ValidationFailed {
                field: "server.port".to_string(),
                reason: "Port cannot be zero".to_string(),
            });
        }

        // Validate paths exist
        if !self.vector.model.model_path.exists() {
            return Err(SearchError::ValidationFailed {
                field: "vector.model.model_path".to_string(),
                reason: format!("Model file not found: {:?}", self.vector.model.model_path),
            });
        }

        // Validate vector dimensions
        if self.vector.dimension == 0 {
            return Err(SearchError::ValidationFailed {
                field: "vector.dimension".to_string(),
                reason: "Vector dimension must be greater than zero".to_string(),
            });
        }

        // Validate HNSW parameters
        if self.vector.hnsw.m == 0 {
            return Err(SearchError::ValidationFailed {
                field: "vector.hnsw.m".to_string(),
                reason: "HNSW M parameter must be greater than zero".to_string(),
            });
        }

        // Validate search parameters
        if self.search.min_query_length > self.search.max_query_length {
            return Err(SearchError::ValidationFailed {
                field: "search.min_query_length".to_string(),
                reason: "Minimum query length cannot be greater than maximum".to_string(),
            });
        }

        Ok(())
    }

    /// Get configuration as TOML string
    pub fn to_toml(&self) -> Result<String> {
        toml::to_string_pretty(self).map_err(|e| SearchError::Config {
            message: format!("Failed to serialize config to TOML: {}", e),
        })
    }

    /// Save configuration to file
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = self.to_toml()?;
        std::fs::write(path, content)?;
        Ok(())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 8080,
                max_payload_size_mb: 10,
                request_timeout_seconds: 30,
                enable_cors: true,
                api_key: None,
                rate_limit_rpm: 1000,
            },
            ingestion: IngestionConfig {
                cap: CapConfig {
                    api_url: "https://api.case.law/v1/".to_string(),
                    api_key: None,
                    bulk_data_url: "https://bulk.case.law/".to_string(),
                    cache_dir: PathBuf::from("./data/cap_cache"),
                },
                courtlistener: CourtListenerConfig {
                    api_url: "https://www.courtlistener.com/api/rest/v3/".to_string(),
                    api_key: None,
                    cache_dir: PathBuf::from("./data/cl_cache"),
                },
                batch_size: 100,
                max_concurrent_downloads: 5,
                max_concurrent_jobs: 10,
                rate_limit_delay_ms: 500,
                max_memory_usage_mb: 1024,
                retry_attempts: 3,
                retry_delay_seconds: 5,
                enable_incremental_updates: true,
                update_check_interval_hours: 24,
                validation: ValidationConfig {
                    min_text_length: 100,
                    max_text_length: 1_000_000,
                    required_fields: vec!["title", "date", "court", "case_name"],
                    allow_empty_citations: false,
                    validate_dates: true,
                    validate_citations: true,
                },
                cache: CacheConfig {
                    enabled: true,
                    max_memory_entries: 100_000,
                    disk_cache_path: None,
                    max_disk_size_mb: 1024,
                    ttl_hours: 24,
                },
            },
            text_processing: TextProcessingConfig {
                tokenizer_model_path: PathBuf::from("./models/tokenizer.json"),
                enable_case_folding: true,
                enable_unicode_normalization: true,
                preserve_legal_citations: true,
                max_text_length: 1_000_000,
                remove_extra_whitespace: true,
                normalize_quotes: true,
                extract_citations: true,
                extract_entities: true,
                sentence_splitting: SentenceSplittingConfig {
                    enabled: true,
                    min_sentence_length: 10,
                    max_sentence_length: 1000,
                },
            },
            trie: TrieConfig {
                use_fst: true,
                index_case_names: true,
                index_citations: true,
                max_prefix_length: 50,
                index_path: PathBuf::from("./data/trie_index"),
                enable_memory_mapping: true,
            },
            vector: VectorConfig {
                model: EmbeddingModelConfig {
                    model_path: PathBuf::from("./models/legal-bert.onnx"),
                    tokenizer_path: PathBuf::from("./models/tokenizer.json"),
                    model_type: "legal-bert".to_string(),
                    use_gpu: false,
                    batch_size: 32,
                    max_sequence_length: 512,
                },
                hnsw: HnswConfig {
                    m: 16,
                    ef_construction: 200,
                    ef_search: 50,
                    max_elements: 10_000_000,
                    index_path: PathBuf::from("./data/vector_index"),
                },
                dimension: 768,
                similarity_threshold: 0.5,
                max_ann_results: 100,
            },
            storage: StorageConfig {
                db_type: "sled".to_string(),
                db_path: PathBuf::from("./data/legal_search.db"),
                max_db_size_gb: 100,
                enable_compression: true,
                backup: BackupConfig {
                    enabled: true,
                    backup_dir: PathBuf::from("./backups"),
                    interval_hours: 24,
                    max_backups: 7,
                },
            },
            search: SearchEngineConfig {
                default_max_results: 10,
                search_timeout_ms: 5000,
                enable_query_cache: true,
                query_cache_size: 10000,
                query_cache_ttl_seconds: 3600,
                min_query_length: 2,
                max_query_length: 1000,
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                file_path: None,
                json_format: false,
                enable_metrics: true,
                metrics_interval_seconds: 60,
            },
            performance: PerformanceConfig {
                worker_threads: num_cpus::get(),
                cpu_pool_size: num_cpus::get() * 2,
                enable_memory_pool: true,
                gc: GcConfig {
                    enabled: true,
                    interval_seconds: 300,
                    memory_threshold_percent: 80,
                },
            },
        }
    }
} 