//! # Data Sources Module
//!
//! ## Purpose
//! Defines the common interface for legal data sources and provides implementations
//! for specific sources like CAP (Harvard) and CourtListener (Free Law Project).
//!
//! ## Input/Output Specification
//! - **Input**: API credentials, query parameters, date ranges
//! - **Output**: Standardized legal case metadata and full text
//! - **Sources**: CAP, CourtListener, extensible for future sources
//!
//! ## Key Features
//! - Unified interface for multiple data sources
//! - Automatic authentication and session management
//! - Rate limiting and quota tracking
//! - Bulk download and incremental update support
//! - Error handling and retry logic
//! - Data format normalization
//!
//! ## Architecture
//! - `DataSource` trait: Common interface for all sources
//! - `cap.rs`: Caselaw Access Project implementation
//! - `courtlistener.rs`: CourtListener implementation
//! - Future sources can be added by implementing the trait
//!
//! ## Usage
//! ```rust
//! use crate::ingestion::sources::{DataSource, cap::CapSource};
//!
//! let source = CapSource::new(config).await?;
//! let cases = source.list_available_cases().await?;
//! let case_data = source.fetch_case(&cases[0]).await?;
//! ```

pub mod cap;
pub mod courtlistener;

use crate::errors::{Result, SearchError};
use crate::search::SearchQuery;
use crate::{CaseMetadata};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Health status of a data source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceHealth {
    pub is_healthy: bool,
    pub last_check: DateTime<Utc>,
    pub response_time_ms: u64,
    pub error_message: Option<String>,
}

/// Information about a data source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceInfo {
    pub name: String,
    pub description: String,
    pub base_url: String,
    pub version: String,
    pub rate_limits: RateLimits,
}

/// Rate limiting information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimits {
    pub requests_per_minute: u32,
    pub requests_per_hour: u32,
    pub concurrent_requests: u32,
}

/// Statistics for a data source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceStats {
    pub source_name: String,
    pub total_available: Option<usize>,
    pub downloaded: usize,
    pub processed: usize,
    pub download_errors: usize,
    pub processing_errors: usize,
    pub last_update: Option<DateTime<Utc>>,
}

/// Trait for legal data sources
#[async_trait]
pub trait DataSource {
    /// Get the name of this data source
    fn name(&self) -> &str;

    /// Get a description of this data source
    fn description(&self) -> &str;

    /// Check the health status of the data source
    async fn health_check(&self) -> Result<SourceHealth>;

    /// List all available case IDs
    async fn list_available_cases(&self) -> Result<Vec<String>>;

    /// List cases updated since a given timestamp
    async fn list_updated_cases(&self, since: Option<DateTime<Utc>>) -> Result<Vec<String>>;

    /// Fetch a specific case by ID
    async fn fetch_case(&self, case_id: &str) -> Result<CaseMetadata>;

    /// Fetch multiple cases by ID
    async fn fetch_cases(&self, case_ids: &[String]) -> Result<Vec<Result<CaseMetadata>>>;

    /// Search for cases matching a query
    async fn search_cases(&self, query: &SearchQuery) -> Result<Vec<String>>;

    /// Get source information
    async fn get_source_info(&self) -> Result<SourceInfo>;

    /// Get rate limiting information
    fn get_rate_limits(&self) -> RateLimits;

    /// Get source statistics
    async fn get_stats(&self) -> Result<SourceStats>;

    /// Get source configuration
    fn get_source_config(&self) -> SourceConfig;
}

/// Configuration for a data source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceConfig {
    pub name: String,
    pub enabled: bool,
    pub priority: u32,
    pub rate_limit_rpm: u32,
    pub timeout_seconds: u64,
    pub retry_attempts: u32,
}

impl Default for SearchQuery {
    fn default() -> Self {
        Self {
            query: None,
            court: None,
            date_range: None,
            judge: None,
            case_type: None,
            limit: Some(100),
            offset: Some(0),
        }
    }
}

impl Default for RateLimits {
    fn default() -> Self {
        Self {
            requests_per_minute: 0,
            requests_per_hour: 0,
            concurrent_requests: 0,
        }
    }
} 