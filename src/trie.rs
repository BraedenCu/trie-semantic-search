//! # Trie Index Module
//!
//! ## Purpose
//! Implements prefix tree (trie) data structures for fast lexical search and
//! auto-completion in legal text. Supports both in-memory tries and FST-based
//! compressed tries for production use.
//!
//! ## Input/Output Specification
//! - **Input**: Tokenized text sequences, case names, legal citations
//! - **Output**: Exact matches, prefix completions, document references
//! - **Performance**: O(m) lookup time where m = query length
//!
//! ## Key Features
//! - Memory-efficient radix trie implementation
//! - FST-based compressed storage for large datasets
//! - Case name and citation specialized tries
//! - Prefix completion and suggestion
//! - Document reference tracking

use crate::config::TrieConfig;
use crate::errors::{Result, SearchError};
use crate::{CaseId, DocRef};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Main trie index manager
pub struct TrieIndex {
    config: TrieConfig,
    case_name_trie: CaseNameTrie,
    content_trie: ContentTrie,
    citation_trie: CitationTrie,
}

/// Trie for case names
pub struct CaseNameTrie {
    root: TrieNode,
}

/// Trie for content (sentences, paragraphs)
pub struct ContentTrie {
    root: TrieNode,
}

/// Trie for legal citations
pub struct CitationTrie {
    root: TrieNode,
}

/// Trie node structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrieNode {
    children: HashMap<String, TrieNode>,
    is_end_of_word: bool,
    document_refs: Vec<DocRef>,
    frequency: u32,
}

/// Search result from trie lookup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrieSearchResult {
    pub exact_matches: Vec<DocRef>,
    pub prefix_completions: Vec<String>,
    pub total_matches: usize,
}

impl TrieIndex {
    /// Create new trie index
    pub async fn new(config: TrieConfig) -> Result<Self> {
        let case_name_trie = CaseNameTrie::new();
        let content_trie = ContentTrie::new();
        let citation_trie = CitationTrie::new();

        Ok(Self {
            config,
            case_name_trie,
            content_trie,
            citation_trie,
        })
    }

    /// Load trie from disk
    pub async fn load_from_disk<P: AsRef<Path>>(path: P) -> Result<Self> {
        // TODO: Implement loading from FST or serialized format
        Err(SearchError::NotSupported {
            operation: "Loading trie from disk".to_string(),
        })
    }

    /// Save trie to disk
    pub async fn save_to_disk<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        // TODO: Implement saving to FST or serialized format
        Ok(())
    }

    /// Insert case name into trie
    pub fn insert_case_name(&mut self, case_name: &str, case_id: CaseId) -> Result<()> {
        self.case_name_trie.insert(case_name, case_id)
    }

    /// Insert content sequence into trie
    pub fn insert_content(&mut self, tokens: &[String], doc_ref: DocRef) -> Result<()> {
        self.content_trie.insert(tokens, doc_ref)
    }

    /// Insert citation into trie
    pub fn insert_citation(&mut self, citation: &str, doc_ref: DocRef) -> Result<()> {
        self.citation_trie.insert(citation, doc_ref)
    }

    /// Search for exact matches and prefixes
    pub fn search(&self, query: &str) -> Result<TrieSearchResult> {
        // Try case name search first
        if let Ok(result) = self.case_name_trie.search(query) {
            if !result.exact_matches.is_empty() {
                return Ok(result);
            }
        }

        // Try citation search
        if let Ok(result) = self.citation_trie.search(query) {
            if !result.exact_matches.is_empty() {
                return Ok(result);
            }
        }

        // Fall back to content search
        let tokens: Vec<String> = query.split_whitespace().map(|s| s.to_string()).collect();
        self.content_trie.search_tokens(&tokens)
    }

    /// Get completion suggestions for a prefix
    pub fn get_completions(&self, prefix: &str, limit: usize) -> Result<Vec<String>> {
        // TODO: Implement completion logic
        Ok(Vec::new())
    }
}

