## MCP Server for Enhanced Memory — Overview

### Executive Summary

This project delivers a high‑performance, local‑first MCP Server in Rust that equips AI agents with an advanced hybrid memory system. It combines a knowledge graph, vector embeddings, and full‑text search with a novel documentary memory capability, enabling accurate recall of exact documents alongside rich semantic relationships.

- Rust performance: 10–100× faster than Python/Node alternatives
- Hybrid graph + vector: ~3.4× accuracy improvement over vector‑only
- Local‑first: full data sovereignty, zero cloud dependency
- Documentary memory: exact document recall with context and references
- Mem0‑inspired layers: distinct STM/LTM plus episodic, semantic, and documentary memory

### Scope and Goals

- Build a production‑grade Rust MCP server exposing Memory, Document, System, and Advanced operations
- Implement a Mem0‑inspired multi‑layer memory engine (STM, LTM, Episodic, Semantic, Documentary)
- Provide a hybrid retrieval pipeline that fuses vector, graph, and full‑text search
- Process and index documents (PDF/Markdown) with semantic chunking and embeddings
- Ensure robust persistence, performance, and observability with fully local storage

### Core Architecture (High‑Level)

- Cursor IDE ↔ MCP Client ↔ Rust MCP Server
- Server orchestrates a Memory Controller over four subsystems:
  - Document Store: PDF/Markdown ingestion, hashing, versioning, storage
  - Knowledge Graph: typed nodes/edges, temporal tracking, relationship mapping
  - Vector Store: local embeddings, HNSW index for ANN search
  - Hybrid Search Engine: query fusion and ranking across vector/graph/text

### Memory Layers (Mem0‑Inspired)

- Short‑Term Memory (STM): session‑scoped, fast in‑memory, relevance‑weighted
- Long‑Term Memory (LTM): persistent with importance weighting and decay
- Episodic Memory: temporal threads of conversation/events with timeline awareness
- Semantic Memory: factual knowledge and entity relationships in a typed KG
- Documentary Memory: document references, semantic chunks, summaries, evidence links

### Recommended Rust Stack (Local‑First)

- Core MCP/Web: rmcp, tokio, serde/serde_json, anyhow, axum, tower/hyper
- Hybrid storage/search: rig‑core, fastembed, hnsw, petgraph, sled, tantivy
- Document processing: lopdf, pulldown‑cmark, text‑splitter, sha2
- Performance & concurrency: rayon, dashmap, parking_lot, bincode, lz4_flex, rocksdb

### Key Capabilities

- Memory operations: add, search, update, delete with graph/vector/text integration
- Document operations: store, retrieve, analyze with semantic chunking and entity extraction
- Advanced features: consolidation (STM→LTM), reference linking, conflict resolution
- Operations & DX: backups, monitoring/analytics, benchmarks, Cursor integration

### Deliverables

- Production‑ready MCP server with stdio and HTTP transports
- Memory Controller + Document Store + Vector/Graph/Text indices
- Comprehensive Memory Operations API and documentation
- Roadmap, tasks, and performance/storage/development/operations guides
