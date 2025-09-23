## Design Details

### Server Design

- Runtime and Transport
  - tokio for async; stdio and HTTP transports
  - axum for HTTP routing; tower/hyper middleware for observability and limits
- Protocol & Tools
  - rmcp for MCP protocol; tool registry with declarative metadata
  - Structured request/response models (serde), consistent error envelopes
- Error & Resilience
  - anyhow with context chains; retries where idempotent; circuit breakers on heavy ops
  - Graceful shutdown; background task supervision; panic boundaries

### Tooling Surface (MCP Tools)

- Memory Tools
  - add_memory, search_memory, update_memory, delete_memory
- Document Tools
  - store_document, retrieve_document, analyze_document
- System Tools
  - status, cleanup, backup, restore
- Advanced Tools
  - consolidate_memories (STM→LTM), analyze_patterns, reindex

### Memory Engine Design

- Layer Manager
  - STM: in‑memory, session‑scoped with expiration and LRU eviction
  - LTM: persistent, importance weighting, decay, consolidation markers
  - Episodic: timeline/thread model linking interactions and events
  - Semantic: typed knowledge graph of entities/relationships
  - Documentary: document references, evidence scoring, summaries
- Operations Flow
  - Add: classify → extract entities → embed → graph upsert → text index → link docs
  - Search: parallel vector/graph/text queries → temporal filters → fusion/ranking
  - Update/Delete: versioning, re‑embedding, index maintenance, cascading cleanup

### Document Processing Pipeline

- Parsers: lopdf (PDF), pulldown‑cmark (Markdown)
- Text handling: structure preservation; semantic chunking (text‑splitter)
- Embeddings: fastembed (batch), rig‑core; async pipelines
- Metadata: hashing (sha2) for dedup; versioning; provenance
- Indexing: vector (HNSW), full‑text (tantivy), KG linking (petgraph)

### Search & Ranking

- Vector: HNSW parameters tuned per use case; SIMD distance calcs
- Graph: multi‑hop traversals; relationship/evidence weighting; temporal validity
- Text: BM25 + query expansion/synonyms; field boosts
- Fusion: weighted ensemble over vector/graph/text; cache hot paths

### Storage & Serialization

- Multi‑tier storage: hot (in‑memory caches), warm (sled), cold (compressed)
- Content store: rocksdb or sled with lz4_flex compression
- Serialization: bincode for speed; JSON for external interchange

### Concurrency & Performance

- rayon for data parallel batch operations (embedding, parsing)
- parking_lot for fast synchronization; dashmap for concurrent maps
- Object pooling for large buffers; lazy loading for large documents

### Observability & Tooling

- Logging: structured, context‑rich logs; spans for pipelines
- Metrics: ingestion rates, query latency, index sizes, cache hit rates
- Benchmarks: Criterion harnesses for critical paths
- Dashboards: minimal local UI or metrics endpoints for visualization

### Configuration & Tuning

- Environment variables for directories/paths and parameter tuning
- Feature flags for experimental capabilities (e.g., consolidation cadence)

### Security & Safety

- Local‑first isolation; avoid untrusted network access by default
- Safe parsing of PDFs/Markdown; input validation; resource limits
- Backups and integrity checks; restore verification hooks
