//! # Data Ingestion Pipeline
//!
//! ## Purpose
//! Orchestrates the complete data ingestion workflow from source fetching
//! through text processing to final storage and indexing.
//!
//! ## Input/Output Specification
//! - **Input**: Data source configurations, processing parameters, storage targets
//! - **Output**: Processed case data stored in database and search indices
//! - **Workflow**: Fetch → Validate → Process → Store → Index
//!
//! ## Key Features
//! - Multi-source data ingestion with parallel processing
//! - Configurable batch processing with memory management
//! - Error handling and recovery with detailed logging
//! - Progress tracking and performance metrics
//! - Incremental updates and deduplication

use crate::config::{IngestionConfig, TextProcessingConfig};
use crate::errors::{Result, SearchError};
use crate::ingestion::cache::CacheManager;
use crate::ingestion::sources::{DataSource, SourceStats};
use crate::ingestion::validation::CaseValidator as DataValidator;
use crate::storage::StorageManager;
use crate::text_processing::{ProcessedText, TextProcessor};
use crate::{CaseId, CaseMetadata};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Semaphore};
use tokio::time::sleep;

/// Main ingestion pipeline
pub struct IngestionPipeline {
    config: IngestionConfig,
    storage: Arc<StorageManager>,
    text_processor: Arc<TextProcessor>,
    validator: Arc<DataValidator>,
    cache_manager: Arc<CacheManager>,
    stats: Arc<RwLock<PipelineStats>>,
    processing_semaphore: Arc<Semaphore>,
}

/// Pipeline execution statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStats {
    /// Total cases processed
    pub total_processed: usize,
    /// Successfully stored cases
    pub successful_stores: usize,
    /// Failed processing attempts
    pub failed_processing: usize,
    /// Validation failures
    pub validation_failures: usize,
    /// Duplicate cases skipped
    pub duplicates_skipped: usize,
    /// Processing rate (cases per second)
    pub processing_rate: f64,
    /// Start time of current run
    pub start_time: Option<chrono::DateTime<chrono::Utc>>,
    /// End time of current run
    pub end_time: Option<chrono::DateTime<chrono::Utc>>,
    /// Memory usage statistics
    pub memory_stats: MemoryStats,
    /// Source-specific statistics
    pub source_stats: HashMap<String, SourceStats>,
}

/// Memory usage tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStats {
    /// Peak memory usage in MB
    pub peak_memory_mb: f64,
    /// Current memory usage in MB
    pub current_memory_mb: f64,
    /// Number of garbage collections triggered
    pub gc_count: usize,
}

/// Processing job for a batch of cases
#[derive(Debug)]
struct ProcessingJob {
    cases: Vec<(CaseMetadata, String)>,
    batch_id: usize,
    source_name: String,
}

/// Processing result for a batch
#[derive(Debug)]
struct ProcessingResult {
    batch_id: usize,
    successful_count: usize,
    failed_count: usize,
    processing_time: Duration,
    errors: Vec<String>,
}

impl Default for PipelineStats {
    fn default() -> Self {
        Self {
            total_processed: 0,
            successful_stores: 0,
            failed_processing: 0,
            validation_failures: 0,
            duplicates_skipped: 0,
            processing_rate: 0.0,
            start_time: None,
            end_time: None,
            memory_stats: MemoryStats {
                peak_memory_mb: 0.0,
                current_memory_mb: 0.0,
                gc_count: 0,
            },
            source_stats: HashMap::new(),
        }
    }
}

impl IngestionPipeline {
    /// Create new ingestion pipeline
    pub async fn new(
        config: IngestionConfig,
        storage: Arc<StorageManager>,
        text_processing_config: TextProcessingConfig,
    ) -> Result<Self> {
        let text_processor = Arc::new(TextProcessor::new(text_processing_config)?);
        let validator = Arc::new(DataValidator::new(config.validation.clone()));
        let cache_manager = Arc::new(CacheManager::new(config.cache.clone()).await?);
        
        let stats = Arc::new(RwLock::new(PipelineStats::default()));
        let processing_semaphore = Arc::new(Semaphore::new(config.max_concurrent_jobs));

        Ok(Self {
            config,
            storage,
            text_processor,
            validator,
            cache_manager,
            stats,
            processing_semaphore,
        })
    }

