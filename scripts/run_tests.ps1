$ErrorActionPreference = "Stop"
$base = "http://127.0.0.1:8080"
$script:results = @()

function Add-Result($name, $ok, $status, $body) {
  $script:results += [pscustomobject]@{ Test=$name; Ok=$ok; Status=$status; Body=$body }
}

function Invoke-JsonPost($url, $obj) {
  $json = $obj | ConvertTo-Json -Depth 8
  return Invoke-WebRequest -Method POST -Uri $url -ContentType 'application/json' -Body $json
}

# 1. System & Health
try { $resp = Invoke-WebRequest -Uri "$base/health"; Add-Result "GET /health" ($resp.StatusCode -eq 200) $resp.StatusCode $resp.Content } catch { Add-Result "GET /health" $false 0 $_.Exception.Message }
try { $resp = Invoke-WebRequest -Uri "$base/status"; Add-Result "GET /status" ($resp.StatusCode -eq 200) $resp.StatusCode $resp.Content } catch { Add-Result "GET /status" $false 0 $_.Exception.Message }
try { $resp = Invoke-WebRequest -Uri "$base/metrics"; Add-Result "GET /metrics" ($resp.StatusCode -eq 200) $resp.StatusCode ($resp.Content.Substring(0, [Math]::Min(200, $resp.Content.Length))) } catch { Add-Result "GET /metrics" $false 0 $_.Exception.Message }
try { $resp = Invoke-WebRequest -Uri "$base/tools"; Add-Result "GET /tools" ($resp.StatusCode -eq 200) $resp.StatusCode $resp.Content } catch { Add-Result "GET /tools" $false 0 $_.Exception.Message }

# 2. Document Pipeline
$docId = $null
$doc2Path = (Join-Path (Get-Location) 'tmp_doc.md')
Set-Content -Path $doc2Path -Value "# TmpDoc`nThis is a small md file." -Encoding UTF8
try { $resp = Invoke-JsonPost "$base/document/store" @{ mime='md'; content='# Title`nHello world' }; $doc = $resp.Content | ConvertFrom-Json; $docId = $doc.id; Add-Result "POST /document/store md" ($resp.StatusCode -eq 200 -and $doc.chunks -ge 1) $resp.StatusCode $resp.Content } catch { Add-Result "POST /document/store md" $false 0 $_.Exception.Message }
if ($docId) {
  try { $resp = Invoke-WebRequest -Uri "$base/document/retrieve?id=$docId"; Add-Result "GET /document/retrieve?id" ($resp.StatusCode -eq 200) $resp.StatusCode $resp.Content } catch { Add-Result "GET /document/retrieve?id" $false 0 $_.Exception.Message }
  try { $resp = Invoke-WebRequest -Uri "$base/document/analyze?id=$docId"; Add-Result "GET /document/analyze" ($resp.StatusCode -eq 200) $resp.StatusCode $resp.Content } catch { Add-Result "GET /document/analyze" $false 0 $_.Exception.Message }
}
try { $resp = Invoke-JsonPost "$base/document/store" @{ path=$doc2Path; mime='md' }; $doc2 = $resp.Content | ConvertFrom-Json; Add-Result "POST /document/store path" ($resp.StatusCode -eq 200) $resp.StatusCode $resp.Content } catch { Add-Result "POST /document/store path" $false 0 $_.Exception.Message }
try { $resp = Invoke-WebRequest -Uri ("$base/document/retrieve?path=" + [uri]::EscapeDataString($doc2Path)); Add-Result "GET /document/retrieve?path" ($resp.StatusCode -eq 200) $resp.StatusCode $resp.Content } catch { Add-Result "GET /document/retrieve?path" $false 0 $_.Exception.Message }
try { $resp = Invoke-JsonPost "$base/document/validate_refs" @{ fix=$false }; Add-Result "POST /document/validate_refs" ($resp.StatusCode -eq 200) $resp.StatusCode $resp.Content } catch { Add-Result "POST /document/validate_refs" $false 0 $_.Exception.Message }

