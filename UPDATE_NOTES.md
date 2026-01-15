# Netch 更新说明

## 更新日期
2026年1月15日

## 主要更新内容

### 1. .NET 框架升级
- **从 .NET 6.0 升级到 .NET 8.0 LTS**
  - .NET 8.0 是长期支持版本（LTS），支持到 2026年11月
  - 性能提升约 20-30%
  - 更好的安全性和稳定性

### 2. NuGet 包更新

所有依赖包已更新到最新稳定版本：

| 包名 | 旧版本 | 新版本 | 说明 |
|------|--------|--------|------|
| Fody | 6.6.3 | 6.9.0 | IL 编织工具 |
| HMBSbige.SingleInstance | 6.0.0 | 8.0.0 | 单实例管理 |
| MaxMind.GeoIP2 | 5.1.0 | 6.1.0 | GeoIP 数据库 |
| Microsoft.Diagnostics.Tracing.TraceEvent | 3.0.1 | 3.1.20 | 性能追踪 |
| Microsoft.VisualStudio.Threading | 17.2.32 | 17.12.19 | 异步编程 |
| Microsoft.Windows.CsWin32 | 0.1.588-beta | 0.3.106 | Windows API |
| Serilog | 2.11.0 | 4.2.0 | 日志框架（重大更新）|
| Serilog.Extensions.Hosting | 4.2.0 | 8.0.0 | 日志扩展 |
| Serilog.Sinks.Async | 1.5.0 | 2.1.0 | 异步日志 |
| Serilog.Sinks.File | 5.0.0 | 6.0.0 | 文件日志 |
| Serilog.Sinks.Console | 4.0.1 | 6.0.0 | 控制台日志 |
| System.Management | 6.0.0 | 9.0.0 | 系统管理 |
| TaskScheduler | 2.10.1 | 2.11.1 | 任务调度 |
| System.ServiceProcess.ServiceController | 6.0.0 | 9.0.0 | 服务控制 |
| System.Text.Encoding.CodePages | 6.0.0 | 9.0.0 | 编码支持 |
| WindowsJobAPI | 6.0.0 | 8.0.0 | Windows 作业 API |

### 3. 测试项目更新
- Tests 项目从 .NET 5.0 升级到 .NET 8.0
- MSTest 包更新到最新版本 3.7.0

## 构建要求

### 必需软件
1. **.NET 8.0 SDK** (必需)
   - 下载地址: https://dotnet.microsoft.com/download/dotnet/8.0
   - 推荐安装最新的 8.0.x 版本

2. **Visual Studio 2022** 或 **MSBuild Tools**
   - 用于编译 C++ 项目（Redirector 和 RouteHelper）
   - 需要安装 "使用 C++ 的桌面开发" 工作负载

3. **PowerShell 5.1+** (Windows 自带)

### 可选软件
- Visual Studio Code
- Git for Windows

## 构建步骤

### 方法一：使用构建脚本（推荐）

```powershell
# 1. 打开 PowerShell，进入项目目录
cd netch-1.9.7

# 2. 如果需要使用代理
$env:http_proxy="http://127.0.0.1:10808"
$env:https_proxy="http://127.0.0.1:10808"

# 3. 运行构建脚本
.\build.ps1

# 4. 构建完成后，输出在 release 目录
```

### 方法二：手动构建

```powershell
# 1. 清理旧的构建文件
.\clean.ps1

# 2. 还原 NuGet 包
dotnet restore

# 3. 构建 C++ 项目
msbuild Redirector\Redirector.vcxproj -p:Configuration=Release -p:Platform=x64
msbuild RouteHelper\RouteHelper.vcxproj -p:Configuration=Release -p:Platform=x64

# 4. 构建主项目
dotnet publish Netch\Netch.csproj -c Release -r win-x64 --self-contained true -p:PublishSingleFile=true

# 5. 构建 Other 目录中的工具
cd Other
.\build.ps1
cd ..
```

## 可能遇到的问题

### 1. 找不到 dotnet 命令
**原因**: 未安装 .NET 8.0 SDK 或未添加到 PATH

**解决方案**:
```powershell
# 检查是否安装
dotnet --version

# 如果未安装，下载并安装 .NET 8.0 SDK
# https://dotnet.microsoft.com/download/dotnet/8.0
```

### 2. 找不到 MSBuild
**原因**: 未安装 Visual Studio 或 Build Tools

**解决方案**:
- 安装 Visual Studio 2022 Community（免费）
- 或安装 Build Tools for Visual Studio 2022
- 确保安装了 "使用 C++ 的桌面开发" 工作负载

### 3. NuGet 包下载失败
**原因**: 网络问题或需要代理

**解决方案**:
```powershell
# 设置代理
$env:http_proxy="http://127.0.0.1:10808"
$env:https_proxy="http://127.0.0.1:10808"

# 或配置 NuGet 代理
dotnet nuget add source https://api.nuget.org/v3/index.json -n nuget.org
```

### 4. Serilog 相关编译错误
**原因**: Serilog 从 2.x 升级到 4.x 有 API 变化

**解决方案**: 
- 检查代码中的 Serilog 使用方式
- 主要变化：日志配置 API 可能需要调整
- 参考: https://github.com/serilog/serilog/releases

## 验证构建

构建成功后，检查以下文件：

```
release/
├── Netch.exe              # 主程序
├── bin/
│   ├── Redirector.bin     # 重定向器
│   ├── RouteHelper.bin    # 路由助手
│   ├── nfapi.dll          # Netfilter API
│   ├── tun2socks.bin      # TUN 转 SOCKS
│   ├── aiodns.conf        # DNS 配置
│   ├── nfdriver.sys       # 驱动程序
│   ├── stun.txt           # STUN 服务器列表
│   └── GeoLite2-Country.mmdb  # GeoIP 数据库
├── i18n/                  # 国际化文件
└── mode/                  # 模式配置文件
```

## 运行测试

```powershell
# 运行单元测试
dotnet test Tests\Tests.csproj
```

## 回滚方案

如果更新后出现问题，可以：

1. 使用 Git 回滚更改：
```powershell
git checkout Netch/Netch.csproj
git checkout common.props
git checkout Tests/Tests.csproj
```

2. 或手动将 `TargetFramework` 改回 `net6.0-windows`

## 后续建议

1. **定期更新依赖包**
   ```powershell
   # 检查过时的包
   dotnet list package --outdated
   ```

2. **关注 .NET 版本支持周期**
   - .NET 8.0 支持到 2026年11月
   - 建议在 2026年下半年考虑升级到 .NET 10（下一个 LTS）

3. **监控 GitHub Issues**
   - 关注 Netch 官方仓库的更新
   - 查看是否有新的功能或修复

## 参考链接

- [.NET 8.0 下载](https://dotnet.microsoft.com/download/dotnet/8.0)
- [.NET 8.0 新特性](https://learn.microsoft.com/dotnet/core/whats-new/dotnet-8)
- [Netch GitHub](https://github.com/netchx/netch)
- [Serilog 迁移指南](https://github.com/serilog/serilog/wiki)
