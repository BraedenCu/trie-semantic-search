//! # Ingestion System Demo
//!
//! This example demonstrates the complete ingestion pipeline working with
//! sample legal case data. It shows:
//! - Setting up the storage system
//! - Creating a mock data source with sample cases
//! - Running the text processing pipeline
//! - Storing processed data
//! - Displaying statistics

use std::sync::Arc;
use tokio;
use tracing::{info, Level};
use tracing_subscriber;

use trie_semantic_search::{
    config::{
        IngestionConfig, StorageConfig, TextProcessingConfig, ValidationConfig, CacheConfig,
    },
    errors::Result,
    ingestion::{
        pipeline::IngestionPipeline,
        sources::{DataSource, SourceConfig, SourceStats},
    },
    storage::StorageManager,
    text_processing::TextProcessor,
    CaseMetadata, Jurisdiction,
};

use async_trait::async_trait;
use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// Mock data source for demonstration
struct MockDataSource {
    cases: Vec<(CaseMetadata, String)>,
    stats: SourceStats,
}

impl MockDataSource {
    fn new() -> Self {
        let cases = vec![
            create_sample_case_1(),
            create_sample_case_2(),
            create_sample_case_3(),
        ];

        let stats = SourceStats {
            total_fetched: cases.len(),
            total_processed: 0,
            total_errors: 0,
            last_fetch_time: Some(Utc::now()),
            rate_limit_hits: 0,
        };

        Self { cases, stats }
    }
}

#[async_trait]
impl DataSource for MockDataSource {
    async fn fetch_cases(&mut self, limit: Option<usize>) -> Result<Vec<(CaseMetadata, String)>> {
        let cases_to_return = if let Some(limit) = limit {
            self.cases.iter().take(limit).cloned().collect()
        } else {
            self.cases.clone()
        };

        info!("Mock data source returning {} cases", cases_to_return.len());
        Ok(cases_to_return)
    }

    async fn get_stats(&self) -> Result<SourceStats> {
        Ok(self.stats.clone())
    }

    async fn health_check(&self) -> Result<()> {
        info!("Mock data source health check: OK");
        Ok(())
    }

    fn get_source_config(&self) -> SourceConfig {
        SourceConfig {
            name: "Mock".to_string(),
            enabled: true,
            batch_size: 10,
            rate_limit_rpm: 100,
            max_retries: 3,
            timeout_seconds: 30,
        }
    }
}

fn create_sample_case_1() -> (CaseMetadata, String) {
    let metadata = CaseMetadata {
        id: Uuid::new_v4(),
        name: "Brown v. Board of Education".to_string(),
        court: "Supreme Court of the United States".to_string(),
        jurisdiction: Jurisdiction::Federal,
        decision_date: NaiveDate::from_ymd_opt(1954, 5, 17).unwrap(),
        citations: vec!["347 U.S. 483 (1954)".to_string()],
        docket_number: Some("1".to_string()),
        judges: vec!["Chief Justice Warren".to_string()],
        source_url: Some("https://example.com/brown-v-board".to_string()),
        word_count: 0, // Will be updated during processing
        ingestion_date: Utc::now(),
    };

    let full_text = r#"
BROWN v. BOARD OF EDUCATION OF TOPEKA
347 U.S. 483 (1954)

Chief Justice Warren delivered the opinion of the Court.

These cases come to us from the States of Kansas, South Carolina, Virginia, and Delaware. 
They are premised on different facts and different local conditions, but a common legal 
question justifies their consideration together in this consolidated opinion.

In each of the cases, minors of the Negro race, through their legal representatives, 
seek the aid of the courts in obtaining admission to the public schools of their 
community on a nonsegregated basis. In each instance, they had been denied admission 
to schools attended by white children under laws requiring or permitting segregation 
according to race.

The plaintiffs contend that segregated public schools are not "equal" and cannot be 
made "equal," and that hence they are deprived of the equal protection of the laws 
guaranteed by the Fourteenth Amendment.

We conclude that, in the field of public education, the doctrine of "separate but equal" 
has no place. Separate educational facilities are inherently unequal. Therefore, we hold 
that the plaintiffs and others similarly situated for whom the actions have been brought 
are, by reason of the segregation complained of, deprived of the equal protection of the 
laws guaranteed by the Fourteenth Amendment.
"#.trim().to_string();

    (metadata, full_text)
}

