#Requires -Version 5.1
<#
.SYNOPSIS
    sing-box TUN + process_name 替代 Redirector/netfilter2 的可行性验证（ADR-002）。

.DESCRIPTION
    五项验证：
      1. 配置检查（sing-box check）
      2. TUN 网卡创建与自动路由
      3. Handle 进程流量经 proxy outbound（clash API 连接链断言）
      4. Bypass 进程流量不经 proxy
      5. DNS 劫持（hijack-dns）与停止后清理

    必须以管理员身份运行（TUN 需要）。不会连接任何真实代理：
    proxy outbound 指向本机 127.0.0.1:18081 的 HTTP CONNECT  sink 服务，
    因此不产生外部流量、不修改任何持久配置。

.PARAMETER SingBox
    sing-box.exe 路径，默认取 release-net10 构建产物。

.PARAMETER Config
    配置文件路径，默认本目录 tun-process.json。

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File .\verify.ps1
#>
[CmdletBinding()]
param(
    [string]$SingBox = "D:\netch.plus\release-net10\Hatch\bin\sing-box.exe",
    [string]$Config = "$PSScriptRoot\tun-process.json"
)

$ErrorActionPreference = "Stop"
$script:Pass = 0
$script:Fail = 0

function Write-Check([string]$Name, [bool]$Ok, [string]$Detail = "") {
    if ($Ok) { $script:Pass++; Write-Host "  [PASS] $Name" -ForegroundColor Green }
    else { $script:Fail++; Write-Host "  [FAIL] $Name $(if ($Detail) { "- $Detail" })" -ForegroundColor Red }
}

# ---------- 0. 环境检查 ----------
Write-Host "`n=== NPGA TUN 进程路由验证 ===`n" -ForegroundColor Cyan
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = New-Object Security.Principal.WindowsPrincipal($identity)
$isAdmin = $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) { Write-Host "请以管理员身份运行（TUN 需要管理员权限）" -ForegroundColor Red; exit 1 }
if (-not (Test-Path $SingBox)) { Write-Host "找不到 sing-box: $SingBox" -ForegroundColor Red; exit 1 }
if (-not (Test-Path $Config)) { Write-Host "找不到配置: $Config" -ForegroundColor Red; exit 1 }

# ---------- 1. 配置检查 ----------
Write-Host "`n[1/5] sing-box check" -ForegroundColor Cyan
& $SingBox check -c $Config 2>&1 | ForEach-Object { Write-Host "    $_" }
Write-Check "sing-box 接受配置（自动路由/进程规则/DNS 劫持字段均有效）" ($LASTEXITCODE -eq 0)

# ---------- 2. 本地 HTTP CONNECT sink（替代真实代理） ----------
$sinkListener = New-Object System.Net.Sockets.TcpListener([System.Net.IPAddress]::Loopback, 18081)
$sinkListener.Start()
$ps = [powershell]::Create()
$sinkScript = {
    param($listener)
    while ($true) {
        try {
            $client = $listener.AcceptTcpClient()
            $stream = $client.GetStream()
            $buffer = New-Object byte[] 8192
            # 读到请求头结束为止
            while ($true) {
                $read = $stream.Read($buffer, 0, $buffer.Length)
                if ($read -le 0) { break }
                $text = [Text.Encoding]::ASCII.GetString($buffer, 0, $read)
                if ($text -match "`r`n`r`n") {
                    $ack = [Text.Encoding]::ASCII.GetBytes("HTTP/1.1 200 Connection Established`r`n`r`n")
                    $stream.Write($ack, 0, $ack.Length)
                    $stream.Flush()
                    break
                }
            }
            # 持续读并丢弃，保持连接存活
            while ($true) {
                $read = $stream.Read($buffer, 0, $buffer.Length)
                if ($read -le 0) { break }
            }
        } catch { }
    }
}
$null = $ps.AddScript($sinkScript).AddArgument($sinkListener)
$handle = $ps.BeginInvoke()
Write-Host "`n[2/5] 本地 CONNECT sink 已启动 (127.0.0.1:18081)" -ForegroundColor Cyan

# ---------- 3. 启动 sing-box ----------
Write-Host "`n[3/5] 启动 sing-box（TUN 模式）" -ForegroundColor Cyan
$logFile = Join-Path $env:TEMP "npga-tun-verify.log"
$logErrFile = Join-Path $env:TEMP "npga-tun-verify.err.log"
if (Test-Path $logFile) { Remove-Item $logFile -Force }
if (Test-Path $logErrFile) { Remove-Item $logErrFile -Force }
$sing = Start-Process -FilePath $SingBox -ArgumentList "run", "-c", $Config -WindowStyle Hidden -PassThru -RedirectStandardOutput $logFile -RedirectStandardError $logErrFile
$started = $false
foreach ($i in 1..40) {
    Start-Sleep -Milliseconds 250
    if ($sing.HasExited) { break }
    if (Select-String -Path $logFile,$logErrFile -Pattern "sing-box started" -Quiet -ErrorAction SilentlyContinue) { $started = $true; break }
}
Write-Check "sing-box 启动" $started

$adapter = $null
if ($started) {
    Start-Sleep -Milliseconds 800
    $adapter = Get-NetAdapter -Name "NPGA-TEST" -ErrorAction SilentlyContinue
    Write-Check "TUN 网卡 NPGA-TEST 已创建" ($null -ne $adapter) $(if (-not $adapter) { "网卡不存在" })
    $routes = (Get-NetRoute -InterfaceAlias "NPGA-TEST" -ErrorAction SilentlyContinue | Measure-Object).Count
    Write-Check "自动路由已下发（$routes 条）" ($routes -gt 0)
}

