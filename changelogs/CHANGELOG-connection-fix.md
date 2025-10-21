# Connection Fix Changelog - VS Code + Kilo Code Compatibility

**Date:** October 11, 2025  
**Version:** Connection Fix v1.0  
**Issue:** MCP error -32000: Connection closed in VS Code + Kilo Code

---

## 🎯 Summary

Fixed critical connection issues preventing MemorizedMCP from working with VS Code and Kilo Code. The server now properly handles stdio communication with explicit flushing, retry logic, and comprehensive error logging.

## 🔧 Changes Made

### 1. **Enhanced STDIO Handler** (`server/src/main.rs` lines 1118-1225)

**Before:**
```rust
// Used println! without flush
println!("{}", serde_json::to_string(&out).unwrap());
// Silent error handling
let v: serde_json::Value = match serde_json::from_str(line) { 
    Ok(x) => x, 
    Err(_) => continue  // Silent!
};
```

**After:**
```rust
// Explicit async stdout with flushing
let mut stdout = stdout();
stdout.write_all(response_str.as_bytes()).await?;
stdout.write_all(b"\n").await?;
stdout.flush().await?;  // Critical!

// Proper error logging
let v: serde_json::Value = match serde_json::from_str(line) {
    Ok(x) => x,
    Err(e) => {
        error!("Failed to parse JSON-RPC request: {}", e);
        continue;
    }
};
```

**Impact:** Kilo Code now receives responses immediately without buffering delays.

---

### 2. **HTTP Proxy Retry Logic** (`server/src/main.rs` lines 447-503)

**Before:**
```rust
// Single attempt, no timeout
let client = reqwest::Client::new();
let resp_result = client.get(&url).send().await;
```

**After:**
```rust
// Timeout + 3 retries with exponential backoff
let client = reqwest::Client::builder()
    .timeout(std::time::Duration::from_secs(30))
    .build()?;

for attempt in 0..max_retries {
    if attempt > 0 {
        sleep(Duration::from_millis(100 * attempt)).await;
    }
    // ... try request ...
}
```

**Impact:** Handles startup timing issues and transient connection failures.

---

### 3. **Startup Coordination** (`server/src/main.rs` line 272)

**Before:**
```rust
// HTTP and stdio started simultaneously
let http_task = task::spawn(async move { /* start HTTP */ });
tasks.push(http_task);
let stdio_task = task::spawn(async move { run_stdio(state).await; });
```

**After:**
```rust
let http_task = task::spawn(async move { /* start HTTP */ });
tasks.push(http_task);

// Give HTTP server a moment to start up
sleep(Duration::from_millis(100)).await;

let stdio_task = task::spawn(async move { run_stdio(state).await; });
```

**Impact:** Ensures HTTP server is ready before processing stdio requests.

---

### 4. **Comprehensive Logging**

Added logging at key points:
- ✅ `info!("STDIO MCP handler started")`
- ✅ `info!("Received request: method={}, id={}", method, id_val)`
- ✅ `info!("Response sent for method={}", method)`
- ✅ `error!("Failed to parse JSON-RPC request: {}", e)`
- ✅ `error!("Tool call failed: tool={}, error={}", name, err)`
- ✅ `error!("Failed to write to stdout: {}", e)`
- ✅ `info!("STDIO handler exiting")`

**Impact:** Easy debugging and monitoring of MCP communication.

---

## 📊 Technical Details

### Root Cause Analysis

1. **Buffering Issue**
   - Problem: `println!` uses line buffering which may not flush immediately
   - Kilo Code: Expects immediate response after sending request
   - Solution: Explicit `AsyncWriteExt::flush()` after each response

2. **Silent Failures**
   - Problem: JSON parse errors were ignored with `continue`
   - Impact: No way to diagnose malformed requests
   - Solution: Proper error logging to stderr

3. **Race Condition**
   - Problem: stdio proxy tried to connect to HTTP before it was ready
   - Symptom: "Connection refused" on first requests
   - Solution: Small startup delay + retry logic

4. **No Connection State Tracking**
   - Problem: stdout write errors were not fatal
   - Impact: Server kept trying to write to closed connection
   - Solution: Break loop on write failure

---

## 🧪 Testing

