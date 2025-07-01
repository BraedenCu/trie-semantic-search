//! # Utilities Module
//!
//! ## Purpose
//! Common utility functions and helpers used throughout the legal search engine
//! for text processing, performance monitoring, and system operations.
//!
//! ## Input/Output Specification
//! - **Input**: Various data types requiring common operations
//! - **Output**: Processed data, performance metrics, system information
//! - **Functions**: Text utilities, performance helpers, validation functions
//!
//! ## Key Features
//! - Text processing utilities
//! - Performance measurement helpers
//! - System information functions
//! - Validation and sanitization
//! - Common data transformations

use chrono::{DateTime, Utc};
use std::time::Instant;

/// Performance timer for measuring operation duration
pub struct Timer {
    start: Instant,
    name: String,
}

/// Text processing utilities
pub struct TextUtils;

/// System utilities
pub struct SystemUtils;

impl Timer {
    /// Start a new timer with a name
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            start: Instant::now(),
            name: name.into(),
        }
    }

    /// Get elapsed time in milliseconds
    pub fn elapsed_ms(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }

    /// Stop timer and log duration
    pub fn stop(self) -> u64 {
        let elapsed = self.elapsed_ms();
        tracing::debug!("Timer '{}' completed in {}ms", self.name, elapsed);
        elapsed
    }
}

impl TextUtils {
    /// Truncate text to specified length with ellipsis
    pub fn truncate(text: &str, max_length: usize) -> String {
        if text.len() <= max_length {
            text.to_string()
        } else {
            format!("{}...", &text[..max_length.saturating_sub(3)])
        }
    }

    /// Extract preview text from longer content
    pub fn extract_preview(text: &str, max_words: usize) -> String {
        let words: Vec<&str> = text.split_whitespace().take(max_words).collect();
        let preview = words.join(" ");
        
        if words.len() >= max_words {
            format!("{}...", preview)
        } else {
            preview
        }
    }

    /// Sanitize text for safe display
    pub fn sanitize(text: &str) -> String {
        text.chars()
            .filter(|c| !c.is_control() || c.is_whitespace())
            .collect()
    }

    /// Count words in text
    pub fn word_count(text: &str) -> usize {
        text.split_whitespace().count()
    }

    /// Generate text hash for caching
    pub fn text_hash(text: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        text.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }
}

impl SystemUtils {
    /// Get current memory usage in bytes
    pub fn memory_usage() -> Option<u64> {
        // TODO: Implement platform-specific memory usage
        None
    }

    /// Get system uptime
    pub fn uptime() -> Option<std::time::Duration> {
        // TODO: Implement platform-specific uptime
        None
    }

    /// Format bytes as human-readable string
    pub fn format_bytes(bytes: u64) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
        let mut size = bytes as f64;
        let mut unit_index = 0;

        while size >= 1024.0 && unit_index < UNITS.len() - 1 {
            size /= 1024.0;
            unit_index += 1;
        }

        if unit_index == 0 {
            format!("{} {}", size as u64, UNITS[unit_index])
        } else {
            format!("{:.2} {}", size, UNITS[unit_index])
        }
    }

    /// Format duration as human-readable string
    pub fn format_duration(duration: std::time::Duration) -> String {
        let total_seconds = duration.as_secs();
        let hours = total_seconds / 3600;
        let minutes = (total_seconds % 3600) / 60;
        let seconds = total_seconds % 60;

        if hours > 0 {
            format!("{}h {}m {}s", hours, minutes, seconds)
        } else if minutes > 0 {
            format!("{}m {}s", minutes, seconds)
        } else {
            format!("{}s", seconds)
        }
    }
}

/// Validation utilities
pub struct ValidationUtils;

impl ValidationUtils {
    /// Validate case ID format
    pub fn is_valid_case_id(id: &str) -> bool {
        uuid::Uuid::parse_str(id).is_ok()
    }

    /// Validate legal citation format (basic check)
    pub fn is_valid_citation(citation: &str) -> bool {
        // Basic pattern: volume reporter page (year)
        let citation_pattern = regex::Regex::new(r"^\d+\s+[A-Za-z.]+\s+\d+.*\(\d{4}\)").unwrap();
        citation_pattern.is_match(citation)
    }

    /// Validate search query
    pub fn is_valid_search_query(query: &str, min_length: usize, max_length: usize) -> bool {
        let trimmed = query.trim();
        !trimmed.is_empty() && trimmed.len() >= min_length && trimmed.len() <= max_length
    }

    /// Sanitize filename for safe file operations
    pub fn sanitize_filename(filename: &str) -> String {
        filename
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' {
                    c
                } else {
                    '_'
                }
            })
            .collect()
    }
}

/// Macro for timing code blocks
#[macro_export]
macro_rules! time_block {
    ($name:expr, $block:block) => {{
        let timer = $crate::utils::Timer::new($name);
        let result = $block;
        timer.stop();
        result
    }};
}

/// Macro for conditional compilation based on features
#[macro_export]
macro_rules! feature_enabled {
    ($feature:expr) => {
        cfg!(feature = $feature)
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_truncate() {
        assert_eq!(TextUtils::truncate("Hello world", 20), "Hello world");
        assert_eq!(TextUtils::truncate("This is a very long text", 10), "This is...");
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(SystemUtils::format_bytes(512), "512 B");
        assert_eq!(SystemUtils::format_bytes(1024), "1.00 KB");
        assert_eq!(SystemUtils::format_bytes(1048576), "1.00 MB");
    }

    #[test]
    fn test_validation() {
        assert!(ValidationUtils::is_valid_search_query("test query", 2, 100));
        assert!(!ValidationUtils::is_valid_search_query("", 2, 100));
        assert!(!ValidationUtils::is_valid_search_query("a", 2, 100));
    }
} 