# ---------- 4. 进程匹配测试 ----------
Write-Host "`n[4/5] 进程匹配（Handle / Bypass）" -ForegroundColor Cyan
$curl = Join-Path $env:SystemRoot "System32\curl.exe"
$testDir = Join-Path $env:TEMP "npga-tun-test"
$null = New-Item -ItemType Directory -Path $testDir -Force
$handleExe = Join-Path $testDir "npga-handle-test.exe"
$bypassExe = Join-Path $testDir "npga-bypass-test.exe"
Copy-Item $curl $handleExe -Force
Copy-Item $curl $bypassExe -Force

function Test-ProcessRule([string]$Exe, [string]$ExpectChain, [int]$Seconds = 12) {
    $sawProxy = $false
    $sawProcess = $false
    $out = Join-Path $env:TEMP "npga-tun-sink.bin"
    # --resolve：让测试进程绕过 DNS —— CONNECT sink 无法承载 DNS（被劫持的查询会失败），
    # 断言只需要 TUN 层面的 TCP 连接建立并由 clash API 记录进程。
    $proc = Start-Process -FilePath $Exe -ArgumentList "-s", "--max-time", "$($Seconds + 4)", "--resolve", "cachefly.cachefly.net:80:93.184.216.34", "http://cachefly.cachefly.net/10mb.test", "-o", $out -WindowStyle Hidden -PassThru
    $deadline = (Get-Date).AddSeconds($Seconds)
    while ((Get-Date) -lt $deadline -and -not $proc.HasExited) {
        Start-Sleep -Milliseconds 400
        try {
            $api = Invoke-RestMethod -Uri "http://127.0.0.1:9199/connections" -TimeoutSec 2
            foreach ($conn in $api.connections.PSObject.Properties) {
                $meta = $conn.Value.metadata
                $chains = $conn.Value.chains -join ">"
                if ($meta.processPath -and $meta.processPath -like "*$(Split-Path $Exe -Leaf)*") {
                    $sawProcess = $true
                    if ($chains -like "*$ExpectChain*") { $sawProxy = $true }
                }
            }
        } catch { }
    }
    if (-not $proc.HasExited) { $proc.Kill() }
    return @{ SawProcess = $sawProcess; SawChain = $sawProxy }
}

$handleResult = Test-ProcessRule $handleExe "proxy"
Write-Check "Handle 进程连接在 clash API 可见（process_path 匹配）" $handleResult.SawProcess
Write-Check "Handle 进程流量走 proxy outbound" $handleResult.SawChain

$bypassResult = Test-ProcessRule $bypassExe "proxy"
Write-Check "Bypass 进程连接在 clash API 可见（process_path 匹配）" $bypassResult.SawProcess
Write-Check "Bypass 进程流量未走 proxy outbound" ($bypassResult.SawProcess -and -not $bypassResult.SawChain) "bypass 进程未出现连接或出现了 proxy 链"

# ---------- 5. DNS 劫持 + 清理 ----------
Write-Host "`n[5/5] DNS 劫持与停止清理" -ForegroundColor Cyan
try {
    $null = Resolve-DnsName -Name "www.mozilla.org" -Type A -ErrorAction SilentlyContinue -DnsOnly
} catch { }
Start-Sleep -Milliseconds 500
# 查询被 sing-box DNS 模块处理即证明 hijack-dns 生效。sink 无法承载 DNS，exchange 必然失败，
# 且不同 sing-box 版本日志文案不同（1.13 无 "hijacked dns query" 字样），故以查询域名出现在日志为准。
$hijacked = Select-String -Path $logFile,$logErrFile -Pattern "www\.mozilla\.org|hijacked dns query" -Quiet -ErrorAction SilentlyContinue
Write-Check "DNS 查询被劫持（hijack-dns 生效）" $hijacked

# 优雅停止（Ctrl+C 语义不存在于控制台子进程，先 taskkill 不带 /F）
$null = taskkill /PID $sing.Id 2>$null
Start-Sleep -Seconds 2
if (-not $sing.HasExited) { $null = taskkill /F /PID $sing.Id 2>$null }
Start-Sleep -Seconds 1
$adapterGone = $null -eq (Get-NetAdapter -Name "NPGA-TEST" -ErrorAction SilentlyContinue)
Write-Check "停止后 TUN 网卡已移除" $adapterGone "残留网卡（可能需要手动清理路由）"
$leftoverRoutes = Get-NetRoute -InterfaceAlias "NPGA-TEST" -ErrorAction SilentlyContinue
Write-Check "停止后路由已清理" (($leftoverRoutes | Measure-Object).Count -eq 0)

try { $sinkListener.Stop() } catch { }
try { $ps.Stop() } catch { }
Remove-Item $testDir -Recurse -Force -ErrorAction SilentlyContinue

Write-Host "`n=== 结果: PASS=$($script:Pass) FAIL=$($script:Fail) ===" -ForegroundColor Cyan
if ($script:Fail -eq 0) {
    Write-Host "全部通过：sing-box TUN + process_name 可替代 Redirector/netfilter2（ADR-002 可标记 Accepted）" -ForegroundColor Green
    exit 0
} else {
    Write-Host "存在失败项，检查日志: $logFile" -ForegroundColor Yellow
    exit 2
}
