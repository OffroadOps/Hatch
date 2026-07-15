# Hatch

[English](#english) | [中文](#中文)

---

## English

A lightweight and powerful network proxy tool for Windows, forked and continued from **Netch 1.9.7** with Xray-core integration.

### 📌 Project Origin

This project is a **continuation and enhancement** of [Netch 1.9.7](https://github.com/netchx/netch), maintaining compatibility while adding new features and optimizations.

### Features

- 🚀 **High Performance**: Built with .NET 8.0 and optimized for speed
- 🔒 **Multiple Protocols**: Support for Hysteria2, Shadowsocks, VMess, VLESS, Trojan, WireGuard, and more
- 🎯 **Process Mode**: Route specific applications through proxy
- 🌐 **TUN/TAP Mode**: System-wide proxy with advanced routing
- 📊 **Real-time Monitoring**: Live bandwidth and latency display
- 🌍 **Multi-language**: Support for English, Chinese (Simplified & Traditional), and Japanese
- ⚡ **Optimized Build**: 56.8% smaller than standard builds while maintaining full functionality

### System Requirements

- Windows 10 version 1607 or later (x64)
- .NET 8.0 Runtime (for framework-dependent version) or use the self-contained version

### Installation

1. Download the latest release from [Releases](https://github.com/OffroadOps/Hatch/releases)
2. Extract all files to a folder
3. Run `Hatch.exe`

### Quick Start

1. **Add Server**: Click the server dropdown → Add server → Select protocol
2. **Select Mode**: Choose a mode from the mode dropdown (e.g., "Bypass LAN")
3. **Start**: Click the "Start" button to connect

### Key Improvements

#### Version 2.0.0 Updates
- **About Dialog**: New menu with version information and update features
  - Display Hatch, Xray-core, and sing-box versions
  - One-click software and core updates
  - Tribute to Netch project
- **Core Management**: Automated download scripts for sing-box with proxy support
- **Enhanced Stability**: Improved core file detection and management

#### UI Enhancements
- Streamlined interface with hidden configuration sections
- Color-coded latency display (Green <80ms, Orange 80-200ms, Red >200ms)
- Real-time speed test with IP geolocation
- Automatic delay testing on startup

#### Protocol Support
- **Hysteria2**: Optimized with ICMP ping for accurate latency measurement
- **Xray-core**: Latest version integrated for enhanced performance

#### Build Optimization
- Single-file deployment with internal compression
- 56.8% size reduction (from 166.55 MB to 72 MB)
- Full multi-language support maintained
- Release binaries are currently unsigned; Windows SmartScreen or antivirus software may show a warning. Verify the published SHA256 before running.

### Configuration

#### Server Configuration
Servers are stored in `data/servers.json`. You can:
- Import from clipboard (subscription links supported)
- Add manually through the UI
- Test latency with one click

#### Mode Configuration
Modes define routing rules and are stored in the `mode/` folder:
- **Process Mode**: Route specific applications
- **TUN/TAP Mode**: System-wide routing with custom rules
- **Bypass Mode**: Exclude local and China IPs

### Building from Source

#### Prerequisites
- .NET SDK 8.0.100 (the version pinned by `global.json`)
- Visual Studio 2022 Build Tools with **Desktop development with C++**
- Go and a MinGW-w64 C compiler (for `aiodns.bin`)
- Inno Setup 6 (omit with `-SkipInstaller` for archive-only builds)

#### Build Commands

```powershell
# Standard build
.\build.ps1 -Configuration Release -OutputPath release

# The build will automatically:
# - Run the test suite and compile Hatch (C# .NET application)
# - Compile Redirector (C++ network redirector)
# - Compile RouteHelper (C++ routing utilities)
# - Build aiodns and download version-pinned dependencies with SHA256 verification
# - Package Hatch.zip and HatchSetup.exe into the release folder
```

For a local archive-only verification using the audited native binaries in `artifacts/`:

```powershell
.\build.ps1 -Configuration Release -OutputPath release -SkipNativeBuild -SkipInstaller
```

### License

GPL-3.0 License - see [LICENSE](LICENSE) for details

### Credits

- Original project: [Netch 1.9.7](https://github.com/netchx/netch)
- Xray-core: [Xray-project](https://github.com/XTLS/Xray-core)
- Hysteria2: [Hysteria](https://github.com/apernet/hysteria)

### Contributing

Contributions are welcome! Please feel free to submit issues and pull requests.

### Disclaimer

This tool is for educational and research purposes only. Users are responsible for complying with local laws and regulations.

---

## 中文

一个轻量级且功能强大的 Windows 网络代理工具，基于 **Netch 1.9.7** 继续开发，集成 Xray-core。

### 📌 项目来源

本项目是 [Netch 1.9.7](https://github.com/netchx/netch) 的**延续和增强版本**，在保持兼容性的同时添加了新功能和优化。

### 功能特性

- 🚀 **高性能**: 基于 .NET 8.0 构建，性能优化
- 🔒 **多协议支持**: 支持 Hysteria2、Shadowsocks、VMess、VLESS、Trojan、WireGuard 等
- 🎯 **进程模式**: 为特定应用程序设置代理
- 🌐 **TUN/TAP 模式**: 系统级代理，支持高级路由规则
- 📊 **实时监控**: 实时显示带宽和延迟
- 🌍 **多语言**: 支持英语、简体中文、繁体中文和日语
- ⚡ **优化构建**: 体积减少 56.8%，同时保持完整功能

### 系统要求

- Windows 10 1607 或更高版本 (x64)
- .NET 8.0 运行时（框架依赖版本）或使用独立版本

### 安装说明

1. 从 [Releases](https://github.com/OffroadOps/Hatch/releases) 下载最新版本
2. 解压所有文件到一个文件夹
3. 运行 `Hatch.exe`

### 快速开始

1. **添加服务器**: 点击服务器下拉菜单 → 添加服务器 → 选择协议
2. **选择模式**: 从模式下拉菜单选择一个模式（例如 "Bypass LAN"）
3. **启动**: 点击"启动"按钮连接

### 主要改进

#### 2.0.0 版本更新
- **关于对话框**: 新增菜单，显示版本信息和更新功能
  - 显示 Hatch、Xray-core 和 sing-box 版本
  - 一键更新软件和核心文件
  - 致敬 Netch 项目
- **核心管理**: sing-box 自动下载脚本，支持代理
- **稳定性增强**: 改进核心文件检测和管理

#### UI 增强
- 简化界面，隐藏配置部分
- 延迟颜色编码显示（绿色 <80ms，橙色 80-200ms，红色 >200ms）
- 实时速度测试，带 IP 地理位置
- 启动时自动延迟测试

#### 协议支持
- **Hysteria2**: 使用 ICMP ping 优化，延迟测量更准确
- **Xray-core**: 集成最新版本，性能增强

#### 构建优化
- 单文件部署，内部压缩
- 体积减少 56.8%（从 166.55 MB 降至 72 MB）
- 保持完整的多语言支持
- 当前发布二进制尚未签名；Windows SmartScreen 或杀毒软件可能提示风险。运行前请核对 Release 中公布的 SHA256。

### 配置说明

#### 服务器配置
服务器存储在 `data/servers.json` 文件中。你可以：
- 从剪贴板导入（支持订阅链接）
- 通过 UI 手动添加
- 一键测试延迟

#### 模式配置
模式定义路由规则，存储在 `mode/` 文件夹中：
- **进程模式**: 为特定应用程序设置路由
- **TUN/TAP 模式**: 系统级路由，支持自定义规则
- **绕过模式**: 排除本地和中国 IP

### 从源码构建

#### 前置要求
- .NET SDK 8.0.100（由 `global.json` 固定）
- Visual Studio 2022 Build Tools，并安装“使用 C++ 的桌面开发”工作负载
- Go 和 MinGW-w64 C 编译器（用于构建 `aiodns.bin`）
- Inno Setup 6（仅构建压缩包时可传 `-SkipInstaller`）

#### 构建命令

```powershell
# 标准构建
.\build.ps1 -Configuration Release -OutputPath release

# 构建将自动：
# - 运行测试并编译 Hatch（C# .NET 应用程序）
# - 编译 Redirector（C++ 网络重定向器）
# - 编译 RouteHelper（C++ 路由工具）
# - 构建 aiodns，并下载固定版本且校验 SHA256 的依赖项
# - 在 release 文件夹生成 Hatch.zip 和 HatchSetup.exe
```

本机仅验证压缩包、复用 `artifacts/` 中已审计原生文件时：

```powershell
.\build.ps1 -Configuration Release -OutputPath release -SkipNativeBuild -SkipInstaller
```

### 许可证

GPL-3.0 许可证 - 详见 [LICENSE](LICENSE)

### 致谢

- 原始项目: [Netch 1.9.7](https://github.com/netchx/netch)
- Xray-core: [Xray-project](https://github.com/XTLS/Xray-core)
- Hysteria2: [Hysteria](https://github.com/apernet/hysteria)

### 贡献

欢迎贡献！请随时提交问题和拉取请求。

### 免责声明

本工具仅供教育和研究目的使用。使用者需遵守当地法律法规。

---

**版本 | Version**: 2.0.0
**版权 | Copyright**: © 2026 OffroadOps
**基于 | Based on**: Netch 1.9.7