# 3. Memory
$memId = $null
try { $resp = Invoke-JsonPost "$base/memory/add" @{ content='project kickoff notes' }; $js = $resp.Content | ConvertFrom-Json; $memId = $js.id; Add-Result "POST /memory/add" ($resp.StatusCode -eq 200) $resp.StatusCode $resp.Content } catch { Add-Result "POST /memory/add" $false 0 $_.Exception.Message }
try { $resp = Invoke-WebRequest -Uri "$base/memory/search?q=kickoff&limit=10"; $ok = ($resp.StatusCode -eq 200); Add-Result "GET /memory/search" $ok $resp.StatusCode $resp.Content } catch { Add-Result "GET /memory/search" $false 0 $_.Exception.Message }
# add with reference to docId if available
$memRefId = $null
if ($docId) {
  try { $resp = Invoke-JsonPost "$base/memory/add" @{ content='notes referencing doc'; references=@(@{ docId=$docId; score=0.8 }) }; $js = $resp.Content | ConvertFrom-Json; $memRefId = $js.id; Add-Result "POST /memory/add with refs" ($resp.StatusCode -eq 200) $resp.StatusCode $resp.Content } catch { Add-Result "POST /memory/add with refs" $false 0 $_.Exception.Message }
  try { $resp = Invoke-WebRequest -Uri "$base/document/refs_for_memory?id=$memRefId"; Add-Result "GET /document/refs_for_memory" ($resp.StatusCode -eq 200) $resp.StatusCode $resp.Content } catch { Add-Result "GET /document/refs_for_memory" $false 0 $_.Exception.Message }
  try { $resp = Invoke-WebRequest -Uri "$base/document/refs_for_document?id=$docId"; Add-Result "GET /document/refs_for_document" ($resp.StatusCode -eq 200) $resp.StatusCode $resp.Content } catch { Add-Result "GET /document/refs_for_document" $false 0 $_.Exception.Message }
}
# update & delete
if ($memId) {
  try { $resp = Invoke-JsonPost "$base/memory/update" @{ id=$memId; content='updated content' }; Add-Result "POST /memory/update" ($resp.StatusCode -eq 200) $resp.StatusCode $resp.Content } catch { Add-Result "POST /memory/update" $false 0 $_.Exception.Message }
  try { $resp = Invoke-JsonPost "$base/memory/delete" @{ id=$memId; backup=$true }; Add-Result "POST /memory/delete" ($resp.StatusCode -eq 200) $resp.StatusCode $resp.Content } catch { Add-Result "POST /memory/delete" $false 0 $_.Exception.Message }
}

