# 🧠 MemorizedMCP

**A high-performance hybrid memory system for AI agents** built on the Model Context Protocol (MCP). MemorizedMCP combines knowledge graphs, vector embeddings, full-text search, and documentary memory to provide intelligent, context-aware information storage and retrieval.

[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)
[![MCP](https://img.shields.io/badge/MCP-2025--10--01-blue.svg)](https://modelcontextprotocol.io/)
[![License](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)

---

## ✨ Features

### 🗄️ **Multi-Layer Memory Architecture**
- **STM (Short-Term Memory)**: Fast, ephemeral storage with automatic expiration
- **LTM (Long-Term Memory)**: Persistent knowledge with importance-based retention
- **Automatic Consolidation**: Smart promotion from STM → LTM based on access patterns

### 🔗 **Knowledge Graph (NEW!)**
- Create and manage entities, documents, memories, and episodes
- Rich relationships with custom edge types (MENTIONS, EVIDENCE, RELATED)
- Tag-based organization and filtering
- Graph traversal and pattern discovery
- Full CRUD operations on nodes and edges

### 📚 **Documentary Memory**
- Ingest PDF, Markdown, and text documents
- Automatic chunking and embedding
- Entity extraction and linking
- Document versioning by path
- Cross-document relationship discovery

### 🔍 **Hybrid Search**
- **Vector Search**: Semantic similarity via embeddings
- **Full-Text Search**: BM25-style keyword matching (Tantivy + Sled)
- **Graph Search**: Entity-based traversal and relation queries
- **Temporal Filters**: Query by time ranges and episodes
- **Query Caching**: Sub-second responses for hot queries

### ⚡ **Performance & Scalability**
- Query percentiles tracking (p50, p95) for health monitoring
- Concurrent request handling with semaphore-based backpressure
- Incremental indexing and background maintenance
- Memory-mapped storage for efficient disk I/O

### 🛠️ **Developer-Friendly**
- **MCP Protocol**: Standard tools interface for AI agents
- **HTTP API**: RESTful endpoints for direct integration
- **Backup/Restore**: Snapshot-based data portability
- **Validation Tools**: Integrity checks and auto-repair

---

## 📋 Table of Contents

- [Architecture](#-architecture)
- [Installation](#-installation)
- [Quick Start](#-quick-start)
- [Usage Examples](#-usage-examples)
- [API Documentation](#-api-documentation)
- [Configuration](#-configuration)
- [Development](#-development)
- [Contributing](#-contributing)
- [License](#-license)

---

## 🏗️ Architecture

MemorizedMCP uses a **fusion architecture** that combines multiple indexing strategies:

```
┌─────────────────────────────────────────────────────────────┐
│                     MCP Protocol Layer                       │
│            (tools/call, tools/list, notifications)           │
└──────────────────────────┬──────────────────────────────────┘
                           │
┌──────────────────────────▼──────────────────────────────────┐
│                   HTTP API & Router (Axum)                   │
└─────┬──────────┬──────────┬──────────┬──────────┬──────────┘
      │          │          │          │          │
┌─────▼────┐ ┌──▼───┐ ┌────▼────┐ ┌───▼────┐ ┌──▼──────┐
│ Vector   │ │ Text │ │ Graph   │ │Document│ │ System  │
│ Index    │ │Index │ │  (KG)   │ │ Store  │ │ Mgmt    │
│(HNSW ANN)│ │(BM25)│ │(Nodes+  │ │(Chunks)│ │(Backup) │
│          │ │      │ │ Edges)  │ │        │ │         │
└─────┬────┘ └──┬───┘ └────┬────┘ └───┬────┘ └──┬──────┘
      │         │          │          │         │
      └─────────┴──────────┴──────────┴─────────┘
                           │
                    ┌──────▼──────┐
                    │ Sled KV     │
                    │ (Embedded)  │
                    └─────────────┘
```

**Storage Tiers:**
- **Hot**: Query cache (in-memory, TTL-based)
- **Warm**: Primary KV store (Sled, memory-mapped)
- **Cold**: Archived snapshots (filesystem)
- **Index**: Tantivy full-text index (disk-backed)

---

## 🚀 Installation

### Prerequisites
- **Rust 1.75+** (for building from source)
- **Windows 10+** / **Linux** / **macOS**

### Build from Source
```bash
git clone https://github.com/PerkyZZ999/MemorizedMCP.git
cd MemorizedMCP
cargo build --release
```

The binary will be at `target/release/memory_mcp_server` (or `.exe` on Windows).

---

## 🎯 Quick Start

### 1. Start the Server

**MCP Mode (STDIO):**
```bash
memory_mcp_server
```

**HTTP Mode:**
```bash
memory_mcp_server --bind 127.0.0.1:8080
```

### 2. Configure Cursor/MCP Client

Add to your MCP config (`~/.cursor/mcp.json` or similar):
```json
{
  "mcpServers": {
    "memorized": {
      "command": "C:/path/to/memory_mcp_server.exe",
      "args": [],
      "env": {
        "DATA_DIR": "./data",
        "HTTP_BIND": "127.0.0.1:8080"
      }
    }
  }
}
```

### 3. Verify Health
```bash
# Via MCP tool
system.status

# Or via HTTP
curl http://127.0.0.1:8080/status
```

### 4. Ingest Your First Document
```json
// Tool: document.store
{
  "mime": "md",
  "content": "# My Project\nThis is a Rust-based memory system."
}
```

### 5. Add a Memory
```json
// Tool: memory.add
{
  "content": "MemorizedMCP uses hybrid search for fast retrieval",
  "layer_hint": "LTM",
  "references": [{ "docId": "<doc_id_from_step_4>" }]
}
```

### 6. Search Your Knowledge
```json
// Tool: memory.search
{
  "q": "hybrid search",
  "limit": 10
}
```

---

## 💡 Usage Examples

### Knowledge Graph Operations

**Create an Entity:**
```json
// Tool: kg.create_entity
{ "entity": "Rust" }
```

**Tag an Entity:**
```json
// Tool: kg.tag_entity
{
  "entity": "Rust",
  "tags": ["programming-language", "systems"]
}
```

**Create a Relation:**
```json
// Tool: kg.create_relation
{
  "src": "Entity::Rust",
  "dst": "Entity::WebAssembly",
  "relation": "COMPILES_TO"
}
```

**Search Entities by Tag:**
```json
// Tool: kg.get_tags
{ "tag": "programming-language" }
```

### Memory Management

**Add Memory with Episode Context:**
```json
// Tool: memory.add
{
  "content": "User prefers dark mode for code editor",
  "layer_hint": "STM",
  "session_id": "session_123",
  "episode_id": "setup_preferences"
}
```

**Search with Temporal Filters:**
```json
// Tool: memory.search
{
  "q": "dark mode",
  "from": 1704067200000,
  "to": 1735689600000,
  "layer": "STM"
}
```

**Consolidate STM → LTM:**
```json
// Tool: advanced.consolidate
{
  "dryRun": false,
  "limit": 50
}
```

---

## 📖 API Documentation

### MCP Tools Reference
- **[MCP_Tools.md](docs/mcp_docs/MCP_Tools.md)** - Complete tool catalog with request/response schemas
- **[User-Guide.md](docs/mcp_docs/User-Guide.md)** - End-user guide for MCP clients
- **[Cursor Rules](/.cursor/rules/guide-for-using-mcp-servers.mdc)** - AI agent integration patterns

### Architecture Docs
- **[Architecture.md](docs/Architecture.md)** - System design and components
- **[Storage.md](docs/Storage.md)** - Storage tiers and indexing strategies
- **[Memory-Layers.md](docs/Memory-Layers.md)** - STM/LTM behavior and consolidation

### Operations
- **[Operations.md](docs/Operations.md)** - Deployment and monitoring
- **[Runbook.md](docs/mcp_docs/Runbook.md)** - Incident response procedures
- **[Troubleshooting.md](docs/mcp_docs/Troubleshooting.md)** - Common issues and fixes

---

## ⚙️ Configuration

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `HTTP_BIND` | `127.0.0.1:8080` | HTTP server address (set empty to disable) |
| `DATA_DIR` | `./data` | Root directory for storage tiers |
| `STM_CLEAN_INTERVAL_MS` | `60000` | STM eviction check interval |
| `LTM_DECAY_PER_CLEAN` | `0.99` | LTM importance decay multiplier |
| `FUSION_CACHE_TTL_MS` | `3000` | Query cache time-to-live |
| `MAX_CONCURRENT_INGEST` | `4` | Document ingestion concurrency limit |
| `STATUS_P95_MS_THRESHOLD` | `250` | P95 latency threshold for health degradation |

### CLI Arguments
```bash
memory_mcp_server [OPTIONS]

Options:
  --bind <ADDR>      HTTP bind address (overrides HTTP_BIND)
  --data-dir <PATH>  Data directory root (overrides DATA_DIR)
  -h, --help         Print help
  -V, --version      Print version
```

---

## 🛠️ Development

### Running Tests
```bash
cargo test
```

### Benchmarks
```bash
cargo bench
```

### Linting
```bash
cargo clippy -- -D warnings
cargo fmt --check
```

### Building Documentation
```bash
cargo doc --open
```

### Project Structure
```
MemorizedMCP/
├── server/
│   ├── src/
│   │   ├── main.rs         # HTTP/MCP server
│   │   ├── kg.rs           # Knowledge graph ops
│   │   ├── embeddings.rs   # Vector index
│   │   ├── vector_index.rs # HNSW ANN
│   │   └── config.rs       # Configuration
│   └── benches/            # Performance benchmarks
├── docs/                   # Documentation
├── scripts/                # Utility scripts
└── data/                   # Runtime data (gitignored)
```

---

## 🤝 Contributing

Contributions are welcome! Please read our [Contributing Guide](CONTRIBUTING.md) for details on:
- Code style and conventions
- Pull request process
- Issue reporting guidelines
- Development workflow

### Areas for Contribution
- 🧪 **Testing**: Expand test coverage for edge cases
- 📊 **Benchmarks**: Add more realistic workload simulations
- 📚 **Docs**: Improve examples and tutorials
- 🔧 **Features**: See [Roadmap.md](docs/Roadmap.md) for planned features
- 🐛 **Bugs**: Check [Issues](https://github.com/PerkyZZ999/MemorizedMCP/issues)

---

## 📄 License

This project is licensed under the **MIT License** - see the [LICENSE](LICENSE) file for details.

---

## 🙏 Acknowledgments

- **[MCP Protocol](https://modelcontextprotocol.io/)** by Anthropic
- **[Tantivy](https://github.com/quickwit-oss/tantivy)** for full-text search
- **[Sled](https://github.com/spacejam/sled)** for embedded KV storage
- **[Axum](https://github.com/tokio-rs/axum)** for HTTP serving

---

## 📬 Contact

- **Issues**: [GitHub Issues](https://github.com/PerkyZZ999/MemorizedMCP/issues)
- **Discussions**: [GitHub Discussions](https://github.com/PerkyZZ999/MemorizedMCP/discussions)

---

<div align="center">

**Built with ❤️ in Rust**

⭐ Star this repo if you find it useful!

[Report Bug](https://github.com/PerkyZZ999/MemorizedMCP/issues) · [Request Feature](https://github.com/PerkyZZ999/MemorizedMCP/issues) · [Documentation](docs/)

</div>
=======
# MCP Quickstart

Use these minimal tool calls from Cursor (or any other IDE that supports MCP servers) to interact with MemorizedMCP.

## Installation
git clone the repo on your computer.
then add :
```JSON
"memorized-mcp": {
      "command": "your\\path\\to\\the\\git\\repo\\cloned\\target\\debug\\memory_mcp_server.exe",
      "args": [],
      "cwd": "your\\path\\to\\the\\git\\repo\\cloned\\MemorizedMCP",
      "env": {
        "DATA_DIR": "${workspaceFolder}\\.cursor\\memory",
        "RUST_LOG": "off"
      }
    }
```
NOTE: You can use ${workspaceFolder} or direct path to your project for the DATA_DIR.

## Status
- Tool: `system.status`
- Arguments: `{}`
- Returns: JSON with `uptime_ms, indices, storage, metrics, memory, health`

## Store a Document
- Tool: `document.store`
- Arguments:
```json
{"mime":"md","content":"# Title\nHello"}
```
- Returns: `{ "id", "hash", "chunks" }`

## Retrieve a Document
- Tool: `document.retrieve`
- Arguments (one of):
```json
{"id":"<DOC_ID>"}
{"path":"./README.md"}
```

## Analyze a Document
- Tool: `document.analyze`
- Arguments:
```json
{"id":"<DOC_ID>","includeEntities":true,"includeSummary":true}
```

## Add a Memory
- Tool: `memory.add`
- Arguments:
```json
{"content":"Project kickoff notes"}
```

## Search Memories
- Tool: `memory.search`
- Arguments:
```json
{"query":"kickoff","limit":5}
```

## Update a Memory
- Tool: `memory.update`
- Arguments:
```json
{"id":"<MEM_ID>","content":"updated"}
```

## Delete a Memory
- Tool: `memory.delete`
- Arguments:
```json
{"id":"<MEM_ID>","backup":true}
```

## Hybrid Search (Fusion)
- Tool: `memory.search` (use `query`) or hit HTTP `/search/fusion`
- Tip: use time window filters: `{ "from": 0, "to": 9999999999999 }`

## Maintenance & Ops
- `advanced.reindex` → `{ "vector":true, "text":true, "graph":true }`
- `system.cleanup` → `{ "compact":true }`
- `system.backup` → `{ "destination":"./backups", "includeIndices":true }`
- `system.restore` → `{ "source":"./backups/<snapshot>", "includeIndices":true }`

## References
- `document.refs_for_memory` → `{ "id":"<MEM_ID>" }`
- `document.refs_for_document` → `{ "id":"<DOC_ID>" }`
- `document.validate_refs` → `{ "fix": true }`

## Advanced Analytics
- `advanced.analyze_patterns` → `{ "window":{ "from":0, "to": 4102444800000 }, "minSupport": 2 }`
- `advanced.trends` → `{ "from": 0, "to": 4102444800000, "buckets": 10 }`
- `advanced.clusters` → `{}`
- `advanced.relationships` → `{}`
- `advanced.effectiveness` → `{}`
