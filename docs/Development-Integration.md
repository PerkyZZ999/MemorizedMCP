## Development & Integration Guide

### Prerequisites

- Rust (stable), Cargo, Rust Analyzer
- Optional: Cargo Watch, Criterion
- Local directories configured via env: DATA_DIR, BACKUP_DIR

### Project Setup

1) Clone repo and open in Cursor
2) Configure `.env` or environment variables:
   - DATA_DIR=./data
   - HOT_CACHE_MB=512
   - VECTOR_HNSW_PARAMS={"M":24,"efConstruction":200,"efSearch":128}
   - LOG_LEVEL=info
3) Build and run server
   - `cargo build`
   - `cargo run` (stdio by default); `--http` to enable HTTP endpoint
   - Recommended dev tools: Rust Analyzer, Cargo Watch (`cargo install cargo-watch`)

### Dev Environment Setup

- Editor
  - Install the Rust Analyzer extension (VS Code/Cursor)
  - Enable format on save and inlay hints (optional)
- Cargo Watch
  - Install: `cargo install cargo-watch`
  - Run with auto-reload: `cargo watch -x run`
- Env files
  - Copy `server/.env.example` to `server/.env` and adjust values
  - Common vars: `DATA_DIR`, `HTTP_BIND`/`PORT`, cache and consolidation params

### MCP Integration with Cursor

- Configure MCP client in Cursor to launch the Rust server (stdio)
- Tool discovery: server advertises tools on startup; verify in Cursor panel
- Test calls: invoke `system.status`, then `document.store` on a small MD file

### Development Workflow

- Use Cargo Watch for hot rebuilds: `cargo watch -x run`
- Logging: structured logs with spans for pipelines
- Testing: unit tests for parsers/indices; integration tests for tool endpoints

### Local Storage Layout

- `DATA_DIR/hot` — in‑memory caches sized by HOT_CACHE_MB (metadata only)
- `DATA_DIR/warm` — sled KV stores for nodes, edges, docs, chunks, memories
- `DATA_DIR/cold` — compressed large blobs (lz4_flex)
- `DATA_DIR/index` — HNSW files and Tantivy directories
 - `DATA_DIR/settings` — persisted system settings (sled tree)

### Configuration Reference

- CONSOLIDATION_INTERVAL (e.g., "15m")
- DECAY_PARAMS (e.g., `{ "halfLifeDays": 30 }`)
- REINDEX_BATCH_SIZE (e.g., 10000)
- TANTIVY_SCHEMA (optional JSON override)
- STM_MAX_ITEMS (cap STM entries; enforce LRU when exceeded)
- LTM_DECAY_PER_CLEAN (e.g., 0.99; applied each maintenance pass)
- LTM_STRENGTHEN_ON_ACCESS (e.g., 1.05 multiplier when accessed)
- STM_STRENGTHEN_DELTA (e.g., 0.05 additive when accessed)
- CONSOLIDATE_IMPORTANCE_MIN (e.g., 1.5; promotion threshold)
- CONSOLIDATE_ACCESS_MIN (e.g., 3; access_count promotion threshold)
- FUSION_CACHE_TTL_MS (e.g., 3000; cache TTL for hybrid search)

### Example Flows

- Ingest a document (Markdown)
  1) `document.store` with content
  2) `document.analyze` to extract concepts/entities
  3) `memory.add` referencing the document chunk(s)

- Run a hybrid search
  1) `memory.search` with `filters.timeFrom/timeTo`
  2) Review docRefs and open supporting documents

### CI/CD Notes

- Lint and test in CI; run Criterion benchmarks on nightly schedule
- Package single binary artifact; provide config via env on target hosts
