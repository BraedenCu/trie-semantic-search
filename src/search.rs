//! # Search Engine Module
//!
//! ## Purpose
//! Main search engine that combines trie-based lexical search with semantic vector search
//! to provide comprehensive, fast, and accurate legal case retrieval.
//!
//! ## Input/Output Specification
//! - **Input**: Search queries (text), search configuration, filters
//! - **Output**: Ranked search results with metadata and snippets
//! - **Hybrid Strategy**: Combines exact matches with semantic similarity
//!
//! ## Key Features
//! - Hybrid search combining trie and vector indices
//! - Intelligent query routing and optimization
//! - Result ranking and deduplication
//! - Query caching and performance optimization
//! - Configurable search behavior

use crate::config::{Config, SearchEngineConfig};
use crate::errors::{Result, SearchError};
use crate::storage::StorageManager;
use crate::trie::{TrieIndex, TrieSearchResult};
use crate::vector::{VectorIndex, VectorSearchResult};
use crate::{CaseId, CaseMetadata, DocRef, SearchConfig};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Main search engine
pub struct SearchEngine {
    config: Arc<Config>,
    trie_index: Arc<RwLock<TrieIndex>>,
    vector_index: Arc<RwLock<VectorIndex>>,
    storage: Arc<StorageManager>,
    query_cache: Arc<RwLock<QueryCache>>,
}

/// Search query with parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchQuery {
    /// Query text
    pub query: String,
    /// Maximum number of results
    pub max_results: Option<usize>,
    /// Court filter
    pub court_filter: Option<Vec<String>>,
    /// Date range filter
    pub date_range: Option<(chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>)>,
    /// Search configuration
    pub config: SearchConfig,
}

/// Search result with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// Case metadata
    pub case_metadata: CaseMetadata,
    /// Relevance score (0.0 to 1.0)
    pub score: f32,
    /// Match type (exact, semantic, etc.)
    pub match_type: MatchType,
    /// Text snippet showing the match
    pub snippet: String,
    /// Highlighted query terms in snippet
    pub highlights: Vec<TextHighlight>,
}

/// Type of match found
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MatchType {
    /// Exact text match from trie
    Exact,
    /// Prefix match from trie
    Prefix,
    /// Semantic similarity match
    Semantic,
    /// Case name match
    CaseName,
    /// Citation match
    Citation,
}

/// Text highlighting information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextHighlight {
    /// Start position in snippet
    pub start: usize,
    /// End position in snippet
    pub end: usize,
    /// Highlight type
    pub highlight_type: HighlightType,
}

/// Type of text highlight
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HighlightType {
    ExactMatch,
    SemanticMatch,
    CaseName,
    Citation,
}

/// Query cache for performance optimization
struct QueryCache {
    cache: HashMap<String, CachedResult>,
    max_size: usize,
}

/// Cached search result
#[derive(Debug, Clone)]
struct CachedResult {
    results: Vec<SearchResult>,
    timestamp: chrono::DateTime<chrono::Utc>,
    ttl_seconds: u64,
}

impl SearchEngine {
    /// Create new search engine
    pub async fn new(
        config: Arc<Config>,
        storage: Arc<StorageManager>,
    ) -> Result<Self> {
        // Initialize trie index
        let trie_index = Arc::new(RwLock::new(
            TrieIndex::new(config.trie.clone()).await?
        ));

        // Initialize vector index
        let vector_index = Arc::new(RwLock::new(
            VectorIndex::new(config.vector.clone()).await?
        ));

        // Initialize query cache
        let query_cache = Arc::new(RwLock::new(
            QueryCache::new(config.search.query_cache_size)
        ));

        Ok(Self {
            config,
            trie_index,
            vector_index,
            storage,
            query_cache,
        })
    }

    /// Perform search with the given query
    pub async fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        let search_query = SearchQuery {
            query: query.to_string(),
            max_results: Some(self.config.search.default_max_results),
            court_filter: None,
            date_range: None,
            config: SearchConfig::default(),
        };

