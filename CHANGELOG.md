# NPGA Changelog

All notable changes to Hatch will be documented in this file.

## [Unreleased]

### Changed
- **架构收敛（ADR-001）**：Rust 客户端 `npga-rust/` 成为唯一主线；.NET/WinForms 客户端 `Hatch/` 冻结于 2.0.x，
  仅接受安全修复。冻结说明见 `Hatch/FROZEN.md`，决策记录见 `refactor/DECISIONS.md`。
- 重构工作区 `refactor/` 建立：计划、决策记录（ADR）、sing-box TUN 进程路由验证资产。
- 重构评估报告 `REFACTORING_PROPOSAL.md`：现状剖析、雷神加速器架构实证对比、八条核心意见、四阶段路线图。

### Added (npga-rust)
- **进程模式 TUN 后端（实验，ADR-006）**：新增 `ProcessEngine` 设置项（`redirector` 默认 / `tun`），
  TUN 后端用 sing-box `process_name` 路由替代 Redirector + netfilter2 驱动；DNS 劫持按进程作用域生效，
  bypass 规则优先于 handle 规则，未列出的进程保持直连。旧正则规则自动转换为精确进程名，无法转换的规则在日志中显式上报。
- **流量统计（ADR-005）**：Xray 配置附加 `api`/`stats` + dokodemo-door 管理入口，sing-box 配置附加
  `experimental.clash_api`；后台按秒轮询，五种路由模式统一显示上下行速率（此前仅进程模式可见）。
- **NAT 类型探测**：RFC 5389/3489 三测试流程经本地 SOCKS5 端口执行，输出映射/过滤行为与出口端点。
- **出口 IP 归属地**：内置纯 Rust MaxMind DB 读取器（无新增依赖），算法与参考实现对拍验证（IPv4/IPv6、指针链、24/28/32 位记录）。
- **网络测速**：经本地 HTTP 代理下载测速，输出 Mbps 与传输量。
- **检查更新**：GitHub Releases 检查 + 版本比较 + 校验和表解析（移植自 C# `UpdateChecker`）。
- `Tools/FetchPinnedCores.ps1`：按 `build.dependencies.json` 下载并 SHA256 校验核心，组装最小运行时（CI 与本地共用）。

### Changed (工程)
- CI 收敛：`build.yml` 的 Rust 作业成为默认门禁（fmt + clippy -D warnings + 全量测试 + release 构建），
  C# 作业仅在冻结树变更或手动触发时运行；`release.yml` 新增 Rust 打包作业，发布同时产出 `NPGA-Rust.zip`。
- C# 作业的触发条件改用 `dorny/paths-filter` 判定变更路径：此前的 commit 列表判断只看 `modified`，
  漏掉新增/删除文件，且 `pull_request` 事件下该条件恒为假，冻结树在合并前完全得不到构建验证。
- `npga-rust/install-driver.ps1` 标注为遗留组件（ADR-002 通过后随驱动一并退役）。

### Fixed (npga-rust)
- 新增统计端口（SOCKS 端口 +3/+4）在启动前预占校验，冲突时给出明确错误。
- `core_integration` 自动选路用例补齐 SOCKS5 握手；此前直接写入载荷，xray 以非法 SOCKS 版本丢弃连接，
  用例必然超时失败，导致 `cargo test --include-ignored` 门禁无法通过。
- `tests/core_integration.rs` 恢复 `cargo fmt --check` 通过，CI 格式门禁不再失败。

### Changed (2.0.x 维护，C# 版冻结前)
- Renamed the product to NPGA (Netch.Plus Game Accelerator), including executable, installer, UI, and release assets; retained import compatibility with Hatch share links.
- Migrated Hatch, Tests, HatchIpProbe, and RedirectorTester to .NET 10 for Windows.
- Pinned .NET SDK 10.0.401 for local builds and Windows build/release workflows.
- Converted RedirectorTester from .NET Framework 4.8 to an SDK-style project.
- Removed the redundant System.Text.Encoding.CodePages package reference; .NET 10 provides it in the shared framework.
- Migrated the log window closing callback to the current WinForms API and restricted runtime roll-forward to .NET 10 patch releases.

