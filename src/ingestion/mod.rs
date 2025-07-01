//! # Data Ingestion Module
//!
//! ## Purpose
//! Handles the ingestion of legal case data from multiple authoritative sources including
//! the Caselaw Access Project (CAP) and CourtListener. Provides robust data pipeline
//! with error handling, rate limiting, and incremental updates.
//!
//! ## Input/Output Specification
//! - **Input**: API endpoints, bulk data files, configuration parameters
//! - **Output**: Structured legal case records with metadata and full text
//! - **Data Sources**: CAP (Harvard), CourtListener (Free Law Project)
//! - **Formats**: JSON, XML, compressed archives
//!
//! ## Key Features
//! - Multi-source data ingestion with unified interface
//! - Concurrent downloads with configurable limits
//! - Automatic retry with exponential backoff
//! - Rate limiting and API quota management
//! - Incremental updates and change detection
//! - Data validation and quality checks
//! - Progress tracking and resumable downloads
//!
//! ## Architecture
//! - `sources/`: Individual data source implementations
//! - `pipeline/`: Data processing pipeline components
//! - `validation/`: Data quality and format validation
//! - `cache/`: Local caching and storage management
//!
//! ## Usage
//! ```rust
//! use crate::ingestion::{IngestionManager, IngestionConfig};
//!
//! let config = IngestionConfig::default();
//! let manager = IngestionManager::new(config).await?;
//! 
//! // Bulk ingestion
//! manager.ingest_bulk().await?;
//! 
//! // Incremental updates
//! manager.check_for_updates().await?;
//! ```

pub mod sources;
pub mod pipeline;
pub mod validation;
pub mod cache;

use crate::config::IngestionConfig;
use crate::errors::{Result, SearchError};
use crate::{CaseId, CaseMetadata};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{Mutex, Semaphore};
use uuid::Uuid;

pub use sources::{cap::CapDataSource, courtlistener::CourtListenerSource, DataSource};
pub use pipeline::{IngestionPipeline, PipelineStats};
pub use validation::{CaseValidator, ValidationResult};

/// Main ingestion manager coordinating all data sources and processing
pub struct IngestionManager {
    config: IngestionConfig,
    sources: Vec<Box<dyn DataSource + Send + Sync>>,
    pipeline: IngestionPipeline,
    validator: CaseValidator,
    semaphore: Arc<Semaphore>,
    cache: cache::CacheManager,
}

/// Ingestion statistics and progress tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestionStats {
    /// Total cases processed
    pub total_processed: usize,
    /// Successfully ingested cases
    pub successful: usize,
    /// Failed cases with errors
    pub failed: usize,
    /// Skipped cases (duplicates, filtered out)
    pub skipped: usize,
    /// Processing start time
    pub start_time: DateTime<Utc>,
    /// Processing end time (if completed)
    pub end_time: Option<DateTime<Utc>>,
    /// Current processing rate (cases per second)
    pub processing_rate: f64,
    /// Estimated time remaining
    pub eta_seconds: Option<u64>,
    /// Per-source statistics
    pub source_stats: HashMap<String, SourceStats>,
}

/// Statistics for individual data sources
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceStats {
    /// Source identifier
    pub source_name: String,
    /// Total records available
    pub total_available: Option<usize>,
    /// Records downloaded
    pub downloaded: usize,
    /// Records processed successfully
    pub processed: usize,
    /// Download errors
    pub download_errors: usize,
    /// Processing errors
    pub processing_errors: usize,
    /// Last successful update
    pub last_update: Option<DateTime<Utc>>,
}

/// Ingestion job configuration and state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestionJob {
    /// Unique job identifier
    pub id: Uuid,
    /// Job type (bulk, incremental, specific source)
    pub job_type: IngestionJobType,
    /// Job status
    pub status: IngestionJobStatus,
    /// Job configuration
    pub config: IngestionJobConfig,
    /// Processing statistics
    pub stats: IngestionStats,
    /// Error messages
    pub errors: Vec<String>,
    /// Created timestamp
    pub created_at: DateTime<Utc>,
    /// Started timestamp
    pub started_at: Option<DateTime<Utc>>,
    /// Completed timestamp
    pub completed_at: Option<DateTime<Utc>>,
}

/// Types of ingestion jobs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IngestionJobType {
    /// Full bulk ingestion from all sources
    BulkAll,
    /// Bulk ingestion from specific source
    BulkSource(String),
    /// Incremental updates from all sources
    IncrementalAll,
    /// Incremental updates from specific source
    IncrementalSource(String),
    /// Reprocess existing data
    Reprocess,
}

/// Job execution status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IngestionJobStatus {
    /// Job is queued for execution
    Queued,
    /// Job is currently running
    Running,
    /// Job completed successfully
    Completed,
    /// Job failed with errors
    Failed,
    /// Job was cancelled
    Cancelled,
    /// Job is paused
    Paused,
}

