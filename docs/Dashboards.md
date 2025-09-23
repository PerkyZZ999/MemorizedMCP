## Monitoring Dashboards

### Prometheus Scrape Config

Add this job to your Prometheus configuration:

```yaml
scrape_configs:
  - job_name: memory_mcp_server
    scrape_interval: 5s
    static_configs:
      - targets: ["127.0.0.1:8080"]
    metrics_path: /metrics
```

### Key Metrics

- mcp_queries_total
- mcp_cache_hits_total
- mcp_cache_misses_total
- mcp_query_last_ms
- mcp_query_avg_ms
- mcp_query_p50_ms
- mcp_query_p95_ms
- mcp_query_qps_1m

### Grafana Panels

Create a dashboard with these panels:

- Queries per second: `rate(mcp_queries_total[1m])`
- Cache hit ratio: `mcp_cache_hits_total / (mcp_cache_hits_total + mcp_cache_misses_total)`
- Latency p50/p95: `mcp_query_p50_ms`, `mcp_query_p95_ms`
- Recent query latency: `mcp_query_last_ms`
- Average latency: `mcp_query_avg_ms`

Set alert rules on p95 thresholds aligned with `STATUS_P95_MS_THRESHOLD`.