### Fixed (2.0.x 维护，C# 版冻结前)
- Validated SOCKS UDP headers and lengths before decoding and used overlap-safe payload copying.
- Handled fragmented TCP handshakes and partial writes; joined the UDP control thread before its owner closes sockets.
- Restricted build outputs to safe workspace directories and rejected reparse points before recursive cleanup.
- Migrated sing-box sniffing to route actions and checked generated configuration against the pinned core during packaging.
- Preserved existing subscription nodes when updates are empty or contain invalid endpoints.
- Fixed Hatch share-link port parsing and retained protocol-specific credentials during export.
- Serialized core log writes, drained output before disposal, and made repeated stops safe.
- Caught asynchronous connection-test failures and corrected directory boundaries before terminating core processes.
- Passed custom source/output paths to Inno Setup and detected per-user compiler installations.
- Stopped `MainController` from releasing its start/stop semaphore twice on a failed start: the duplicate
  release silently raised `AsyncSemaphore.CurrentCount` above `MaxCount`, permanently allowing two cores to
  start at once (port conflicts, orphaned child processes). The failure path now shares a lock-held stop helper.
- Verified the "update bundled cores" download against the SHA256 digest published by the GitHub releases API
  before extracting it into `bin\`; previously the computed hash was only logged and the size check was a
  tautology, so a tampered archive would have been installed and later executed.
- Matched firewall rules against the application directory case-insensitively, at a directory boundary, and with
  a null guard on rules that carry no application name; the add path could previously throw or accept
  `D:\NPGA-backup` as belonging to `D:\NPGA`.
- Required PowerShell 7 for `build.ps1` and the build-path safety helpers so an unsupported host fails with a
  clear version error instead of `[IO.Path]::GetRelativePath` being reported as missing.

## [2.0.0] - 2026-07-16

### Release hardening
- Connected the Hatch source tree to the existing Git history and automated Windows build/release workflows.
- Added a reproducible root `build.ps1` that pins external dependency versions and verifies every download with SHA256.
- Added `GeoLite2-Country.mmdb` to release packaging so IP country lookup no longer fails silently because of a missing database.
- Added real update-checker tests and a project reference from `Tests` to `Hatch`.
- Normalized legacy GB18030 C# sources to UTF-8 BOM and repaired the update checksum marker and copyright metadata.
- Made the updater accept `v`-prefixed GitHub tags, reject empty release lists cleanly, and parse only valid SHA256 table rows.
- Moved the Inno Setup definition into the source tree and consolidated the three legacy publish scripts into one build entry point.
- Corrected legacy Netch branding and obsolete dependency guidance in the Simplified Chinese, Traditional Chinese, and Japanese translations.
- Documented that current binaries are unsigned and may trigger SmartScreen or antivirus warnings.

### Added
- **About Dialog**: New "About" menu item in the main menu bar
  - Display Hatch version (v2.0.0)
  - Display Xray-core version with auto-detection
  - Display sing-box version with auto-detection
  - Show installation status for each core (green for installed, red for missing)

- **Update Features**:
  - Check for Hatch software updates from GitHub
  - One-click update for Xray-core
  - One-click update for sing-box
  - Automatic version detection after updates

- **Project Attribution**:
  - Tribute to Netch project: "🥚 Hatched from Netch - Without Netch, no Hatch"
  - Links to GitHub repositories (Hatch and Netch)

- **Core Management**:
  - Version-pinned core dependency manifest (`build.dependencies.json`)
  - SHA256 verification for build-time downloads
  - Version constant in `Constants.cs`

### Changed
- Menu structure: Added "About" before "Exit"
- Enhanced core file management

### Fixed
- sing-box.exe placement in correct directories
- Core version detection and display

### Technical Details
- New files:
  - `Hatch/Forms/AboutForm.cs`
  - `Hatch/Forms/AboutForm.Designer.cs`
  - `build.dependencies.json`
- Modified files:
  - `Hatch/Forms/MainForm.cs`
  - `Hatch/Forms/MainForm.Designer.cs`
  - `Hatch/Constants.cs`

---

## [1.9.7] - Base Version

### Initial Features
- Forked from Netch 1.9.7
- Support for multiple proxy protocols
- Process mode and TUN/TAP mode
- Real-time bandwidth monitoring
- Multi-language support

---

**Note**: This project is a continuation of Netch 1.9.7 with enhancements and new features.

**Repository**: https://github.com/OffroadOps/Hatch
**Original Project**: https://github.com/netchx/netch
