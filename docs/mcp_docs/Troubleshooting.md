# Troubleshooting

## MCP Connection Issues

### "Connection closed" error in VS Code / Kilo Code

**Symptoms:** `MCP error -32000: Connection closed` when trying to use MemorizedMCP

**Solution:** This was a known issue with stdout buffering and has been fixed. See the detailed guide:

- **[VS Code + Kilo Code Connection Fix](./VS-Code-Kilo-Fix.md)**

**Quick fix:**

1. Rebuild the server: `cargo build --release`
2. Ensure `RUST_LOG=info` is set in your MCP config
3. Restart VS Code / Kilo Code

If the issue persists, check the comprehensive troubleshooting guide linked above.

### "Connection closed" when auto-approving tools OR "Database locked" error

**Symptoms:**

- Server works fine initially, but fails when adding tools to auto-allow
- Error: `IO error: could not acquire lock on "...\warm\kv\db"`
- Error: `The process cannot access the file because another process has locked a portion of the file`
- Connection drops when Kilo Code restarts the server
- Need to manually kill `memory_mcp_server.exe` processes

**Solution:** ✅ **DEFINITIVE FIX - PID-Based Stale Instance Detection**

The server now automatically detects and kills old server processes that are holding database locks. This is the complete solution to all database locking issues. See:

- **[PID Lock Fix](./PID-Lock-Fix.md)** ← **✅ COMPLETE SOLUTION - Start here!**
- [Auto-Allow Fix](./Auto-Allow-Fix.md) ← _Deprecated - superseded by PID Lock Fix_
- [Concurrent Request Handling Fix](./Concurrent-Request-Fix.md) ← For rapid tool calls

**CRITICAL - Quick fix:**

1. **Ensure `"RUST_LOG": "info"` in your mcp.json/mcp_settings.json** ← **MUST DO THIS!**
2. **Kill all old server instances:** `taskkill /F /IM memory_mcp_server.exe`
3. **Rebuild the server:** `cargo build --release`
4. **Restart Kilo Code completely**
5. Auto-approve now works seamlessly - server will kill stale processes automatically

**How it works:**

- Server writes its PID to `<DATA_DIR>/warm/server.pid` on startup
- On restart, checks if old PID is still running
- If yes, forcefully kills the old process (`taskkill /F` on Windows)
- Waits 500ms for OS to release file locks
- Opens database cleanly without lock conflicts
- On shutdown, removes PID file for clean next start

**Why this fix is better:**

- ✅ Fully automatic - no manual intervention needed
- ✅ Fast - restarts in <1 second
- ✅ Works on Windows, Linux, and macOS
- ✅ Handles crashes and zombie processes
- ✅ No data loss - database is flushed before restart

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

### HTTP server not starting

- Check if port `8080` (or your configured port) is already in use
- Verify `HTTP_BIND` environment variable
- Check logs for "bind failed" errors

## Logging

- Set `RUST_LOG=debug` for detailed traces
- All logs go to **stderr** (stdout is reserved for JSON-RPC)
- HTTP tracing via tower-http TraceLayer
- In VS Code: Check Output panel > "MCP: memorized-mcp"
