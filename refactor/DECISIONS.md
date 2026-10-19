# NPGA 架构决策记录（ADR）

新决策追加在文件末尾；已生效的条目不修改，只可标记 Superseded。

---

## ADR-001：C# 版 Hatch 冻结，npga-rust 成为唯一主线

- **日期**：2026-10-14
- **状态**：Accepted
- **背景**：Hatch（.NET 10 WinForms，MainForm.cs 1689 行 + Global 静态类）与 npga-rust（Rust/egui，ui 与 runtime 分离）是两套全功能客户端。功能以 C# 为全集、Rust 为子集，bug 报告与测试成本分裂到两个代码库。PROJECT_REVIEW.md 与 MIGRATION.md 均记录了双轨的维护负担。
- **决策**：Hatch 冻结在 2.0.x，只接受安全问题修复；所有新功能、新 UI 只进入 npga-rust。Hatch 保留在仓库中供迁移参考，不删除。
- **理由**：Rust 版的 runtime/ui 分层是测试与后续 Tauri 迁移的正确基础；WinForms 已停止功能演进（仅安全维护）。
- **后果**：Rust 版未迁移的功能（流量监控、自动更新、NAT/STUN、IP 归属地、测速）必须补齐后才能关闭冻结；C# 特有 bug 不再修复，需在文档中说明。

## ADR-002：用 sing-box TUN 进程路由替代 Redirector + netfilter2.sys

- **日期**：2026-10-14
- **状态**：Accepted（2026-10-17 管理员环境实测通过，PASS=11/11）
- **背景**：进程模式当前依赖 netfilter2.sys（未签名内核驱动）+ Redirector.bin（自研 C++ SOCKS 转发，PROJECT_REVIEW.md 的 P1/P2 bug 集中地）。sing-box 1.13 的 route 规则原生支持 `process_name` / `process_path`。
- **决策**：进程模式改为 sing-box TUN + process_name/process_path 规则；删除 netfilter2 安装逻辑与 Redirector 组件；DNS 处理移交 sing-box DNS hijack。
- **验证门槛**：`refactor/verify/tun-process/` 全项通过后才执行删除。老系统（TUN 不可用）保留 WinDivert 兜底方案的评估，不保留 netfilter2。
- **验证记录**：2026-10-17，管理员环境运行 `refactor/verify/tun-process/verify.ps1`（sing-box 1.13.14）：11/11 全过——进程匹配（Handle 走 proxy / Bypass 直连）、DNS 劫持、停止后网卡与路由清理。默认 `ProcessEngine` 已切为 `tun`；删除动作（`runtime-native/`、`Storage/nfdriver.sys`、`install-driver.ps1`、`runtime.rs` 驱动路径与 `--install-driver`）同日完成（Phase 2，2026-10-17）。
- **理由**：消除未签名驱动的杀软误报与系统更新风险；消除自研 C++ 安全债；DNS/规则表达力是超集。
- **后果**：Redirector 特有开关（dnsOnly/dnsProx/dnsHost）需重新设计 UI 暴露方式；进程匹配依赖连接归属查询，启动器套娃场景需实测。

## ADR-003：数据层从 settings.json 迁移到 SQLite

- **日期**：2026-10-14
- **状态**：Proposed
- **背景**：节点/订阅/模式/凭据全量 JSON 读写，节点上百后性能与管理困难。
- **决策**：引入 SQLite（rusqlite），schema 版本化 + 迁移函数；凭据列继续 DPAPI；导入导出保持 settings.json 兼容。
- **理由**：事务性（订阅替换语义已在 Rust 版实现，需要持久层事务支持）、查询能力、崩溃安全。
- **后果**：新增 rusqlite 依赖（捆绑编译，无系统依赖）；一次性迁移代码。

## ADR-004：UI 框架从 egui 迁移 Tauri 2.x

- **日期**：2026-10-14
- **状态**：Proposed（产品岔路，需拍板）
- **背景**：要接近雷神/UU 级产品观感，egui 样式天花板过低；Tauri 2.x 用系统 WebView2，UI 生产力接近 Electron 而体积接近原生，且支持未来移动端。
- **决策（待确认）**：前端 React/Solid + 现有 Rust runtime 作为 command 后端，重写 ui.rs。
- **备选**：保留 egui 并做到 ToDesk 级打磨，把精力投入 Diagnostics/Selector（体验收益更大的两项）。
- **后果**：引入 Web 前端技术栈；包体增加 WebView2 运行时依赖（Win10/11 普遍已具备）。

## ADR-005：流量统计统一走核心 stats API

- **日期**：2026-10-14
- **状态**：Accepted
- **背景**：流量显示此前仅进程模式有（Redirector 计数器），其他模式为「—」。
- **决策**：sing-box 配置附加 `experimental.clash_api`，Xray 配置附加 `api`/`stats` + dokodemo-door 管理入口；runtime 周期性查询并广播事件，UI 三种模式一致显示。
- **理由**：复用核心自带能力，不引入旁路监听/抓包。
- **后果**：本地额外监听两个端口（随 SOCKS 端口派生）；需在会话停止时释放（随子进程生命周期）。

