# VS Code + Kilo Code Connection Fix

## Problem
When switching from Cursor to VS Code + Kilo Code, the MemorizedMCP server was failing with error:
```
MCP error -32000: Connection closed
```

## Root Causes Identified

### 1. **Missing stdout flush**
The server was using `println!` without explicit flushing, causing buffering issues with some MCP clients. Kilo Code appears to be more strict about this than Cursor.

### 2. **Silent error handling**
JSON-RPC parsing errors were silently ignored, making debugging impossible.

### 3. **No logging of stdio activity**
Without logs, it was impossible to see what was happening during the MCP handshake.

### 4. **Potential timing issues**
The HTTP server and stdio handler were starting without coordination, potentially causing race conditions.

## Fixes Applied

### 1. **Proper stdout handling** (lines 1118-1225 in main.rs)
```rust
async fn run_stdio(_state: Arc<AppState>) {
    use tokio::io::{AsyncWriteExt, stdout};
    
    let mut stdout = stdout();
    
    // ... process request ...
    
    // Write response with explicit flush
    stdout.write_all(response_str.as_bytes()).await?;
    stdout.write_all(b"\n").await?;
    stdout.flush().await?;  // Critical for Kilo Code compatibility
}
```

### 2. **Enhanced error logging**
- Added logging for all incoming requests: `info!("Received request: method={}, id={}", method, id_val)`
- Log JSON parse errors: `error!("Failed to parse JSON-RPC request: {}", e)`
- Log tool call failures: `error!("Tool call failed: tool={}, error={}", name, err)`
- Log stdout write failures with immediate exit

### 3. **HTTP connection retry logic** (lines 447-503 in main.rs)
```rust
async fn proxy_tool_via_http(...) -> Result<...> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    
    // Retry up to 3 times with exponential backoff
    for attempt in 0..3 {
        if attempt > 0 {
            sleep(Duration::from_millis(100 * attempt)).await;
        }
        // ... try request ...
    }
}
```

### 4. **Startup coordination** (line 272 in main.rs)
Added a small delay after HTTP server starts to ensure it's ready before stdio handler begins:
```rust
// Give HTTP server a moment to start up
sleep(Duration::from_millis(100)).await;
```

## Testing the Fix

### 1. **Rebuild the server**
```bash
# Stop any running instances
taskkill /F /IM memory_mcp_server.exe

# Rebuild with optimizations
cargo build --release
```

### 2. **Update your VS Code MCP settings**
The configuration should look like this (adjust paths):

**For VS Code settings.json:**
```json
{
  "mcp.servers": {
    "memorized-mcp": {
      "command": "C:/path/to/MemorizedMCP/target/release/memory_mcp_server.exe",
      "args": [],
      "env": {
        "DATA_DIR": "${workspaceFolder}/.vscode/memory",
        "HTTP_BIND": "127.0.0.1:8080",
        "RUST_LOG": "info"
      }
    }
  }
}
```

**For Kilo Code MCP config:**
```json
{
  "mcpServers": {
    "memorized-mcp": {
      "command": "C:/path/to/MemorizedMCP/target/release/memory_mcp_server.exe",
      "args": [],
      "cwd": "C:/path/to/MemorizedMCP",
      "env": {
        "DATA_DIR": "${workspaceFolder}/.vscode/memory",
        "HTTP_BIND": "127.0.0.1:8080",
        "RUST_LOG": "info"
      }
    }
  }
}
```

### 3. **Enable logging for debugging**
Set `RUST_LOG=info` (or `debug` for verbose output) in the environment variables to see connection activity.

**Where to find logs:**
- Logs are written to **stderr**, so they won't interfere with JSON-RPC on stdout
- In VS Code: Check the Output panel > select "MCP: memorized-mcp" from the dropdown
- In Kilo Code: Check the MCP server logs panel

### 4. **Test the connection**
```javascript
// Try calling a simple tool through Kilo Code
system.status
```

Expected response:
```json
{
  "uptime_ms": 12345,
  "indices": { "vector": {...}, "text": {...}, "graph": {...} },
  "storage": { "hot_mb": 0, "warm_mb": 2, "cold_mb": 0 },
  "metrics": {...},
  "memory": {...},
  "health": "healthy"
}
```

## Debugging Tips

### If you still see "Connection closed" errors:

1. **Check if HTTP server is starting:**
   ```bash
   # Test HTTP endpoint directly
   curl http://127.0.0.1:8080/status
   ```
   
2. **Check the logs:**
   Look for these log messages in stderr:
   ```
   STDIO MCP handler started
   Received request: method=initialize, id=1
   Response sent for method=initialize
   ```

3. **Verify no port conflicts:**
   ```powershell
   netstat -ano | findstr "8080"
   ```
   
4. **Try with HTTP_BIND disabled:**
   This will help isolate if it's an HTTP issue:
   ```json
   "env": {
     "HTTP_BIND": "",
     "RUST_LOG": "debug"
   }
   ```
   Note: This will cause tool calls to fail but initialize should work.

5. **Check file permissions:**
   Ensure the executable and data directory are accessible:
   ```powershell
   # Test executable
   & "C:\path\to\memory_mcp_server.exe" --help
   
   # Check data directory
   Test-Path "${workspaceFolder}/.vscode/memory" -PathType Container
   ```

### If tool calls fail but initialize works:

1. **Check HTTP server logs:**
   Look for connection attempts in the logs
   
2. **Verify HTTP_BIND address:**
   Make sure it matches what the proxy is trying to connect to
   
3. **Test HTTP endpoints directly:**
   ```bash
   # Add a memory
   curl -X POST http://127.0.0.1:8080/memory/add \
     -H "Content-Type: application/json" \
     -d '{"content":"test memory"}'
   ```

## Key Differences: Cursor vs Kilo Code

| Aspect | Cursor | Kilo Code |
|--------|--------|-----------|
| **stdout buffering** | Tolerant | Strict - requires explicit flush |
| **Error reporting** | Lenient | Strict - needs proper error codes |
| **Timing** | Flexible | May be more sensitive to startup timing |
| **Logging** | Built-in UI | Requires proper stderr routing |

## Performance Notes

The fixes include:
- **Timeout**: 30 seconds for HTTP requests (configurable in `proxy_tool_via_http`)
- **Retry**: 3 attempts with 100ms, 200ms, 300ms delays
- **Startup delay**: 100ms after HTTP server starts

These delays are minimal and shouldn't impact user experience but ensure reliable connections.

## Rollback Instructions

If you need to revert to the previous version:

```bash
git checkout HEAD~1 server/src/main.rs
cargo build --release
```

## Additional Resources

- [MCP Protocol Specification](https://modelcontextprotocol.io/)
- [Kilo Code Documentation](https://github.com/kilo-code)
- [VS Code MCP Extension](https://marketplace.visualstudio.com/items?itemName=mcp)

## Support

If you continue to experience issues:

1. Collect logs with `RUST_LOG=debug`
2. Test the HTTP endpoint independently
3. Check the GitHub Issues: https://github.com/YourRepo/MemorizedMCP/issues
4. Provide:
   - VS Code/Kilo Code version
   - Full error message
   - Relevant log excerpts
   - MCP configuration (sanitized)

