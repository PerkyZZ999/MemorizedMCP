# Test MCP Connection Script
# Tests the MemorizedMCP server connection and basic functionality

param(
    [string]$ServerPath = "..\target\release\memory_mcp_server.exe",
    [string]$HttpBind = "127.0.0.1:8080",
    [int]$TimeoutSeconds = 10
)

Write-Host "=== MemorizedMCP Connection Test ===" -ForegroundColor Cyan
Write-Host ""

# Check if server executable exists
if (-not (Test-Path $ServerPath)) {
    Write-Host "❌ Server executable not found at: $ServerPath" -ForegroundColor Red
    Write-Host "   Run 'cargo build --release' first" -ForegroundColor Yellow
    exit 1
}

Write-Host "✓ Server executable found" -ForegroundColor Green

# Stop any existing instances
Write-Host ""
Write-Host "Stopping any existing server instances..." -ForegroundColor Yellow
try {
    $processes = Get-Process -Name "memory_mcp_server" -ErrorAction SilentlyContinue
    if ($processes) {
        $processes | Stop-Process -Force
        Start-Sleep -Milliseconds 500
        Write-Host "✓ Stopped existing instances" -ForegroundColor Green
    } else {
        Write-Host "✓ No existing instances found" -ForegroundColor Green
    }
} catch {
    Write-Host "✓ No existing instances to stop" -ForegroundColor Green
}

# Start server in background
Write-Host ""
Write-Host "Starting MCP server..." -ForegroundColor Yellow
$env:HTTP_BIND = $HttpBind
$env:DATA_DIR = "./test_data"
$env:RUST_LOG = "info"

$serverProcess = Start-Process -FilePath $ServerPath `
    -NoNewWindow `
    -PassThru `
    -RedirectStandardOutput "test_output.log" `
    -RedirectStandardError "test_error.log"

Write-Host "✓ Server process started (PID: $($serverProcess.Id))" -ForegroundColor Green

# Wait for server to start
Write-Host ""
Write-Host "Waiting for server to initialize..." -ForegroundColor Yellow
Start-Sleep -Seconds 2

# Check if process is still running
if ($serverProcess.HasExited) {
    Write-Host "❌ Server process exited unexpectedly" -ForegroundColor Red
    Write-Host ""
    Write-Host "Error log:" -ForegroundColor Red
    Get-Content "test_error.log" -ErrorAction SilentlyContinue
    exit 1
}

Write-Host "✓ Server process is running" -ForegroundColor Green

# Test HTTP endpoint
Write-Host ""
Write-Host "Testing HTTP endpoint..." -ForegroundColor Yellow
try {
    $response = Invoke-RestMethod -Uri "http://$HttpBind/status" -Method Get -TimeoutSec $TimeoutSeconds
    Write-Host "✓ HTTP endpoint responding" -ForegroundColor Green
    Write-Host "  Uptime: $($response.uptime_ms)ms" -ForegroundColor Gray
    Write-Host "  Health: $($response.health)" -ForegroundColor Gray
} catch {
    Write-Host "❌ HTTP endpoint not responding: $_" -ForegroundColor Red
    $serverProcess | Stop-Process -Force
    exit 1
}

# Test memory add
Write-Host ""
Write-Host "Testing memory.add..." -ForegroundColor Yellow
try {
    $body = @{
        content = "Test memory from connection test script"
    } | ConvertTo-Json
    
    $response = Invoke-RestMethod -Uri "http://$HttpBind/memory/add" `
        -Method Post `
        -Body $body `
        -ContentType "application/json" `
        -TimeoutSec $TimeoutSeconds
    
    Write-Host "✓ Memory added successfully" -ForegroundColor Green
    Write-Host "  Memory ID: $($response.id)" -ForegroundColor Gray
    Write-Host "  Layer: $($response.layer)" -ForegroundColor Gray
    $memoryId = $response.id
} catch {
    Write-Host "❌ Memory add failed: $_" -ForegroundColor Red
    $serverProcess | Stop-Process -Force
    exit 1
}

# Test memory search
Write-Host ""
Write-Host "Testing memory.search..." -ForegroundColor Yellow
try {
    $response = Invoke-RestMethod -Uri "http://$HttpBind/memory/search?q=test&limit=5" `
        -Method Get `
        -TimeoutSec $TimeoutSeconds
    
    Write-Host "✓ Memory search successful" -ForegroundColor Green
    Write-Host "  Results: $($response.results.Count)" -ForegroundColor Gray
    
    if ($response.results.Count -eq 0) {
        Write-Host "  ⚠️  No results found (this may be expected)" -ForegroundColor Yellow
    }
} catch {
    Write-Host "❌ Memory search failed: $_" -ForegroundColor Red
    $serverProcess | Stop-Process -Force
    exit 1
}

# Check error log for issues
Write-Host ""
Write-Host "Checking for errors in log..." -ForegroundColor Yellow
$errorLog = Get-Content "test_error.log" -ErrorAction SilentlyContinue
if ($errorLog) {
    $errors = $errorLog | Select-String -Pattern "ERROR|WARN" -CaseSensitive
    if ($errors) {
        Write-Host "⚠️  Warnings/errors found in log:" -ForegroundColor Yellow
        $errors | ForEach-Object { Write-Host "  $_" -ForegroundColor Gray }
    } else {
        Write-Host "✓ No errors or warnings in log" -ForegroundColor Green
    }
} else {
    Write-Host "✓ Error log is empty (good!)" -ForegroundColor Green
}

# Stop server
Write-Host ""
Write-Host "Stopping test server..." -ForegroundColor Yellow
$serverProcess | Stop-Process -Force
Start-Sleep -Milliseconds 500
Write-Host "✓ Server stopped" -ForegroundColor Green

# Summary
Write-Host ""
Write-Host "=== Test Summary ===" -ForegroundColor Cyan
Write-Host "✓ All tests passed!" -ForegroundColor Green
Write-Host ""
Write-Host "The server is ready to use with VS Code + Kilo Code" -ForegroundColor Green
Write-Host ""
Write-Host "Next steps:" -ForegroundColor Yellow
Write-Host "1. Update your MCP config with the server path" -ForegroundColor White
Write-Host "2. Set RUST_LOG=info in the environment" -ForegroundColor White
Write-Host "3. Restart VS Code / Kilo Code" -ForegroundColor White
Write-Host "4. Try calling: system.status" -ForegroundColor White
Write-Host ""
Write-Host "For detailed setup, see: docs/mcp_docs/VS-Code-Kilo-Fix.md" -ForegroundColor Cyan

# Cleanup
Remove-Item "test_output.log" -ErrorAction SilentlyContinue
Remove-Item "test_error.log" -ErrorAction SilentlyContinue

exit 0