## ADR-006：进程模式 TUN 后端落地 + 运行时装配与 C# 构建解耦

- **日期**：2026-10-15
- **状态**：Accepted（2026-10-17 起 `tun` 为唯一后端；`ProcessEngine` 回退开关随 Phase 2 移除，见 ADR-002 验证记录）
- **背景**：ADR-002 的验证资产已就绪，但客户端侧还没有可切换的实现；同时 CI 与运行时包仍强依赖冻结的 C# 构建。
- **决策**：
  1. 在 `core_config::tun_process()` 实现进程模式的 sing-box TUN 配置：`process_name` 路由 + 仅对受管进程生效的 DNS 劫持（`{"protocol":"dns","process_name":[...],"action":"hijack-dns"}`），bypass 规则优先于 handle 规则；`final: direct` 保证只有列表内进程被代理。
  2. 旧模式规则（C++ 正则）通过 `process_names()` 转换为精确进程名，**无法转换的规则显式上报**（sing-box 只支持精确匹配），全部不可转换时直接报错而不是静默降级。
  3. 通过设置项 `ProcessEngine`（2026-10-17 起 `tun` 默认 / `redirector` 可回退）切换，UI 在「高级设置」暴露复选框；ADR-002 端到端验证已通过（11/11），默认值已切换。
  4. 新增 `Tools/FetchPinnedCores.ps1`：按 `build.dependencies.json` 下载并 SHA256 校验核心，组装最小运行时；CI 与本地共用，使 Rust 侧的测试与校验不再需要先构建 C#。
- **理由**：把「验证」与「实现」解耦但保持可回退；先具备能力再切换默认值，避免把未验证的路径强加给用户。运行时装配脚本是 Phase 2 彻底去掉 C# 构建依赖的前提（`build-rust.ps1` 对 C# 包产物的依赖已随 ADR-007 与 Phase 2 消除）。
- **后果**：
  - 进程模式的 DNS 语义变化：TUN 后端下 DNS 劫持按进程作用域生效，不再有 Redirector 的 dnsOnly/dnsProx 细粒度开关。
  - 正则型进程规则在 TUN 后端下不可用，UI 会提示无法转换的规则数量。
  - CI 结构：`build.yml` 的 Rust 作业为主门禁，C# 作业仅在 `Hatch/**` 变更或手动触发时运行；`release.yml` 增加 Rust 打包作业，发布同时产出 `NPGA-Rust.zip`。

## ADR-007：Hatch 完整构建链移出主仓库，运行时改为装配式供应链

- **日期**：2026-10-17
- **状态**：Accepted
- **背景**：ADR-001 冻结后 C# 源码仍留在仓库中，CI 与发布仍依赖 `build.ps1` 用 .NET/C++/Go 全链构建运行时包；Rust 打包（`build-rust.ps1`）需要该产物提供 `Redirector.bin`、`nfapi.dll`、`nfdriver.sys` 与 `mode/`。
- **决策**：
  1. 将 Hatch、Tests、Redirector、RouteHelper、Other（含 aiodns）、RedirectorTester、Tools/HatchIpProbe、installer、`Hatch.sln`、`build.ps1`、`common.props`、`global.json`、`NuGet.Config`、`Tools/BuildPathSafety.ps1`、`Tools/SetToolchainEnvironment.ps1` 及图标资产整体移出主仓库，归档到独立目录 `D:\netch.plus-archive`（独立 Git 仓库，导入提交记录归档前主仓库快照 `a934f42`）。
  2. 主仓库只保留 Rust 主线与共享资产（`Storage/`、`build.dependencies.json`、`Tools/FetchPinnedCores.ps1`）。
  3. 运行时改为装配式：新增 `Tools/AssembleRuntime.ps1` = FetchPinnedCores（固定版本核心）+ `Storage/`（nfdriver.sys、mode）+ `runtime-native/`（审计过的 `Redirector.bin`、`nfapi.dll`，SHA256 校验）。CI 与本地共用。
  4. CI/发布：删除 C# 作业；`build.yml` 增加装配 + 打包冒烟作业；`release.yml` 只发布 `NPGA-Rust.zip`。
- **理由**：主仓库成为单一（Rust）产品树；发布不再需要 .NET/C++/Go 工具链；消除 Phase 1.4 遗留的 `build-rust.ps1` 对 C# 构建产物的依赖。
- **后果**：
  - `Redirector.bin`/`nfapi.dll` 以二进制形式入库（`runtime-native/`，含 SHA256SUMS.txt）；更新需在归档目录重建后复制回来并更新校验表（流程见其 README）。已于 2026-10-17（Phase 2）随 Redirector 一并移除。
  - 旧 `NPGA.zip`/`NPGASetup.exe` 安装器不再随新版本发布；历史版本资产仍保留在 GitHub Releases。
  - C# 测试与原生 SOCKS 测试随 `Tests/` 归档，不再在 CI 运行；如需回归验证须在归档目录执行。
  - 主仓库 Git 历史保留归档前的全部内容（`a934f42` 及之前），恢复可通过 revert 或从归档目录取回。
