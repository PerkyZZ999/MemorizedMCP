# MCP Agent Usage

This directory contains agent-facing documentation for using the Memory MCP Server tools.

- TOOLS.md — canonical list of tool calls with params and examples
- API.md — developer API surface (see root docs)
- Dashboards.md — monitoring metrics and dashboards

## Quick Start

1. Discover tools (Cursor MCP panel or `GET /tools`).
2. Call `system.status` to verify health.
3. Ingest a document via `document.store` (content or path).
4. Add a memory via `memory.add` (optionally reference a doc chunk).
5. Retrieve with `memory.search` (use `filters.timeFrom/timeTo` as needed).
6. Periodically `advanced.consolidate` to promote STM → LTM.

## Error Contract

All errors return:
```json
{ "error": { "code": "INVALID_INPUT", "message": "...", "details": { } } }
```
Use the `code` to branch agent behavior.

## Health Degradation

The `system.status.health` becomes `degraded` if:
- `p95Ms` exceeds `STATUS_P95_MS_THRESHOLD`
- or `rssMb` exceeds `STATUS_RSS_MB_THRESHOLD`

Agents should backoff, reduce `limit`, or switch to cached/hybrid modes.
