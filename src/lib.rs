//! # Trie-Structured Legal Search Engine
//!
//! ## Overview
//! This library implements a high-performance search engine for U.S. federal constitutional law
//! that combines trie-based lexical search with semantic vector search capabilities.
//!
//! ## Architecture
//! The system is composed of several key modules:
//! - `ingestion`: Data pipeline for legal case ingestion and preprocessing
//! - `text_processing`: Tokenization, normalization, and text analysis
//! - `trie`: Prefix tree implementation for fast lexical search
//! - `vector`: Semantic embedding and vector similarity search
//! - `search`: Hybrid search engine combining trie and vector search
//! - `api`: REST and GraphQL API endpoints
//! - `storage`: Persistent storage and metadata management
//! - `config`: Configuration management and settings
//! - `errors`: Centralized error handling and types
//!
//! ## Input/Output Specification
//! - **Input**: Legal case documents (JSON/XML), search queries (text)
//! - **Output**: Ranked search results with case metadata and snippets
//! - **Performance**: Sub-second query response times, deterministic results
//!
//! ## Usage
//! ```rust,no_run
//! use trie_semantic_search::{SearchEngine, Config};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = Config::from_file("config.toml")?;
//!     let engine = SearchEngine::new(config).await?;
//!     let results = engine.search("freedom of speech").await?;
//!     println!("Found {} results", results.len());
//!     Ok(())
//! }
//! ```

// Core modules
pub mod config;
pub mod errors;
pub mod ingestion;
pub mod text_processing;
pub mod trie;
pub mod vector;
pub mod search;
pub mod storage;
pub mod api;

// Utilities
pub mod utils;

// Re-exports for convenience
pub use config::Config;
pub use errors::{Result, SearchError};
pub use search::{SearchEngine, SearchResult, SearchQuery};

// Core types used throughout the system
use chrono::{DateTime, Utc, NaiveDate};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use std::sync::Arc;

/// Unique identifier for legal cases
pub type CaseId = Uuid;

/// Document reference containing case ID and position information
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DocRef {
    /// Unique case identifier
    pub case_id: CaseId,
    /// Paragraph or section index within the case
    pub paragraph_index: usize,
    /// Character offset within the paragraph (optional)
    pub char_offset: Option<usize>,
}

/// Legal jurisdiction levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Jurisdiction {
    Federal,
    State(String),
    Local(String),
    International,
}

/// Case metadata with all required fields
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseMetadata {
    /// Unique case identifier
    pub id: Uuid,
    /// Case name/title
    pub name: String,
    /// Primary citation
    pub citation: String,
    /// Court that decided the case
    pub court: String,
    /// Decision date
    pub decision_date: NaiveDate,
    /// Judge(s) who decided the case
    pub judges: Vec<String>,
    /// Legal topics/categories
    pub topics: Vec<String>,
    /// Full text of the case
    pub full_text: String,
    /// Legal jurisdiction
    pub jurisdiction: Jurisdiction,
    /// All citations for this case
    pub citations: Vec<String>,
    /// Docket number
    pub docket_number: Option<String>,
    /// Source URL
    pub source_url: Option<String>,
    /// Word count
    pub word_count: usize,
    /// Ingestion timestamp
    pub ingestion_date: DateTime<Utc>,
}

/// Configuration for search behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchConfig {
    /// Maximum number of results to return
    pub max_results: usize,
    /// Minimum similarity score for semantic results
    pub min_similarity: f32,
    /// Weight for exact matches vs semantic matches
    pub exact_match_weight: f32,
    /// Enable/disable semantic search
    pub enable_semantic: bool,
    /// Enable/disable prefix matching
    pub enable_prefix: bool,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            max_results: 10,
            min_similarity: 0.5,
            exact_match_weight: 2.0,
            enable_semantic: true,
            enable_prefix: true,
        }
    }
}

/// Application state shared across components
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<config::Config>,
    pub search_engine: Arc<search::SearchEngine>,
    pub storage: Arc<storage::StorageManager>,
} 