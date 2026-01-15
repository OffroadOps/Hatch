# Netch 一键构建脚本（带代理支持）
# 此脚本会自动设置代理并构建项目

param (
    [Parameter()]
    [ValidateSet('Debug', 'Release')]
    [string]
    $Configuration = 'Release',

    [Parameter()]
    [string]
    $ProxyUrl = "http://127.0.0.1:10808"
)

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  Netch 构建脚本" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# 设置代理
Write-Host "设置代理: $ProxyUrl" -ForegroundColor Yellow
$env:http_proxy = $ProxyUrl
$env:https_proxy = $ProxyUrl
$env:HTTP_PROXY = $ProxyUrl
$env:HTTPS_PROXY = $ProxyUrl
$env:all_proxy = "socks5://127.0.0.1:10808"
$env:ALL_PROXY = "socks5://127.0.0.1:10808"

Write-Host "✓ 代理设置完成" -ForegroundColor Green
Write-Host ""

# 检查 .NET SDK
Write-Host "检查 .NET SDK..." -ForegroundColor Yellow
try {
    $dotnetVersion = dotnet --version
    Write-Host "✓ .NET SDK 版本: $dotnetVersion" -ForegroundColor Green
    
    $majorVersion = [int]($dotnetVersion.Split('.')[0])
    if ($majorVersion -lt 8) {
        Write-Host "✗ 需要 .NET 8.0 或更高版本" -ForegroundColor Red
        Write-Host ""
        Write-Host "请运行以下命令安装 .NET 8.0 SDK:" -ForegroundColor Yellow
        Write-Host "  .\install-dotnet.ps1 -UseProxy" -ForegroundColor Cyan
        exit 1
    }
} catch {
    Write-Host "✗ 未找到 .NET SDK" -ForegroundColor Red
    Write-Host ""
    Write-Host "请运行以下命令安装 .NET 8.0 SDK:" -ForegroundColor Yellow
    Write-Host "  .\install-dotnet.ps1 -UseProxy" -ForegroundColor Cyan
    exit 1
}

Write-Host ""
Write-Host "开始构建 Netch ($Configuration)..." -ForegroundColor Yellow
Write-Host ""

# 调用原始构建脚本
try {
    .\build.ps1 -Configuration $Configuration
    
    if ($LASTEXITCODE -eq 0) {
        Write-Host ""
        Write-Host "========================================" -ForegroundColor Cyan
        Write-Host "  构建成功！" -ForegroundColor Green
        Write-Host "========================================" -ForegroundColor Cyan
        Write-Host ""
        Write-Host "输出目录: .\release\" -ForegroundColor Cyan
        Write-Host "主程序: .\release\Netch.exe" -ForegroundColor Cyan
        Write-Host ""
    } else {
        Write-Host ""
        Write-Host "✗ 构建失败，请检查错误信息" -ForegroundColor Red
        exit $LASTEXITCODE
    }
} catch {
    Write-Host ""
    Write-Host "✗ 构建过程出错: $_" -ForegroundColor Red
    exit 1
}
