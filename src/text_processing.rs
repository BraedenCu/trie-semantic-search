//! # Text Processing Module
//!
//! ## Purpose
//! Advanced text processing pipeline for legal documents with specialized
//! tokenization, normalization, and feature extraction capabilities.
//!
//! ## Input/Output Specification
//! - **Input**: Raw legal case text, metadata, processing options
//! - **Output**: Processed tokens, normalized text, extracted features
//! - **Features**: Citations, legal terms, entities, key phrases
//!
//! ## Key Features
//! - Legal-aware tokenization and sentence splitting
//! - Citation extraction and normalization
//! - Legal terminology recognition
//! - Named entity extraction (judges, courts, parties)
//! - Text normalization and cleaning
//! - Stopword filtering with legal context

use crate::config::TextProcessingConfig;
use crate::errors::{Result, SearchError};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use unicode_normalization::UnicodeNormalization;

/// Text processing pipeline
pub struct TextProcessor {
    config: TextProcessingConfig,
    citation_regex: Vec<Regex>,
    legal_terms: HashSet<String>,
    stopwords: HashSet<String>,
    court_patterns: Vec<Regex>,
    judge_patterns: Vec<Regex>,
}

/// Processed text result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessedText {
    /// Original text
    pub original: String,
    /// Normalized text
    pub normalized: String,
    /// Extracted tokens
    pub tokens: Vec<Token>,
    /// Extracted sentences
    pub sentences: Vec<String>,
    /// Legal citations found
    pub citations: Vec<Citation>,
    /// Legal terms identified
    pub legal_terms: Vec<LegalTerm>,
    /// Named entities
    pub entities: Vec<NamedEntity>,
    /// Text statistics
    pub stats: TextStats,
}

/// Individual token with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Token {
    /// Token text
    pub text: String,
    /// Normalized form
    pub normalized: String,
    /// Position in original text
    pub position: usize,
    /// Token type
    pub token_type: TokenType,
    /// Whether it's a stopword
    pub is_stopword: bool,
    /// Part of speech (if available)
    pub pos_tag: Option<String>,
}

/// Token classification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TokenType {
    Word,
    Number,
    Punctuation,
    Citation,
    LegalTerm,
    ProperNoun,
    Other,
}

/// Legal citation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Citation {
    /// Full citation text
    pub full_text: String,
    /// Normalized citation
    pub normalized: String,
    /// Citation type
    pub citation_type: CitationType,
    /// Volume number
    pub volume: Option<String>,
    /// Reporter abbreviation
    pub reporter: Option<String>,
    /// Page number
    pub page: Option<String>,
    /// Year
    pub year: Option<u32>,
    /// Position in text
    pub position: usize,
}

/// Citation types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CitationType {
    Case,
    Statute,
    Regulation,
    Constitutional,
    Secondary,
    Unknown,
}

/// Legal term with context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegalTerm {
    /// Term text
    pub term: String,
    /// Term category
    pub category: LegalTermCategory,
    /// Confidence score
    pub confidence: f32,
    /// Position in text
    pub position: usize,
}

/// Legal term categories
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LegalTermCategory {
    Procedure,
    Evidence,
    Contract,
    Criminal,
    Constitutional,
    Tort,
    Property,
    Corporate,
    Family,
    Tax,
    Other,
}

/// Named entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamedEntity {
    /// Entity text
    pub text: String,
    /// Entity type
    pub entity_type: EntityType,
    /// Confidence score
    pub confidence: f32,
    /// Position in text
    pub position: usize,
}

/// Entity types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EntityType {
    Person,
    Court,
    Judge,
    Attorney,
    Party,
    Organization,
    Location,
    Date,
    Money,
    Other,
}

/// Text processing statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextStats {
    /// Total character count
    pub char_count: usize,
    /// Word count
    pub word_count: usize,
    /// Sentence count
    pub sentence_count: usize,
    /// Paragraph count
    pub paragraph_count: usize,
    /// Unique words
    pub unique_words: usize,
    /// Reading level estimate
    pub reading_level: Option<f32>,
    /// Language detected
    pub language: Option<String>,
}

