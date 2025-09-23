# Testing Plan

This document enumerates manual and scripted tests to validate the MemorizedMCP server. Execute in order. HTTP examples use curl; MCP tools mirror the same behavior via stdio.

## 0. Environment
- Start server with HTTP: `cargo run -- --http --data-dir ./data`
- Ensure DATA_DIR is clean (optional): remove ./data before start
- Confirm health: `GET /status` expects `health: ok`

## 1. System & Health
1. GET /health: expect `{ status: "ok" }`
2. GET /status: fields present: `indices, storage, metrics, memory, health`
3. GET /metrics: Prometheus text includes `mcp_queries_total` and latency gauges
4. GET /tools: includes all descriptors (memory.*, document.*, system.*, advanced.*)

## 2. Document Pipeline
5. POST /document/store (Markdown via content)
   - Body: `{ "mime":"md","content":"# Title\nHello world" }`
   - Expect: `{ id, hash, chunks>0 }`
6. GET /document/retrieve?id=<id>
   - Expect: chunk headers and optional metadata
7. GET /document/analyze?id=<id>
   - Expect: `keyConcepts[]`, `entities[]`, `summary`, `docRefs[]` (maybe empty)
8. POST /document/store (Markdown with path)
   - Body: `{ "path":"./README.md", "mime":"md" }` (adjust path to a small file)
   - Expect: versioning updates (`doc_path_latest`, `doc_versions` are maintained)
9. GET /document/retrieve?path=... (path used above)
   - Expect: resolves latest id and returns chunk headers
10. POST /document/validate_refs { fix:false }
   - Expect: `{ invalid:[], removed:null }` (or list invalid if any)

## 3. Memory Operations
11. POST /memory/add
   - Body: `{ "content":"project kickoff notes" }`
   - Expect: `{ id, layer: "STM" | "LTM" }`
12. POST /memory/add with references
   - Body: `{ "content":"notes referencing doc", "references":[{ "docId":"<docId>", "score":0.9 }] }`
   - Expect: EVIDENCE edges and docRefs recorded
13. GET /memory/search?q=kickoff&limit=10
   - Expect: results with `docRefs?`, `layer`, `tookMs`
14. POST /memory/update
   - Body: `{ "id":"<memId>", "content":"updated" }`
   - Expect: `{ version>0, reembedded:true, updatedIndices:[...] }`
15. POST /memory/delete { id, backup:true }
   - Expect: `{ deleted:true, cascaded:true }` and removal from indices/KG
16. GET /document/refs_for_memory?id=<memId>
   - Before delete: expect refs; after delete: expect `{ docRefs: [] }` or 404
17. GET /document/refs_for_document?id=<docId>
   - Expect: list of memories referencing this document

## 4. Hybrid Search & Analytics
18. GET /search/fusion?q=kickoff&limit=10
   - Expect: fused results (text/KG/vector) with `explain` details
19. GET /advanced/analyze_patterns { window, minSupport }
   - Expect: `{ patterns: [...] }`
20. POST /advanced/trends { from, to, buckets }
   - Expect: `{ timeline: [...] }`
21. POST /advanced/clusters {}
   - Expect: clusters with doc ids (may be empty if limited data)
22. POST /advanced/relationships {}
   - Expect: relationship counts
23. POST /advanced/effectiveness {}
   - Expect: scores array

## 5. Consolidation & Maintenance
24. POST /advanced/consolidate { dryRun:false, limit:100 }
   - Expect: `{ promoted, candidates }` and STM→LTM promotions
25. POST /system/cleanup { compact:true }
   - Expect: `{ removedText, removedEdges, compacted:true }`
26. POST /advanced/reindex { vector:true, text:true, graph:true }
   - Expect: `{ vector:true, text:true, graph:true }`

## 6. Backup, Restore, Validate
27. POST /system/backup { destination:"./backups", includeIndices:true }
   - Expect: `{ path, sizeMb, tookMs }` and `manifest.json` exists
28. GET /system/validate
   - Expect: `{ embeddings:{ total, invalid, orphans }, kg:{ badEdges } }` (invalid/orphans ideally 0)
29. POST /system/restore { source:"<snapshot>", includeIndices:true }
   - Expect: `{ restored:true, validated:<bool>, tookMs }`

## 7. Performance & Health Checks
30. GET /status after search/ingest load
   - Expect: `metrics` to reflect increased counts; `health` should remain `ok`
31. GET /metrics and review p50/p95/qps over time

## 8. Error Handling
32. POST /document/store with neither `content` nor `path`
   - Expect: 400 INVALID_INPUT
33. POST /memory/add with empty content
   - Expect: 400 INVALID_INPUT
34. GET /document/retrieve with unknown id
   - Expect: 404 NOT_FOUND

## 9. HTTP to MCP Parity (Optional)
- Mirror critical flows via stdio tool calls and confirm same results format (errors included)

## 10. Regression Checklist
- memory.add/update/delete maintain all indices and graph consistency
- document versioning works and `retrieve?path=` returns latest
- hybrid fusion includes vector ANN and KG hits, temporal filters honored
- backup/restore/validate yields consistent state and indices

---

## Recording Results
- Capture p50/p95 from `/metrics` during tests
- Note any failures with request/response payloads
- List snapshot path used for backup/restore
