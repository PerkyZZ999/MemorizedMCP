# MemorizedMCP User Guide

## Overview

A local-first Rust MCP server that exposes Memory, Document, System, and Advanced tools via stdio and optional HTTP.

## Prerequisites

- Rust stable toolchain
- Optional: cargo-watch, Prometheus/Grafana for metrics

## Install & Build

```bash
cargo build
```

## Run

- Stdio (default):

```bash
cargo run
```

- HTTP:

```bash
cargo run -- --http --data-dir ./data
```

## Configuration (env)

- DATA_DIR (default: ./data)
- HTTP_BIND (default: 127.0.0.1:8080)
- FUSION_CACHE_TTL_MS (default: 3000)
- FUSION_CACHE_MAX (default: 1000)
- STM_MAX_ITEMS, LTM_DECAY_PER_CLEAN
- CONSOLIDATE_IMPORTANCE_MIN, CONSOLIDATE_ACCESS_MIN
- STATUS_P95_MS_THRESHOLD, STATUS_RSS_MB_THRESHOLD
- PDF_MAX_PAGES, PDF_MAX_BYTES, PDF_MAX_TIME_MS

## Quick Workflow

1. Store a document

```bash
curl -s -X POST http://127.0.0.1:8080/document/store -H "content-type: application/json" \
  -d '{"mime":"md","content":"# Doc\ncontent here"}'
```

2. Analyze

```bash
curl -s "http://127.0.0.1:8080/document/analyze?id=<DOC_ID>"
```

3. Add a memory

```bash
curl -s -X POST http://127.0.0.1:8080/memory/add -H "content-type: application/json" \
  -d '{"content":"project kickoff notes"}'
```

4. Hybrid search

```bash
curl -s "http://127.0.0.1:8080/search/fusion?q=kickoff&limit=5"
```

5. Status & metrics

```bash
curl -s http://127.0.0.1:8080/status
curl -s http://127.0.0.1:8080/metrics
```

## MCP Usage (Cursor)

- Point Cursor MCP to the server (stdio).
- Discover tools via the MCP panel.
- Use entries from TOOLS.md for tool names/params.

## Backup & Restore

```bash
curl -s -X POST http://127.0.0.1:8080/system/backup -H "content-type: application/json" -d '{"destination":"./backups","includeIndices":true}'
curl -s -X POST http://127.0.0.1:8080/system/restore -H "content-type: application/json" -d '{"source":"./backups/<snapshot>","includeIndices":true}'
```

## Consolidation

```bash
curl -s -X POST http://127.0.0.1:8080/advanced/consolidate -H "content-type: application/json" -d '{"dryRun":false,"limit":25}'
```