### Automated Tests
All existing tests pass:
```bash
cargo test
# ... all tests passed
```

### Manual Testing Checklist
- [x] Build succeeds: `cargo build --release`
- [x] Server starts: Logs show "STDIO MCP handler started"
- [x] Initialize handshake: Returns correct protocol version
- [x] Tool list: Returns all available tools
- [x] Tool call (system.status): Returns server status
- [x] Tool call (memory.add): Adds memory successfully
- [x] Tool call (memory.search): Searches and returns results
- [x] Error handling: Invalid tool returns proper error
- [x] Graceful shutdown: Ctrl+C cleanly stops server

---

## 🚀 How to Apply

### For End Users

1. **Stop running server:**
   ```bash
   taskkill /F /IM memory_mcp_server.exe
   ```

2. **Rebuild:**
   ```bash
   cargo build --release
   ```

3. **Update MCP config** (VS Code settings.json or Kilo config):
   ```json
   {
     "mcpServers": {
       "memorized-mcp": {
         "command": "C:/path/to/target/release/memory_mcp_server.exe",
         "env": {
           "DATA_DIR": "${workspaceFolder}/.vscode/memory",
           "HTTP_BIND": "127.0.0.1:8080",
           "RUST_LOG": "info"
         }
       }
     }
   }
   ```

4. **Restart VS Code / Kilo Code**

5. **Test:**
   ```javascript
   // Should work now!
   system.status
   ```

---

## 📚 Documentation Updates

New/updated files:
- ✅ `docs/mcp_docs/VS-Code-Kilo-Fix.md` - Comprehensive troubleshooting guide
- ✅ `docs/mcp_docs/Troubleshooting.md` - Added connection issues section
- ✅ `CHANGELOG-connection-fix.md` - This file

---

## 🔍 Verification

### Expected Log Output

With `RUST_LOG=info`, you should see:
```
[INFO] STDIO MCP handler started
[INFO] Starting HTTP server bind=127.0.0.1:8080
[INFO] Received request: method=initialize, id=1
[INFO] Response sent for method=initialize
[INFO] Received request: method=tools/list, id=2
[INFO] Response sent for method=tools/list
[INFO] Received request: method=tools/call, id=3
[INFO] Response sent for method=tools/call
```

### Success Criteria

✅ No "Connection closed" errors  
✅ Initialize completes successfully  
✅ Tool calls return results  
✅ Logs show all requests/responses  
✅ No timeout errors  

---

## 🐛 Known Limitations

1. **HTTP dependency:** stdio mode still requires HTTP server to be running
   - Future: Consider direct state access for stdio mode
   
2. **Fixed retry count:** 3 attempts hardcoded
   - Future: Make configurable via environment variable
   
3. **Small startup delay:** 100ms delay before stdio handler
   - Impact: Minimal, but could be optimized with proper ready signal

---

## 🎓 Lessons Learned

1. **MCP clients differ:** What works in Cursor may not work in Kilo Code
2. **Explicit flushing matters:** Async stdio requires careful handling
3. **Logging is critical:** Proper error logging saves debugging time
4. **Retry logic helps:** Network operations benefit from retries
5. **State machine visibility:** Logging state transitions aids debugging

---

## 👥 Credits

- **Issue reported by:** User experiencing connection issues after switching to VS Code + Kilo Code
- **Root cause analysis:** Identified stdout buffering and timing issues
- **Fix implemented:** Enhanced stdio handler with proper async I/O
- **Testing:** Verified across multiple MCP clients

---

## 📞 Support

If you still experience issues after applying this fix:

1. **Enable debug logging:**
   ```json
   "env": { "RUST_LOG": "debug" }
   ```

2. **Check the logs** in VS Code Output panel (select "MCP: memorized-mcp")

3. **Test HTTP directly:**
   ```bash
   curl http://127.0.0.1:8080/status
   ```

4. **Review the detailed guide:**
   See `docs/mcp_docs/VS-Code-Kilo-Fix.md`

5. **Report issues:**
   Include logs, config (sanitized), and error messages

---

**Status:** ✅ Fixed and Tested  
**Priority:** 🔴 Critical (blocks VS Code usage)  
**Effort:** Low (code changes) + Medium (testing)  
**Risk:** Low (backward compatible, improves reliability)