impl TextProcessor {
    /// Create new text processor
    pub fn new(config: TextProcessingConfig) -> Result<Self> {
        let mut processor = Self {
            config,
            citation_regex: Vec::new(),
            legal_terms: HashSet::new(),
            stopwords: HashSet::new(),
            court_patterns: Vec::new(),
            judge_patterns: Vec::new(),
        };

        processor.initialize_patterns()?;
        processor.load_legal_terms()?;
        processor.load_stopwords()?;

        Ok(processor)
    }

    /// Process legal text
    pub async fn process_text(&self, text: &str) -> Result<ProcessedText> {
        tracing::debug!("Processing text: {} characters", text.len());

        // Normalize text
        let normalized = self.normalize_text(text)?;

        // Tokenize
        let tokens = self.tokenize(&normalized)?;

        // Extract sentences
        let sentences = self.extract_sentences(&normalized)?;

        // Extract citations
        let citations = self.extract_citations(&normalized)?;

        // Extract legal terms
        let legal_terms = self.extract_legal_terms(&tokens)?;

        // Extract named entities
        let entities = self.extract_entities(&normalized)?;

        // Calculate statistics
        let stats = self.calculate_stats(&normalized, &tokens, &sentences)?;

        Ok(ProcessedText {
            original: text.to_string(),
            normalized,
            tokens,
            sentences,
            citations,
            legal_terms,
            entities,
            stats,
        })
    }

    /// Initialize regex patterns
    fn initialize_patterns(&mut self) -> Result<()> {
        // Citation patterns
        let citation_patterns = vec![
            // Case citations: Volume Reporter Page (Year)
            r"(\d+)\s+([A-Z][a-z]*\.?\s*[A-Z]*\.?)\s+(\d+)(?:\s*\((\d{4})\))?",
            // U.S. citations: Volume U.S. Page (Year)
            r"(\d+)\s+U\.S\.\s+(\d+)(?:\s*\((\d{4})\))?",
            // Federal citations: Volume F.2d/F.3d Page (Circuit Year)
            r"(\d+)\s+F\.\s*(?:2d|3d)\s+(\d+)\s*\([^)]*(\d{4})\)",
            // Supreme Court citations
            r"(\d+)\s+S\.\s*Ct\.\s+(\d+)(?:\s*\((\d{4})\))?",
            // State citations
            r"(\d+)\s+[A-Z][a-z]*\.?\s*(?:2d|3d)?\s+(\d+)\s*\([^)]*(\d{4})\)",
        ];

        for pattern in citation_patterns {
            self.citation_regex.push(
                Regex::new(pattern).map_err(|e| SearchError::Internal {
                    message: format!("Invalid citation regex: {}", e),
                })?
            );
        }

        // Court patterns
        let court_patterns = vec![
            r"(?i)supreme\s+court",
            r"(?i)court\s+of\s+appeals",
            r"(?i)district\s+court",
            r"(?i)circuit\s+court",
            r"(?i)bankruptcy\s+court",
            r"(?i)magistrate\s+judge",
        ];

        for pattern in court_patterns {
            self.court_patterns.push(
                Regex::new(pattern).map_err(|e| SearchError::Internal {
                    message: format!("Invalid court regex: {}", e),
                })?
            );
        }

        // Judge patterns
        let judge_patterns = vec![
            r"(?i)judge\s+([A-Z][a-z]+(?:\s+[A-Z][a-z]+)*)",
            r"(?i)justice\s+([A-Z][a-z]+(?:\s+[A-Z][a-z]+)*)",
            r"(?i)chief\s+judge\s+([A-Z][a-z]+(?:\s+[A-Z][a-z]+)*)",
            r"(?i)magistrate\s+judge\s+([A-Z][a-z]+(?:\s+[A-Z][a-z]+)*)",
        ];

        for pattern in judge_patterns {
            self.judge_patterns.push(
                Regex::new(pattern).map_err(|e| SearchError::Internal {
                    message: format!("Invalid judge regex: {}", e),
                })?
            );
        }

        Ok(())
    }