        self.search_with_params(search_query).await
    }

    /// Perform search with detailed parameters
    pub async fn search_with_params(&self, query: SearchQuery) -> Result<Vec<SearchResult>> {
        // Check cache first
        if self.config.search.enable_query_cache {
            if let Some(cached) = self.get_cached_result(&query.query).await? {
                return Ok(cached);
            }
        }

        // Validate query
        self.validate_query(&query)?;

        // Execute hybrid search
        let results = self.execute_hybrid_search(&query).await?;

        // Cache results
        if self.config.search.enable_query_cache {
            self.cache_results(&query.query, &results).await?;
        }

        Ok(results)
    }

    /// Execute hybrid search combining trie and vector search
    async fn execute_hybrid_search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>> {
        let mut all_results = Vec::new();
        let mut seen_cases = HashSet::new();

        // 1. Trie search for exact matches
        if query.config.enable_prefix {
            let trie_results = self.search_trie(&query.query).await?;
            for trie_result in trie_results.exact_matches {
                if let Some(case_metadata) = self.storage.get_case_metadata(&trie_result.case_id).await? {
                    if seen_cases.insert(trie_result.case_id) {
                        let search_result = SearchResult {
                            case_metadata,
                            score: query.config.exact_match_weight,
                            match_type: MatchType::Exact,
                            snippet: self.generate_snippet(&trie_result, &query.query).await?,
                            highlights: Vec::new(), // TODO: Generate highlights
                        };
                        all_results.push(search_result);
                    }
                }
            }
        }

        // 2. Vector search for semantic matches
        if query.config.enable_semantic && all_results.len() < query.config.max_results {
            let vector_results = self.search_vector(&query.query).await?;
            for vector_result in vector_results {
                if vector_result.similarity_score >= query.config.min_similarity {
                    if let Some(case_metadata) = self.storage.get_case_metadata(&vector_result.doc_ref.case_id).await? {
                        if seen_cases.insert(vector_result.doc_ref.case_id) {
                            let search_result = SearchResult {
                                case_metadata,
                                score: vector_result.similarity_score,
                                match_type: MatchType::Semantic,
                                snippet: self.generate_snippet(&vector_result.doc_ref, &query.query).await?,
                                highlights: Vec::new(), // TODO: Generate highlights
                            };
                            all_results.push(search_result);
                        }
                    }
                }
            }
        }

        // 3. Sort by score and apply filters
        all_results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        // Apply filters
        all_results = self.apply_filters(all_results, query).await?;

        // Limit results
        let max_results = query.max_results.unwrap_or(query.config.max_results);
        all_results.truncate(max_results);

        Ok(all_results)
    }

    /// Search trie index
    async fn search_trie(&self, query: &str) -> Result<TrieSearchResult> {
        let trie = self.trie_index.read().await;
        trie.search(query)
    }

    /// Search vector index
    async fn search_vector(&self, query: &str) -> Result<Vec<VectorSearchResult>> {
        let mut vector = self.vector_index.write().await;
        vector.search(query, 50).await // Get top 50 from vector search
    }

    /// Apply filters to search results
    async fn apply_filters(
        &self,
        mut results: Vec<SearchResult>,
        query: &SearchQuery,
    ) -> Result<Vec<SearchResult>> {
        // Court filter
        if let Some(court_filter) = &query.court_filter {
            results.retain(|result| court_filter.contains(&result.case_metadata.court));
        }

        // Date range filter
        if let Some((start_date, end_date)) = &query.date_range {
            results.retain(|result| {
                result.case_metadata.decision_date >= *start_date
                    && result.case_metadata.decision_date <= *end_date
            });
        }

        Ok(results)
    }

    /// Generate text snippet for search result
    async fn generate_snippet(&self, doc_ref: &DocRef, query: &str) -> Result<String> {
        // TODO: Generate intelligent snippet with context
        // For now, return placeholder
        Ok(format!("Snippet for case {} paragraph {}", doc_ref.case_id, doc_ref.paragraph_index))
    }

    /// Validate search query
    fn validate_query(&self, query: &SearchQuery) -> Result<()> {
        if query.query.len() < self.config.search.min_query_length {
            return Err(SearchError::InvalidSearchQuery {
                query: query.query.clone(),
                reason: format!("Query too short: minimum {} characters", self.config.search.min_query_length),
            });
        }

        if query.query.len() > self.config.search.max_query_length {
            return Err(SearchError::InvalidSearchQuery {
                query: query.query.clone(),
                reason: format!("Query too long: maximum {} characters", self.config.search.max_query_length),
            });
        }

        Ok(())
    }

    /// Get cached search result
    async fn get_cached_result(&self, query: &str) -> Result<Option<Vec<SearchResult>>> {
        let cache = self.query_cache.read().await;
        Ok(cache.get(query))
    }

    /// Cache search results
    async fn cache_results(&self, query: &str, results: &[SearchResult]) -> Result<()> {
        let mut cache = self.query_cache.write().await;
        cache.insert(
            query.to_string(),
            results.to_vec(),
            self.config.search.query_cache_ttl_seconds,
        );
        Ok(())
    }

    /// Health check for search engine
    pub async fn health_check(&self) -> Result<()> {
        // Check if indices are loaded
        let _trie = self.trie_index.read().await;
        let _vector = self.vector_index.read().await;
        
        // Check storage connectivity
        self.storage.health_check().await?;
        
        Ok(())
    }

    /// Get search engine statistics
    pub async fn get_stats(&self) -> SearchEngineStats {
        let vector = self.vector_index.read().await;
        let cache = self.query_cache.read().await;
        
        SearchEngineStats {
            total_cases_indexed: 0, // TODO: Get from storage
            vector_index_stats: vector.get_stats(),
            cache_stats: cache.get_stats(),
        }
    }
}

impl QueryCache {
    fn new(max_size: usize) -> Self {
        Self {
            cache: HashMap::new(),
            max_size,
        }
    }

    fn get(&self, query: &str) -> Option<Vec<SearchResult>> {
        if let Some(cached) = self.cache.get(query) {
            let now = chrono::Utc::now();
            let age = now.timestamp() - cached.timestamp.timestamp();
            
            if age < cached.ttl_seconds as i64 {
                return Some(cached.results.clone());
            }
        }
        None
    }

    fn insert(&mut self, query: String, results: Vec<SearchResult>, ttl_seconds: u64) {
        if self.cache.len() >= self.max_size {
            // Simple eviction: remove oldest entry
            if let Some(oldest_key) = self.cache.keys().next().cloned() {
                self.cache.remove(&oldest_key);
            }
        }

        self.cache.insert(query, CachedResult {
            results,
            timestamp: chrono::Utc::now(),
            ttl_seconds,
        });
    }

    fn get_stats(&self) -> CacheStats {
        CacheStats {
            size: self.cache.len(),
            max_size: self.max_size,
        }
    }
}

/// Search engine statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchEngineStats {
    pub total_cases_indexed: usize,
    pub vector_index_stats: crate::vector::VectorIndexStats,
    pub cache_stats: CacheStats,
}

/// Cache statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    pub size: usize,
    pub max_size: usize,
} 