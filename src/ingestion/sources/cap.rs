//! # CAP (Caselaw Access Project) Data Source
//!
//! ## Purpose
//! Interfaces with Harvard Law School's Caselaw Access Project API to fetch
//! legal case data. Provides authenticated access to case metadata and full text.
//!
//! ## Input/Output Specification
//! - **Input**: API credentials, jurisdiction filters, date ranges, pagination params
//! - **Output**: Structured case metadata and full text content
//! - **Rate Limits**: Respects API rate limits and implements backoff strategies
//!
//! ## Key Features
//! - Authenticated API access with token management
//! - Jurisdiction and date filtering
//! - Automatic pagination handling
//! - Rate limiting and retry logic
//! - Incremental updates support

use super::{DataSource, SourceConfig, SourceStats, SourceHealth, SourceInfo, RateLimits};
// CapConfig is defined locally in this module
use crate::errors::{Result, SearchError};
use crate::{CaseId, CaseMetadata, Jurisdiction};
use crate::search::SearchQuery;
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use reqwest::{Client, Response, StatusCode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration, Instant};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Simple rate limiter
struct RateLimiter {
    requests_per_minute: u32,
    last_request_time: Option<Instant>,
}

impl RateLimiter {
    fn new(requests_per_minute: u32) -> Self {
        Self {
            requests_per_minute,
            last_request_time: None,
        }
    }

    async fn enforce(&mut self) -> Result<()> {
        if let Some(last_time) = self.last_request_time {
            let min_interval = Duration::from_secs(60) / self.requests_per_minute;
            let elapsed = last_time.elapsed();
            
            if elapsed < min_interval {
                let sleep_duration = min_interval - elapsed;
                sleep(sleep_duration).await;
            }
        }
        
        self.last_request_time = Some(Instant::now());
        Ok(())
    }
}

/// CAP API configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapConfig {
    /// API base URL
    pub base_url: String,
    /// API authentication token
    pub api_token: String,
    /// Jurisdictions to fetch (empty = all)
    pub jurisdictions: Vec<String>,
    /// Start date for case filtering
    pub start_date: Option<DateTime<Utc>>,
    /// End date for case filtering
    pub end_date: Option<DateTime<Utc>>,
    /// Maximum cases per request
    pub page_size: usize,
    /// Request timeout in seconds
    pub timeout_seconds: u64,
    /// Rate limit: requests per minute
    pub rate_limit_rpm: usize,
    /// Whether to fetch full text (requires authentication)
    pub fetch_full_text: bool,
}

impl Default for CapConfig {
    fn default() -> Self {
        Self {
            base_url: "https://api.case.law/v1".to_string(),
            api_token: String::new(),
            jurisdictions: Vec::new(),
            start_date: None,
            end_date: None,
            page_size: 100,
            timeout_seconds: 30,
            rate_limit_rpm: 1000,
            fetch_full_text: true,
        }
    }
}

/// CAP data source implementation
pub struct CapDataSource {
    config: CapConfig,
    client: Client,
    stats: Arc<RwLock<SourceStats>>,
    rate_limiter: Arc<RwLock<RateLimiter>>,
}

/// CAP API response for cases list
#[derive(Debug, Deserialize)]
struct CapCasesResponse {
    count: usize,
    next: Option<String>,
    previous: Option<String>,
    results: Vec<CapCase>,
}

/// CAP case data structure
#[derive(Debug, Deserialize)]
struct CapCase {
    id: u64,
    url: String,
    name: String,
    name_abbreviation: String,
    decision_date: String,
    docket_number: Option<String>,
    first_page: Option<String>,
    last_page: Option<String>,
    citations: Vec<CapCitation>,
    volume: Option<CapVolume>,
    reporter: Option<CapReporter>,
    court: CapCourt,
    jurisdiction: CapJurisdiction,
    casebody: Option<CapCasebody>,
    analysis: Option<CapAnalysis>,
}

#[derive(Debug, Deserialize)]
struct CapCitation {
    cite: String,
    #[serde(rename = "type")]
    citation_type: String,
}