fn create_sample_case_2() -> (CaseMetadata, String) {
    let metadata = CaseMetadata {
        id: Uuid::new_v4(),
        name: "Miranda v. Arizona".to_string(),
        court: "Supreme Court of the United States".to_string(),
        jurisdiction: Jurisdiction::Federal,
        decision_date: NaiveDate::from_ymd_opt(1966, 6, 13).unwrap(),
        citations: vec!["384 U.S. 436 (1966)".to_string()],
        docket_number: Some("759".to_string()),
        judges: vec!["Chief Justice Warren".to_string()],
        source_url: Some("https://example.com/miranda-v-arizona".to_string()),
        word_count: 0,
        ingestion_date: Utc::now(),
    };

    let full_text = r#"
MIRANDA v. ARIZONA
384 U.S. 436 (1966)

Chief Justice Warren delivered the opinion of the Court.

The cases before us raise questions which go to the roots of our concepts of American 
criminal jurisprudence: the restraints society must observe consistent with the Federal 
Constitution in prosecuting individuals for crime.

We dealt with certain phases of this problem recently in Escobedo v. Illinois, where we 
held that an accused person has the right to have counsel present when being interrogated 
by police.

The constitutional issue we decide in each of these cases is the admissibility of statements 
obtained from a defendant questioned while in custody or otherwise deprived of his freedom 
of action in any significant way.

We hold that when an individual is taken into custody or otherwise deprived of his freedom 
by the authorities in any significant way and is subjected to questioning, the privilege 
against self-incrimination is jeopardized. Procedural safeguards must be employed to protect 
the privilege, and unless other fully effective means are adopted to notify the person of 
his right of silence and to assure that the exercise of the right will be scrupulously 
honored, the following measures are required.

He must be warned prior to any questioning that he has the right to remain silent, that 
anything he says can be used against him in a court of law, that he has the right to the 
presence of an attorney, and that, if he cannot afford an attorney, one will be appointed 
for him prior to any questioning if he so desires.
"#.trim().to_string();

    (metadata, full_text)
}