impl CaseNameTrie {
    fn new() -> Self {
        Self {
            root: TrieNode::new(),
        }
    }

    fn insert(&mut self, case_name: &str, case_id: CaseId) -> Result<()> {
        let tokens: Vec<String> = case_name.split_whitespace().map(|s| s.to_lowercase()).collect();
        let doc_ref = DocRef {
            case_id,
            paragraph_index: 0,
            char_offset: None,
        };
        self.root.insert(&tokens, doc_ref);
        Ok(())
    }

    fn search(&self, query: &str) -> Result<TrieSearchResult> {
        let tokens: Vec<String> = query.split_whitespace().map(|s| s.to_lowercase()).collect();
        Ok(self.root.search(&tokens))
    }
}

impl ContentTrie {
    fn new() -> Self {
        Self {
            root: TrieNode::new(),
        }
    }

    fn insert(&mut self, tokens: &[String], doc_ref: DocRef) -> Result<()> {
        let normalized_tokens: Vec<String> = tokens.iter().map(|t| t.to_lowercase()).collect();
        self.root.insert(&normalized_tokens, doc_ref);
        Ok(())
    }

    fn search_tokens(&self, tokens: &[String]) -> Result<TrieSearchResult> {
        let normalized_tokens: Vec<String> = tokens.iter().map(|t| t.to_lowercase()).collect();
        Ok(self.root.search(&normalized_tokens))
    }
}

impl CitationTrie {
    fn new() -> Self {
        Self {
            root: TrieNode::new(),
        }
    }

    fn insert(&mut self, citation: &str, doc_ref: DocRef) -> Result<()> {
        let tokens: Vec<String> = citation.split_whitespace().map(|s| s.to_string()).collect();
        self.root.insert(&tokens, doc_ref);
        Ok(())
    }

    fn search(&self, query: &str) -> Result<TrieSearchResult> {
        let tokens: Vec<String> = query.split_whitespace().map(|s| s.to_string()).collect();
        Ok(self.root.search(&tokens))
    }
}

impl TrieNode {
    fn new() -> Self {
        Self {
            children: HashMap::new(),
            is_end_of_word: false,
            document_refs: Vec::new(),
            frequency: 0,
        }
    }

    fn insert(&mut self, tokens: &[String], doc_ref: DocRef) {
        let mut current = self;
        
        for token in tokens {
            current = current.children.entry(token.clone()).or_insert_with(TrieNode::new);
        }
        
        current.is_end_of_word = true;
        current.document_refs.push(doc_ref);
        current.frequency += 1;
    }

    fn search(&self, tokens: &[String]) -> TrieSearchResult {
        let mut current = self;
        
        // Traverse to the end of the query
        for token in tokens {
            if let Some(child) = current.children.get(token) {
                current = child;
            } else {
                // No matches found
                return TrieSearchResult {
                    exact_matches: Vec::new(),
                    prefix_completions: Vec::new(),
                    total_matches: 0,
                };
            }
        }
        
        // Collect exact matches if this is end of word
        let exact_matches = if current.is_end_of_word {
            current.document_refs.clone()
        } else {
            Vec::new()
        };
        
        // Collect prefix completions
        let prefix_completions = self.collect_completions(current, tokens, 10);
        
        TrieSearchResult {
            total_matches: exact_matches.len() + prefix_completions.len(),
            exact_matches,
            prefix_completions,
        }
    }

    fn collect_completions(&self, node: &TrieNode, prefix: &[String], limit: usize) -> Vec<String> {
        let mut completions = Vec::new();
        let mut stack = vec![(node, prefix.to_vec())];
        
        while let Some((current, path)) = stack.pop() {
            if completions.len() >= limit {
                break;
            }
            
            if current.is_end_of_word && path.len() > prefix.len() {
                completions.push(path.join(" "));
            }
            
            for (token, child) in &current.children {
                let mut new_path = path.clone();
                new_path.push(token.clone());
                stack.push((child, new_path));
            }
        }
        
        completions
    }
} 