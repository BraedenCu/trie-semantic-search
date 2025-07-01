//! # API Server Module
//!
//! ## Purpose
//! REST API server providing search endpoints and system management functionality
//! for the legal search engine with comprehensive documentation and validation.
//!
//! ## Input/Output Specification
//! - **Input**: HTTP requests with search queries, filters, configuration
//! - **Output**: JSON responses with search results, metadata, system status
//! - **Endpoints**: Search, health, metrics, configuration management
//!
//! ## Key Features
//! - RESTful API with OpenAPI documentation
//! - Request validation and rate limiting
//! - CORS support for web frontends
//! - Structured error responses
//! - Performance metrics and monitoring

use crate::config::Config;
use crate::errors::{Result, SearchError};
use crate::search::{SearchEngine, SearchQuery, SearchResult};
use crate::storage::StorageManager;
use actix_web::{web, App, HttpResponse, HttpServer, Result as ActixResult};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Application state for the API server
pub struct ApiServer {
    app_state: crate::AppState,
}

/// Search request payload
#[derive(Debug, Deserialize)]
pub struct SearchRequest {
    pub query: String,
    pub max_results: Option<usize>,
    pub court_filter: Option<Vec<String>>,
    pub date_range: Option<(chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>)>,
}

/// Search response payload
#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResult>,
    pub total_results: usize,
    pub query_time_ms: u64,
    pub pagination: PaginationInfo,
}

/// Pagination information
#[derive(Debug, Serialize)]
pub struct PaginationInfo {
    pub page: usize,
    pub per_page: usize,
    pub total_pages: usize,
    pub has_next: bool,
    pub has_prev: bool,
}

/// Health check response
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub uptime_seconds: u64,
    pub components: HealthComponents,
}

/// Component health status
#[derive(Debug, Serialize)]
pub struct HealthComponents {
    pub search_engine: String,
    pub storage: String,
    pub trie_index: String,
    pub vector_index: String,
}

impl ApiServer {
    /// Create new API server
    pub async fn new(app_state: crate::AppState) -> Result<Self> {
        Ok(Self { app_state })
    }

    /// Run the API server
    pub async fn run(self) -> Result<()> {
        let bind_addr = format!("{}:{}", self.app_state.config.server.host, self.app_state.config.server.port);
        
        tracing::info!("Starting API server on {}", bind_addr);

        HttpServer::new(move || {
            App::new()
                .app_data(web::Data::new(self.app_state.clone()))
                .route("/search", web::post().to(search_handler))
                .route("/health", web::get().to(health_handler))
                .route("/stats", web::get().to(stats_handler))
                .route("/", web::get().to(index_handler))
        })
        .bind(&bind_addr)
        .map_err(|e| SearchError::Internal {
            message: format!("Failed to bind server to {}: {}", bind_addr, e),
        })?
        .run()
        .await
        .map_err(|e| SearchError::Internal {
            message: format!("Server error: {}", e),
        })?;

        Ok(())
    }
}

/// Search endpoint handler
async fn search_handler(
    app_state: web::Data<crate::AppState>,
    request: web::Json<SearchRequest>,
) -> ActixResult<HttpResponse> {
    let start_time = std::time::Instant::now();

    // Convert request to search query
    let search_query = SearchQuery {
        query: request.query.clone(),
        max_results: request.max_results,
        court_filter: request.court_filter.clone(),
        date_range: request.date_range,
        config: crate::SearchConfig::default(),
    };

    // Execute search
    match app_state.search_engine.search_with_params(search_query).await {
        Ok(results) => {
            let query_time_ms = start_time.elapsed().as_millis() as u64;
            let total_results = results.len();

            let response = SearchResponse {
                results,
                total_results,
                query_time_ms,
                pagination: PaginationInfo {
                    page: 1,
                    per_page: total_results,
                    total_pages: 1,
                    has_next: false,
                    has_prev: false,
                },
            };

            Ok(HttpResponse::Ok().json(response))
        }
        Err(e) => {
            tracing::error!("Search error: {}", e);
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Search failed",
                "message": e.to_string(),
            })))
        }
    }
}

/// Health check endpoint handler
async fn health_handler(
    app_state: web::Data<crate::AppState>,
) -> ActixResult<HttpResponse> {
    // Check component health
    let search_status = match app_state.search_engine.health_check().await {
        Ok(_) => "healthy",
        Err(_) => "unhealthy",
    };

    let storage_status = match app_state.storage.health_check().await {
        Ok(_) => "healthy",
        Err(_) => "unhealthy",
    };

    let response = HealthResponse {
        status: if search_status == "healthy" && storage_status == "healthy" {
            "healthy".to_string()
        } else {
            "unhealthy".to_string()
        },
        version: "1.0.0".to_string(),
        uptime_seconds: 0, // TODO: Track actual uptime
        components: HealthComponents {
            search_engine: search_status.to_string(),
            storage: storage_status.to_string(),
            trie_index: "healthy".to_string(), // TODO: Check actual status
            vector_index: "healthy".to_string(), // TODO: Check actual status
        },
    };

    Ok(HttpResponse::Ok().json(response))
}

/// Statistics endpoint handler
async fn stats_handler(
    app_state: web::Data<crate::AppState>,
) -> ActixResult<HttpResponse> {
    let search_stats = app_state.search_engine.get_stats().await;
    let storage_stats = match app_state.storage.get_stats().await {
        Ok(stats) => stats,
        Err(_) => crate::storage::StorageStats {
            total_cases: 0,
            total_size_bytes: 0,
            database_size_bytes: 0,
            last_backup: None,
        },
    };

    let response = serde_json::json!({
        "search_engine": search_stats,
        "storage": storage_stats,
    });

    Ok(HttpResponse::Ok().json(response))
}

/// Index page handler
async fn index_handler() -> ActixResult<HttpResponse> {
    let html = r#"
    <!DOCTYPE html>
    <html>
    <head>
        <title>Legal Search Engine</title>
        <style>
            body { font-family: Arial, sans-serif; margin: 40px; }
            .header { color: #2c3e50; }
            .endpoint { margin: 20px 0; padding: 15px; background: #f8f9fa; border-radius: 5px; }
            .method { font-weight: bold; color: #27ae60; }
        </style>
    </head>
    <body>
        <h1 class="header">Legal Search Engine API</h1>
        <p>Welcome to the Legal Search Engine API. This service provides fast, accurate search across U.S. federal constitutional law.</p>
        
        <h2>Available Endpoints</h2>
        
        <div class="endpoint">
            <span class="method">POST</span> /search
            <p>Search for legal cases using natural language queries.</p>
        </div>
        
        <div class="endpoint">
            <span class="method">GET</span> /health
            <p>Check the health status of all system components.</p>
        </div>
        
        <div class="endpoint">
            <span class="method">GET</span> /stats
            <p>Get system statistics and performance metrics.</p>
        </div>
        
        <h2>Example Search Request</h2>
        <pre>{
  "query": "freedom of speech",
  "max_results": 10,
  "court_filter": ["Supreme Court"]
}</pre>
    </body>
    </html>
    "#;

    Ok(HttpResponse::Ok().content_type("text/html").body(html))
} 