#[derive(Debug, Deserialize)]
struct CapVolume {
    url: String,
    volume_number: String,
    barcode: String,
}

#[derive(Debug, Deserialize)]
struct CapReporter {
    url: String,
    full_name: String,
    short_name: String,
}

#[derive(Debug, Deserialize)]
struct CapCourt {
    url: String,
    name: String,
    name_abbreviation: String,
    slug: String,
}

#[derive(Debug, Deserialize)]
struct CapJurisdiction {
    url: String,
    name: String,
    name_long: String,
    slug: String,
}

#[derive(Debug, Deserialize)]
struct CapCasebody {
    status: String,
    data: Option<CapCasebodyData>,
}

#[derive(Debug, Deserialize)]
struct CapCasebodyData {
    head_matter: Option<String>,
    opinions: Vec<CapOpinion>,
    attorneys: Vec<String>,
    parties: Vec<String>,
    judges: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CapOpinion {
    #[serde(rename = "type")]
    opinion_type: String,
    author: Option<String>,
    text: String,
}

#[derive(Debug, Deserialize)]
struct CapAnalysis {
    cardinality: Option<usize>,
    char_count: Option<usize>,
    ocr_confidence: Option<f64>,
    pagerank: Option<CapPagerank>,
    sha256: Option<String>,
    simhash: Option<String>,
    word_count: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct CapPagerank {
    raw: Option<f64>,
    percentile: Option<f64>,
}

impl CapDataSource {
    /// Create new CAP data source
    pub fn new(config: CapConfig) -> Result<Self> {
        // Validate configuration
        if config.api_token.is_empty() && config.fetch_full_text {
            return Err(SearchError::Config {
                message: "API token required for full text access".to_string(),
            });
        }

        // Build HTTP client
        let mut headers = reqwest::header::HeaderMap::new();
        if !config.api_token.is_empty() {
            headers.insert(
                reqwest::header::AUTHORIZATION,
                format!("Token {}", config.api_token).parse()
                    .map_err(|e| SearchError::Config {
                        message: format!("Invalid API token format: {}", e),
                    })?,
            );
        }

        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .default_headers(headers)
            .user_agent("trie-semantic-search/1.0")
            .build()
            .map_err(|e| SearchError::NetworkError {
                details: e.to_string(),
            })?;

        let stats = Arc::new(RwLock::new(SourceStats {
            source_name: "CAP".to_string(),
            total_available: None,
            downloaded: 0,
            processed: 0,
            download_errors: 0,
            processing_errors: 0,
            last_update: None,
        }));

        Ok(Self {
            config,
            client,
            stats,
            rate_limiter: Arc::new(RwLock::new(RateLimiter::new(config.rate_limit_rpm as u32))),
        })
    }

    /// Fetch cases with pagination
    async fn fetch_cases_page(&self, url: Option<String>) -> Result<CapCasesResponse> {
        // Rate limiting
        self.enforce_rate_limit().await?;

        let request_url = url.unwrap_or_else(|| {
            let mut base_url = format!("{}/cases/", self.config.base_url);
            let mut params = Vec::new();

            // Add jurisdiction filter
            if !self.config.jurisdictions.is_empty() {
                params.push(format!("jurisdiction={}", self.config.jurisdictions.join(",")));
            }

            // Add date filters
            if let Some(start_date) = self.config.start_date {
                params.push(format!("decision_date_min={}", start_date.format("%Y-%m-%d")));
            }
            if let Some(end_date) = self.config.end_date {
                params.push(format!("decision_date_max={}", end_date.format("%Y-%m-%d")));
            }

            // Add pagination
            params.push(format!("page_size={}", self.config.page_size));

            // Add full text option
            if self.config.fetch_full_text {
                params.push("full_case=true".to_string());
            }

            if !params.is_empty() {
                base_url.push('?');
                base_url.push_str(&params.join("&"));
            }

            base_url
        });

        tracing::debug!("Fetching CAP cases from: {}", request_url);

        let response = self.client
            .get(&request_url)
            .send()
            .await
            .map_err(|e| SearchError::NetworkError {
                operation: "CAP API request".to_string(),
                details: e.to_string(),
            })?;

        // Handle rate limiting
        if response.status() == StatusCode::TOO_MANY_REQUESTS {
            let mut stats = self.stats.write().await;
            stats.rate_limit_hits += 1;
            
            tracing::warn!("CAP API rate limit hit, backing off");
            sleep(Duration::from_secs(60)).await;
            
            return Err(SearchError::RateLimitExceeded {
                source: "CAP API".to_string(),
                retry_after_seconds: Some(60),
            });
        }

        // Check for other HTTP errors
        if !response.status().is_success() {
            return Err(SearchError::NetworkError {
                details: format!("HTTP {}: {}", response.status(), 
                    response.text().await.unwrap_or_default()),
            });
        }

        let cases_response: CapCasesResponse = response.json().await
            .map_err(|e| SearchError::DataParsing {
                source: "CAP API".to_string(),
                details: e.to_string(),
            })?;

        Ok(cases_response)
    }

