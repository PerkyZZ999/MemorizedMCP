## MCP Tools API

This server exposes MCP tools grouped by domain. Names use dot‑notation; legacy underscore variants are supported as aliases for compatibility.

- Naming: `memory.add`, `document.store`, `system.status`, `advanced.consolidate`
- Aliases: `add_memory`, `store_document`, `status`, `consolidate_memories`

### Common Conventions

- All requests/returns are JSON objects.
- `id` fields are strings (uuid/ksuid). Timestamps are epoch ms.
- Errors return `{ error: { code, message, details? } }`.

---

### Memory

#### memory.add (alias: add_memory)

- Purpose: Add a memory; classify STM/LTM; index into vector/graph/text; link docs.
- Params:
  - `content: string`
  - `metadata?: object`
  - `references?: { docId?: string, path?: string, chunkId?: string, score?: number }[]`
  - `layerHint?: "STM" | "LTM"`
- Returns:
  - `{ id, layer, entities: Entity[], graphLinks: number, indices: { vector: boolean, text: boolean } }`

Example

```json
{
  "tool": "memory.add",
  "params": { "content": "Project kickoff call notes", "layerHint": "STM" }
}
```

#### memory.search (alias: search_memory)

- Purpose: Hybrid search over vector, graph, and text indices with temporal filters.
- Params:
  - `query: string`
  - `filters?: { timeFrom?: number, timeTo?: number, types?: string[], layer?: string }`
  - `limit?: number`
- Returns:
  - `{ results: [{ id, score, snippet?, layer, timeline?, docRefs?: DocRef[] }], tookMs }`

#### memory.update (alias: update_memory)

- Params: `{ id: string, content?: string, metadata?: object }`
- Returns: `{ id, version: number, reembedded: boolean, updatedIndices: string[] }`

#### memory.delete (alias: delete_memory)

- Params: `{ id: string, backup?: boolean }`
- Returns: `{ id, deleted: boolean, cascaded: number }`

---

### Document

#### document.store (alias: store_document)

- Purpose: Ingest PDF/Markdown; parse, chunk, embed, and index.
- Params:
  - `path?: string` (absolute/relative)
  - `mime?: "pdf" | "md" | "txt"`
  - `content?: string` (for md/txt)
  - `metadata?: object`
- Returns: `{ id, hash, chunks: number, entities: number, summary?: string }`

#### document.retrieve (alias: retrieve_document)

- Params: `{ id?: string, hash?: string, path?: string, includeText?: boolean }`
- Returns: `{ id, path, hash, metadata, text?, chunks: ChunkHeader[] }`

#### document.analyze (alias: analyze_document)

- Params: `{ id: string, includeEntities?: boolean, includeSummary?: boolean }`
- Returns: `{ id, keyConcepts: string[], entities?: Entity[], summary?: string }`

---

### System

#### system.status (alias: status)

- Returns: `{ uptimeMs, indices: { vector:{ items }, text:{ docs }, graph:{ nodes, edges } }, storage:{ hotMb, warmMb, coldMb }, queue:{ ingest, indexing }, health:"ok"|"degraded" }`

#### system.cleanup (alias: cleanup)

- Params: `{ reindex?: boolean, compact?: boolean }`
- Returns: `{ compacted: boolean, reindexed: boolean, freedMb?: number }`

#### system.backup (alias: backup)

- Params: `{ destination?: string, includeIndices?: boolean }`
- Returns: `{ path, sizeMb, tookMs }`

#### system.restore (alias: restore)

- Params: `{ source: string, includeIndices?: boolean }`
- Returns: `{ restored: boolean, tookMs }`

---

### Advanced

#### advanced.consolidate (alias: consolidate_memories)

- Purpose: Promote STM → LTM based on importance and access patterns.
- Params: `{ dryRun?: boolean, limit?: number }`
- Returns: `{ promoted: number, candidates: number, tookMs }`

#### advanced.analyze_patterns

- Params: `{ window?: { from?: number, to?: number }, minSupport?: number }`
- Returns: `{ patterns: [{ concept:string, support:number, trend:"up"|"down"|"flat" }] }`

#### advanced.reindex

- Params: `{ vector?: boolean, text?: boolean, graph?: boolean }`
- Returns: `{ vector:boolean, text:boolean, graph:boolean, tookMs }`

---

### Types

- Entity: `{ type:"Person"|"Organization"|"Concept"|"Location", value:string, aliases?:string[] }`
- DocRef: `{ docId:string, chunkId?:string, score?:number }`
- ChunkHeader: `{ id:string, position:{ page?:number, start:number, end:number } }`

### Error Codes

- `INVALID_INPUT`: validation failed
- `NOT_FOUND`: resource does not exist
- `CONFLICT`: versioning or dependency issues
- `UNAVAILABLE`: subsystem not ready
- `INTERNAL_ERROR`: unexpected failure
