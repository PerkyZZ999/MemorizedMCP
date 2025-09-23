## Requirements

### Functional Requirements

- Memory Operations API
  - Add memory: classify STM/LTM, extract entities/relationships, embed, index
  - Search memory: multi‑modal (vector/graph/text), temporal filters, fusion ranking
  - Update memory: versioning, relationship updates, re‑embedding/index refresh
  - Delete memory: safe removal, dependency checks, cascading cleanup, backup
- Document Operations
  - Store: parse PDF/Markdown, hash/dedup, version, chunk, embed, index
  - Retrieve: by id/hash/path; return content, chunks, metadata, references
  - Analyze: entity extraction, key concepts, summaries, reference linking
- System Operations
  - Status: health, index sizes, cache stats, throughput
  - Cleanup: index maintenance, orphan prevention, compaction
  - Backup/Restore: export/import memory and indices
- Advanced Operations
  - Consolidate: STM→LTM promotion with importance scoring and decay
  - Analyze patterns: temporal trends, cross‑document clusters
  - Reindex: rebuild indices with new parameters

### Non‑Functional Requirements

- Performance
  - Low‑latency searches via HNSW and caching; batch embedding
  - High‑throughput document ingestion with parallel parsing
- Reliability
  - Durable persistence; integrity checks; recoverable operations
  - Graceful degradation; backpressure on heavy workloads
- Security & Privacy
  - Local‑first storage; no network dependency by default
  - Safe parsers; resource limits; input validation
- Observability
  - Structured logs, metrics, dashboards, and benchmarks
- Maintainability
  - Modular subsystems; clear interfaces; comprehensive docs

### Technology Stack

- Rust crates: rmcp, tokio, serde/serde_json, anyhow, axum, tower/hyper
- Storage & indices: sled, rocksdb, tantivy, hnsw, bincode, lz4_flex
- Graph & vectors: petgraph, fastembed, rig‑core
- Docs: lopdf, pulldown‑cmark, text‑splitter, sha2
- Perf: rayon, parking_lot, dashmap

### Environment & Setup

- Requires Rust toolchain (stable), Rust Analyzer, Cargo Watch (dev)
- Local directories for hot/warm/cold/index tiers
- Environment variables for:
  - DATA_DIR, HOT_CACHE_MB, VECTOR_HNSW_PARAMS, TANTIVY_SCHEMA, LOG_LEVEL
  - CONSOLIDATION_INTERVAL, DECAY_PARAMS, BACKUP_DIR

### Constraints & Assumptions

- Entirely local operation with optional HTTP exposure
- Embedding dimension: 384 (tuned with chosen model)
- Large PDFs supported with lazy loading and chunked processing

### Acceptance Criteria

- End‑to‑end: ingest PDF/Markdown → add/search/update/delete memories
- Query fusion returns relevant, ranked results with temporal filters
- STM→LTM consolidation runs and is observable; backups/restores succeed
- Documentation produced: overview, architecture, design, schema, roadmap, tasks, ops