fn create_sample_case_3() -> (CaseMetadata, String) {
    let metadata = CaseMetadata {
        id: Uuid::new_v4(),
        name: "Roe v. Wade".to_string(),
        court: "Supreme Court of the United States".to_string(),
        jurisdiction: Jurisdiction::Federal,
        decision_date: NaiveDate::from_ymd_opt(1973, 1, 22).unwrap(),
        citations: vec!["410 U.S. 113 (1973)".to_string()],
        docket_number: Some("70-18".to_string()),
        judges: vec!["Justice Blackmun".to_string()],
        source_url: Some("https://example.com/roe-v-wade".to_string()),
        word_count: 0,
        ingestion_date: Utc::now(),
    };

    let full_text = r#"
ROE v. WADE
410 U.S. 113 (1973)

Justice Blackmun delivered the opinion of the Court.

This Texas federal appeal and its Georgia companion, Doe v. Bolton, present constitutional 
challenges to state criminal abortion legislation. The Texas statutes under attack here 
are typical of those that have been in effect in many States for approximately a century.

We forthwith acknowledge our awareness of the sensitive and emotional nature of the abortion 
controversy, of the vigorous opposing views, even among physicians, and of the deep and 
seemingly absolute convictions that the subject inspires. One's philosophy, one's experiences, 
one's exposure to the raw edges of human existence, one's religious training, one's attitudes 
toward life and family and their values, and the moral standards one establishes and seeks 
to observe, are all likely to influence and to color one's thinking and conclusions about 
abortion.

The Constitution does not explicitly mention any right of privacy. In a line of decisions, 
however, the Court has recognized that a right of personal privacy, or a guarantee of certain 
areas or zones of privacy, does exist under the Constitution.

We, therefore, conclude that the right of personal privacy includes the abortion decision, 
but that this right is not unqualified, and must be considered against important state 
interests in regulation.
"#.trim().to_string();

    (metadata, full_text)
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    info!("Starting Legal Search Engine Ingestion Demo");

    // Create temporary directory for demo
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("demo_db");

    // Configure storage
    let storage_config = StorageConfig {
        db_path: db_path.clone(),
        enable_compression: true,
        backup_interval_hours: 24,
        max_backup_files: 5,
    };

    // Configure text processing
    let text_processing_config = TextProcessingConfig {
        max_text_length: 1_000_000,
        enable_case_folding: true,
        enable_unicode_normalization: true,
        remove_extra_whitespace: true,
        normalize_quotes: true,
        extract_citations: true,
        extract_entities: true,
    };

    // Configure validation
    let validation_config = ValidationConfig {
        min_text_length: 10,
        max_text_length: 1_000_000,
        required_fields: vec![
            "name".to_string(),
            "court".to_string(),
            "decision_date".to_string(),
        ],
        allow_empty_citations: true,
        validate_dates: true,
        validate_citations: false, // Disabled for demo
    };

    // Configure cache
    let cache_config = CacheConfig {
        enabled: true,
        max_memory_entries: 1000,
        disk_cache_path: Some(temp_dir.path().join("cache")),
        max_disk_size_mb: 100,
        ttl_hours: 24,
    };

    // Configure ingestion pipeline
    let ingestion_config = IngestionConfig {
        batch_size: 2,
        max_concurrent_jobs: 2,
        rate_limit_delay_ms: 100,
        max_memory_usage_mb: 512,
        retry_attempts: 3,
        retry_delay_ms: 1000,
        validation: validation_config,
        cache: cache_config,
    };

    info!("Initializing storage manager...");
    let storage = Arc::new(StorageManager::new(storage_config).await?);

    info!("Creating ingestion pipeline...");
    let pipeline = IngestionPipeline::new(
        ingestion_config,
        storage.clone(),
        text_processing_config,
    ).await?;

    info!("Running pipeline health check...");
    pipeline.health_check().await?;

    info!("Creating mock data source with sample cases...");
    let data_source = MockDataSource::new();

    info!("Starting ingestion process...");
    let stats = pipeline.run_ingestion(data_source, None).await?;

    // Display results
    info!("=== INGESTION COMPLETED ===");
    info!("Total processed: {}", stats.total_processed);
    info!("Successfully stored: {}", stats.successful_stores);
    info!("Failed processing: {}", stats.failed_processing);
    info!("Validation failures: {}", stats.validation_failures);
    info!("Duplicates skipped: {}", stats.duplicates_skipped);
    info!("Processing rate: {:.2} cases/sec", stats.processing_rate);

    if let (Some(start), Some(end)) = (&stats.start_time, &stats.end_time) {
        let duration = *end - *start;
        info!("Total time: {:.2} seconds", duration.num_milliseconds() as f64 / 1000.0);
    }

    info!("Peak memory usage: {:.1} MB", stats.memory_stats.peak_memory_mb);

    // Show stored cases
    info!("\n=== STORED CASES ===");
    let case_ids = storage.list_case_ids().await?;
    info!("Found {} cases in storage:", case_ids.len());

    for case_id in &case_ids {
        if let Some(metadata) = storage.get_case_metadata(case_id).await? {
            info!("- {} ({})", metadata.name, metadata.court);
            info!("  Citations: {:?}", metadata.citations);
            info!("  Word count: {}", metadata.word_count);
            
            // Show a snippet of the processed text
            if let Some(text) = storage.get_case_text(case_id).await? {
                let snippet = if text.len() > 200 {
                    format!("{}...", &text[..200])
                } else {
                    text
                };
                info!("  Text snippet: {}", snippet.replace('\n', " "));
            }
            info!("");
        }
    }

    // Storage statistics
    let storage_stats = storage.get_stats().await?;
    info!("=== STORAGE STATISTICS ===");
    info!("Total cases: {}", storage_stats.total_cases);
    info!("Database size: {:.2} MB", storage_stats.database_size_bytes as f64 / 1_000_000.0);
    info!("Total size: {:.2} MB", storage_stats.total_size_bytes as f64 / 1_000_000.0);

    info!("Demo completed successfully!");
    info!("Database created at: {:?}", db_path);

    Ok(())
} 