/// Configuration for specific ingestion job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestionJobConfig {
    /// Sources to process
    pub sources: Vec<String>,
    /// Date range filter (optional)
    pub date_range: Option<(DateTime<Utc>, DateTime<Utc>)>,
    /// Court filter (optional)
    pub court_filter: Option<Vec<String>>,
    /// Maximum cases to process (optional)
    pub max_cases: Option<usize>,
    /// Enable parallel processing
    pub parallel_processing: bool,
    /// Batch size for processing
    pub batch_size: usize,
}

impl IngestionManager {
    /// Create new ingestion manager with configuration
    pub async fn new(config: IngestionConfig) -> Result<Self> {
        let semaphore = Arc::new(Semaphore::new(config.max_concurrent_downloads));
        
        // Initialize data sources
        let mut sources: Vec<Box<dyn DataSource + Send + Sync>> = Vec::new();
        
        // Add CAP source
        let cap_source = sources::cap::CapDataSource::new(config.cap.clone())?;
        sources.push(Box::new(cap_source));
        
        // Add CourtListener source
        let cl_source = sources::courtlistener::CourtListenerSource::new(config.courtlistener.clone()).await?;
        sources.push(Box::new(cl_source));
        
        // Initialize pipeline
        let pipeline = IngestionPipeline::new(config.clone()).await?;
        
        // Initialize validator
        let validator = CaseValidator::new()?;
        
        // Initialize cache manager
        let cache = cache::CacheManager::new(&config).await?;
        
        Ok(Self {
            config,
            sources,
            pipeline,
            validator,
            semaphore,
            cache,
        })
    }
    
    /// Start bulk ingestion from all configured sources
    pub async fn ingest_bulk(&self) -> Result<IngestionJob> {
        let job_config = IngestionJobConfig {
            sources: self.sources.iter().map(|s| s.name().to_string()).collect(),
            date_range: None,
            court_filter: None,
            max_cases: None,
            parallel_processing: true,
            batch_size: self.config.batch_size,
        };
        
        let job = IngestionJob {
            id: Uuid::new_v4(),
            job_type: IngestionJobType::BulkAll,
            status: IngestionJobStatus::Queued,
            config: job_config,
            stats: IngestionStats::new(),
            errors: Vec::new(),
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
        };
        
        self.execute_job(job).await
    }
    
    /// Check for and process incremental updates
    pub async fn check_for_updates(&self) -> Result<IngestionJob> {
        let job_config = IngestionJobConfig {
            sources: self.sources.iter().map(|s| s.name().to_string()).collect(),
            date_range: None,
            court_filter: None,
            max_cases: None,
            parallel_processing: true,
            batch_size: self.config.batch_size,
        };
        
        let job = IngestionJob {
            id: Uuid::new_v4(),
            job_type: IngestionJobType::IncrementalAll,
            status: IngestionJobStatus::Queued,
            config: job_config,
            stats: IngestionStats::new(),
            errors: Vec::new(),
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
        };
        
        self.execute_job(job).await
    }
    
    /// Execute a specific ingestion job
    async fn execute_job(&self, mut job: IngestionJob) -> Result<IngestionJob> {
        job.status = IngestionJobStatus::Running;
        job.started_at = Some(Utc::now());
        
        tracing::info!("Starting ingestion job: {:?}", job.job_type);
        
        let result = match job.job_type {
            IngestionJobType::BulkAll | IngestionJobType::BulkSource(_) => {
                self.execute_bulk_ingestion(&mut job).await
            }
            IngestionJobType::IncrementalAll | IngestionJobType::IncrementalSource(_) => {
                self.execute_incremental_ingestion(&mut job).await
            }
            IngestionJobType::Reprocess => {
                self.execute_reprocessing(&mut job).await
            }
        };
        
        match result {
            Ok(_) => {
                job.status = IngestionJobStatus::Completed;
                job.completed_at = Some(Utc::now());
                tracing::info!("Ingestion job completed successfully: {}", job.id);
            }
            Err(e) => {
                job.status = IngestionJobStatus::Failed;
                job.completed_at = Some(Utc::now());
                job.errors.push(e.to_string());
                tracing::error!("Ingestion job failed: {} - {}", job.id, e);
            }
        }
        
        Ok(job)
    }
    
    /// Execute bulk data ingestion
    async fn execute_bulk_ingestion(&self, job: &mut IngestionJob) -> Result<()> {
        for source in &self.sources {
            if job.config.sources.contains(&source.name().to_string()) {
                tracing::info!("Processing bulk data from source: {}", source.name());
                
                let source_stats = self.process_source_bulk(source.as_ref(), job).await?;
                job.stats.source_stats.insert(source.name().to_string(), source_stats);
            }
        }
        
        Ok(())
    }
    
    /// Execute incremental data ingestion
    async fn execute_incremental_ingestion(&self, job: &mut IngestionJob) -> Result<()> {
        for source in &self.sources {
            if job.config.sources.contains(&source.name().to_string()) {
                tracing::info!("Processing incremental updates from source: {}", source.name());
                
                let source_stats = self.process_source_incremental(source.as_ref(), job).await?;
                job.stats.source_stats.insert(source.name().to_string(), source_stats);
            }
        }
        
        Ok(())
    }
    
