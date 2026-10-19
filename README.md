# NPGA — Netch.Plus Game Accelerator

[English](#english) | [中文](#中文)

> **主线状态（2026-10-17）**：本项目已完成架构收敛，**Rust 原生客户端 [`npga-rust/`](npga-rust/) 是唯一活跃主线**，
> 也是本仓库中唯一的产品代码树。原 .NET/WinForms 客户端（Hatch 2.0.x）及其完整构建链（C#/C++/Go、
> 安装器、`build.ps1`）已冻结并整体移出仓库，归档于独立目录 `D:\netch.plus-archive`（独立 Git 仓库，
> 归档前状态见历史提交 `a934f42`）。重构计划与决策记录见 [`refactor/`](refactor/README.md)
> （ADR-001 冻结、ADR-007 归档）与 [`REFACTORING_PROPOSAL.md`](REFACTORING_PROPOSAL.md)。

---

## English

A lightweight and powerful network proxy tool for Windows, forked and continued from **Netch 1.9.7** with Xray-core integration.

### 📌 Project Origin

This project is a **continuation and enhancement** of [Netch 1.9.7](https://github.com/netchx/netch), maintaining compatibility while adding new features and optimizations.

### Features

- 🚀 **High Performance**: native Rust client shipped as a single portable executable with no .NET/WebView dependency
- 🔒 **Multiple Protocols**: Support for Hysteria2, Shadowsocks, VMess, VLESS, Trojan, WireGuard, and more
- 🎯 **Process Mode**: Route specific applications through proxy
- 🌐 **TUN/TAP Mode**: System-wide proxy with advanced routing
- 📊 **Real-time Monitoring**: Live bandwidth and latency display
- 🌍 **Multi-language**: Support for English, Chinese (Simplified & Traditional), and Japanese
- ⚡ **Optimized Build**: 56.8% smaller than standard builds while maintaining full functionality

### System Requirements

- Windows x64. Driver and proxy-core compatibility must be checked on the target system.
- No .NET runtime required: the client ships as a self-contained portable package (`NPGA-Rust.zip`).

### Installation

1. Download the latest release from [Releases](https://github.com/OffroadOps/Hatch/releases)
2. Extract all files to a folder
3. Run `NPGA-Rust.exe`

### Quick Start

1. **Add Server**: Click the server dropdown → Add server → Select protocol
2. **Select Mode**: Choose a mode from the mode dropdown (e.g., "Bypass LAN")
3. **Start**: Click the "Start" button to connect

### Key Improvements

#### Version 2.0.0 Updates (final .NET client release, now archived)
- **About Dialog**: New menu with version information and update features
  - Display NPGA, Xray-core, and sing-box versions
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
Servers are stored in `data/rust/settings.json` (the Rust client's data directory;
the archived 2.0.x client used `data/settings.json`). You can:
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
- Windows x64, PowerShell 7, Rust 1.99.0 (pinned by `npga-rust/rust-toolchain.toml`) with Visual C++ Build Tools / Windows SDK

#### Build Commands

```powershell
# Assemble the runtime bundle: pinned proxy cores + Storage assets + audited native components
.\Tools\AssembleRuntime.ps1 -RuntimeRoot .release-runtime

# Build the portable package (runs tests, clippy and fmt; outputs release-rust/)
.\build-rust.ps1 -RuntimePath .release-runtime
```

The frozen .NET/C++ build chain (`build.ps1`, `Hatch.sln`, the installer) is no longer part of this
repository; it lives in the external archive `D:\netch.plus-archive` (see `refactor/DECISIONS.md`
ADR-007) and is not required to build or release the client.

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

- 🚀 **高性能**: 原生 Rust 客户端，单文件便携，无 .NET/WebView 依赖
- 🔒 **多协议支持**: 支持 Hysteria2、Shadowsocks、VMess、VLESS、Trojan、WireGuard 等
- 🎯 **进程模式**: 为特定应用程序设置代理
- 🌐 **TUN/TAP 模式**: 系统级代理，支持高级路由规则
- 📊 **实时监控**: 实时显示带宽和延迟
- 🌍 **多语言**: 支持英语、简体中文、繁体中文和日语
- ⚡ **优化构建**: 体积减少 56.8%，同时保持完整功能

### 系统要求

- Windows x64；请在目标系统验证驱动及代理核心的兼容性。
- 不需要 .NET 运行时：客户端以自包含便携包（`NPGA-Rust.zip`）发布。

### 安装说明

1. 从 [Releases](https://github.com/OffroadOps/Hatch/releases) 下载最新版本
2. 解压所有文件到一个文件夹
3. 运行 `NPGA-Rust.exe`

### 快速开始

1. **添加服务器**: 点击服务器下拉菜单 → 添加服务器 → 选择协议
2. **选择模式**: 从模式下拉菜单选择一个模式（例如 "Bypass LAN"）
3. **启动**: 点击"启动"按钮连接

### 主要改进

#### 2.0.0 版本更新（.NET 客户端最后一版，现已归档）
- **关于对话框**: 新增菜单，显示版本信息和更新功能
  - 显示 NPGA、Xray-core 和 sing-box 版本
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
服务器存储在 `data/rust/settings.json`（Rust 客户端的数据目录；已归档的 2.0.x 客户端使用
`data/settings.json`）。你可以：
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
- Windows x64、PowerShell 7、Rust 1.99.0（由 `npga-rust/rust-toolchain.toml` 固定），以及 Visual C++ Build Tools / Windows SDK

#### 构建命令

```powershell
# 装配运行时：固定版本核心 + Storage 资产 + 审计过的原生组件
.\Tools\AssembleRuntime.ps1 -RuntimeRoot .release-runtime

# 构建便携包（自动运行测试、clippy 和 fmt；输出到 release-rust/）
.\build-rust.ps1 -RuntimePath .release-runtime
```

原 .NET/C++ 构建链（`build.ps1`、`Hatch.sln`、安装器）已不属于本仓库，位于独立归档
`D:\netch.plus-archive`（见 `refactor/DECISIONS.md` ADR-007），构建与发布客户端不再需要它。

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

**版本 | Version**: 3.0.0 (Rust)
**版权 | Copyright**: © 2026 OffroadOps
**基于 | Based on**: Netch 1.9.7

### Rust 原生客户端

Rust 客户端位于 [`npga-rust/`](npga-rust/README.md)，采用 ToDesk 风格的蓝白侧栏界面，兼容旧节点配置，内置固定版本的 Xray 与 sing-box（进程模式走 sing-box TUN，不依赖内核驱动）。运行 `./Tools/AssembleRuntime.ps1` 装配运行时，再运行 `./build-rust.ps1` 生成独立便携包。已完成能力、验证记录与兼容性边界见 [`MIGRATION.md`](npga-rust/MIGRATION.md)。

**该仓库是唯一主线**：新功能、新界面、发布包都以 Rust 版为准。功能对齐情况见
[`npga-rust/MIGRATION.md`](npga-rust/MIGRATION.md)，已补齐流量统计、自动更新、NAT 类型、
出口归属地、网络测速等原有能力。