## ADR-008：Xray 统计接口改用核心自带 API 监听，查询改为直接 gRPC

- **日期**：2026-10-19
- **状态**：Accepted
- **背景**：ADR-005 定下「流量统计统一走核心 stats API」，但当时为 Xray 选定的承载方式是「`api`/`stats` + dokodemo-door 管理入口 + 路由规则」，查询方式是 `xray api statsquery` CLI 子进程。两条实现都存在问题：
  1. **接口配置在当前核心上根本不工作**。固定的 Xray 26.3.27 已把 dokodemo-door 的 `address` 改名为 `rewriteAddress`，旧写法被忽略，入站退化为动态端口模式并吞掉 gRPC 前导，导致每一次统计查询都失败。核心日志显示请求确实被路由到 `[api]` 出口，但握手始终无法完成。官方文档同时提供了自 v1.8.12 起的简化写法：`api.listen` 由核心自行开启 API 监听，无需额外入站与路由规则。
  2. **查询方式的成本与可靠性都不合格**。工作线程每 250 ms 循环一次，Xray 后端每次都 `spawn` 一个 `xray api statsquery` 进程（约 4 次/秒），既浪费 CPU 又与主循环争抢时间。
- **决策**：
  1. Xray 侧改用核心自带的 API 监听：配置写入 `"api": {"tag": "api", "listen": "<host>:<port>", "services": ["StatsService"]}`，不再生成 dokodemo-door 入站，也不再插入路由规则；`api_ports` 仍按 SOCKS 端口派生（+3），并在会话启动前预占校验。
  2. 查询改为客户端直接以 h2c + gRPC 调用核心的 `StatsService/QueryStats`：请求/响应用手写 protobuf 编解码，不引入 gRPC 代码生成与 tonic 依赖。服务名按 `xray.app.stats.command.StatsService` 优先、`v2ray.core.app.stats.command.StatsService` 兜底，两者都探测并记住可用者。
  3. `xray api statsquery` CLI 仅作为 gRPC 不可用时的一次性兜底；两者都失败则本次会话不再查询，界面显示「—」，**不得**退化为每轮循环拉起子进程。
  4. 采样节流到每秒一次（`TRAFFIC_INTERVAL`），与工作线程的 250 ms 唤醒解耦。
- **理由**：接口承载方式必须与固定的核心版本匹配，否则功能等于不存在；查询方式属于高频路径，不应每轮拉起进程。手写 protobuf 约 150 行、依赖面为零，比引入 tonic 及其传递依赖更契合本项目的依赖纪律。
- **后果**：
  - 可执行文件因新增 HTTP/2 客户端（reqwest 的 `http2` 特性）增大约 1 MB。
  - 统计接口只监听回环端口。本机若装有 AdGuard 等 HTTP 过滤软件，回环 h2c 请求会被替换成拦截页，统计不可用（旧的 CLI 方案同样被拦截，非本次引入的退化）；集成测试用同栈控制探针识别该情况并跳过，不误判为缺陷。
  - `api.listen` 简化写法要求核心 ≥ v1.8.12；固定核心 26.3.27 满足，ADR-007 的固定版本机制保证不会出现版本漂移。

## ADR-009：release 配置使用 panic = "unwind" 以保证异常时执行资源清理

- **日期**：2026-10-19
- **状态**：Accepted
- **背景**：release 配置此前使用 `panic = "abort"`，目的是压缩体积。但运行时的资源回收全部依赖 `Drop`：`Session::drop` 恢复 Windows 系统代理、`ManagedChild::drop` 终止受管核心进程树并回收日志线程、`TemporaryConfigs::drop` 清理临时核心配置、`Worker::drop` 通知后台线程退出。`abort` 会跳过所有析构函数，任何 panic 都会留下「系统代理仍指向本机失效端口」与「孤儿 xray/sing-box 进程」。
- **决策**：release 改用 `panic = "unwind"`（即 Rust 默认值），使 panic 时析构链正常执行。同时：
  1. 编辑器等 UI 路径不得再触发越界 panic（编辑器按 identity 校验目标，见 MIGRATION.md 同日记录）；
  2. 后台线程（订阅刷新、诊断工具）用 `catch_unwind` 包裹，确保线程异常终止时仍回报事件，避免界面永久停留在忙碌状态。
- **理由**：崩溃时「进程没了但系统代理还开着」比多 2 MB 严重得多。可执行文件由 5.8 MB 增至 7.9 MB，整包体积不变（核心二进制占绝大部分）。
- **后果**：
  - 体积增加约 2 MB（展开表）。
  - 依赖 `Drop` 做清理的假设现在成立；但仍需保证 `Drop` 中的清理本身不 panic。
