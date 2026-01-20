# Hatch

[English](#english) | [中文](#中文)

---

## English

A lightweight and powerful network proxy tool for Windows, forked and continued from **Netch 1.9.7** with Xray-core integration.

### 📌 Project Origin

This project is a **continuation and enhancement** of [Netch 1.9.7](https://github.com/netchx/netch), maintaining compatibility while adding new features and optimizations.

### Features

- 🚀 **High Performance**: Built with .NET 10.0 and optimized for speed
- 🔒 **Multiple Protocols**: Support for Hysteria2, Shadowsocks, VMess, VLESS, Trojan, WireGuard, and more
- 🎯 **Process Mode**: Route specific applications through proxy
- 🌐 **TUN/TAP Mode**: System-wide proxy with advanced routing
- 📊 **Real-time Monitoring**: Live bandwidth and latency display
- 🌍 **Multi-language**: Support for English, Chinese (Simplified & Traditional), and Japanese
- ⚡ **Optimized Build**: 56.8% smaller than standard builds while maintaining full functionality

### System Requirements

- Windows 7 or later (x64)
- .NET 10.0 Runtime (for framework-dependent version) or use the self-contained version

### Installation

1. Download the latest release from [Releases](https://github.com/OffroadOps/Hatch/releases)
2. Extract all files to a folder
3. Run `Hatch.exe`

### Quick Start

1. **Add Server**: Click the server dropdown → Add server → Select protocol
2. **Select Mode**: Choose a mode from the mode dropdown (e.g., "Bypass LAN")
3. **Start**: Click the "Start" button to connect

### Key Improvements

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
- No antivirus false positives

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
- .NET 10.0 SDK
- Visual Studio 2022 or later (optional)
- MSBuild for C++ components

#### Build Commands

```powershell
# Standard build
.\build.ps1 -Configuration Release -OutputPath release

# The build will automatically:
# - Compile Netch (C# .NET application)
# - Compile Redirector (C++ network redirector)
# - Compile RouteHelper (C++ routing utilities)
# - Download dependencies (aiodns, xray, etc.)
# - Package everything into the release folder
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

- 🚀 **高性能**: 基于 .NET 10.0 构建，性能优化
- 🔒 **多协议支持**: 支持 Hysteria2、Shadowsocks、VMess、VLESS、Trojan、WireGuard 等
- 🎯 **进程模式**: 为特定应用程序设置代理
- 🌐 **TUN/TAP 模式**: 系统级代理，支持高级路由规则
- 📊 **实时监控**: 实时显示带宽和延迟
- 🌍 **多语言**: 支持英语、简体中文、繁体中文和日语
- ⚡ **优化构建**: 体积减少 56.8%，同时保持完整功能

### 系统要求

- Windows 7 或更高版本 (x64)
- .NET 10.0 运行时（框架依赖版本）或使用独立版本

### 安装说明

1. 从 [Releases](https://github.com/OffroadOps/Hatch/releases) 下载最新版本
2. 解压所有文件到一个文件夹
3. 运行 `Hatch.exe`

### 快速开始

1. **添加服务器**: 点击服务器下拉菜单 → 添加服务器 → 选择协议
2. **选择模式**: 从模式下拉菜单选择一个模式（例如 "Bypass LAN"）
3. **启动**: 点击"启动"按钮连接

### 主要改进

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
- 无杀毒软件误报

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
- .NET 10.0 SDK
- Visual Studio 2022 或更高版本（可选）
- MSBuild（用于 C++ 组件）

#### 构建命令

```powershell
# 标准构建
.\build.ps1 -Configuration Release -OutputPath release

# 构建将自动：
# - 编译 Netch（C# .NET 应用程序）
# - 编译 Redirector（C++ 网络重定向器）
# - 编译 RouteHelper（C++ 路由工具）
# - 下载依赖项（aiodns、xray 等）
# - 将所有内容打包到 release 文件夹
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
