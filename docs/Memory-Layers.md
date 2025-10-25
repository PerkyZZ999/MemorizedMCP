## Memory Layers

### Short‑Term Memory (STM)

- Purpose: Active session context; immediate relevance
- Storage: In‑memory STM containers; LRU eviction; expiration timestamps
- Features: Relevance scoring; frequency tracking; promotion candidates
- APIs: managed via `memory.add/search`; consolidation considers STM signals

### Long‑Term Memory (LTM)

- Purpose: Persistent knowledge and learned patterns
- Storage: Persistent DB with compression and indexing
- Features: Importance weighting; decay factors; cross‑reference links; consolidation markers
- APIs: `memory.update/delete`; consolidation promotes from STM

### Episodic Memory

- Purpose: Conversation history and contextual experiences
- Storage: Temporal KG with threads; timeline awareness
- Features: Time windows; event linking; retrieval by time range
- APIs: part of `memory.search` filters (timeFrom/timeTo)

### Semantic Memory

- Purpose: Factual knowledge, concepts, and entity relationships
- Storage: Knowledge graph (petgraph); typed nodes/edges; multi‑hop traversal
- Features: Concept hierarchies; relationship inference; synthesis via traversals
- APIs: implicit in search fusion; explicit graph ops internal

### Documentary Memory (Novel)

- Purpose: Reference document storage and retrieval
- Storage: Document store with chunk embeddings and full‑text index
- Features: Exact recall; semantic chunks; summaries; evidence‑backed references
- APIs: `document.store/retrieve/analyze`; references surface in `memory.search`

### Consolidation (STM → LTM)

- Trigger: Scheduled background job or manual `advanced.consolidate`
- Criteria: Importance score, access frequency, recency, confidence
- Steps:
  1. Mark promotion candidates; create/merge LTM entries
  2. Update KG links; reweight relationships
  3. Re‑embed if content changed; update indices

### Decay & Strengthening

- Decay: Time‑based weight reduction on LTM unless reinforced
- Strengthening: Access patterns increase importance; slows decay
- Cleanup: Invalidate stale links; compress low‑value entries
