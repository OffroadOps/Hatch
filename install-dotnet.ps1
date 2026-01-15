# Netch - .NET 8.0 SDK 安装脚本
# 此脚本会自动下载并安装 .NET 8.0 SDK

param(
    [Parameter()]
    [switch]$UseProxy = $false,
    
    [Parameter()]
    [string]$ProxyUrl = "http://127.0.0.1:10808"
)

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  Netch - .NET 8.0 SDK 安装脚本" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# 检查是否已安装 .NET SDK
Write-Host "检查当前 .NET SDK 安装情况..." -ForegroundColor Yellow

try {
    $dotnetVersion = dotnet --version 2>$null
    if ($dotnetVersion) {
        Write-Host "已安装 .NET SDK 版本: $dotnetVersion" -ForegroundColor Green
        
        $majorVersion = [int]($dotnetVersion.Split('.')[0])
        if ($majorVersion -ge 8) {
            Write-Host "✓ 已安装 .NET 8.0 或更高版本，无需重新安装" -ForegroundColor Green
            Write-Host ""
            Write-Host "可以直接运行构建脚本:" -ForegroundColor Cyan
            Write-Host "  .\build.ps1" -ForegroundColor White
            exit 0
        } else {
            Write-Host "⚠ 当前版本低于 .NET 8.0，需要安装新版本" -ForegroundColor Yellow
        }
    }
} catch {
    Write-Host "未检测到 .NET SDK" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "准备安装 .NET 8.0 SDK..." -ForegroundColor Yellow
Write-Host ""

# 设置代理（如果需要）
if ($UseProxy) {
    Write-Host "使用代理: $ProxyUrl" -ForegroundColor Cyan
    $env:http_proxy = $ProxyUrl
    $env:https_proxy = $ProxyUrl
}

# .NET 8.0 SDK 下载链接
$dotnetInstallerUrl = "https://dotnet.microsoft.com/download/dotnet/thank-you/sdk-8.0.404-windows-x64-installer"
$installerPath = "$env:TEMP\dotnet-sdk-8.0-installer.exe"

Write-Host "下载 .NET 8.0 SDK 安装程序..." -ForegroundColor Yellow
Write-Host "下载地址: $dotnetInstallerUrl" -ForegroundColor Gray

try {
    # 使用 Invoke-WebRequest 下载
    $ProgressPreference = 'SilentlyContinue'
    Invoke-WebRequest -Uri $dotnetInstallerUrl -OutFile $installerPath -UseBasicParsing
    Write-Host "✓ 下载完成" -ForegroundColor Green
} catch {
    Write-Host "✗ 下载失败: $_" -ForegroundColor Red
    Write-Host ""
    Write-Host "请手动下载并安装 .NET 8.0 SDK:" -ForegroundColor Yellow
    Write-Host "  https://dotnet.microsoft.com/download/dotnet/8.0" -ForegroundColor Cyan
    exit 1
}

Write-Host ""
Write-Host "开始安装 .NET 8.0 SDK..." -ForegroundColor Yellow
Write-Host "注意: 安装过程可能需要几分钟，请耐心等待" -ForegroundColor Gray

try {
    # 运行安装程序
    Start-Process -FilePath $installerPath -ArgumentList "/quiet", "/norestart" -Wait -NoNewWindow
    Write-Host "✓ 安装完成" -ForegroundColor Green
} catch {
    Write-Host "✗ 安装失败: $_" -ForegroundColor Red
    Write-Host ""
    Write-Host "请尝试手动运行安装程序:" -ForegroundColor Yellow
    Write-Host "  $installerPath" -ForegroundColor Cyan
    exit 1
}

# 清理安装文件
Remove-Item -Path $installerPath -Force -ErrorAction SilentlyContinue

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  安装完成！" -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# 验证安装
Write-Host "验证安装..." -ForegroundColor Yellow
Write-Host "注意: 可能需要重新打开 PowerShell 窗口才能使用 dotnet 命令" -ForegroundColor Gray
Write-Host ""

try {
    # 刷新环境变量
    $env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path","User")
    
    $newVersion = dotnet --version 2>$null
    if ($newVersion) {
        Write-Host "✓ .NET SDK 版本: $newVersion" -ForegroundColor Green
    } else {
        Write-Host "⚠ 请重新打开 PowerShell 窗口，然后运行:" -ForegroundColor Yellow
        Write-Host "  dotnet --version" -ForegroundColor White
    }
} catch {
    Write-Host "⚠ 请重新打开 PowerShell 窗口，然后运行:" -ForegroundColor Yellow
    Write-Host "  dotnet --version" -ForegroundColor White
}

Write-Host ""
Write-Host "下一步操作:" -ForegroundColor Cyan
Write-Host "  1. 重新打开 PowerShell（如果需要）" -ForegroundColor White
Write-Host "  2. 进入项目目录: cd netch-1.9.7" -ForegroundColor White
Write-Host "  3. 运行构建脚本: .\build.ps1" -ForegroundColor White
Write-Host ""
