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
- **状态**：Proposed（待 Phase 1 验证）
- **背景**：进程模式当前依赖 netfilter2.sys（未签名内核驱动）+ Redirector.bin（自研 C++ SOCKS 转发，PROJECT_REVIEW.md 的 P1/P2 bug 集中地）。sing-box 1.13 的 route 规则原生支持 `process_name` / `process_path`。
- **决策**（待验证）：进程模式改为 sing-box TUN + process_name/process_path 规则；删除 netfilter2 安装逻辑与 Redirector 组件；DNS 处理移交 sing-box DNS hijack。
- **验证门槛**：`refactor/verify/tun-process/` 全项通过后才执行删除。老系统（TUN 不可用）保留 WinDivert 兜底方案的评估，不保留 netfilter2。
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
- **状态**：Accepted（默认关闭，验证通过后切换默认值）
- **背景**：ADR-002 的验证资产已就绪，但客户端侧还没有可切换的实现；同时 CI 与运行时包仍强依赖冻结的 C# 构建。
- **决策**：
  1. 在 `core_config::tun_process()` 实现进程模式的 sing-box TUN 配置：`process_name` 路由 + 仅对受管进程生效的 DNS 劫持（`{"protocol":"dns","process_name":[...],"action":"hijack-dns"}`），bypass 规则优先于 handle 规则；`final: direct` 保证只有列表内进程被代理。
  2. 旧模式规则（C++ 正则）通过 `process_names()` 转换为精确进程名，**无法转换的规则显式上报**（sing-box 只支持精确匹配），全部不可转换时直接报错而不是静默降级。
  3. 通过设置项 `ProcessEngine`（`redirector` 默认 / `tun`）切换，UI 在「高级设置」暴露复选框；ADR-002 端到端验证通过后把默认值改为 `tun`。
  4. 新增 `Tools/FetchPinnedCores.ps1`：按 `build.dependencies.json` 下载并 SHA256 校验核心，组装最小运行时；CI 与本地共用，使 Rust 侧的测试与校验不再需要先构建 C#。
- **理由**：把「验证」与「实现」解耦但保持可回退；先具备能力再切换默认值，避免把未验证的路径强加给用户。运行时装配脚本是 Phase 2 彻底去掉 C# 构建依赖的前提（当前 `build-rust.ps1` 仍需 C# 包提供 Redirector.bin/nfapi.dll/nfdriver.sys）。
- **后果**：
  - 进程模式的 DNS 语义变化：TUN 后端下 DNS 劫持按进程作用域生效，不再有 Redirector 的 dnsOnly/dnsProx 细粒度开关。
  - 正则型进程规则在 TUN 后端下不可用，UI 会提示无法转换的规则数量。
  - CI 结构：`build.yml` 的 Rust 作业为主门禁，C# 作业仅在 `Hatch/**` 变更或手动触发时运行；`release.yml` 增加 Rust 打包作业，发布同时产出 `NPGA-Rust.zip`。
