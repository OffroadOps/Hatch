# NPGA Changelog

All notable changes to NPGA will be documented in this file.

## [3.0.0] - 2026-10-20

Rust 原生客户端成为唯一主线，2.0.x 的 .NET/WinForms 客户端冻结并移出仓库（ADR-001 / ADR-007）。
本版同时修复了若干会导致进程终止、配置丢失或功能完全不可用的问题。

### Fixed (npga-rust)
- **节点/订阅编辑器越界写入（会终止进程）**：编辑器只记录下标，节点被删除或订阅被刷新后仍按旧下标写回，
  越界即 panic；release 的 `panic = "abort"` 会直接终止进程并跳过 `Drop`（系统代理不恢复、核心进程不被回收）。
  编辑器现同时记录节点 identity / 订阅 id，写入前校验目标未变并拒绝陈旧写入。
- **release 改回 `panic = "unwind"`**：`Session`/`ManagedChild`/`Worker` 的 `Drop` 负责恢复系统代理、
  终止受管进程树与清理临时配置，abort 会让这些清理全部落空。
- **Xray 统计接口配置修正**：Xray 26.x 不再支持 `dokodemo-door` 的 `address` 写法，旧配置会让核心吞掉
  gRPC 前导，所有统计查询必然失败；改用核心自带的 `api.listen` 监听（无需额外入站与路由规则）。
- **Xray 流量统计不再每轮循环拉起子进程**：原先每 250 ms 就会 `spawn` 一次 `xray api statsquery`。
  现在直接以 h2c + gRPC 查询核心 `StatsService`，采样节流到每秒一次，CLI 仅作一次性兜底。
- **订阅刷新不再覆盖并发修改**：刷新结果原先整体替换配置，下载期间的用户改动会丢失；现在只合并
  该订阅自己的节点。
- **自动选路与界面显示同步**：自动选路/切换节点后界面仍显示手动选择的节点，现跟随运行时同步。
- **路由模式与加速方式不匹配**：切换加速方式后残留的旧模式文件会被提交并导致连接失败；
  现在自动选择新方式下的模式并校验 `RouteKind`。
- **NAT 类型探测 panic**：`transaction_id()` 把 `u128` 的 16 字节写进 8 字节切片，每次调用都 panic，
  NAT 探测必然失败；同时修正 `CHANGED-ADDRESS` 被误当作异或地址解码的问题。
- **后台任务 panic 不再卡死界面**：订阅刷新与诊断工具线程现在捕获 panic 并回报错误。
- **配置写入移出 UI 线程**：原先每次改动都在渲染过程中同步做 DPAPI 加密与原子写盘，节点多时明显掉帧；
  现在由后台线程按序写入。
- **日志面板虚拟化**：1500 行日志原先每帧渲染 1500 个控件，现在只绘制可见行。

### Changed (npga-rust)
- 测试覆盖扩大：新增 `stats.rs`（protobuf/gRPC 编解码、API 配置）、`nat.rs`（STUN 报文与属性解析）、
  `speedtest.rs`（测速采样判定）单元测试；GeoIP 用例改为 `#[ignore]` + `NPGA_RUNTIME_ROOT` 约定，
  不再在缺少数据库时静默通过。`cargo test --include-ignored` 现为 35 单元 + 18 迁移 + 7 真实核心集成。
- 移除仅供已归档 C# 客户端使用的死资产：`Storage/tun2socks.bin`、`Storage/aiodns.conf`、
  `Storage/i18n/*`、`Storage/stun.txt`、`hatch_logo_no_background.png`（约 8.4 MB），
  并重写 `Storage/README.md` 说明当前内容。
- **UI 层拆分**：`ui.rs` 拆为 `ui.rs`（App 与页面）、`ui/theme.rs`、`ui/dialogs.rs`、`ui/node_form.rs`、
  `ui/tray.rs`，行为不变。
- 可执行文件由 5.8 MB 增至 7.9 MB：`panic = "unwind"` 需要展开表（保证 panic 时仍执行清理），
  HTTP/2 客户端为 gRPC 统计查询所需。整包体积不变（核心二进制占绝大部分）。

