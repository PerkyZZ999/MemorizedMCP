## Schemas

### Knowledge Graph Schema

- Node (common)
  - id: string (uuid/ksuid)
  - type: enum { Entity, Episode, Document, Memory }
  - content: optional string
  - embedding: optional f32[384]
  - properties: map<string, value>
  - created_at: timestamp
  - updated_at: timestamp
  - doc_refs: list<DocRef>
- Entity Node
  - entity_type: enum { Person, Organization, Concept, Location }
  - aliases: string[]
- Episode Node
  - session_id: string
  - timeline: { start_ts, end_ts? }
- Document Node
  - mime: enum { pdf, md, txt }
  - path: string
  - hash: string (sha256)
- Memory Node
  - memory_type: enum { STM, LTM, Procedural }
  - importance: f32
  - decay_state: { last_access_ts, weight }

- Edge
  - id: string
  - src: node_id
  - dst: node_id
  - relation: string
  - confidence: f32
  - valid_period: { start_ts, end_ts? }
  - evidence: list<DocRef>

- DocRef
  - doc_id: node_id
  - chunk_id: string
  - score: f32

### Documentary Memory Schema

- Document
  - id, path, hash, mime, metadata
  - extracted_text: string (optional lazy)
  - summary: string
  - key_concepts: string[]
  - entities: { type, value }[]
- Chunk
  - id: string
  - doc_id: document.id
  - content: string
  - position: { page?, start_char, end_char }
  - embedding: f32[384]
  - neighbors: string[] (cross‑document links)

### Vector Index (HNSW)

- Params
  - dim: 384
  - space: cosine or dot
  - M (max connections): tuned
  - ef_construction, ef_search: tuned
- Items
  - key: chunk.id | memory.id
  - vector: f32[384]
  - payload: { type: chunk|memory, ref_id }

### Full‑Text Index (Tantivy)

- Schema
  - id: string (stored)
  - type: enum (chunk|doc|memory) (indexed)
  - content: text (analyzed)
  - metadata: json (stored)
  - timestamp: i64 (indexed)

### Storage Layout

- Hot (in‑memory)
  - LRU caches: recent queries, embeddings, doc headers
- Warm (sled)
  - keyspaces: nodes, edges, docs, chunks, memories, settings
- Cold (compressed)
  - large document blobs; archived chunks
- Index Tier
  - HNSW files; Tantivy index directories

### Temporal & Versioning

- Validity windows on edges and episodic nodes
- Document version chain by hash; latest pointer per path
- Memory version with bump on update; reembedding triggers index update
