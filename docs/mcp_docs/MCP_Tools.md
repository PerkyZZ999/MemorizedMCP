# MCP Tool Calls Reference

## Conventions
- All arguments and returns are JSON.
- Timestamps are epoch milliseconds.
- MCP responses use structured content:
  - tools/call result: `{"content": [ { "type": "json", "json": <payload> } ] }`
- Errors:
  - `-32000`: Underlying HTTP error (message includes status/body)
  - `-32601`: Unknown method/tool
- HTTP parity: Tools proxy to HTTP endpoints; request shapes match HTTP handlers.

---

## Memory

### memory.add
- Description: Add a memory; classify STM/LTM; index into vector/text/KG; optionally link docs.
- Arguments:
```json
{
  "content": "string",
  "metadata": {"any": "optional"},
  "references": [{ "docId": "string", "chunkId": "string(optional)", "score": 0.0 }],
  "layer_hint": "STM|LTM",          
  "session_id": "string(optional)",
  "episode_id": "string(optional)"
}
```
- Returns: `{ "id": string, "layer": "STM"|"LTM" }`
- Notes:
  - Field is `layer_hint` (snake case) in JSON.
  - `references[].docId/chunkId/score` are supported; missing `score` is computed by Jaccard over entities.

### memory.search
- Description: Hybrid search across vector, graph, and text indices with temporal filters.
- Arguments (GET semantics):
```json
{
  "q": "string",                  
  "limit": 10,                      
  "layer": "STM|LTM(optional)",
  "episode": "string(optional)",
  "from": 0,                       
  "to": 9999999999999              
}
```
- Returns: `{ "results": [{ "id", "score", "layer", "docRefs"?, "explain"? }], "tookMs": number }`
- Notes:
  - Query parameter is `q` (alias fields like `query` are not interpreted by the server).
  - `from`/`to` are epoch ms filters.

### memory.update
- Arguments:
```json
{ "id": "string", "content": "string(optional)", "metadata": { } }
```
- Returns: `{ "id", "version", "reembedded": boolean, "updatedIndices": ["text","vector"] }`

### memory.delete
- Arguments:
```json
{ "id": "string", "backup": true }
```
- Returns: `{ "deleted": boolean, "cascaded": boolean }`

---

## Document

### document.store
- Description: Ingest PDF/Markdown/Text; parse, chunk, embed, index; version by path.
- Arguments (POST):
```json
{ "path": "string(optional)", "mime": "pdf|md|txt(optional)", "content": "string(optional)", "metadata": { } }
```
- Returns: `{ "id": string, "hash": string, "chunks": number }`

### document.retrieve
- Description: Retrieve document by id/hash/path.
- Arguments (GET):
```json
{ "id": "string" }   // or { "hash": "..." } or { "path": "..." }
```
- Returns: `{ "id": string, "chunks": [{ "id": string, "position": { "start": number, "end": number } }], "metadata": object|null }`
- Notes:
  - `includeText` is not currently used by the server.

### document.analyze
- Description: Analyze document; returns key concepts, entities, summary, and related docs.
- Arguments (GET):
```json
{ "id": "string" }
```
- Returns: `{ "id", "keyConcepts": string[], "entities": string[], "summary": string|null, "docRefs": [{ "docId": string, "score": number }] }`
- Notes:
  - Flags like `includeEntities`/`includeSummary` are currently ignored.

### document.refs_for_memory
- Arguments (GET): `{ "id": "<MEM_ID>" }`
- Returns: `{ "id": string, "docRefs": [{ "docId": string, "chunkId": string|null, "score": number }] }`

### document.refs_for_document
- Arguments (GET): `{ "id": "<DOC_ID>" }`
- Returns: `{ "id": string, "memories": [{ "memoryId": string, "chunkId": string|null, "score": number }] }`

### document.validate_refs
- Arguments (POST): `{ "fix": boolean }`
- Returns: `{ "invalid": string[], "removed": number|null }`

---

## System

### system.status
- Returns:
```json
{
  "uptime_ms": number,
  "indices": { "vector": {"items": number}, "text": {"docs": number}, "graph": {"nodes": number, "edges": number} },
  "storage": { "hot_mb": number, "warm_mb": number, "cold_mb": number },
  "metrics": { "count": number, "cacheHits": number, "cacheMisses": number, "avgMs": number, "lastMs": number, "p50Ms": number, "p95Ms": number, "qps1m": number },
  "memory": { "rss_mb": number, "stm_count": number, "ltm_count": number },
  "health": "ok"|"degraded"
}
```
- Notes:
  - Field names are snake_case for `memory` (rss_mb, stm_count, ltm_count).

### system.cleanup
- Arguments (POST): `{ "reindex": boolean, "compact": boolean }`
- Returns: `{ "removedText": number, "removedEdges": number, "reindexed": boolean, "compacted": boolean }`

### system.backup
- Arguments (POST): `{ "destination": "string(optional)", "includeIndices": boolean }`
- Returns: `{ "path": string, "sizeMb": number, "tookMs": number }`

### system.restore
- Arguments (POST): `{ "source": "string", "includeIndices": boolean }`
- Returns: `{ "restored": boolean, "validated": boolean, "tookMs": number }`

---

## Advanced

### advanced.consolidate
- Arguments (POST): `{ "dryRun": boolean, "limit": number }`
- Returns: `{ "promoted": number, "candidates": number, "tookMs": number }`

### advanced.analyze_patterns
- Arguments (POST): `{ "window": { "from": number, "to": number }, "minSupport": number }`
- Returns: `{ "patterns": [{ "concept": string, "support": number, "trend": "flat"|"up"|"down" }] }`

### advanced.reindex
- Arguments (POST): `{ "vector": boolean, "text": boolean, "graph": boolean }`
- Returns: `{ "vector": boolean, "text": boolean, "graph": boolean, "tookMs": number }`

### advanced.trends
- Arguments (POST): `{ "from": number, "to": number, "buckets": number }`
- Returns: `{ "timeline": [{ "start": number, "end": number, "STM": number, "LTM": number }] }`

### advanced.clusters
- Arguments (POST): `{}`
- Returns: `{ "clusters": [{ "docs": string[] }] }`

### advanced.relationships
- Arguments (POST): `{}`
- Returns: `{ "relationships": [{ "group": string, "count": number }] }`

### advanced.effectiveness
- Arguments (POST): `{}`
- Returns: `{ "effectiveness": [{ "id": string, "score": number }] }`

---

## Examples

### Add + Search
```json
// memory.add
{"content":"Project kickoff notes"}

// memory.search
{"q":"kickoff","limit":5}
```

### Document Flow
```json
// document.store
{"mime":"md","content":"# Title\nHello"}

// document.analyze
{"id":"<DOC_ID>"}

// document.retrieve
{"id":"<DOC_ID>"}
```

### Ops
```json
// advanced.reindex
{"vector":true,"text":true,"graph":true}

// system.backup
{"destination":"./backups","includeIndices":true}

// system.restore
{"source":"./backups/<snapshot>","includeIndices":true}
```
