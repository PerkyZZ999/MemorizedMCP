## Operations Guide

### Monitoring & Analytics

- Metrics (system.status)
  - indices: vector (items), text (docs), graph (nodes/edges)
  - storage: hot/warm/cold usage; index sizes
  - queues: ingest, indexing, consolidation backlog
  - latency: search P50/P95, ingestion throughput
- Dashboards
  - Time series of query latency, ingest rates, index growth
  - Promotion rate (STM→LTM), decay effectiveness, cache hit ratio

### Maintenance

- Cleanup
  - Run `system.cleanup` for compaction/reindex toggles
  - Schedule periodic merge/compaction windows
- Reindex
  - `advanced.reindex` selective toggles (vector/text/graph)
  - Monitor resource usage; apply backpressure if needed

### Backup & Recovery

- Backup
  - `system.backup` to BACKUP_DIR; include indices when downtime acceptable
  - Verify checksum and sizes; keep rotation policy
- Restore
  - `system.restore` from snapshot; ensure schema version match
  - Post‑restore validation: status checks, sample queries

### Consolidation & Decay

- Consolidation
  - Schedule `advanced.consolidate` cadence (e.g., hourly)
  - Audit trail of promotions, conflicts, merges
- Decay
  - Tune DECAY_PARAMS; monitor distribution of importance weights

### Capacity Planning

- Track documents/day, average size, chunk density
- Estimate vector index growth and Tantivy footprint
- Provision HOT_CACHE_MB and disk for warm/cold/index tiers

### Incident Response

- Degraded performance
  - Raise ef_search temporarily; enable query caching
  - Throttle ingestion; pause reindex/consolidation jobs
- Data corruption
  - Switch to last known good backup; enable write‑ahead logs (if configured)

### Access & Security

- Run as least‑privileged user; restrict DATA_DIR permissions
- Optional local HTTP restricted to loopback; prefer stdio for IDE

### Change Management

- Parameter changes via env; document effective configs in release notes
- Canary scoring configs for fusion; A/B compare before rollout