    /// Run full ingestion pipeline with a data source
    pub async fn run_ingestion<T: DataSource + Send + Sync>(
        &self,
        mut data_source: T,
        limit: Option<usize>,
    ) -> Result<PipelineStats> {
        tracing::info!("Starting ingestion pipeline");
        let start_time = Instant::now();

        // Initialize statistics
        {
            let mut stats = self.stats.write().await;
            *stats = PipelineStats::default();
            stats.start_time = Some(chrono::Utc::now());
        }

        // Health check on data source
        data_source.health_check().await.map_err(|e| {
            tracing::error!("Data source health check failed: {}", e);
            e
        })?;

        // Fetch cases from data source
        tracing::info!("Fetching cases from data source");
        let cases = data_source.fetch_cases(limit).await?;
        tracing::info!("Fetched {} cases from source", cases.len());

        if cases.is_empty() {
            tracing::warn!("No cases fetched from data source");
            return Ok(self.stats.read().await.clone());
        }

        // Update source statistics
        {
            let mut stats = self.stats.write().await;
            let source_config = data_source.get_source_config();
            let source_stats = data_source.get_stats().await?;
            stats.source_stats.insert(source_config.name.clone(), source_stats);
        }

        // Process cases in batches
        let batch_size = self.config.batch_size;
        let total_batches = (cases.len() + batch_size - 1) / batch_size;
        
        tracing::info!("Processing {} cases in {} batches of size {}", 
            cases.len(), total_batches, batch_size);

        let mut batch_results = Vec::new();
        
        for (batch_id, batch) in cases.chunks(batch_size).enumerate() {
            let job = ProcessingJob {
                cases: batch.to_vec(),
                batch_id,
                source_name: data_source.get_source_config().name.clone(),
            };

            let result = self.process_batch(job).await?;
            batch_results.push(result);

            // Update progress
            self.update_progress_stats(&batch_results).await;

            // Memory management
            if batch_id % 10 == 0 {
                self.check_memory_usage().await?;
            }

            // Rate limiting between batches
            if self.config.rate_limit_delay_ms > 0 {
                sleep(Duration::from_millis(self.config.rate_limit_delay_ms)).await;
            }
        }

        // Finalize statistics
        {
            let mut stats = self.stats.write().await;
            stats.end_time = Some(chrono::Utc::now());
            
            let total_time = start_time.elapsed();
            if total_time.as_secs() > 0 {
                stats.processing_rate = stats.total_processed as f64 / total_time.as_secs_f64();
            }
        }

        let final_stats = self.stats.read().await.clone();
        tracing::info!(
            "Ingestion completed: {} processed, {} stored, {} failed in {:.2}s (rate: {:.1} cases/sec)",
            final_stats.total_processed,
            final_stats.successful_stores,
            final_stats.failed_processing,
            start_time.elapsed().as_secs_f64(),
            final_stats.processing_rate
        );

        Ok(final_stats)
    }

    /// Process a batch of cases
    async fn process_batch(&self, job: ProcessingJob) -> Result<ProcessingResult> {
        let _permit = self.processing_semaphore.acquire().await.unwrap();
        let batch_start = Instant::now();
        
        tracing::debug!("Processing batch {} with {} cases", job.batch_id, job.cases.len());

        let mut successful_count = 0;
        let mut failed_count = 0;
        let mut errors = Vec::new();

        for (metadata, raw_text) in job.cases {
            match self.process_single_case(metadata, raw_text).await {
                Ok(processed) => {
                    if processed {
                        successful_count += 1;
                    }
                }
                Err(e) => {
                    failed_count += 1;
                    errors.push(e.to_string());
                    tracing::error!("Failed to process case: {}", e);
                }
            }
        }

        let processing_time = batch_start.elapsed();
        
        tracing::debug!(
            "Batch {} completed: {} successful, {} failed in {:.2}s",
            job.batch_id, successful_count, failed_count, processing_time.as_secs_f64()
        );

        Ok(ProcessingResult {
            batch_id: job.batch_id,
            successful_count,
            failed_count,
            processing_time,
            errors,
        })
    }

    /// Process a single case through the complete pipeline
    async fn process_single_case(&self, metadata: CaseMetadata, raw_text: String) -> Result<bool> {
        // Check for duplicates
        if self.storage.case_exists(&metadata.id).await? {
            let mut stats = self.stats.write().await;
            stats.duplicates_skipped += 1;
            return Ok(false);
        }

        // Check cache
        if let Some(cached_result) = self.cache_manager.get_processed_case(&metadata.id).await? {
            tracing::debug!("Using cached result for case: {}", metadata.id);
            self.storage.store_case_metadata(&cached_result.metadata).await?;
            self.storage.store_case_text(&metadata.id, &cached_result.processed_text.normalized).await?;
            return Ok(true);
        }

        // Validate input data
        if let Err(validation_error) = self.validator.validate_case(&metadata, &raw_text).await {
            tracing::warn!("Case validation failed: {}", validation_error);
            let mut stats = self.stats.write().await;
            stats.validation_failures += 1;
            return Err(validation_error);
        }

        // Process text
        let processed_text = self.text_processor.process_text(&raw_text).await?;

        // Create enhanced metadata with processing results
        let enhanced_metadata = self.enhance_metadata(metadata, &processed_text)?;

        // Store in database
        self.storage.store_case_metadata(&enhanced_metadata).await?;
        self.storage.store_case_text(&enhanced_metadata.id, &processed_text.normalized).await?;

        // Cache the result
        let cache_entry = CachedProcessingResult {
            metadata: enhanced_metadata.clone(),
            processed_text: processed_text.clone(),
            processing_timestamp: chrono::Utc::now(),
        };
        
        if let Err(e) = self.cache_manager.store_processed_case(&enhanced_metadata.id, &cache_entry).await {
            tracing::warn!("Failed to cache processing result: {}", e);
            // Don't fail the entire operation for cache errors
        }

        tracing::debug!("Successfully processed case: {}", enhanced_metadata.name);
        Ok(true)
    }