### Changed
- **架构收敛（ADR-001）**：Rust 客户端 `npga-rust/` 成为唯一主线；.NET/WinForms 客户端 `Hatch/` 冻结于 2.0.x，
  仅接受安全修复。冻结说明见 `Hatch/FROZEN.md`（随归档移出，现位于 `D:\netch.plus-archive`），决策记录见 `refactor/DECISIONS.md`。
- 重构工作区 `refactor/` 建立：计划、决策记录（ADR）、sing-box TUN 进程路由验证资产。
- **进程模式默认后端切换为 sing-box TUN**（ADR-002 标记 Accepted）：`refactor/verify/tun-process/` 管理员环境实测
  11/11 全过（进程匹配 Handle 走 proxy / Bypass 直连、DNS 劫持、停止后网卡与路由清理）；`ProcessEngine` 默认值
  由 `redirector` 改为 `tun`，旧 Redirector/netfilter2 随 Phase 2 移除（见 Removed）。
- 重构评估报告 `REFACTORING_PROPOSAL.md`：现状剖析、雷神加速器架构实证对比、八条核心意见、四阶段路线图。

### Added (npga-rust)
- **进程模式 TUN 后端（ADR-006，2026-10-17 起为唯一后端）**：用 sing-box `process_name` 路由替代
  Redirector + netfilter2 驱动；DNS 劫持按进程作用域生效，
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
- `npga-rust/install-driver.ps1` 标注为遗留组件，并于 Phase 2 随驱动一并移除（见 Removed）。
- **归档（ADR-007，2026-10-17）**：Hatch（.NET/WinForms）完整构建链——`Hatch/`、`Tests/`、`Redirector/`、
  `RouteHelper/`、`Other/`（含 aiodns）、`RedirectorTester/`、`Tools/HatchIpProbe/`、`installer/`、
  `Hatch.sln`、`build.ps1`、`common.props`、`global.json`、`NuGet.Config`——整体移出主仓库，
  归档到独立目录 `D:\netch.plus-archive`（独立 Git 仓库）。主仓库只保留 Rust 主线与共享资产
  （`Storage/`、`build.dependencies.json`）。
- 运行时改为装配式供应链：新增 `Tools/AssembleRuntime.ps1`（固定版本核心 + Storage 资产，SHA256 校验）；
  `build-rust.ps1` 不再需要 C# 构建产物；`runtime-native/`（`Redirector.bin`、`nfapi.dll`）于 Phase 2 移除（见 Removed）。
- CI/发布：删除 C# 作业；`build.yml` 新增打包冒烟作业；`release.yml` 只发布 `NPGA-Rust.zip`。
- TUN 验证资产修复（`refactor/verify/tun-process/verify.ps1`）：stderr 日志捕获、Handle 用例 `--resolve` 去 DNS 依赖、
  bypass 非空真断言、版本无关的 hijack 断言；`verify.ps1` 等 3 个含中文脚本补 UTF-8 BOM，PowerShell 5.1 下可解析。
  修复后管理员环境实测 11/11（细节见 `refactor/verify/tun-process/README.md` 修复记录）。

### Fixed (npga-rust)
- 新增统计端口（SOCKS 端口 +3/+4）在启动前预占校验，冲突时给出明确错误。
- `core_integration` 自动选路用例补齐 SOCKS5 握手；此前直接写入载荷，xray 以非法 SOCKS 版本丢弃连接，
  用例必然超时失败，导致 `cargo test --include-ignored` 门禁无法通过。
- `tests/core_integration.rs` 恢复 `cargo fmt --check` 通过，CI 格式门禁不再失败。
- 便携包补齐 `GeoLite2-Country.mmdb`：此前打包白名单未包含该文件，发布包内 IP 归属地不可用。

### Removed (npga-rust)
- **Redirector/netfilter2 遗留组件退役（Phase 2，ADR-002）**：删除 `runtime.rs` 的 Redirector 动态 FFI、驱动安装路径与
  启动器 `--install-driver` 入口；进程模式统一为 sing-box TUN，`ProcessEngine` 设置项不再读取，高级设置中的回退开关移除。
- 删除 `npga-rust/install-driver.ps1`、`runtime-native/`（`Redirector.bin`、`nfapi.dll`、校验表）与 `Storage/nfdriver.sys`；
  `Tools/AssembleRuntime.ps1` 与 `build-rust.ps1` 的必需组件与打包白名单同步移除遗留项，发布包不再包含内核驱动。

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
