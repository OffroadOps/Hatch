# Hysteria2 连接测试脚本
# 用于快速测试 Hysteria2 配置是否正确

Write-Host "=== Hysteria2 连接测试 ===" -ForegroundColor Cyan
Write-Host ""

# 1. 检查 sing-box.exe
Write-Host "[1/5] 检查 sing-box.exe..." -ForegroundColor Yellow
$singboxPath = "Netch\bin\Release\bin\sing-box.exe"
if (Test-Path $singboxPath) {
    Write-Host "  ✓ sing-box.exe 存在" -ForegroundColor Green
    $version = & $singboxPath version 2>&1 | Select-Object -First 1
    Write-Host "  版本: $version" -ForegroundColor Gray
} else {
    Write-Host "  ✗ sing-box.exe 不存在！" -ForegroundColor Red
    Write-Host "  路径: $singboxPath" -ForegroundColor Red
    exit 1
}

# 2. 创建测试配置
Write-Host ""
Write-Host "[2/5] 创建测试配置..." -ForegroundColor Yellow
$testConfig = @"
{
  "log": {
    "level": "debug"
  },
  "inbounds": [
    {
      "type": "socks",
      "tag": "socks-in",
      "listen": "127.0.0.1",
      "listen_port": 12080
    }
  ],
  "outbounds": [
    {
      "type": "hysteria2",
      "tag": "proxy",
      "server": "85.121.50.184",
      "server_port": 47897,
      "password": "7c28bb3a-ce8d-4641-b60c-7831712be627",
      "server_ports": ["61111-62222"],
      "tls": {
        "enabled": true,
        "server_name": "addons.mozilla.org",
        "insecure": true,
        "alpn": ["h3"]
      }
    }
  ]
}
"@

$configPath = "test-hy2-config.json"
$testConfig | Out-File -FilePath $configPath -Encoding UTF8
Write-Host "  ✓ 配置文件已创建: $configPath" -ForegroundColor Green

# 3. 测试服务器连通性
Write-Host ""
Write-Host "[3/5] 测试服务器连通性..." -ForegroundColor Yellow
try {
    $result = Test-NetConnection -ComputerName "85.121.50.184" -Port 47897 -WarningAction SilentlyContinue
    if ($result.TcpTestSucceeded) {
        Write-Host "  ✓ 服务器 85.121.50.184:47897 可达" -ForegroundColor Green
    } else {
        Write-Host "  ✗ 服务器 85.121.50.184:47897 不可达" -ForegroundColor Red
        Write-Host "  这可能是正常的，因为 Hysteria2 使用 UDP" -ForegroundColor Yellow
    }
} catch {
    Write-Host "  ! 无法测试连通性: $_" -ForegroundColor Yellow
}

# 4. 启动 sing-box
Write-Host ""
Write-Host "[4/5] 启动 sing-box..." -ForegroundColor Yellow
Write-Host "  按 Ctrl+C 停止测试" -ForegroundColor Gray
Write-Host ""

$process = Start-Process -FilePath $singboxPath -ArgumentList "run", "-c", $configPath -NoNewWindow -PassThru -RedirectStandardOutput "test-output.log" -RedirectStandardError "test-error.log"

Start-Sleep -Seconds 3

if ($process.HasExited) {
    Write-Host "  ✗ sing-box 启动失败！" -ForegroundColor Red
    Write-Host ""
    Write-Host "错误日志:" -ForegroundColor Red
    Get-Content "test-error.log"
    exit 1
} else {
    Write-Host "  ✓ sing-box 已启动 (PID: $($process.Id))" -ForegroundColor Green
}

# 5. 测试连接
Write-Host ""
Write-Host "[5/5] 测试 SOCKS5 代理..." -ForegroundColor Yellow
Start-Sleep -Seconds 2

try {
    # 测试代理是否工作
    $env:http_proxy = "socks5://127.0.0.1:12080"
    $env:https_proxy = "socks5://127.0.0.1:12080"
    
    Write-Host "  正在通过代理访问 Google..." -ForegroundColor Gray
    $response = Invoke-WebRequest -Uri "https://www.google.com" -TimeoutSec 10 -UseBasicParsing
    
    if ($response.StatusCode -eq 200) {
        Write-Host "  ✓ 代理连接成功！" -ForegroundColor Green
        Write-Host "  HTTP 状态码: $($response.StatusCode)" -ForegroundColor Gray
    }
} catch {
    Write-Host "  ✗ 代理连接失败: $_" -ForegroundColor Red
    Write-Host ""
    Write-Host "请查看日志文件:" -ForegroundColor Yellow
    Write-Host "  - test-output.log" -ForegroundColor Gray
    Write-Host "  - test-error.log" -ForegroundColor Gray
}

# 清理
Write-Host ""
Write-Host "正在停止 sing-box..." -ForegroundColor Yellow
Stop-Process -Id $process.Id -Force
Start-Sleep -Seconds 1

Write-Host ""
Write-Host "=== 测试完成 ===" -ForegroundColor Cyan
Write-Host ""
Write-Host "日志文件:" -ForegroundColor Yellow
Write-Host "  - test-output.log (标准输出)" -ForegroundColor Gray
Write-Host "  - test-error.log (错误输出)" -ForegroundColor Gray
Write-Host "  - $configPath (配置文件)" -ForegroundColor Gray
Write-Host ""
Write-Host "如需查看详细日志，请运行:" -ForegroundColor Yellow
Write-Host "  Get-Content test-output.log" -ForegroundColor Gray
Write-Host "  Get-Content test-error.log" -ForegroundColor Gray
