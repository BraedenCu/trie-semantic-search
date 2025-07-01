//! Simple demonstration of the legal search engine ingestion system
//!
//! This demo shows the core functionality working with minimal dependencies.

use std::sync::Arc;
use tokio;
use trie_semantic_search::{
    config::{Config, StorageConfig, TextProcessingConfig},
    storage::StorageManager,
    text_processing::TextProcessor,
    CaseMetadata, Jurisdiction,
};
use chrono::{NaiveDate, Utc};
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    println!("🏛️  Legal Search Engine - Simple Demo");
    println!("=====================================");
    
    // Create simple storage configuration
    let storage_config = StorageConfig {
        data_dir: "./demo_data".into(),
        max_memory_usage_mb: 100,
        compression_enabled: true,
        backup_enabled: false,
        backup_interval_hours: 24,
        max_backup_files: 5,
    };
    
    // Create text processing configuration
    let text_processing_config = TextProcessingConfig {
        tokenizer_model_path: "./models/tokenizer.json".into(),
        enable_case_folding: true,
        enable_unicode_normalization: true,
        preserve_legal_citations: true,
        max_text_length: 1_000_000,
        remove_extra_whitespace: true,
        normalize_quotes: true,
        extract_citations: true,
        extract_entities: true,
        min_word_length: 2,
        max_word_length: 50,
        stop_word_removal: true,
    };
    
    // Initialize storage
    println!("📦 Initializing storage...");
    let storage = Arc::new(StorageManager::new(storage_config).await?);
    
    // Initialize text processor
    println!("🔤 Initializing text processor...");
    let text_processor = TextProcessor::new(text_processing_config)?;
    
    // Create sample legal cases
    let sample_cases = create_sample_cases();
    
    println!("📚 Processing {} sample cases...", sample_cases.len());
    
    // Process and store each case
    for (i, (metadata, full_text)) in sample_cases.iter().enumerate() {
        println!("  Processing case {}: {}", i + 1, metadata.name);
        
        // Process the text
        let processed_text = text_processor.process_text(full_text).await?;
        
        // Store the case
        storage.store_case_metadata(metadata).await?;
        storage.store_case_text(&metadata.id.to_string(), full_text).await?;
        
        println!("    ✅ Stored case with {} words, {} citations", 
                processed_text.word_count, processed_text.citations.len());
    }
    
    // Display statistics
    println!("\n📊 Final Statistics:");
    let stats = storage.get_stats().await?;
    println!("  Total cases: {}", stats.total_cases);
    println!("  Total size: {} bytes", stats.total_size_bytes);
    println!("  Database size: {} bytes", stats.database_size_bytes);
    
    // Test retrieval
    println!("\n🔍 Testing case retrieval...");
    let first_case_id = sample_cases[0].0.id.to_string();
    if let Some(retrieved_metadata) = storage.get_case_metadata(&first_case_id).await? {
        println!("  ✅ Successfully retrieved: {}", retrieved_metadata.name);
    }
    
    println!("\n🎉 Demo completed successfully!");
    println!("   The ingestion system is working correctly.");
    println!("   Data stored in: ./demo_data/");
    
    Ok(())
}

