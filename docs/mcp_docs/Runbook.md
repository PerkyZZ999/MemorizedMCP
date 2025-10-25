# Operations Runbook

## Routine Tasks

- Check status: `/status` health == ok
- Review metrics: `/metrics` (Prometheus scrape)
- Cleanup: `/system/cleanup` with `compact:true` during low-traffic windows
- Reindex: `/advanced/reindex` after parameter changes
- Consolidation: schedule `/advanced/consolidate`

## Backup & Recovery

- Backup: `/system.backup` to BACKUP_DIR
- Verify manifest.json in snapshot
- Restore: `/system.restore`
- Post-restore: run `/system.validate`, check `/status`

## Incident Response

- Degraded health (p95 high):
  - Reduce `limit` on queries; increase cache TTL; enable compaction
  - Pause heavy ingestion; run `/system/compact`
- Data integrity warnings:
  - Run `/system.validate` and `/system.cleanup`
- Index bloat:
  - `/advanced.reindex` vector/text; rebuild neighbor graph

## Configuration Changes

- Apply via env; document effective values
- Roll out scoring weights with A/B via env profiles

## Troubleshooting

- Logs: structured tracing (set `RUST_LOG=debug`)
- PDF parsing limits: tune `PDF_MAX_*` envs
- Memory pressure: watch `rssMb` and increase consolidation cadence
