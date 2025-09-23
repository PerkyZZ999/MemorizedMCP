## Roadmap

### Timeline Overview (10 Weeks)

- Phase 1 (Weeks 1–2): Core Infrastructure
- Phase 2 (Weeks 3–4): Document Processing Pipeline
- Phase 3 (Weeks 5–6): Mem0‑Inspired Memory Core
- Phase 4 (Weeks 7–8): Advanced Hybrid Features
- Phase 5 (Weeks 9–10): Operations & Production Readiness

### Phase 1 — Core Infrastructure (Weeks 1–2)

- Deliverables
  - Rust MCP server (stdio + HTTP) using rmcp
  - sled‑backed local storage engine and config/logging
  - Tool registry with Memory/Document/System/Advanced placeholders
  - Reliable Cursor IDE connection and error handling baseline
- Milestones
  - M1.1: Server boots, registers tools, responds to health
  - M1.2: Persistent KV operational; config/env wired; logging in place
  - M1.3: Basic tool handlers return mocked data; Cursor integration validated
- Exit Criteria
  - End‑to‑end request/response over stdio and HTTP
  - Persistent settings stored and reloaded; logs visible and structured

### Phase 2 — Document Processing Pipeline (Weeks 3–4)

- Deliverables
  - PDF/Markdown ingestion with metadata
  - Semantic chunking, hashing/dedup, versioning
  - Embeddings generation and storage; indices scaffolding
- Milestones
  - M2.1: Parse & extract text from PDF/MD with structure preservation
  - M2.2: Chunk + embed (batch); persist chunks and embeddings
  - M2.3: Index chunks in vector and text indices
- Exit Criteria
  - `document.store` ingests PDFs/MDs and exposes retrievable structured output

### Phase 3 — Mem0‑Inspired Memory Core (Weeks 5–6)

- Deliverables
  - HNSW vector index; petgraph knowledge graph
  - STM/LTM with session management, persistence, and decay
  - Temporal relationship tracking and episodic threading
- Milestones
  - M3.1: memory.add → embed, KG upsert, text index
  - M3.2: memory.search → hybrid (vector/graph/text) fusion
  - M3.3: memory.update/delete maintain indices and graph consistency
- Exit Criteria
  - Hybrid search returns relevant results with temporal filters

### Phase 4 — Advanced Hybrid Features (Weeks 7–8)

- Deliverables
  - Hybrid search fusion with tuned scoring and caching
  - Documentary reference linking with evidence scoring
  - Consolidation (STM→LTM), conflict resolution, invalidation & cleanup
- Milestones
  - M4.1: advanced.consolidate promotes STM→LTM with audit trail
  - M4.2: Cross‑document relationship detection and clustering
  - M4.3: Intelligent cache hot paths and performance tuning
- Exit Criteria
  - Document references enrich results; consolidation periodically runs

### Phase 5 — Production Readiness (Weeks 9–10)

- Deliverables
  - Memory Operations API finalized and documented
  - Monitoring/analytics dashboards and benchmarks
  - Backup/restore; export/import; storage compaction
  - Comprehensive tests and performance baselines
- Milestones
  - M5.1: system.status exposes key metrics
  - M5.2: system.backup/restore validated with integrity checks
  - M5.3: Benchmarks (Criterion) meet performance targets
- Exit Criteria
  - Release candidate build; docs complete and consistent across modules

### Risks & Mitigations

- Embedding performance bottlenecks → batch with rayon, cache embeddings
- Large PDF handling → lazy loading, chunked processing, memory caps
- Index update contention → background workers, backpressure, lock granularity
- Query fusion complexity → incremental rollout with A/B scoring configs

### Success Metrics

- P50/P95 search latency; ingestion throughput (docs/min)
- Accuracy uplift vs vector‑only baseline (~3.4× target)
- Storage footprint per N documents; backup/restore time
- Consolidation effectiveness (promotion rate, retrieval impact)