fn create_sample_cases() -> Vec<(CaseMetadata, String)> {
    vec![
        (
            CaseMetadata {
                id: Uuid::new_v4(),
                name: "Brown v. Board of Education".to_string(),
                court: "Supreme Court of the United States".to_string(),
                jurisdiction: Jurisdiction::Federal,
                decision_date: NaiveDate::from_ymd_opt(1954, 5, 17).unwrap(),
                citations: vec!["347 U.S. 483 (1954)".to_string()],
                docket_number: Some("1".to_string()),
                judges: vec!["Warren, C.J.".to_string()],
                source_url: Some("https://supreme.justia.com/cases/federal/us/347/483/".to_string()),
                word_count: 2847,
                ingestion_date: Utc::now(),
                citation: "347 U.S. 483 (1954)".to_string(),
                full_text: "".to_string(), // Will be filled from the second element
                topics: vec!["Education".to_string(), "Civil Rights".to_string(), "Equal Protection".to_string()],
            },
            r#"MR. CHIEF JUSTICE WARREN delivered the opinion of the Court.

These cases come to us from the States of Kansas, South Carolina, Virginia, and Delaware. They are premised on different facts and different local conditions, but a common legal question justifies their consideration together in this consolidated opinion.

In each of the cases, minors of the Negro race, through their legal representatives, seek the aid of the courts in obtaining admission to the public schools of their community on a nonsegregated basis. In each instance, they had been denied admission to schools attended by white children under laws requiring or permitting segregation according to race. This segregation was alleged to deprive the plaintiffs of the equal protection of the laws under the Fourteenth Amendment.

We conclude that, in the field of public education, the doctrine of "separate but equal" has no place. Separate educational facilities are inherently unequal. Therefore, we hold that the plaintiffs and others similarly situated for whom the actions have been brought are, by reason of the segregation complained of, deprived of the equal protection of the laws guaranteed by the Fourteenth Amendment."#
        ),
        (
            CaseMetadata {
                id: Uuid::new_v4(),
                name: "Miranda v. Arizona".to_string(),
                court: "Supreme Court of the United States".to_string(),
                jurisdiction: Jurisdiction::Federal,
                decision_date: NaiveDate::from_ymd_opt(1966, 6, 13).unwrap(),
                citations: vec!["384 U.S. 436 (1966)".to_string()],
                docket_number: Some("759".to_string()),
                judges: vec!["Warren, C.J.".to_string()],
                source_url: Some("https://supreme.justia.com/cases/federal/us/384/436/".to_string()),
                word_count: 4521,
                ingestion_date: Utc::now(),
                citation: "384 U.S. 436 (1966)".to_string(),
                full_text: "".to_string(),
                topics: vec!["Criminal Law".to_string(), "Constitutional Rights".to_string(), "Fifth Amendment".to_string()],
            },
            r#"MR. CHIEF JUSTICE WARREN delivered the opinion of the Court.

The cases before us raise questions which go to the roots of our concepts of American criminal jurisprudence: the restraints society must observe consistent with the Federal Constitution in prosecuting individuals for crime.

Prior to any questioning, the person must be warned that he has a right to remain silent, that any statement he does make may be used as evidence against him, and that he has a right to the presence of an attorney, either retained or appointed. The defendant may waive effectuation of these rights, provided the waiver is made voluntarily, knowingly and intelligently.

The constitutional issue we decide in each of these cases is the admissibility of statements obtained from a defendant questioned while in custody or otherwise deprived of his freedom of action in any significant way."#
        ),
        (
            CaseMetadata {
                id: Uuid::new_v4(),
                name: "Roe v. Wade".to_string(),
                court: "Supreme Court of the United States".to_string(),
                jurisdiction: Jurisdiction::Federal,
                decision_date: NaiveDate::from_ymd_opt(1973, 1, 22).unwrap(),
                citations: vec!["410 U.S. 113 (1973)".to_string()],
                docket_number: Some("70-18".to_string()),
                judges: vec!["Blackmun, J.".to_string()],
                source_url: Some("https://supreme.justia.com/cases/federal/us/410/113/".to_string()),
                word_count: 8936,
                ingestion_date: Utc::now(),
                citation: "410 U.S. 113 (1973)".to_string(),
                full_text: "".to_string(),
                topics: vec!["Privacy Rights".to_string(), "Reproductive Rights".to_string(), "Due Process".to_string()],
            },
            r#"MR. JUSTICE BLACKMUN delivered the opinion of the Court.

This Texas federal appeal and its Georgia companion, Doe v. Bolton, present constitutional challenges to state criminal abortion legislation. The Texas statutes under attack here are typical of those that have been in effect in many States for approximately a century.

We forthwith acknowledge our awareness of the sensitive and emotional nature of the abortion controversy, of the vigorous opposing views, even among physicians, and of the deep and seemingly absolute convictions that the subject inspires. One's philosophy, one's experiences, one's exposure to the raw edges of human existence, one's religious training, one's attitudes toward life and family and their values, and the moral standards one establishes and seeks to observe, are all likely to influence and to color one's thinking and conclusions about abortion.

The Constitution does not explicitly mention any right of privacy. In a line of decisions, however, the Court has recognized that a right of personal privacy, or a guarantee of certain areas or zones of privacy, does exist under the Constitution."#
        ),
    ]
} 