    /// Enhance metadata with processing results
    fn enhance_metadata(&self, mut metadata: CaseMetadata, processed_text: &ProcessedText) -> Result<CaseMetadata> {
        // Update word count from processed text
        metadata.word_count = processed_text.stats.word_count;

        // Extract additional metadata from processed text
        if let Some(first_citation) = processed_text.citations.first() {
            if metadata.citations.is_empty() {
                metadata.citations = processed_text.citations
                    .iter()
                    .map(|c| c.full_text.clone())
                    .collect();
            }
        }

        // Extract judges if not already present
        if metadata.judges.is_empty() {
            metadata.judges = processed_text.entities
                .iter()
                .filter(|e| matches!(e.entity_type, crate::text_processing::EntityType::Judge))
                .map(|e| e.text.clone())
                .collect();
        }

        Ok(metadata)
    }

    /// Update progress statistics
    async fn update_progress_stats(&self, batch_results: &[ProcessingResult]) {
        let mut stats = self.stats.write().await;
        
        // Reset counters and recalculate from batch results
        stats.total_processed = 0;
        stats.successful_stores = 0;
        stats.failed_processing = 0;

        for result in batch_results {
            stats.total_processed += result.successful_count + result.failed_count;
            stats.successful_stores += result.successful_count;
            stats.failed_processing += result.failed_count;
        }

        // Update processing rate
        if let Some(start_time) = stats.start_time {
            let elapsed = chrono::Utc::now() - start_time;
            let elapsed_secs = elapsed.num_seconds() as f64;
            if elapsed_secs > 0.0 {
                stats.processing_rate = stats.total_processed as f64 / elapsed_secs;
            }
        }
    }

    /// Check memory usage and trigger cleanup if needed
    async fn check_memory_usage(&self) -> Result<()> {
        // Get current memory usage (simplified - would use proper memory monitoring)
        let current_memory = self.get_memory_usage_mb();
        
        {
            let mut stats = self.stats.write().await;
            stats.memory_stats.current_memory_mb = current_memory;
            if current_memory > stats.memory_stats.peak_memory_mb {
                stats.memory_stats.peak_memory_mb = current_memory;
            }
        }

        // Trigger cleanup if memory usage is high
        if current_memory > self.config.max_memory_usage_mb as f64 {
            tracing::warn!("High memory usage detected: {:.1} MB", current_memory);
            
            // Clear caches
            self.cache_manager.clear_memory_cache().await?;
            
            // Force garbage collection (in a real implementation)
            {
                let mut stats = self.stats.write().await;
                stats.memory_stats.gc_count += 1;
            }
            
            tracing::info!("Memory cleanup completed");
        }

        Ok(())
    }

    /// Get current memory usage in MB (simplified implementation)
    fn get_memory_usage_mb(&self) -> f64 {
        // This is a placeholder - in a real implementation, you'd use
        // system APIs or crates like `sysinfo` to get actual memory usage
        100.0 // Dummy value
    }

    /// Get current pipeline statistics
    pub async fn get_stats(&self) -> PipelineStats {
        self.stats.read().await.clone()
    }

    /// Reset pipeline statistics
    pub async fn reset_stats(&self) {
        let mut stats = self.stats.write().await;
        *stats = PipelineStats::default();
    }

    /// Health check for the pipeline
    pub async fn health_check(&self) -> Result<()> {
        // Check storage health
        self.storage.health_check().await?;

        // Check cache health
        self.cache_manager.health_check().await?;

        // Check memory usage
        let current_memory = self.get_memory_usage_mb();
        if current_memory > self.config.max_memory_usage_mb as f64 * 0.9 {
            return Err(SearchError::Internal {
                message: format!("Memory usage too high: {:.1} MB", current_memory),
            });
        }

        tracing::debug!("Pipeline health check passed");
        Ok(())
    }

    /// Graceful shutdown of the pipeline
    pub async fn shutdown(&self) -> Result<()> {
        tracing::info!("Shutting down ingestion pipeline");

        // Wait for all processing jobs to complete
        let permits_needed = self.processing_semaphore.available_permits();
        let _permits = self.processing_semaphore.acquire_many(permits_needed as u32).await.unwrap();

        // Flush cache
        self.cache_manager.flush().await?;

        // Final statistics
        let final_stats = self.stats.read().await.clone();
        tracing::info!(
            "Pipeline shutdown completed. Final stats: {} processed, {} stored",
            final_stats.total_processed,
            final_stats.successful_stores
        );

        Ok(())
    }
}

/// Cached processing result
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedProcessingResult {
    metadata: CaseMetadata,
    processed_text: ProcessedText,
    processing_timestamp: chrono::DateTime<chrono::Utc>,
} 