    /// Load legal terms dictionary
    fn load_legal_terms(&mut self) -> Result<()> {
        // Core legal terms - in production this would be loaded from a file
        let terms = vec![
            // Procedure
            "motion", "petition", "complaint", "answer", "discovery", "deposition",
            "subpoena", "summons", "jurisdiction", "venue", "standing", "joinder",
            
            // Evidence
            "hearsay", "objection", "sustained", "overruled", "exhibit", "testimony",
            "witness", "cross-examination", "direct examination", "impeachment",
            
            // Criminal
            "indictment", "arraignment", "plea", "guilty", "not guilty", "felony",
            "misdemeanor", "sentence", "probation", "parole", "bail", "warrant",
            
            // Constitutional
            "due process", "equal protection", "first amendment", "fourth amendment",
            "search and seizure", "miranda", "habeas corpus", "constitutional",
            
            // Contract
            "consideration", "breach", "damages", "specific performance", "contract",
            "agreement", "offer", "acceptance", "counteroffer", "rescission",
            
            // Tort
            "negligence", "liability", "damages", "causation", "duty", "breach",
            "proximate cause", "strict liability", "intentional tort", "defamation",
            
            // Property
            "title", "deed", "easement", "lien", "mortgage", "foreclosure",
            "adverse possession", "eminent domain", "zoning", "covenant",
        ];

        for term in terms {
            self.legal_terms.insert(term.to_lowercase());
        }

        Ok(())
    }

    /// Load stopwords
    fn load_stopwords(&mut self) -> Result<()> {
        // Common English stopwords with legal context
        let stopwords = vec![
            "a", "an", "and", "are", "as", "at", "be", "by", "for", "from",
            "has", "he", "in", "is", "it", "its", "of", "on", "that", "the",
            "to", "was", "will", "with", "the", "this", "but", "they", "have",
            "had", "what", "said", "each", "which", "she", "do", "how", "their",
            "if", "up", "out", "many", "then", "them", "these", "so", "some",
            "her", "would", "make", "like", "into", "him", "time", "two", "more",
            "go", "no", "way", "could", "my", "than", "first", "been", "call",
            "who", "oil", "sit", "now", "find", "down", "day", "did", "get",
            "come", "made", "may", "part",
        ];

        for word in stopwords {
            self.stopwords.insert(word.to_string());
        }

        Ok(())
    }

    /// Normalize text
    fn normalize_text(&self, text: &str) -> Result<String> {
        let mut normalized = text.nfc().collect::<String>();

        if self.config.remove_extra_whitespace {
            // Remove extra whitespace
            normalized = Regex::new(r"\s+")
                .unwrap()
                .replace_all(&normalized, " ")
                .to_string();
        }

        if self.config.normalize_quotes {
            // Normalize quotes
            normalized = normalized
                .replace('"', "\"")
                .replace('"', "\"")
                .replace('\u{2018}', "'")
                .replace('\u{2019}', "'");
        }

        // Remove control characters but preserve line breaks
        normalized = normalized
            .chars()
            .filter(|&c| c == '\n' || c == '\t' || !c.is_control())
            .collect();

        Ok(normalized.trim().to_string())
    }

    /// Tokenize text
    fn tokenize(&self, text: &str) -> Result<Vec<Token>> {
        let mut tokens = Vec::new();
        let word_regex = Regex::new(r"\b\w+\b").unwrap();
        
        for mat in word_regex.find_iter(text) {
            let word = mat.as_str();
            let normalized = word.to_lowercase();
            let is_stopword = self.stopwords.contains(&normalized);
            
            let token_type = if self.legal_terms.contains(&normalized) {
                TokenType::LegalTerm
            } else if word.chars().all(|c| c.is_numeric()) {
                TokenType::Number
            } else if word.chars().next().unwrap_or('a').is_uppercase() {
                TokenType::ProperNoun
            } else {
                TokenType::Word
            };

            tokens.push(Token {
                text: word.to_string(),
                normalized,
                position: mat.start(),
                token_type,
                is_stopword,
                pos_tag: None, // Would be filled by POS tagger
            });
        }

        Ok(tokens)
    }

