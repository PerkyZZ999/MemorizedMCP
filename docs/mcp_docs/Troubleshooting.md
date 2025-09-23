# Troubleshooting

## Common Issues

### High p95 latency
- Check `/metrics` for `mcp_query_p95_ms`
- Increase `FUSION_CACHE_TTL_MS` or reduce query `limit`
- Run `/system/compact`; consider `/advanced.reindex`

### Large PDF timeouts
- Tune `PDF_MAX_PAGES`, `PDF_MAX_BYTES`, `PDF_MAX_TIME_MS`
- Prefer Markdown ingestion for massive files

### Missing search results
- Verify indices in `/status`
- Reindex via `/advanced.reindex`
- Validate KG edges via `/system.validate`

### Storage growth
- Schedule `/system.cleanup` and compaction
- Review `DATA_DIR` sizes in `/status`

### Integrity errors
- Run `/system.validate` and fix with `/document.validate_refs?fix=true`

## Logging
- Set `RUST_LOG=debug` for detailed traces
- HTTP tracing via tower-http TraceLayer
