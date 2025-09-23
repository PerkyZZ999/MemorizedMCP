# ADR-0001: Architecture

## Context
We need a local-first, performant memory system combining vector search, knowledge graph, and full-text retrieval.

## Decision
Implement a Rust MCP server with:
- sled for warm storage; tantivy for text; custom HNSW-like neighbor graph for vectors
- petgraph for KG; tokio/rayon for concurrency; axum for HTTP
- Hybrid fusion search with tunable weights and caching

## Consequences
- Low latency, no external dependencies
- Added operational complexity (indices, maintenance)
- Clear tool surface for IDE/agent integrations
