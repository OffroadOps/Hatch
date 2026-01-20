# Netch 优化发布脚本
# 创建单文件、压缩、优化的发布版本

Write-Host "=== Netch 优化发布脚本 ===" -ForegroundColor Green
Write-Host ""

# 清理旧的发布文件
Write-Host "1. 清理旧文件..." -ForegroundColor Yellow
if (Test-Path "publish-optimized") {
    Remove-Item "publish-optimized" -Recurse -Force
}
New-Item -ItemType Directory -Path "publish-optimized" -Force | Out-Null

# 发布单文件版本（包含运行时）
Write-Host "2. 发布单文件版本（自包含）..." -ForegroundColor Yellow
Write-Host "   应用优化: 单文件 + 压缩 + 移除ICU + 无调试符号" -ForegroundColor Gray
dotnet publish Netch/Netch.csproj `
    -c Release `
    -r win-x64 `
    --self-contained true `
    -p:PublishSingleFile=true `
    -p:EnableCompressionInSingleFile=true `
    -p:IncludeNativeLibrariesForSelfExtract=true `
    -p:InvariantGlobalization=true `
    -p:DebugType=none `
    -p:DebugSymbols=false `
    -o "publish-optimized/self-contained"

Write-Host ""
Write-Host "3. 发布单文件版本（依赖框架）..." -ForegroundColor Yellow
Write-Host "   应用优化: 单文件 + 无调试符号" -ForegroundColor Gray
dotnet publish Netch/Netch.csproj `
    -c Release `
    -r win-x64 `
    --self-contained false `
    -p:PublishSingleFile=true `
    -p:DebugType=none `
    -p:DebugSymbols=false `
    -o "publish-optimized/framework-dependent"

# 复制必要的运行时文件
Write-Host ""
Write-Host "4. 复制运行时文件..." -ForegroundColor Yellow

# 复制到自包含版本
Copy-Item "release/bin" -Destination "publish-optimized/self-contained/bin" -Recurse -Force
Copy-Item "release/mode" -Destination "publish-optimized/self-contained/mode" -Recurse -Force
Copy-Item "release/i18n" -Destination "publish-optimized/self-contained/i18n" -Recurse -Force

# 复制到框架依赖版本
Copy-Item "release/bin" -Destination "publish-optimized/framework-dependent/bin" -Recurse -Force
Copy-Item "release/mode" -Destination "publish-optimized/framework-dependent/mode" -Recurse -Force
Copy-Item "release/i18n" -Destination "publish-optimized/framework-dependent/i18n" -Recurse -Force

# 显示文件大小
Write-Host ""
Write-Host "=== 发布完成 ===" -ForegroundColor Green
Write-Host ""
Write-Host "自包含版本（包含 .NET 运行时）:" -ForegroundColor Cyan
$selfContainedSize = (Get-ChildItem "publish-optimized/self-contained" -Recurse | Measure-Object -Property Length -Sum).Sum / 1MB
Write-Host "  位置: publish-optimized/self-contained/"
Write-Host "  大小: $([math]::Round($selfContainedSize, 2)) MB"
Write-Host "  优点: 无需安装 .NET 运行时，可直接运行"
Write-Host "  缺点: 体积较大"
Write-Host ""

Write-Host "框架依赖版本（需要 .NET 10.0）:" -ForegroundColor Cyan
$frameworkSize = (Get-ChildItem "publish-optimized/framework-dependent" -Recurse | Measure-Object -Property Length -Sum).Sum / 1MB
Write-Host "  位置: publish-optimized/framework-dependent/"
Write-Host "  大小: $([math]::Round($frameworkSize, 2)) MB"
Write-Host "  优点: 体积小"
Write-Host "  缺点: 需要安装 .NET 10.0 运行时"
Write-Host ""

# 创建压缩包
Write-Host "5. 创建压缩包..." -ForegroundColor Yellow

if (Test-Path "Netch/Resources/7za.exe") {
    # 使用 7za 压缩
    & "Netch/Resources/7za.exe" a -t7z -mx=9 "publish-optimized/Netch-SelfContained.7z" ".\publish-optimized\self-contained\*" | Out-Null
    & "Netch/Resources/7za.exe" a -t7z -mx=9 "publish-optimized/Netch-FrameworkDependent.7z" ".\publish-optimized\framework-dependent\*" | Out-Null
    
    $selfContained7z = (Get-Item "publish-optimized/Netch-SelfContained.7z").Length / 1MB
    $frameworkDependent7z = (Get-Item "publish-optimized/Netch-FrameworkDependent.7z").Length / 1MB
    
    Write-Host ""
    Write-Host "压缩包已创建:" -ForegroundColor Green
    Write-Host "  Netch-SelfContained.7z: $([math]::Round($selfContained7z, 2)) MB"
    Write-Host "  Netch-FrameworkDependent.7z: $([math]::Round($frameworkDependent7z, 2)) MB"
} else {
    Write-Host "未找到 7za.exe，跳过压缩" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "=== 完成 ===" -ForegroundColor Green
Write-Host ""
Write-Host "优化说明:" -ForegroundColor Cyan
Write-Host "  ✓ PublishSingleFile - 单文件发布"
Write-Host "  ✓ EnableCompressionInSingleFile - 内部压缩（自包含版本）"
Write-Host "  ✓ InvariantGlobalization - 移除ICU（减少10-30MB）"
Write-Host "  ✓ 移除调试符号（减少5-15MB）"
Write-Host "  ✗ PublishTrimmed - Windows Forms不支持"
Write-Host ""
Write-Host "注意事项:" -ForegroundColor Yellow
Write-Host "  - 自包含版本: 约72MB，无需安装.NET，可直接运行"
Write-Host "  - 框架依赖版本: 约0.5MB，需要安装.NET 10.0运行时"
Write-Host "  - InvariantGlobalization会影响'系统'语言自动检测"
Write-Host "  - 用户仍可手动选择语言（zh-CN, en-US, ja-JP等）"
Write-Host ""
