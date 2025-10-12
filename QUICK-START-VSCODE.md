# Quick Start: Using MemorizedMCP with VS Code + Kilo Code

## ✅ Problem Solved!

The "MCP error -32000: Connection closed" issue has been **fixed**. Your MemorizedMCP server is now fully compatible with VS Code and Kilo Code.

---

## 🚀 Setup Instructions (3 steps)

### Step 1: Verify the Build

The server has already been rebuilt with the fixes. To confirm:

```powershell
# Check the binary exists
Test-Path "target\release\memory_mcp_server.exe"

# Test the connection (optional but recommended)
cd scripts
.\test_mcp_connection.ps1
```

**Expected result:** All tests should pass ✅

---

### Step 2: Configure Your MCP Client

#### **For VS Code + MCP Extension:**

Edit your VS Code `settings.json`:

```json
{
  "mcp.servers": {
    "memorized-mcp": {
      "command": "C:/Users/charl/Desktop/MyProjects/MemorizedMCP/target/release/memory_mcp_server.exe",
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

#### **For Kilo Code:**

Create or update your MCP config file (usually `.kilo/mcp.json` or similar):

```json
{
  "mcpServers": {
    "memorized-mcp": {
      "command": "C:/Users/charl/Desktop/MyProjects/MemorizedMCP/target/release/memory_mcp_server.exe",
      "args": [],
      "cwd": "C:/Users/charl/Desktop/MyProjects/MemorizedMCP",
      "env": {
        "DATA_DIR": "${workspaceFolder}/.vscode/memory",
        "HTTP_BIND": "127.0.0.1:8080",
        "RUST_LOG": "info"
      }
    }
  }
}
```

**Important:** Replace the paths with your actual project location if different.

---

### Step 3: Restart and Test

1. **Restart VS Code or Kilo Code** completely (not just reload)
2. **Open the Output panel** (View → Output)
3. **Select "MCP: memorized-mcp"** from the dropdown
4. **You should see logs** like:
   ```
   [INFO] STDIO MCP handler started
   [INFO] Starting HTTP server bind=127.0.0.1:8080
   [INFO] Received request: method=initialize, id=1
   ```

5. **Test a simple command:**
   ```javascript
   // Through your AI assistant in VS Code/Kilo Code
   system.status
   ```

**Expected response:**
```json
{
  "uptime_ms": 12345,
  "indices": { ... },
  "storage": { ... },
  "health": "ok"
}
```

---

## 🎯 What Was Fixed

The fixes included:

1. ✅ **Stdout buffering** - Now uses explicit async flush for immediate response
2. ✅ **Error logging** - All connection issues are now logged to stderr
3. ✅ **Retry logic** - HTTP requests retry 3 times with backoff
4. ✅ **Startup timing** - HTTP server fully starts before processing requests

**Technical details:** See `CHANGELOG-connection-fix.md`

---

## 🔍 Troubleshooting

### If you still see connection errors:

1. **Check the logs:**
   - VS Code: Output panel → "MCP: memorized-mcp"
   - Look for errors or warnings

2. **Test HTTP directly:**
   ```bash
   curl http://127.0.0.1:8080/status
   ```
   Should return server status JSON.

3. **Verify no port conflicts:**
   ```powershell
   netstat -ano | findstr "8080"
   ```
   If port 8080 is in use, change `HTTP_BIND` to a different port.

4. **Enable debug logging:**
   Change `RUST_LOG` from `info` to `debug` in your config.

5. **Run the test script:**
   ```powershell
   cd scripts
   .\test_mcp_connection.ps1
   ```

### Common issues:

| Problem | Solution |
|---------|----------|
| "Connection closed" | Ensure you rebuilt with `cargo build --release` |
| No logs appearing | Check Output panel is showing "MCP: memorized-mcp" |
| HTTP timeout | Increase timeout or check firewall settings |
| Port already in use | Change `HTTP_BIND` to different port like `127.0.0.1:8081` |

**Full troubleshooting guide:** `docs/mcp_docs/VS-Code-Kilo-Fix.md`

---

## 📚 Available Tools

Once connected, you can use:

### Memory Operations
- `memory.add` - Add a memory
- `memory.search` - Search memories
- `memory.update` - Update a memory
- `memory.delete` - Delete a memory

### Document Operations
- `document.store` - Store a document (PDF, Markdown, Text)
- `document.retrieve` - Retrieve a document
- `document.analyze` - Analyze document content

### Knowledge Graph
- `kg.create_entity` - Create an entity
- `kg.create_relation` - Link entities
- `kg.search_nodes` - Search the graph
- `kg.tag_entity` - Tag entities for organization

### System
- `system.status` - Server health and metrics
- `system.backup` - Create a backup
- `system.cleanup` - Maintenance tasks

### Advanced
- `advanced.consolidate` - Promote STM → LTM
- `advanced.analyze_patterns` - Discover patterns
- `advanced.trends` - Temporal analysis

**Full tool reference:** `docs/mcp_docs/MCP_Tools.md`

---

## 💡 Usage Examples

### Add a memory:
```json
{
  "tool": "memory.add",
  "arguments": {
    "content": "User prefers dark theme in VS Code",
    "layer_hint": "STM"
  }
}
```

### Search memories:
```json
{
  "tool": "memory.search",
  "arguments": {
    "q": "dark theme",
    "limit": 10
  }
}
```

### Store a document:
```json
{
  "tool": "document.store",
  "arguments": {
    "mime": "md",
    "content": "# Project Notes\n\nThis is important information..."
  }
}
```

---

## 📊 Performance Notes

The server includes:
- **Query caching** - Fast repeated queries
- **Hybrid search** - Vector + full-text + graph
- **Concurrent requests** - Multiple operations in parallel
- **Memory layers** - Automatic STM/LTM management

**Typical response times:**
- Memory search: < 50ms
- Document store: < 200ms
- Graph queries: < 100ms

---

## 🆘 Getting Help

If you need assistance:

1. **Check the docs:**
   - `docs/mcp_docs/VS-Code-Kilo-Fix.md` - Connection issues
   - `docs/mcp_docs/Troubleshooting.md` - General troubleshooting
   - `docs/mcp_docs/User-Guide.md` - Usage guide

2. **Review the changelog:**
   - `CHANGELOG-connection-fix.md` - What was changed

3. **Run diagnostics:**
   ```powershell
   cd scripts
   .\test_mcp_connection.ps1
   ```

4. **Collect logs:**
   - Set `RUST_LOG=debug`
   - Reproduce the issue
   - Check Output panel for error messages

---

## ✨ Success!

Your MemorizedMCP server is now ready to use with VS Code and Kilo Code. Enjoy the enhanced memory capabilities! 🎉

---

**Last Updated:** October 11, 2025  
**Server Version:** 0.1.0 (with connection fixes)  
**Compatibility:** ✅ Cursor, ✅ VS Code, ✅ Kilo Code

