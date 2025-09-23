$exe = 'C:\Users\charl\Desktop\MyProjects\MemorizedMCP\target\debug\memory_mcp_server.exe'
$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = $exe
$psi.RedirectStandardInput = $true
$psi.RedirectStandardOutput = $true
$psi.UseShellExecute = $false
$psi.WorkingDirectory = 'C:\Users\charl\Desktop\MyProjects\MemorizedMCP'
$psi.EnvironmentVariables['HTTP_BIND'] = '127.0.0.1:18080'
$psi.EnvironmentVariables['DATA_DIR'] = '.\data_mcp_test'
$proc = New-Object System.Diagnostics.Process
$proc.StartInfo = $psi
$null = $proc.Start()
function Send-Req([string]$json) {
  $proc.StandardInput.WriteLine($json)
  $proc.StandardInput.Flush()
  Start-Sleep -Milliseconds 150
  return $proc.StandardOutput.ReadLine()
}
$o1 = Send-Req('{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}')
$o2 = Send-Req('{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}')
$o3 = Send-Req('{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"system.status","arguments":{}}}')
Write-Output $o1
Write-Output $o2
Write-Output $o3
$proc.Kill()