    /// Extract sentences
    fn extract_sentences(&self, text: &str) -> Result<Vec<String>> {
        // Simple sentence splitting - in production would use more sophisticated NLP
        let sentence_regex = Regex::new(r"[.!?]+\s+").unwrap();
        let sentences: Vec<String> = sentence_regex
            .split(text)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        Ok(sentences)
    }

    /// Extract legal citations
    fn extract_citations(&self, text: &str) -> Result<Vec<Citation>> {
        let mut citations = Vec::new();

        for regex in &self.citation_regex {
            for captures in regex.captures_iter(text) {
                if let Some(full_match) = captures.get(0) {
                    let citation = Citation {
                        full_text: full_match.as_str().to_string(),
                        normalized: self.normalize_citation(full_match.as_str()),
                        citation_type: self.classify_citation(full_match.as_str()),
                        volume: captures.get(1).map(|m| m.as_str().to_string()),
                        reporter: captures.get(2).map(|m| m.as_str().to_string()),
                        page: captures.get(3).map(|m| m.as_str().to_string()),
                        year: captures.get(4)
                            .and_then(|m| m.as_str().parse().ok()),
                        position: full_match.start(),
                    };
                    citations.push(citation);
                }
            }
        }

        // Remove duplicates and sort by position
        citations.sort_by_key(|c| c.position);
        citations.dedup_by_key(|c| c.normalized.clone());

        Ok(citations)
    }

    /// Extract legal terms
    fn extract_legal_terms(&self, tokens: &[Token]) -> Result<Vec<LegalTerm>> {
        let mut terms = Vec::new();

        for token in tokens {
            if matches!(token.token_type, TokenType::LegalTerm) {
                let category = self.classify_legal_term(&token.normalized);
                terms.push(LegalTerm {
                    term: token.text.clone(),
                    category,
                    confidence: 0.8, // Would be calculated by ML model
                    position: token.position,
                });
            }
        }

        Ok(terms)
    }

    /// Extract named entities
    fn extract_entities(&self, text: &str) -> Result<Vec<NamedEntity>> {
        let mut entities = Vec::new();

        // Extract judges
        for regex in &self.judge_patterns {
            for captures in regex.captures_iter(text) {
                if let Some(full_match) = captures.get(0) {
                    entities.push(NamedEntity {
                        text: full_match.as_str().to_string(),
                        entity_type: EntityType::Judge,
                        confidence: 0.9,
                        position: full_match.start(),
                    });
                }
            }
        }

        // Extract courts
        for regex in &self.court_patterns {
            for mat in regex.find_iter(text) {
                entities.push(NamedEntity {
                    text: mat.as_str().to_string(),
                    entity_type: EntityType::Court,
                    confidence: 0.85,
                    position: mat.start(),
                });
            }
        }

        // Extract dates (simple pattern)
        let date_regex = Regex::new(r"\b\d{1,2}/\d{1,2}/\d{4}\b|\b\d{4}\b").unwrap();
        for mat in date_regex.find_iter(text) {
            entities.push(NamedEntity {
                text: mat.as_str().to_string(),
                entity_type: EntityType::Date,
                confidence: 0.7,
                position: mat.start(),
            });
        }

        Ok(entities)
    }

    /// Calculate text statistics
    fn calculate_stats(&self, text: &str, tokens: &[Token], sentences: &[String]) -> Result<TextStats> {
        let word_count = tokens.len();
        let unique_words = tokens.iter()
            .map(|t| &t.normalized)
            .collect::<HashSet<_>>()
            .len();

        let paragraph_count = text.matches("\n\n").count() + 1;

        // Simple reading level calculation (Flesch-Kincaid approximation)
        let avg_sentence_length = if sentences.is_empty() {
            0.0
        } else {
            word_count as f32 / sentences.len() as f32
        };

        let syllable_count = tokens.iter()
            .map(|t| self.count_syllables(&t.text))
            .sum::<usize>() as f32;

        let avg_syllables_per_word = if word_count == 0 {
            0.0
        } else {
            syllable_count / word_count as f32
        };

        let reading_level = 206.835 - (1.015 * avg_sentence_length) - (84.6 * avg_syllables_per_word);

        Ok(TextStats {
            char_count: text.len(),
            word_count,
            sentence_count: sentences.len(),
            paragraph_count,
            unique_words,
            reading_level: Some(reading_level),
            language: Some("en".to_string()), // Would be detected by language detection
        })
    }

