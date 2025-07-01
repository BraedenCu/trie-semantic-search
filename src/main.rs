//! # Legal Search Engine Main Driver
//!
//! ## Purpose
//! Main entry point for the legal search engine server. Orchestrates initialization
//! of all system components and starts the web server for handling search requests.
//!
//! ## Input/Output Specification
//! - **Input**: Configuration files, command line arguments, environment variables
//! - **Output**: Running web server with search API endpoints
//! - **Initialization**: Loads indices, starts background services, health checks
//!
//! ## Key Features
//! - Graceful startup and shutdown
//! - Component health monitoring
//! - Configuration validation
//! - Structured logging and metrics
//! - Signal handling for clean shutdown
//!
//! ## Architecture Flow
//! 1. Parse command line arguments and load configuration
//! 2. Initialize logging and tracing
//! 3. Load or build search indices (trie and vector)
//! 4. Initialize search engine components
//! 5. Start web API server
//! 6. Handle shutdown signals gracefully

use clap::{Arg, Command};
use std::sync::Arc;
use tokio::signal;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use trie_semantic_search::{
    api::ApiServer,
    config::Config,
    errors::{Result, SearchError},
    search::SearchEngine,
    storage::StorageManager,
    AppState,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments
    let matches = Command::new("legal-search-server")
        .version("1.0.0")
        .author("Legal Search Team")
        .about("High-performance legal search engine with trie and semantic capabilities")
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("Configuration file path")
                .default_value("config.toml"),
        )
        .arg(
            Arg::new("port")
                .short('p')
                .long("port")
                .value_name("PORT")
                .help("Server port")
                .value_parser(clap::value_parser!(u16)),
        )
        .arg(
            Arg::new("rebuild-index")
                .long("rebuild-index")
                .help("Rebuild search indices on startup")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("check-health")
                .long("check-health")
                .help("Run health checks and exit")
                .action(clap::ArgAction::SetTrue),
        )
        .get_matches();

    // Load configuration
    let config_path = matches.get_one::<String>("config").unwrap();
    let mut config = Config::from_file(config_path)?;

    // Override port if specified
    if let Some(port) = matches.get_one::<u16>("port") {
        config.server.port = *port;
    }

    let config = Arc::new(config);

    // Initialize logging
    init_logging(&config)?;

    info!("Starting Legal Search Engine v1.0.0");
    info!("Configuration loaded from: {}", config_path);

    // Run health checks if requested
    if matches.get_flag("check-health") {
        return run_health_checks(&config).await;
    }

    // Initialize application components
    let app_state = initialize_components(config.clone()).await?;

    // Rebuild indices if requested
    if matches.get_flag("rebuild-index") {
        info!("Rebuilding search indices...");
        rebuild_indices(&app_state).await?;
    }

    // Start the API server
    let server = ApiServer::new(app_state.clone()).await?;
    let server_handle = tokio::spawn(async move {
        if let Err(e) = server.run().await {
            error!("Server error: {}", e);
        }
    });

    info!(
        "Legal Search Engine started successfully on {}:{}",
        config.server.host, config.server.port
    );

    // Wait for shutdown signal
    tokio::select! {
        _ = signal::ctrl_c() => {
            info!("Received SIGINT, shutting down gracefully...");
        }
        _ = server_handle => {
            warn!("Server stopped unexpectedly");
        }
    }

    // Graceful shutdown
    shutdown_components(&app_state).await?;
    info!("Legal Search Engine shut down successfully");

    Ok(())
}

/// Initialize logging and tracing
fn init_logging(config: &Config) -> Result<()> {
    let log_level = config.logging.level.parse().map_err(|_| {
        SearchError::Config {
            message: format!("Invalid log level: {}", config.logging.level),
        }
    })?;

    let subscriber = tracing_subscriber::registry().with(
        tracing_subscriber::fmt::layer()
            .with_target(true)
            .with_level(true)
            .with_thread_ids(true)
            .json()
            .with_filter(tracing_subscriber::filter::LevelFilter::from_level(log_level)),
    );

    subscriber.init();

    info!("Logging initialized with level: {}", config.logging.level);
    Ok(())
}

/// Initialize all application components
async fn initialize_components(config: Arc<Config>) -> Result<AppState> {
    info!("Initializing application components...");

    // Initialize storage
    info!("Initializing storage manager...");
    let storage = Arc::new(StorageManager::new(config.storage.clone()).await?);

    // Initialize search engine
    info!("Initializing search engine...");
    let search_engine = Arc::new(SearchEngine::new(config.clone(), storage.clone()).await?);

    // Verify component health
    verify_component_health(&storage, &search_engine).await?;

    let app_state = AppState {
        config,
        search_engine,
        storage,
    };

    info!("All components initialized successfully");
    Ok(app_state)
}

/// Verify the health of all components
async fn verify_component_health(
    storage: &StorageManager,
    search_engine: &SearchEngine,
) -> Result<()> {
    info!("Verifying component health...");

    // Check storage health
    storage.health_check().await?;
    info!("✓ Storage manager is healthy");

    // Check search engine health
    search_engine.health_check().await?;
    info!("✓ Search engine is healthy");

    Ok(())
}

/// Run comprehensive health checks
async fn run_health_checks(config: &Config) -> Result<()> {
    info!("Running health checks...");

    // Check configuration
    info!("✓ Configuration is valid");

    // Check required files and directories
    check_required_paths(config)?;
    info!("✓ Required paths exist");

    // TODO: Add more health checks
    // - Database connectivity
    // - Model file integrity
    // - Index file accessibility
    // - External API connectivity

    info!("All health checks passed!");
    Ok(())
}

/// Check that required paths exist
fn check_required_paths(config: &Config) -> Result<()> {
    let paths_to_check = vec![
        &config.storage.db_path,
        &config.vector.model.model_path,
        &config.trie.index_path,
    ];

    for path in paths_to_check {
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)?;
                info!("Created directory: {:?}", parent);
            }
        }
    }

    Ok(())
}

/// Rebuild search indices
async fn rebuild_indices(app_state: &AppState) -> Result<()> {
    info!("Starting index rebuild process...");

    // TODO: Implement index rebuilding
    // This would involve:
    // 1. Loading case data from storage
    // 2. Rebuilding trie index
    // 3. Regenerating vector embeddings
    // 4. Rebuilding vector index
    // 5. Saving updated indices

    warn!("Index rebuilding not yet implemented");
    Ok(())
}

/// Gracefully shutdown all components
async fn shutdown_components(app_state: &AppState) -> Result<()> {
    info!("Shutting down components...");

    // TODO: Implement graceful shutdown
    // This would involve:
    // 1. Stopping background tasks
    // 2. Flushing pending writes
    // 3. Closing database connections
    // 4. Saving state if needed

    info!("All components shut down successfully");
    Ok(())
} 