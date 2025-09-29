# MCP Quickstart

Use these minimal tool calls from Cursor (or any other IDE that supports MCP servers) to interact with MemorizedMCP.

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
