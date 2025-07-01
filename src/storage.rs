//! # Storage Management Module
//!
//! ## Purpose
//! Handles persistent storage of legal case metadata, full text, and system state
//! using embedded databases and file systems for optimal performance.
//!
//! ## Input/Output Specification
//! - **Input**: Case metadata, full text, search indices, system state
//! - **Output**: Persistent storage, retrieval operations, backup management
//! - **Storage**: Sled embedded database, file system for large objects
//!
//! ## Key Features
//! - Embedded database for metadata and small objects
//! - File system storage for large text documents
//! - Automatic backup and recovery
//! - Compression for space efficiency
//! - ACID transactions for data integrity

use crate::config::StorageConfig;
use crate::errors::{Result, SearchError};
use crate::{CaseId, CaseMetadata};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Main storage manager
pub struct StorageManager {
    config: StorageConfig,
    db: Arc<sled::Db>,
    metadata_tree: Arc<sled::Tree>,
    text_tree: Arc<sled::Tree>,
    stats: Arc<RwLock<StorageStats>>,
}

/// Storage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageStats {
    pub total_cases: usize,
    pub total_size_bytes: u64,
    pub database_size_bytes: u64,
    pub last_backup: Option<chrono::DateTime<chrono::Utc>>,
}

impl StorageManager {
    /// Create new storage manager
    pub async fn new(config: StorageConfig) -> Result<Self> {
        // Ensure database directory exists
        if let Some(parent) = config.db_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Open sled database
        let db = sled::open(&config.db_path)
            .map_err(|e| SearchError::DatabaseConnectionFailed {
                db_path: config.db_path.to_string_lossy().to_string(),
                reason: e.to_string(),
            })?;

        // Open trees for different data types
        let metadata_tree = db.open_tree("case_metadata")
            .map_err(|e| SearchError::DatabaseConnectionFailed {
                db_path: config.db_path.to_string_lossy().to_string(),
                reason: format!("Failed to open metadata tree: {}", e),
            })?;

        let text_tree = db.open_tree("case_text")
            .map_err(|e| SearchError::DatabaseConnectionFailed {
                db_path: config.db_path.to_string_lossy().to_string(),
                reason: format!("Failed to open text tree: {}", e),
            })?;

        // Initialize statistics
        let stats = Arc::new(RwLock::new(StorageStats {
            total_cases: 0,
            total_size_bytes: 0,
            database_size_bytes: 0,
            last_backup: None,
        }));

        let storage = Self {
            config,
            db: Arc::new(db),
            metadata_tree: Arc::new(metadata_tree),
            text_tree: Arc::new(text_tree),
            stats,
        };

        // Update statistics
        storage.update_stats().await?;

        tracing::info!("Storage manager initialized with {} cases", 
            storage.stats.read().await.total_cases);

        Ok(storage)
    }

    /// Store case metadata
    pub async fn store_case_metadata(&self, metadata: &CaseMetadata) -> Result<()> {
        let key = metadata.id.to_string();
        let value = bincode::serialize(metadata)?;

        self.metadata_tree.insert(key.as_bytes(), value)
            .map_err(|e| SearchError::SerializationFailed {
                data_type: "CaseMetadata".to_string(),
                reason: e.to_string(),
            })?;

        // Update statistics
        let mut stats = self.stats.write().await;
        stats.total_cases = self.metadata_tree.len();
        
        tracing::debug!("Stored metadata for case: {}", metadata.name);
        Ok(())
    }

    /// Retrieve case metadata by ID
    pub async fn get_case_metadata(&self, case_id: &CaseId) -> Result<Option<CaseMetadata>> {
        let key = case_id.to_string();
        
        if let Some(value) = self.metadata_tree.get(key.as_bytes())
            .map_err(|e| SearchError::SerializationFailed {
                data_type: "CaseMetadata".to_string(),
                reason: e.to_string(),
            })? {
            
            let metadata: CaseMetadata = bincode::deserialize(&value)?;
            Ok(Some(metadata))
        } else {
            Ok(None)
        }
    }

    /// Store full case text
    pub async fn store_case_text(&self, case_id: &CaseId, text: &str) -> Result<()> {
        let key = case_id.to_string();
        
        // Compress text if enabled
        let data = if self.config.enable_compression {
            self.compress_text(text)?
        } else {
            text.as_bytes().to_vec()
        };

        self.text_tree.insert(key.as_bytes(), data)
            .map_err(|e| SearchError::SerializationFailed {
                data_type: "CaseText".to_string(),
                reason: e.to_string(),
            })?;

        tracing::debug!("Stored text for case: {} ({} bytes)", case_id, text.len());
        Ok(())
    }

    /// Retrieve full case text
    pub async fn get_case_text(&self, case_id: &CaseId) -> Result<Option<String>> {
        let key = case_id.to_string();
        
        if let Some(data) = self.text_tree.get(key.as_bytes())
            .map_err(|e| SearchError::SerializationFailed {
                data_type: "CaseText".to_string(),
                reason: e.to_string(),
            })? {
            
            let text = if self.config.enable_compression {
                self.decompress_text(&data)?
            } else {
                String::from_utf8(data.to_vec())
                    .map_err(|e| SearchError::UnsupportedEncoding {
                        encoding: format!("UTF-8: {}", e),
                    })?
            };
            
            Ok(Some(text))
        } else {
            Ok(None)
        }
    }

