# 本目录已冻结（Archived / Frozen）

**状态**：自 2026-10-14 起冻结，版本停在 2.0.x。

**决策**：见 [`../refactor/DECISIONS.md`](../refactor/DECISIONS.md) ADR-001 —
NPGA 的重构主线是 [`../npga-rust/`](../npga-rust/)，本目录只作为迁移参考保留。

## 允许的改动

- 安全漏洞修复（凭据处理、远程代码执行、越权类问题）
- 阻塞构建或发布的修复

## 不再接受的改动

- 新功能、新协议、新 UI、新语言、性能优化
- 依赖升级（除非安全修复必须）
- 文档与品牌同步（新文档一律写在 `npga-rust/` 与 `refactor/`）

## 为什么

两套全功能客户端（本目录 .NET 10 WinForms + `npga-rust` Rust）并存，
功能以本目录为全集、Rust 版为子集，导致 bug 报告、测试与发布成本分裂。
Rust 版的 runtime/ui 分层是后续维护与迁移（Tauri 2.x）的正确基础。

## 迁移映射

| 能力 | C# 实现 | Rust 对应 |
| --- | --- | --- |
| 界面 | `Forms/MainForm.cs` 等 | `npga-rust/src/ui.rs` |
| 连接编排 | `Controllers/MainController.cs` | `npga-rust/src/runtime.rs` |
| 核心进程 | `Controllers/Guard.cs` | `runtime.rs` 的 `ManagedChild` |
| 配置生成 | `Servers/V2ray`、`Servers/SingBox` | `npga-rust/src/core_config.rs` |
| 进程模式 | `Controllers/NFController.cs` + Redirector | 见 ADR-002（TUN 进程路由验证中） |
| 订阅/分享 | `Utils/SubscriptionUtil.cs`、`ShareLink.cs` | `npga-rust/src/subscription.rs`、`links.rs` |
| 延迟/带宽 | `Utils/DelayTestHelper.cs`、`Bandwidth.cs` | `runtime.rs::latency`、`stats.rs` |
| NAT/STUN | `Utils/Socks5ServerTestUtils.cs` | `npga-rust/src/nat.rs` |
| IP 归属地 | MaxMind GeoLite2 查询 | `npga-rust/src/geoip.rs` |
| 更新检查 | `Controllers/UpdateChecker.cs` | `npga-rust/src/update.rs` |

## 已知未迁移项

- 多语言界面（Rust 版目前为简体中文）
- Npcap 网络共享模式的完整参数界面（运行时已支持，UI 简化）
- 旧的 `Hatch://` 分享链接导入（Rust 版支持 `NPGA://` 与主流协议链接）
