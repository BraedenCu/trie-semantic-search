# Trie-Structured Legal Search Engine

A high-performance search engine for U.S. federal constitutional law, built with Rust. This engine combines trie-based lexical search with semantic vector search to provide rapid, accurate retrieval of legal precedents and clauses.

## Overview

This search engine is designed specifically for paralegals and legal researchers who need sub-second latency and high accuracy when searching through constitutional case law. It indexes the full corpus of U.S. constitutional case law (Supreme Court opinions and related federal cases) and returns deterministically ranked results with case metadata.

### Key Features

- **Corpus Coverage**: All U.S. federal constitutional law decisions (~6.5 million pages of case law)
- **Trie-Based Index**: Prefix tree structure for O(m) prefix searches and efficient storage
- **Semantic Vector Search**: Neural embeddings for conceptual similarity matching
- **Hybrid Search Algorithm**: Combines lexical filtering and semantic ranking
- **Real-Time Performance**: Sub-second query responses with "blink-of-an-eye" performance
- **Deterministic Results**: Consistent, reproducible search results
- **Rust Implementation**: High-performance, memory-safe core components

## Architecture

### Data Sources

The engine ingests data from authoritative legal datasets:

- **Caselaw Access Project (CAP)**: Harvard Law School dataset with 6.5 million U.S. court cases

### Core Components

#### 1. Data Ingestion Pipeline
- Bulk loading of historical cases
- Incremental updates for new cases
- Text cleaning and normalization
- Metadata extraction and storage

#### 2. Text Processing
- Word-level tokenization preserving legal tokens
- Case-folding for case-insensitive search
- Phrase identification for constitutional clauses
- Stop word preservation (important for legal context)

#### 3. Trie-Based Index
- **Case Name/Citation Trie**: Fast lookup of cases by name or citation
- **Content Trie**: Full-text indexing at sentence/paragraph level
- **Finite State Transducer (FST)**: Memory-mapped, read-only optimization

#### 4. Semantic Vector Index
- **Embedding Models**: Legal-BERT, CaseLaw-BERT, or MiniLM variants
- **ONNX Runtime**: Optimized inference for real-time performance
- **HNSW Index**: Approximate nearest neighbor search
- **Vector Database**: Optional Qdrant integration for advanced features

#### 5. Hybrid Search Algorithm
1. **Lexical Search**: Trie traversal for exact matches and prefixes
2. **Semantic Search**: Vector similarity for conceptual matching
3. **Result Merging**: Prioritize exact matches, include semantic results
4. **Filtering**: Apply metadata filters (court, date range, etc.)

## Technology Stack

### Core Dependencies

```toml
[dependencies]
# Data handling
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
reqwest = { version = "0.11", features = ["json"] }

# Trie and indexing
fst = "0.4"  # Finite State Transducer

# Machine learning and embeddings
onnxruntime = "0.1"  # ONNX model inference
tokenizers = "0.15"  # Text tokenization

# Vector search
hnsw_rs = "0.1"      # HNSW implementation
qdrant-client = "0.8" # Vector database client

# Web framework
actix-web = "4.4"    # High-performance web server
tokio = { version = "1.0", features = ["full"] }

# Utilities
bincode = "1.3"      # Binary serialization
sled = "0.34"        # Embedded database
```

### Key Rust Crates

- **`fst`**: Memory-mapped trie for lightning-fast string lookups
- **`onnxruntime`**: Optimized transformer model inference
- **`hnsw_rs`**: Approximate nearest neighbor search
- **`actix-web`**: High-performance async web framework
- **`serde`**: Serialization/deserialization for data handling

## API Design

### REST Endpoints

```rust
// Search endpoint
GET /search?query=freedom+of+speech&limit=10

// Response format
{
  "query": "freedom of speech",
  "results": [
    {
      "case_name": "Schenck v. United States",
      "citation": "249 U.S. 47 (1919)",
      "decision_date": "1919-03-03",
      "snippet": "... the most stringent protection of free speech would not protect...",
      "score": 1.0,
      "match_type": "exact"
    }
  ]
}
```

### GraphQL Support

```graphql
query {
  search(query: "freedom of press", court: "SCOTUS", limit: 5) {
    caseName
    citation
    score
    snippet
  }
}
```

## Performance Characteristics

### Speed Targets
- **Query Response**: < 100ms for typical queries
- **Prefix Search**: O(m) where m = query length
- **Vector Search**: Sub-linear time with HNSW
- **Concurrent Queries**: Hundreds of QPS support

### Memory Usage
- **Vector Storage**: ~1.5KB per embedding (384 dimensions)
- **Trie Index**: Compressed with FST for minimal overhead
- **Total Memory**: Scalable based on corpus size

### Optimization Strategies
- **Embedding Caching**: LRU cache for frequent queries
- **Parallel Processing**: Concurrent trie and vector search
- **Memory Mapping**: Zero-copy access to static indices
- **Early Termination**: Limit result collection for efficiency

## Deployment

### Architecture
- **Standalone Service**: Single Rust binary with HTTP server
- **Stateless Design**: All state in memory for horizontal scaling
- **Containerized**: Docker deployment with orchestration support
- **Health Monitoring**: `/health` endpoint for load balancers

### Update Strategy
- **Background Indexing**: Separate indexer process for updates
- **Hot Reloading**: Atomic index swapping for zero downtime
- **Incremental Updates**: Support for new case additions
- **Batch Processing**: Nightly rebuilds for large updates

## Development Roadmap

### Phase 1: Core Implementation
- [ ] Data ingestion pipeline
- [ ] Basic trie implementation
- [ ] Simple web API
- [ ] Initial corpus loading

### Phase 2: Semantic Search
- [ ] Embedding model integration
- [ ] Vector index implementation
- [ ] Hybrid search algorithm
- [ ] Performance optimization

### Phase 3: Production Features
- [ ] Advanced filtering
- [ ] GraphQL API
- [ ] Monitoring and logging
- [ ] Deployment automation

## Trade-offs and Limitations

### Design Decisions
- **Deterministic Results**: No generative AI, purely retrieval-based
- **Memory vs. Speed**: FST compression for large indices
- **Complexity vs. Customization**: Rust implementation for performance
- **Static vs. Dynamic**: Read-only indices for consistency

### Known Limitations
- **Index Size**: Large memory footprint for full corpus
- **Maintenance**: Complex integration of multiple components
- **Semantic Quality**: Depends on embedding model selection
- **No Reasoning**: Retrieval-only, no question answering

## Contributing

This project follows Rust best practices and conventions:

1. **Code Style**: Follow `rustfmt` and `clippy` guidelines
2. **Testing**: Comprehensive unit and integration tests
3. **Documentation**: Inline docs and examples
4. **Performance**: Benchmark critical paths

## License

[License information to be added]

## References

- [Caselaw Access Project](https://hls.harvard.edu/today/caselaw-access-project-launches-api-and-bulk-data-service/)
- [Free Law Project](https://free.law/projects/supreme-court-data)
- [Rust FST Crate](https://docs.rs/fst/latest/fst/)
- [Qdrant Vector Database](https://qdrant.tech/)
- [Rust-BERT](https://github.com/guillaume-be/rust-bert)

---

*This project implements a blueprint for high-performance legal search using Rust's performance and safety guarantees.* 