    /// Convert CAP case to our internal format
    fn convert_cap_case(&self, cap_case: CapCase) -> Result<(CaseMetadata, String)> {
        // Generate UUID for case ID
        let case_id = uuid::Uuid::new_v4();

        // Parse decision date
        let decision_date = chrono::NaiveDate::parse_from_str(&cap_case.decision_date, "%Y-%m-%d")
            .map_err(|e| SearchError::DataParsing {
                source: "CAP decision_date".to_string(),
                details: e.to_string(),
            })?;

        // Extract citations
        let citations: Vec<String> = cap_case.citations
            .into_iter()
            .map(|c| c.cite)
            .collect();

        // Determine jurisdiction
        let jurisdiction = match cap_case.jurisdiction.slug.as_str() {
            "us" => Jurisdiction::Federal,
            slug if slug.len() == 2 => {
                // State jurisdiction - convert slug to state name
                Jurisdiction::State(slug.to_uppercase())
            }
            _ => Jurisdiction::Federal, // Default fallback
        };

        // Create metadata
        let metadata = CaseMetadata {
            id: case_id,
            name: cap_case.name.clone(),
            court: cap_case.court.name,
            jurisdiction,
            decision_date,
            citations,
            docket_number: cap_case.docket_number,
            judges: cap_case.casebody
                .as_ref()
                .and_then(|cb| cb.data.as_ref())
                .map(|data| data.judges.clone())
                .unwrap_or_default(),
            source_url: Some(cap_case.url),
            word_count: cap_case.analysis
                .as_ref()
                .and_then(|a| a.word_count)
                .unwrap_or(0),
            ingestion_date: Utc::now(),
        };

        // Extract full text
        let full_text = if let Some(casebody) = cap_case.casebody {
            if let Some(data) = casebody.data {
                let mut text_parts = Vec::new();

                // Add head matter
                if let Some(head_matter) = data.head_matter {
                    text_parts.push(head_matter);
                }

                // Add parties
                if !data.parties.is_empty() {
                    text_parts.push(format!("PARTIES: {}", data.parties.join("; ")));
                }

                // Add attorneys
                if !data.attorneys.is_empty() {
                    text_parts.push(format!("ATTORNEYS: {}", data.attorneys.join("; ")));
                }

                // Add opinions
                for opinion in data.opinions {
                    let mut opinion_text = format!("OPINION ({})", opinion.opinion_type.to_uppercase());
                    if let Some(author) = opinion.author {
                        opinion_text.push_str(&format!(" by {}", author));
                    }
                    opinion_text.push_str(":\n\n");
                    opinion_text.push_str(&opinion.text);
                    text_parts.push(opinion_text);
                }

                text_parts.join("\n\n")
            } else {
                format!("Case: {}\nCourt: {}\nDate: {}", 
                    cap_case.name, cap_case.court.name, cap_case.decision_date)
            }
        } else {
            format!("Case: {}\nCourt: {}\nDate: {}", 
                cap_case.name, cap_case.court.name, cap_case.decision_date)
        };

        Ok((metadata, full_text))
    }

