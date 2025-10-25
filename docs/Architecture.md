## System Architecture

### High-Level Topology

- Cursor IDE → MCP Client → Rust MCP Server (stdio and HTTP transports)
- MCP Server → Memory Controller → Subsystems:
  - Document Store
  - Knowledge Graph Engine
  - Vector Store (HNSW)
  - Full‑Text Search (Tantivy)
  - Hybrid Search Engine (fusion/ranking)
  - Storage Layer (sled/rocksdb + caching)

### Components and Responsibilities

- MCP Server (rmcp, tokio, axum, serde)
  - Tool discovery/registration, request handling, error propagation
  - Transport abstraction (stdio, HTTP), logging/metrics
- Memory Controller
  - Orchestrates memory operations across vector/graph/text indices
  - Manages STM/LTM/Episodic/Semantic/Documentary layers
  - Consolidation (STM→LTM), decay, conflict resolution
- Document Store
  - Ingestion: PDF/Markdown parsing (lopdf, pulldown‑cmark)
  - Hashing/dedup (sha2), versioning, persistence
  - Semantic chunking (text‑splitter), embeddings, entity/concept extraction
- Knowledge Graph (petgraph)
  - Typed nodes/edges, temporal validity periods, multi‑hop traversal
  - Entity/relationship extraction and updates, reference linking
- Vector Store
  - Local embedding generation (fastembed/rig‑core)
  - HNSW ANN index for fast similarity search; batch embedding
- Full‑Text Search (tantivy)
  - Content indexing with metadata; query expansion/synonyms
  - Relevance ranking; integration in fusion layer
- Hybrid Search Engine
  - Fuses vector, graph, and text results with tuned scoring
  - Temporal/semantic filters; caching and batch ops
- Storage Layer
  - Multi‑tier (hot/warm/cold/index) with compression (lz4_flex)
  - sled/rocksdb for metadata/content; bincode for serialization
  - Caches (LRU) and object pools; lazy loading of large blobs

### Data Flows

- Add Memory
  1. Analyze/classify (STM vs LTM), extract entities/relationships
  2. Generate embeddings; write to vector index (HNSW)
  3. Upsert KG nodes/edges with temporal properties
  4. Index text in Tantivy; link documentary references

- Search Memory
  1. Run vector, graph, and text queries in parallel
  2. Apply temporal filters and layer‑aware constraints
  3. Fuse results; score and rank; return enriched context

- Update/Delete Memory
  - Update: re‑embed as needed; update indices and graph consistency
  - Delete: dependency check; cascade cleanup; create backups

- Document Ingestion
  1. Parse PDF/Markdown → extract structured text and metadata
  2. Chunk + embed; store chunks with positions and references
  3. Index chunks in vector and full‑text; link entities to KG

### Cross-Cutting Concerns

- Concurrency: tokio async + rayon data parallelism; parking_lot locks
- Observability: structured logging, metrics, dashboards, benchmarks (Criterion)
- Configuration: env‑driven; feature flags for performance tuning
- Error Handling: anyhow for rich contexts; recovery mechanisms
- Security/Privacy: local‑first, controlled access, safe parsing

### Integration Points

- Cursor integration: tool registry, response formatting, error surfacing
- MCP Tools: Memory (add/search/update/delete), Document (store/retrieve/analyze), System (status/cleanup/backup), Advanced (consolidate/analyze patterns)

### Deployment Model

- Single binary, local‑first server
- Stdio transport for IDE workflows; optional HTTP endpoint
- Durable local storage directories per environment