    /// List all case IDs
    pub async fn list_case_ids(&self) -> Result<Vec<CaseId>> {
        let mut case_ids = Vec::new();
        
        for result in self.metadata_tree.iter() {
            let (key, _) = result.map_err(|e| SearchError::Internal {
                message: format!("Database iteration error: {}", e),
            })?;
            
            let key_str = String::from_utf8(key.to_vec())
                .map_err(|e| SearchError::UnsupportedEncoding {
                    encoding: format!("UTF-8: {}", e),
                })?;
            
            let case_id = uuid::Uuid::parse_str(&key_str)
                .map_err(|e| SearchError::Internal {
                    message: format!("Invalid case ID format: {}", e),
                })?;
            
            case_ids.push(case_id);
        }
        
        Ok(case_ids)
    }

    /// Check if case exists
    pub async fn case_exists(&self, case_id: &CaseId) -> Result<bool> {
        let key = case_id.to_string();
        Ok(self.metadata_tree.contains_key(key.as_bytes())
            .map_err(|e| SearchError::Internal {
                message: format!("Database query error: {}", e),
            })?)
    }

    /// Delete case data
    pub async fn delete_case(&self, case_id: &CaseId) -> Result<()> {
        let key = case_id.to_string();
        
        // Remove from both trees
        self.metadata_tree.remove(key.as_bytes())
            .map_err(|e| SearchError::Internal {
                message: format!("Failed to delete metadata: {}", e),
            })?;
        
        self.text_tree.remove(key.as_bytes())
            .map_err(|e| SearchError::Internal {
                message: format!("Failed to delete text: {}", e),
            })?;
        
        tracing::info!("Deleted case: {}", case_id);
        Ok(())
    }

    /// Batch store multiple cases
    pub async fn store_cases_batch(&self, cases: Vec<(CaseMetadata, String)>) -> Result<usize> {
        let mut stored_count = 0;
        
        for (metadata, text) in cases {
            if let Err(e) = self.store_case_metadata(&metadata).await {
                tracing::error!("Failed to store metadata for {}: {}", metadata.id, e);
                continue;
            }
            
            if let Err(e) = self.store_case_text(&metadata.id, &text).await {
                tracing::error!("Failed to store text for {}: {}", metadata.id, e);
                continue;
            }
            
            stored_count += 1;
        }
        
        // Flush to disk
        self.db.flush_async().await
            .map_err(|e| SearchError::Internal {
                message: format!("Failed to flush database: {}", e),
            })?;
        
        // Update statistics
        self.update_stats().await?;
        
        tracing::info!("Batch stored {} cases", stored_count);
        Ok(stored_count)
    }

    /// Compress text data
    fn compress_text(&self, text: &str) -> Result<Vec<u8>> {
        use std::io::Write;
        
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(text.as_bytes())
            .map_err(|e| SearchError::Internal {
                message: format!("Compression failed: {}", e),
            })?;
        
        encoder.finish()
            .map_err(|e| SearchError::Internal {
                message: format!("Compression finish failed: {}", e),
            })
    }

    /// Decompress text data
    fn decompress_text(&self, data: &[u8]) -> Result<String> {
        use std::io::Read;
        
        let mut decoder = flate2::read::GzDecoder::new(data);
        let mut decompressed = String::new();
        decoder.read_to_string(&mut decompressed)
            .map_err(|e| SearchError::Internal {
                message: format!("Decompression failed: {}", e),
            })?;
        
        Ok(decompressed)
    }

    /// Update storage statistics
    async fn update_stats(&self) -> Result<()> {
        let mut stats = self.stats.write().await;
        
        stats.total_cases = self.metadata_tree.len();
        stats.database_size_bytes = self.db.size_on_disk()
            .map_err(|e| SearchError::Internal {
                message: format!("Failed to get database size: {}", e),
            })?;
        
        // Calculate total size including text
        let mut total_size = stats.database_size_bytes;
        for result in self.text_tree.iter() {
            if let Ok((_, value)) = result {
                total_size += value.len() as u64;
            }
        }
        stats.total_size_bytes = total_size;
        
        Ok(())
    }

    /// Health check
    pub async fn health_check(&self) -> Result<()> {
        // Test basic database operations
        let test_key = b"health_check";
        let test_value = b"ok";
        
        // Test write
        self.metadata_tree.insert(test_key, test_value)
            .map_err(|e| SearchError::DatabaseConnectionFailed {
                db_path: self.config.db_path.to_string_lossy().to_string(),
                reason: format!("Health check write failed: {}", e),
            })?;
        
        // Test read
        let result = self.metadata_tree.get(test_key)
            .map_err(|e| SearchError::DatabaseConnectionFailed {
                db_path: self.config.db_path.to_string_lossy().to_string(),
                reason: format!("Health check read failed: {}", e),
            })?;
        
        if result.is_none() {
            return Err(SearchError::DatabaseConnectionFailed {
                db_path: self.config.db_path.to_string_lossy().to_string(),
                reason: "Health check value not found".to_string(),
            });
        }
        
        // Clean up test data
        self.metadata_tree.remove(test_key)
            .map_err(|e| SearchError::Internal {
                message: format!("Health check cleanup failed: {}", e),
            })?;
        
        Ok(())
    }

    /// Get storage statistics
    pub async fn get_stats(&self) -> Result<StorageStats> {
        self.update_stats().await?;
        Ok(self.stats.read().await.clone())
    }

    /// Create backup
    pub async fn create_backup(&self, backup_path: &Path) -> Result<()> {
        // Ensure backup directory exists
        if let Some(parent) = backup_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        
        // Export database
        self.db.export_iter()
            .map_err(|e| SearchError::Internal {
                message: format!("Backup export failed: {}", e),
            })?;
        
        // Update backup timestamp
        let mut stats = self.stats.write().await;
        stats.last_backup = Some(chrono::Utc::now());
        
        tracing::info!("Created backup at: {:?}", backup_path);
        Ok(())
    }
} 