    /// Enforce rate limiting
    async fn enforce_rate_limit(&self) -> Result<()> {
        self.rate_limiter.write().await.enforce()?;
        Ok(())
    }
}

#[async_trait]
impl DataSource for CapDataSource {
    fn name(&self) -> &str {
        "CAP"
    }

    fn description(&self) -> &str {
        "Caselaw Access Project - Harvard Law School's digital repository of court cases"
    }

    async fn health_check(&self) -> Result<SourceHealth> {
        let start_time = Instant::now();
        
                 let response = self.client
             .get(&format!("{}cases/", self.config.api_url))
             .query(&[("limit", "1")])
            .send()
            .await;

        let response_time_ms = start_time.elapsed().as_millis() as u64;

        match response {
            Ok(resp) if resp.status().is_success() => {
                Ok(SourceHealth {
                    is_healthy: true,
                    last_check: Utc::now(),
                    response_time_ms,
                    error_message: None,
                })
            }
            Ok(resp) => {
                let error_msg = format!("HTTP {}: {}", resp.status(), 
                    resp.text().await.unwrap_or_else(|_| "Unknown error".to_string()));
                Ok(SourceHealth {
                    is_healthy: false,
                    last_check: Utc::now(),
                    response_time_ms,
                    error_message: Some(error_msg),
                })
            }
            Err(e) => {
                Ok(SourceHealth {
                    is_healthy: false,
                    last_check: Utc::now(),
                    response_time_ms,
                    error_message: Some(e.to_string()),
                })
            }
        }
    }

    async fn list_available_cases(&self) -> Result<Vec<String>> {
        // Implementation would fetch case IDs from CAP API
        // For now, return empty list
        Ok(vec![])
    }

    async fn list_updated_cases(&self, _since: Option<DateTime<Utc>>) -> Result<Vec<String>> {
        // Implementation would fetch recently updated cases
        Ok(vec![])
    }

         async fn fetch_case(&self, case_id: &str) -> Result<CaseMetadata> {
         let url = format!("{}cases/{}/", self.config.api_url, case_id);
        
        let response = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| SearchError::NetworkError {
                details: e.to_string(),
            })?;

        if !response.status().is_success() {
            return Err(SearchError::NetworkError {
                details: format!("HTTP {}: {}", response.status(), 
                    response.text().await.unwrap_or_default()),
            });
        }

        let cap_case: CapCase = response.json().await
            .map_err(|e| SearchError::DataParsing {
                source: "CAP API".to_string(),
                details: format!("Failed to parse case JSON: {}", e),
            })?;

        self.convert_cap_case(cap_case).map(|(metadata, _)| metadata)
    }

    async fn fetch_cases(&self, case_ids: &[String]) -> Result<Vec<Result<CaseMetadata>>> {
        let mut results = Vec::new();
        for case_id in case_ids {
            results.push(self.fetch_case(case_id).await);
        }
        Ok(results)
    }

    async fn search_cases(&self, _query: &SearchQuery) -> Result<Vec<String>> {
        // Implementation would search CAP API
        Ok(vec![])
    }

    async fn get_source_info(&self) -> Result<SourceInfo> {
                 Ok(SourceInfo {
             name: "CAP".to_string(),
             description: "Caselaw Access Project".to_string(),
             base_url: self.config.api_url.clone(),
             version: "v1".to_string(),
             rate_limits: self.get_rate_limits(),
         })
    }

    fn get_rate_limits(&self) -> RateLimits {
        RateLimits {
            requests_per_minute: self.config.rate_limit_rpm,
            requests_per_hour: self.config.rate_limit_rpm * 60,
            concurrent_requests: 10,
        }
    }

    async fn get_stats(&self) -> Result<SourceStats> {
        Ok(self.stats.read().await.clone())
    }

    fn get_source_config(&self) -> SourceConfig {
        SourceConfig {
            name: "CAP".to_string(),
            enabled: true,
            priority: 1,
            rate_limit_rpm: self.config.rate_limit_rpm,
            timeout_seconds: self.config.timeout_seconds,
            retry_attempts: 3,
        }
    }
} 