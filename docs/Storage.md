## Storage Architecture

### Multi‑Tier Design

- Hot Tier (in‑memory)
  - LRU caches for query results, doc headers, and embeddings
  - Session‑scoped STM containers
- Warm Tier (sled)
  - Metadata KV: nodes, edges, docs, chunks, memories, settings
  - Transactional updates where needed; integrity checks
- Cold Tier (compressed)
  - Archived documents and large blobs; lz4_flex compression
- Index Tier
  - HNSW files for vectors; Tantivy directories for text

### Data Organization

- Separate keyspaces/namespaces per data type to minimize contention
- bincode serialization for hot paths; JSON for exported artifacts
- Versioned records with update counters; last_access stamps for decay

### Backup & Recovery

- Snapshots to BACKUP_DIR with optional index inclusion
- Validate restore with checksum and schema version checks
- Automated retention policy with rotation and size caps

### Compaction & Maintenance

- Periodic compaction of sled/rocksdb; Tantivy merge tuning
- Reindex tasks for vector/text after parameter updates
- Orphan detection and cleanup for graph and document links

### Security & Access

- Local filesystem paths with least privilege
- Optional encryption at rest (future); scrub temp files on crash recovery

### Configuration

- Env‑driven directories: DATA_DIR, HOT_CACHE_MB, BACKUP_DIR
- Tunables: COMPRESSION_LEVEL, REINDEX_BATCH_SIZE, RETENTION_DAYS
