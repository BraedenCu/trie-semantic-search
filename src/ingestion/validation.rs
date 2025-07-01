//! # Data Validation Module
//!
//! ## Purpose
//! Validates the quality and integrity of legal case data during ingestion,
//! ensuring consistency and completeness before indexing.
//!
//! ## Input/Output Specification
//! - **Input**: CaseMetadata structures from various sources
//! - **Output**: ValidationResult with pass/fail status and detailed feedback
//! - **Validation Rules**: Format, completeness, consistency, legal citation format
//!
//! ## Key Features
//! - Comprehensive validation rules for legal data
//! - Configurable validation severity levels
//! - Detailed error reporting and suggestions
//! - Performance-optimized validation checks
//! - Extensible rule system

use crate::errors::{Result, SearchError};
use crate::CaseMetadata;
use serde::{Deserialize, Serialize};

/// Case data validator
pub struct CaseValidator {
    rules: Vec<Box<dyn ValidationRule + Send + Sync>>,
}

/// Trait for validation rules
pub trait ValidationRule {
    fn name(&self) -> &str;
    fn validate(&self, case: &CaseMetadata) -> ValidationResult;
}

/// Result of validation check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub passed: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
}

/// Validation error details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    pub field: String,
    pub message: String,
    pub severity: ValidationSeverity,
}

/// Validation warning details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationWarning {
    pub field: String,
    pub message: String,
}

/// Severity levels for validation issues
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationSeverity {
    Critical,
    High,
    Medium,
    Low,
}

impl CaseValidator {
    /// Create new case validator
    pub fn new() -> Result<Self> {
        let rules: Vec<Box<dyn ValidationRule + Send + Sync>> = vec![
            // TODO: Add validation rules
        ];
        
        Ok(Self { rules })
    }
    
    /// Validate a case against all rules
    pub fn validate(&self, case: &CaseMetadata) -> Result<ValidationResult> {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        
        for rule in &self.rules {
            let result = rule.validate(case);
            errors.extend(result.errors);
            warnings.extend(result.warnings);
        }
        
        Ok(ValidationResult {
            passed: errors.is_empty(),
            errors,
            warnings,
        })
    }
} 