    /// Normalize citation format
    fn normalize_citation(&self, citation: &str) -> String {
        // Basic citation normalization
        citation.trim()
            .replace("  ", " ")
            .replace(" ,", ",")
            .to_string()
    }

    /// Classify citation type
    fn classify_citation(&self, citation: &str) -> CitationType {
        let citation_lower = citation.to_lowercase();
        
        if citation_lower.contains("u.s.") || citation_lower.contains("s. ct.") {
            CitationType::Case
        } else if citation_lower.contains("u.s.c.") {
            CitationType::Statute
        } else if citation_lower.contains("c.f.r.") {
            CitationType::Regulation
        } else if citation_lower.contains("const") {
            CitationType::Constitutional
        } else {
            CitationType::Case // Default for most legal citations
        }
    }

    /// Classify legal term category
    fn classify_legal_term(&self, term: &str) -> LegalTermCategory {
        // Simple classification - would use ML in production
        match term {
            t if ["motion", "petition", "complaint", "discovery"].contains(&t) => LegalTermCategory::Procedure,
            t if ["hearsay", "objection", "testimony", "exhibit"].contains(&t) => LegalTermCategory::Evidence,
            t if ["indictment", "guilty", "felony", "sentence"].contains(&t) => LegalTermCategory::Criminal,
            t if ["due process", "constitutional", "amendment"].contains(&t) => LegalTermCategory::Constitutional,
            t if ["contract", "breach", "damages", "consideration"].contains(&t) => LegalTermCategory::Contract,
            t if ["negligence", "liability", "tort", "causation"].contains(&t) => LegalTermCategory::Tort,
            t if ["title", "deed", "property", "easement"].contains(&t) => LegalTermCategory::Property,
            _ => LegalTermCategory::Other,
        }
    }

    /// Count syllables in a word (approximation)
    fn count_syllables(&self, word: &str) -> usize {
        let word = word.to_lowercase();
        let vowels = ['a', 'e', 'i', 'o', 'u', 'y'];
        let mut count = 0;
        let mut prev_was_vowel = false;

        for ch in word.chars() {
            let is_vowel = vowels.contains(&ch);
            if is_vowel && !prev_was_vowel {
                count += 1;
            }
            prev_was_vowel = is_vowel;
        }

        // Handle silent 'e'
        if word.ends_with('e') && count > 1 {
            count -= 1;
        }

        // Ensure at least one syllable
        if count == 0 {
            count = 1;
        }

        count
    }

    /// Extract key phrases (simple implementation)
    pub fn extract_key_phrases(&self, tokens: &[Token], max_phrases: usize) -> Vec<String> {
        let mut phrases = Vec::new();
        let mut current_phrase = Vec::new();

        for token in tokens {
            if token.is_stopword || matches!(token.token_type, TokenType::Punctuation) {
                if current_phrase.len() >= 2 {
                    phrases.push(current_phrase.join(" "));
                }
                current_phrase.clear();
            } else {
                current_phrase.push(token.text.clone());
                if current_phrase.len() >= 5 {
                    phrases.push(current_phrase.join(" "));
                    current_phrase.clear();
                }
            }
        }

        // Add final phrase
        if current_phrase.len() >= 2 {
            phrases.push(current_phrase.join(" "));
        }

        // Sort by length and take top phrases
        phrases.sort_by_key(|p| std::cmp::Reverse(p.len()));
        phrases.truncate(max_phrases);
        phrases
    }
} 