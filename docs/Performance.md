## Performance Optimization Guide

### Vector Database (HNSW) Tuning

- Parameters
  - M (max connections): trade memory vs recall; start 16–32
  - ef_construction: 100–400 depending on build latency tolerance
  - ef_search: dynamic per query (e.g., 64/128/256 tiered by latency budget)
- Distance
  - Prefer cosine/dot with normalized vectors; enable SIMD for distance calcs
- Index Lifecycle
  - Batch insert with rayon; snapshot checkpoints; background merges
  - Warmup common vectors; pin hot layers in memory

### Embedding Pipeline

- Batch generation to maximize throughput; bounded concurrency
- Cache embeddings for identical content hashes; reuse between versions
- Avoid small batch fragmentation; coalesce chunk streams

### Concurrent Processing

- Use rayon for CPU‑bound (parsing/embedding); tokio for I/O
- Reduce lock contention with fine‑grained locks (parking_lot) and lock‑free queues
- DashMap for concurrent maps; object pooling for large buffers

### Full‑Text Search (Tantivy)

- Tune schema with per‑field boosts; pre‑tokenize where applicable
- Optimize merge policy; incremental commits; searcher warming
- Query expansion and synonyms lookups cached

### Knowledge Graph

- Compact node/edge representations; avoid redundant properties
- Precompute traversals for frequent multi‑hop paths; memoize small neighborhoods

### Hybrid Fusion

- Weighted ensemble across vector/graph/text; calibrate with held‑out set
- Apply temporal priors; normalize scores; early‑exit when confidence is high
- Cache top‑k for popular queries and session‑local contexts

### Memory & Storage

- Multi‑tier caches: results, doc headers, embeddings
- Lazy loading of large blobs; streaming parse for PDFs
- Compression (lz4_flex) on cold tiers; bincode for hot paths

### Job Scheduling

- Background workers for reindex, consolidation, compaction
- Backpressure on ingestion; circuit breakers on heavy queries

### Benchmarking & Targets

- Criterion suites for: embedding throughput, HNSW search latency, fusion ranking
- Targets (initial):
  - P50 search < 30 ms; P95 < 120 ms on commodity hardware
  - Ingestion ≥ 50 PDF pages/s end‑to‑end (parallelized)
  - Reindex ≥ 100k vectors/min (batch + SIMD)