# 4. Fusion & Analytics
try { $resp = Invoke-WebRequest -Uri "$base/search/fusion?q=kickoff&limit=10"; Add-Result "GET /search/fusion" ($resp.StatusCode -eq 200) $resp.StatusCode $resp.Content } catch { Add-Result "GET /search/fusion" $false 0 $_.Exception.Message }
try { $resp = Invoke-JsonPost "$base/advanced/analyze_patterns" @{ window=@{ }; minSupport=1 }; Add-Result "POST /advanced/analyze_patterns" ($resp.StatusCode -eq 200) $resp.StatusCode $resp.Content } catch { Add-Result "POST /advanced/analyze_patterns" $false 0 $_.Exception.Message }
try { $now = [int64]([DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()); $resp = Invoke-JsonPost "$base/advanced/trends" @{ from=($now-3600000); to=$now; buckets=4 }; Add-Result "POST /advanced/trends" ($resp.StatusCode -eq 200) $resp.StatusCode $resp.Content } catch { Add-Result "POST /advanced/trends" $false 0 $_.Exception.Message }
try { $resp = Invoke-JsonPost "$base/advanced/clusters" @{}; Add-Result "POST /advanced/clusters" ($resp.StatusCode -eq 200) $resp.StatusCode $resp.Content } catch { Add-Result "POST /advanced/clusters" $false 0 $_.Exception.Message }
try { $resp = Invoke-JsonPost "$base/advanced/relationships" @{}; Add-Result "POST /advanced/relationships" ($resp.StatusCode -eq 200) $resp.StatusCode $resp.Content } catch { Add-Result "POST /advanced/relationships" $false 0 $_.Exception.Message }
try { $resp = Invoke-JsonPost "$base/advanced/effectiveness" @{}; Add-Result "POST /advanced/effectiveness" ($resp.StatusCode -eq 200) $resp.StatusCode $resp.Content } catch { Add-Result "POST /advanced/effectiveness" $false 0 $_.Exception.Message }

# 5. Consolidation & Maintenance
try { $resp = Invoke-JsonPost "$base/advanced/consolidate" @{ dryRun=$false; limit=10 }; Add-Result "POST /advanced/consolidate" ($resp.StatusCode -eq 200) $resp.StatusCode $resp.Content } catch { Add-Result "POST /advanced/consolidate" $false 0 $_.Exception.Message }
try { $resp = Invoke-JsonPost "$base/system/cleanup" @{ compact=$true }; Add-Result "POST /system/cleanup" ($resp.StatusCode -eq 200) $resp.StatusCode $resp.Content } catch { Add-Result "POST /system/cleanup" $false 0 $_.Exception.Message }
try { $resp = Invoke-JsonPost "$base/advanced/reindex" @{ vector=$true; text=$true; graph=$true }; Add-Result "POST /advanced/reindex" ($resp.StatusCode -eq 200) $resp.StatusCode $resp.Content } catch { Add-Result "POST /advanced/reindex" $false 0 $_.Exception.Message }

# 6. Backup/Restore/Validate
$backupPath = $null
try { $resp = Invoke-JsonPost "$base/system/backup" @{ destination='./backups'; includeIndices=$true }; $js = $resp.Content | ConvertFrom-Json; $backupPath = $js.path; Add-Result "POST /system/backup" ($resp.StatusCode -eq 200) $resp.StatusCode $resp.Content } catch { Add-Result "POST /system/backup" $false 0 $_.Exception.Message }
try { $resp = Invoke-WebRequest -Uri "$base/system/validate"; Add-Result "GET /system/validate" ($resp.StatusCode -eq 200) $resp.StatusCode $resp.Content } catch { Add-Result "GET /system/validate" $false 0 $_.Exception.Message }
if ($backupPath) {
  try { $resp = Invoke-JsonPost "$base/system/restore" @{ source=$backupPath; includeIndices=$true }; Add-Result "POST /system/restore" ($resp.StatusCode -eq 200) $resp.StatusCode $resp.Content } catch { Add-Result "POST /system/restore" $false 0 $_.Exception.Message }
}

# 7. Error Handling
try { $resp = Invoke-JsonPost "$base/document/store" @{ }; Add-Result "POST /document/store INVALID_INPUT" $false $resp.StatusCode $resp.Content } catch { Add-Result "POST /document/store INVALID_INPUT" $true 400 $_.Exception.Message }
try { $resp = Invoke-JsonPost "$base/memory/add" @{ content='' }; Add-Result "POST /memory/add INVALID_INPUT" $false $resp.StatusCode $resp.Content } catch { Add-Result "POST /memory/add INVALID_INPUT" $true 400 $_.Exception.Message }
try { $resp = Invoke-WebRequest -Uri "$base/document/retrieve?id=unknown"; Add-Result "GET /document/retrieve NOT_FOUND" $false $resp.StatusCode $resp.Content } catch { Add-Result "GET /document/retrieve NOT_FOUND" $true 404 $_.Exception.Message }

# Write results
$md = "# Testing Results`n`n| Test | OK | Status | Snippet |`n|---|---:|---:|---|`n"
foreach ($r in $script:results) {
  $snippet = if ($r.Body -is [string]) { $r.Body.Substring(0, [Math]::Min(200, $r.Body.Length)).Replace("`n"," ") } else { "" }
  $ok = if ($r.Ok) { 'true' } else { 'false' }
  $md += "| $($r.Test) | $ok | $($r.Status) | $snippet |`n"
}
Set-Content -Path "docs/TestingResult.md" -Value $md -Encoding UTF8
Write-Host "Wrote docs/TestingResult.md"
