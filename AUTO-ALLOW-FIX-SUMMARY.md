# 🎯 AUTO-ALLOW FIX - Quick Guide

## The Problem
When you check "auto-allow" on a tool in Kilo Code, the connection closes with error `-32000: Connection closed`.

## The Root Cause
1. **Kilo Code restarts the server** when you check auto-allow
2. **The database stayed locked** from the previous instance
3. **New instance couldn't start** → connection failed
4. **`RUST_LOG: "off"` hid all error messages** → impossible to debug!

---

## ✅ THE FIX (2 Steps)

### **Step 1: Change Your Config** ⚠️ **CRITICAL!**

Open your `mcp_settings.json` in Kilo Code and change this line:

**❌ WRONG:**
```json
"RUST_LOG": "off"
```

**✅ CORRECT:**
```json
"RUST_LOG": "info"
```

**Full config should look like:**
```json
{
  "mcpServers": {
    "memorized-mcp": {
      "command": "C:/Users/charl/Desktop/MyProjects/MemorizedMCP/target/release/memory_mcp_server.exe",
      "args": [],
      "cwd": "C:/Users/charl/Desktop/MyProjects/MemorizedMCP",
      "env": {
        "DATA_DIR": "${workspaceFolder}/.kilo/memory",
        "HTTP_BIND": "127.0.0.1:8080",
        "RUST_LOG": "info"  ← CHANGE THIS!
      }
    }
  }
}
```

### **Step 2: Restart Kilo Code**

1. Save the config file
2. **Completely close Kilo Code** (not just reload)
3. Reopen Kilo Code

---

## 🧪 Test It

1. Open Settings → MCP Servers → memorized-mcp
2. **Click the auto-allow checkbox on any tool** (like `system.status`)
3. **Should work now!** ✅

---

## 🔍 What You'll See Now

With logging enabled, you'll see helpful information in the Output panel:

**When auto-allow restarts the connection:**
```
[INFO] Stdin closed (EOF), shutting down stdio handler
[INFO] Flushing database...
[INFO] Server shutdown complete
[INFO] STDIO MCP handler started  ← New instance starts clean!
```

**Before (with logging off):**
```
[Silent... connection just dies]
```

---

## 🎉 What's Fixed

The server now:
- ✅ Flushes the database properly before shutdown
- ✅ Detects when Kilo Code closes the connection
- ✅ Shuts down gracefully so restarts work
- ✅ **Shows you what's happening** (with logging enabled!)

---

## ⚠️ Why Logging Matters

### `"RUST_LOG": "off"` = **BLIND**
- You see NOTHING when things fail
- No way to debug issues
- Silent failures

### `"RUST_LOG": "info"` = **CLEAR VISIBILITY**
- See connection events
- See shutdown process
- See errors with context
- Easy to debug problems

---

## 📞 Still Having Issues?

If auto-allow still fails:

1. **Double-check** `RUST_LOG` is set to `"info"` (not `"off"`)
2. **Look at the Output panel** (View → Output → "MCP: memorized-mcp")
3. **See what error appears** - the logs will tell you exactly what's wrong
4. **Check** `docs/mcp_docs/Auto-Allow-Fix.md` for detailed troubleshooting

---

## 🚀 Summary

**What you need to do:**
1. Change `"RUST_LOG": "off"` → `"RUST_LOG": "info"`
2. Restart Kilo Code
3. Try auto-allow again

**That's it!** The server has been rebuilt with proper shutdown handling, so as long as logging is enabled, auto-allow will work perfectly.

---

**Status:** ✅ Server rebuilt with fix  
**Action Required:** Change RUST_LOG to "info" in your config  
**Expected Result:** Auto-allow works without connection drops

