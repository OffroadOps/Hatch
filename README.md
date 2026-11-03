# NPGA — Netch.Plus Game Accelerator

[English](#english) | [中文](#中文)

> **主线状态（2026-11-03）**：**Rust 原生客户端 [`npga-rust/`](npga-rust/) 是本仓库唯一的产品代码树**，
> 新功能、新界面与发布包均以它为准。原 .NET/WinForms 客户端（Hatch 2.0.x）及其完整构建链
> （C#/C++/Go、安装器、`build.ps1`）已冻结并移出仓库，存于外部归档仓库（见
> [`refactor/DECISIONS.md`](refactor/DECISIONS.md) ADR-007，不含任何本机路径）。重构计划与
> 全部决策记录见 [`refactor/`](refactor/README.md)：历史评估 [`refactor/PROJECT_REVIEW.md`](refactor/PROJECT_REVIEW.md)、
> 路线图 [`refactor/REFACTORING_PROPOSAL.md`](refactor/REFACTORING_PROPOSAL.md)、ADR 汇编 [`refactor/DECISIONS.md`](refactor/DECISIONS.md)。

---

## English

A lightweight and powerful network proxy tool for Windows, forked and continued from **Netch 1.9.7** with Xray-core integration.

### 📌 Project Origin

This project is a **continuation and enhancement** of [Netch 1.9.7](https://github.com/netchx/netch), maintaining compatibility while adding new features and optimizations.

### Features

- 🚀 **High Performance**: native Rust client shipped as a portable package with no .NET/WebView dependency
- 🔒 **Multiple Protocols**: Hysteria2, Shadowsocks, VMess, VLESS, Trojan, WireGuard, and more
- 🎯 **Process Mode**: route specific applications through sing-box TUN with `process_name` rules — no kernel driver involved
- 🌐 **TUN Mode**: system-wide proxy with custom routing rules
- 📡 **LAN Sharing**: forward traffic from other devices through a Npcap interface
- 📊 **Monitoring**: live traffic counters, latency display, bandwidth speed test, NAT type detection and egress geolocation
- 🔄 **Update Check**: the About page shows newer releases with the published package name and SHA256
- 🛡 **Credential Safety**: node credentials are DPAPI-encrypted on disk; config writes are atomic with a backup copy
- 🌍 **Multi-language**: English, Chinese (Simplified & Traditional), and Japanese

### System Requirements

- Windows x64. Driver and proxy-core compatibility must be checked on the target system.
- No .NET runtime required: the client ships as a self-contained portable package (`NPGA-Rust.zip`).
- Release binaries are currently unsigned; Windows SmartScreen or antivirus software may show a warning. Verify the published SHA256 before running.

### Installation

1. Download the latest release from [Releases](https://github.com/OffroadOps/Hatch/releases)
2. Extract all files to a folder
3. Run `NPGA-Rust.exe`

### Quick Start

1. **Add Server**: Click the server dropdown → Add server → Select protocol
2. **Select Mode**: Choose a mode from the mode dropdown (e.g., "Bypass LAN")
3. **Start**: Click the "Start" button to connect

### Configuration

#### Server Configuration
Servers are stored in `data/rust/settings.json` next to the executable. You can:
- Import from clipboard (subscription links supported)
- Add manually through the UI
- Test latency with one click

#### Mode Configuration
Modes define routing rules and are stored in the `mode/` folder:
- **Process Mode**: route specific applications
- **TUN Mode**: system-wide routing with custom rules
- **Bypass Mode**: exclude local and China IPs

### Building from Source

#### Prerequisites
- Windows x64, PowerShell 7, Rust (pinned by `npga-rust/rust-toolchain.toml`; the declared MSRV is `rust-version` in `npga-rust/Cargo.toml`) with Visual C++ Build Tools / Windows SDK

#### Build Commands

```powershell
# Assemble the runtime bundle: pinned proxy cores + Storage assets + audited native components
.\Tools\AssembleRuntime.ps1 -RuntimeRoot .release-runtime

# Build the portable package (runs tests, clippy and fmt; outputs release-rust/)
.\build-rust.ps1 -RuntimePath .release-runtime
```

The frozen .NET/C++ build chain (`build.ps1`, `Hatch.sln`, the installer) is no longer part of this
repository; it lives in an external archive repository (see `refactor/DECISIONS.md` ADR-007) and is
not required to build or release the client.

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

- 🚀 **高性能**：原生 Rust 客户端，便携包发布，无 .NET/WebView 依赖
- 🔒 **多协议支持**：Hysteria2、Shadowsocks、VMess、VLESS、Trojan、WireGuard 等
- 🎯 **进程模式**：sing-box TUN + `process_name` 规则为特定应用程序分流，不依赖内核驱动
- 🌐 **TUN 模式**：系统级代理，支持自定义路由规则
- 📡 **网络共享**：通过 Npcap 网卡转发其他设备的流量
- 📊 **实时监控**：流量统计、延迟显示、网络测速、NAT 类型检测与出口归属地
- 🔄 **更新检查**：关于页展示新版本，附发布包名与 SHA256
- 🛡 **凭据安全**：节点凭据经 DPAPI 加密落盘；配置原子写入并留有备份
- 🌍 **多语言**：英语、简体中文、繁体中文和日语

### 系统要求

- Windows x64；请在目标系统验证驱动及代理核心的兼容性。
- 不需要 .NET 运行时：客户端以自包含便携包（`NPGA-Rust.zip`）发布。
- 当前发布二进制尚未签名；Windows SmartScreen 或杀毒软件可能提示风险。运行前请核对 Release 中公布的 SHA256。

### 安装说明

1. 从 [Releases](https://github.com/OffroadOps/Hatch/releases) 下载最新版本
2. 解压所有文件到一个文件夹
3. 运行 `NPGA-Rust.exe`

### 快速开始

1. **添加服务器**：点击服务器下拉菜单 → 添加服务器 → 选择协议
2. **选择模式**：从模式下拉菜单选择一个模式（例如 "Bypass LAN"）
3. **启动**：点击"启动"按钮连接

### 配置说明

#### 服务器配置
服务器存储在可执行文件旁的 `data/rust/settings.json`。你可以：
- 从剪贴板导入（支持订阅链接）
- 通过 UI 手动添加
- 一键测试延迟

#### 模式配置
模式定义路由规则，存储在 `mode/` 文件夹中：
- **进程模式**：为特定应用程序设置路由
- **TUN 模式**：系统级路由，支持自定义规则
- **绕过模式**：排除本地和中国 IP

### 从源码构建

#### 前置要求
- Windows x64、PowerShell 7、Rust（版本由 `npga-rust/rust-toolchain.toml` 固定；声明的 MSRV 见 `npga-rust/Cargo.toml` 的 `rust-version`），以及 Visual C++ Build Tools / Windows SDK

#### 构建命令

```powershell
# 装配运行时：固定版本核心 + Storage 资产 + 审计过的原生组件
.\Tools\AssembleRuntime.ps1 -RuntimeRoot .release-runtime

# 构建便携包（自动运行测试、clippy 和 fmt；输出到 release-rust/）
.\build-rust.ps1 -RuntimePath .release-runtime
```

原 .NET/C++ 构建链（`build.ps1`、`Hatch.sln`、安装器）已不属于本仓库，位于独立归档仓库
（见 `refactor/DECISIONS.md` ADR-007），构建与发布客户端不再需要它。

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

Rust 客户端位于 [`npga-rust/`](npga-rust/README.md)，采用 ToDesk 风格的蓝白侧栏界面，兼容旧节点配置，内置固定版本的 Xray 与 sing-box。运行时装配与构建见上文；已完成能力、验证记录与兼容性边界见 [`npga-rust/MIGRATION.md`](npga-rust/MIGRATION.md)。