    /// Execute data reprocessing
    async fn execute_reprocessing(&self, _job: &mut IngestionJob) -> Result<()> {
        // TODO: Implement reprocessing logic
        Err(SearchError::NotSupported {
            operation: "Data reprocessing".to_string(),
        })
    }
    
    /// Process bulk data from a specific source
    async fn process_source_bulk(&self, source: &dyn DataSource, job: &mut IngestionJob) -> Result<SourceStats> {
        let mut stats = SourceStats {
            source_name: source.name().to_string(),
            total_available: None,
            downloaded: 0,
            processed: 0,
            download_errors: 0,
            processing_errors: 0,
            last_update: None,
        };
        
        // Get available case IDs from source
        let case_ids = source.list_available_cases().await?;
        stats.total_available = Some(case_ids.len());
        
        // Process cases in batches
        for batch in case_ids.chunks(job.config.batch_size) {
            let batch_results = self.process_case_batch(source, batch, &mut stats).await?;
            
            // Update job statistics
            job.stats.total_processed += batch_results.len();
            job.stats.successful += batch_results.iter().filter(|r| r.is_ok()).count();
            job.stats.failed += batch_results.iter().filter(|r| r.is_err()).count();
        }
        
        stats.last_update = Some(Utc::now());
        Ok(stats)
    }
    
    /// Process incremental updates from a specific source
    async fn process_source_incremental(&self, source: &dyn DataSource, job: &mut IngestionJob) -> Result<SourceStats> {
        let mut stats = SourceStats {
            source_name: source.name().to_string(),
            total_available: None,
            downloaded: 0,
            processed: 0,
            download_errors: 0,
            processing_errors: 0,
            last_update: None,
        };
        
        // Get last update timestamp for this source
        let last_update = self.cache.get_last_update_time(source.name()).await?;
        
        // Get cases updated since last update
        let updated_cases = source.list_updated_cases(last_update).await?;
        stats.total_available = Some(updated_cases.len());
        
        if updated_cases.is_empty() {
            tracing::info!("No updates available from source: {}", source.name());
            return Ok(stats);
        }
        
        // Process updated cases
        for batch in updated_cases.chunks(job.config.batch_size) {
            let batch_results = self.process_case_batch(source, batch, &mut stats).await?;
            
            // Update job statistics
            job.stats.total_processed += batch_results.len();
            job.stats.successful += batch_results.iter().filter(|r| r.is_ok()).count();
            job.stats.failed += batch_results.iter().filter(|r| r.is_err()).count();
        }
        
        // Update last update timestamp
        self.cache.set_last_update_time(source.name(), Utc::now()).await?;
        stats.last_update = Some(Utc::now());
        
        Ok(stats)
    }
    
    /// Process a batch of cases from a data source
    async fn process_case_batch(
        &self,
        source: &dyn DataSource,
        case_ids: &[String],
        stats: &mut SourceStats,
    ) -> Result<Vec<Result<CaseMetadata>>> {
        let mut results = Vec::new();
        
        // Create futures for concurrent processing
        let mut futures = Vec::new();
        
        for case_id in case_ids {
            let permit = self.semaphore.clone().acquire_owned().await.unwrap();
            let case_id = case_id.clone();
            let source_name = source.name().to_string();
            
            let future = async move {
                let _permit = permit; // Hold permit for duration of operation
                
                // Download case data
                let case_data = match source.fetch_case(&case_id).await {
                    Ok(data) => {
                        stats.downloaded += 1;
                        data
                    }
                    Err(e) => {
                        stats.download_errors += 1;
                        tracing::warn!("Failed to download case {} from {}: {}", case_id, source_name, e);
                        return Err(e);
                    }
                };
                
                // Validate case data
                match self.validator.validate(&case_data) {
                    Ok(_) => {
                        stats.processed += 1;
                        Ok(case_data)
                    }
                    Err(e) => {
                        stats.processing_errors += 1;
                        tracing::warn!("Case validation failed for {}: {}", case_id, e);
                        Err(e)
                    }
                }
            };
            
            futures.push(future);
        }
        
        // Execute all futures concurrently
        let batch_results = futures::future::join_all(futures).await;
        results.extend(batch_results);
        
        Ok(results)
    }
    
    /// Get current ingestion statistics
    pub fn get_stats(&self) -> IngestionStats {
        // TODO: Implement real-time statistics collection
        IngestionStats::new()
    }
}

impl IngestionStats {
    fn new() -> Self {
        Self {
            total_processed: 0,
            successful: 0,
            failed: 0,
            skipped: 0,
            start_time: Utc::now(),
            end_time: None,
            processing_rate: 0.0,
            eta_seconds: None,
            source_stats: HashMap::new(),
        }
    }
} 