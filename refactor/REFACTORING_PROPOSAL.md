# NPGA 重构评估与建议

> 评估日期：2026-10-14。评估范围：D:/netch.plus 全量源码、构建链、两代客户端（C# / Rust），以及本机安装的雷神加速器 11.3.3.0（Electron + leishenSdk 组件栈）。
> 定位：本文是重构决策输入，不是实施手册。每个判断都给出代码位置或安装目录证据。
>
> **2026-10-17 注**：C# 完整构建链已随 ADR-007 移出主仓库至独立归档 `D:\netch.plus-archive`；
> 本文中关于原构建链与 C# 客户端的现状描述为归档前历史，路线图以外的主线决策以 `refactor/DECISIONS.md` 为准。

---

## 一、总体评价

NPGA（Netch.Plus Game Accelerator）当前处于「**可用但不勾整**」的中间态：

- 业务上它是合格的：协议覆盖全（Hysteria2/SS/VMess/VLESS/Trojan/WireGuard）、双核心（Xray 26.3.27 + sing-box 1.13.14）已固定并做 SHA256 校验、PROJECT_REVIEW.md 记录的 P1/P2 级 bug 已基本修复（UDP 越界、订阅误删、日志并发等）。
- 架构上它背着三层历史包袱：**WinForms 时代的 UI 耦合**（MainForm.cs 1689 行 + Global 静态类 33 处直接引用）、**Netch 1.9.7 时代的进程拦截栈**（netfilter2.sys 内核驱动 + 自研 Redirector SOCKS 转发）、**两代客户端并存**（Hatch/ C# 与 npga-rust/ Rust 各有一套完整实现）。
- 与雷神这类商业产品的差距，**不在功能列表，而在三个根基层面**：代码签名与信任链、链路层优化能力（自建/专线节点 vs 依赖机场节点）、工程化的诊断与运维体系。这三项都不是功能堆叠能追上的，是架构决策问题。

结论先行：**建议以 npga-rust 为唯一主线做重构，退役 C# WinForms 客户端，用 sing-box TUN 的进程路由能力整体替换 Redirector + netfilter2.sys，UI 层从 egui 升级为 Tauri 2.x（或保留 egui 并深度打磨）**。详细论证见下文。

---

## 二、现状架构剖析

### 2.1 组件清单

| 层 | 组件 | 技术 | 代码规模 | 评估 |
| --- | --- | --- | --- | --- |
| UI | Hatch/ WinForms | .NET 10 WinForms | MainForm.cs 1689 行 + Designer 761/1246 行 | 事件直连 Global，难测试 |
| UI | npga-rust/ | egui/eframe 0.33.3 | ui.rs 2059 行，总 4732 行 | UI 与 runtime 已分离，架构正确 |
| 代理核心控制 | Guard.cs | 子进程管理 | 220 行 | 日志/生命周期已有修复，仍偏薄 |
| 进程模式 | NFController → Redirector | C++ + netfilter2.sys | SocksHelper 等 8 个 cpp | **最大技术债，见 2.3** |
| TUN | TUNController + RouteHelper | C++ + tun2socks.bin | RouteHelper.cpp | 与 sing-box TUN 能力重叠 |
| DNS | aiodns.bin | Go | 外部二进制 | 可被 sing-box DNS 替代 |
| 抓包共享 | PcapController | pcap2socks + Npcap | 118 行 | 功能窄，Npcap 依赖重 |
| 配置生成 | XrayConfigBuilder / SingBoxConfigUtils | 匿名对象 JSON | 数千行分散 | 风格不统一，sing-box 侧仅支持 Hysteria2，其余抛 NotSupportedException（SingBoxConfigUtils.cs:62） |
| 数据 | data/settings.json | JSON + DPAPI | — | 节点量大后可扩展性差 |
| 构建 | build.ps1 | .NET 10 + VS2022 C++ + Go + MinGW + Inno Setup | — | 工具链全栈，无跨平台可能 |

### 2.2 两个客户端的「同室操戈」

- Rust 版 MIGRATION.md 明确写了兼容性边界表：流量监控（仅进程模式有）、自动更新、NAT/STUN、IP 归属地、测速均未迁移。
- 与此同时 C# 版仍具备全部功能。**功能以 C# 为全集、Rust 为子集**的状态每持续一个月，用户的 bug 报告就会分裂到两个代码库，测试和维护成本翻倍。
- 这是当前第一战略问题：不是「哪个语言好」，而是「必须只保留一个」。

### 2.3 Redirector + netfilter2.sys：应当退役的遗产

PROJECT_REVIEW.md 记录的 P1/P2 bug 全部集中在这个自研 C++ SOCKS 转发器（UDP 短包越界、TCP 分段握手、socket 双重关闭、线程所有权）。根因是：**这是一个把用户态 SOCKS 客户端塞进了内核过滤路径的设计**，每一个字节的边界都要自己负责，而同样的效果 sing-box 用 Go 实现并且经过全球规模验证。

关键事实：**sing-box 1.13 的 route 规则原生支持 `process_name` / `process_path`（Windows）**，也就是说「进程模式」完全可以由 `sing-box TUN + process_name 规则`独立完成，不需要 Redirector.bin，更不需要 netfilter2.sys 这个未签名内核驱动。

退役它的收益是复合的：

1. 消除 Storage/nfdriver.sys 的安装/卸载/版本比较逻辑（NFController.cs:150-253，约 100 行驱动管理代码作废）；
2. 消除整个 Redirector C++ 工程及其全部历史 bug；
3. 消除杀毒软件对未签名 .sys 的误报（README 已承认 SmartScreen/杀软警告问题，驱动是主因之一）；
4. DNS 劫持、ICMP 处理由 sing-box TUN 的 DNS hijack + 规则完成，能力反而更强（支持 DoH/DoT 上游、fakeip）。

代价评估：进程模式的 DNS 细粒度控制（Redirector 的 dnsOnly/dnsProx/dnsHost）会从「自定义开关」变成「sing-box DNS 规则」，表达力是超集，只需要重新设计 UI 暴露方式。

### 2.4 其他具体问题

- **配置生成不统一**：XrayConfigBuilder 用强类型对象，SingBoxConfigUtils 用匿名对象，Hysteria2 之外的 sing-box outbound 全部 NotSupportedException。重构时应统一为「协议模型 → 核心配置」的单一编译管线，两个核心共用。
- **Global 静态类是测试天敌**：MainForm/Settings/JobObject 全部静态，MIGRATION.md 显示 Rust 版已修正此问题（Store + runtime 分离），这条正确道路应坚持。
- **数据层**：settings.json 全量读写，节点 + 订阅 + 模式 + 凭据混排。节点上百后性能和管理都成问题，应引入 SQLite（schema 版本化，凭据继续 DPAPI）。
- **构建链**：一次完整构建需要 .NET 10 SDK + VS2022 C++ 负载 + Go + MinGW + Inno Setup（README 前置要求节）。退役 C++/Go 组件后构建链可收缩为「cargo + dotnet(仅可选)」，这是退役 Redirector/aiodns 的额外收益。
- **流量统计来源分裂**：Rust 版只有进程模式显示 Redirector 计数。统一到 sing-box/Xray 的 stats API（clash API / v2ray stats）后三种模式一致。

---

## 三、雷神加速器架构参考（11.3.3.0，本机安装目录实证）

### 3.1 组件栈

| 层 | 组件 | 说明 |
| --- | --- | --- |
| UI | leigod.exe / leigod_launcher.exe + app.asar + renderer.asar | Electron 双 asar 分离（主进程/渲染进程），launcher 负责启动与自更新 |
| 服务 | leishenSdk.exe | 常驻 SDK 服务，与 UI 进程解耦 |
| 拦截 | NNDivert32/64/132/164.sys + NNDivert.dll | **WinDivert 定制版**（dll 内含 `WinDivertDriverInstallMutex`、`\\.\NNDivert` 设备名证据），按系统版本分发多个驱动 |
| 拦截 | nfwfp.sys + nfwfp.inf/.cat | WFP 过滤驱动，双通道兜底 |
| 拦截 | tap0901.sys / tapinstall.exe / wintun.dll / tun2socks.dll | TAP + TUN + tun2socks 三件套 |
| 代理 | GateProxyEx.dll / VpnProxy.dll / HookProxy.dll | 多层代理：网关代理/VPN 代理/API Hook 代理（L7 层补充手段） |
| 传输 | msquic.dll | **QUIC 协议栈**，自研加速跑在 QUIC 上，非 TLS over TCP |
| 辅助 | sslproxy.zip / LIBCURL.DLL / sentry.dll | TLS 代理、崩溃上报 |
| 诊断 | mtrtool.exe / net_test.exe / fixnet / route/hk.dat | 内置 MTR、网络修复、路由数据 |
| 游戏侧 | tool_xxx.exe（动态下载）/ slgtool / leigod_pubg_tool.exe | **按游戏分发加速插件** |
| 数据 | ipdatacloud_country.dat / game_path.json / proto.ini / hardward.json | IP 归属地、游戏路径库、硬件画像 |

### 3.2 五个值得借鉴的设计（按性价比排序）

1. **按游戏工具模块（tool_xxx.exe）**：每个游戏/场景一个可独立下载、独立更新的加速插件，游戏库扩容不依赖主程序发版。这对 NPGA 的场景模式（Bypass/Handle 规则集）有直接启发：mode 可以变成「按游戏下载的规则包」。
2. **launcher 与主程序分离**：自更新、诊断、修复不占用主进程。Rust 版已有 `--install-driver`、`--diagnostics`、`--screenshot`、`--wait-for-parent` 子命令雏形（main.rs），方向对，应产品化为独立 launcher.exe。
3. **拦截手段多层兜底**：Divert + WFP + TAP + TUN + Hook 并存，按系统环境自动降级。NPGA 目前只有 netfilter2 一条路，Win11 新版本上兼容性风险集中。建议 WFP（或 WinDivert 用户态）+ sing-box TUN 双通道，netfilter2 仅在老系统兜底，最终退役。
4. **内置诊断套件**：mtrtool/fixnet 是用户留存功能——加速不了的时候用户第一需要的是「为什么」。Rust 版已有 diagnostics 命令，应补齐 MTR（Rust 生态有现成实现思路，如 pariapulse/mtr-rs 或基于 raw socket 自研）、路由跟踪、NAT 类型变化的连续记录。
5. **QUIC 原生隧道 + 专线节点**：这是商业加速器真正的护城河，NPGA 作为开源客户端无法复制服务端。但客户端侧可以贴近：**默认推荐 Hysteria2/TUIC 节点（原生 QUIC），把 Brutal 拥塞控制、端口跳跃（port hopping）、多路复用做成一等公民配置项**，并做多节点同时建连、按游戏实测延迟自动选路（雷神的「AI 智能选路」本质就是实时探测 + 预测，客户端可先做确定性版本：并行探测 + 最低 RTT 胜出 + 故障自动切换）。

### 3.3 明确不要学雷神的部分

- Electron 体积与内存占用（雷神安装目录超过 500MB，runtime 常驻数百 MB）。若追求同等 UI 表现力，**Tauri 2.x（系统 WebView2 + Rust 后端）是更优解**：UI 生产力=Electron，体积≈原生程序，且 Tauri 2 的 Rust 命令通道正好接入现有 npga-rust 的 runtime 层。
- 自研内核驱动的模式：没有 EV 签名证书和商业法务能力前，.sys 驱动是负资产（见 2.3）。

---

## 四、核心意见（八条）

1. **[战略] 立即冻结 C# 版，Rust 版成为唯一主线。** C# 冻结在 2.0.x 只做安全修复；新功能、新 UI 全部进 npga-rust。两套全功能客户端并存是维护成本黑洞，且 Rust 版的 ui/runtime 分离架构本来就是对的。
2. **[网络] 退役 Redirector + netfilter2.sys + aiodns + tun2socks.bin。** 用 sing-box TUN（process_name/process_path 路由 + DNS hijack）统一进程模式/系统模式/DNS。这一步同时消灭：自研 C++ 的全部历史 bug、未签名内核驱动的杀软误报、Go/MGW 构建依赖、约 1200 行胶水代码。
3. **[UI] 从 egui 升级到 Tauri 2.x（WebView2），或保留 egui 做到 ToDesk 级打磨。** 如果目标是「雷神/UU 级别的产品观感」，egui 的样式天花板太低（MIGRATION.md 里自评 ToDesk 风格侧栏，已达 egui 合理上限）；Tauri 2.x 前端可交付更精致的 UI，且后端 runtime.rs/platform.rs/config.rs 可整体复用，迁移增量主要在 ui.rs 重写。Windows-only 的话也可评估 Avalonia（若团队更想留在 .NET 生态，但内核交互用 Rust 库更顺）。
4. **[传输] 节点模型增加 QUIC 一等公民**：Hysteria2/TUIC 的端口跳跃、Brutal/BBR 拥塞控制、obfs、多路复用（mux）进节点表单；启动时按模式并行探测多节点（游戏场景建议按 target 游戏服 IP 探测），自动选路 + 掉线秒级切换。这是客户端层面离「商业加速体验」最近的一步。
5. **[数据] settings.json → SQLite。** 节点/订阅/模式/凭据四张表，schema 版本化 + 迁移函数；凭据列继续 DPAPI。订阅替换保持 Rust 版已实现的 SubscriptionId 事务语义（这是已修对的 bug，不要回退）。
6. **[可观测] 每会话记录连接质量指标**：握手耗时、首包时间、丢包率、抖动、NAT 类型、STUN 结果，落库后可回看「昨晚为什么卡」。叠加 MTR/路由诊断（借鉴雷神 mtrtool）。
7. **[工程化] EV 代码签名 + ARM64。** README 已承认未签名导致 SmartScreen 警告，这是开源转产品的第一信任门槛；退役内核驱动后签名面只剩 exe/dll，成本可控。构建链收敛到 `cargo build --target x86_64-pc-windows-msvc / aarch64`，C++/Go 工具链退场。
8. **[模式系统] mode 从静态规则文件升级为可下载规则包**（借鉴雷神 tool_xxx）：mode/ 目录加 `meta.json`（适用游戏、版本、校验和），支持从仓库增量更新，用户自建模式保留本地。Bypass/Handle 规则兼容现有格式。

---

## 五、目标架构

```
┌──────────────────────────────────────────────────────────────┐
│  NPGA.exe (Tauri 2.x: WebView2 UI + Rust 后端，用户权限)        │
│  ├─ 主界面 / 节点编辑 / 订阅 / 模式市场 / 连接质量看板            │
│  └─ 托盘 + 游戏内悬浮窗 (延迟/丢包实时显示)                      │
├──────────────────────────────────────────────────────────────┤
│  NPGA-launcher.exe (参考雷神 launcher: 自更新/诊断/修复/安装)    │
├──────────────────────────────────────────────────────────────┤
│  NPGA Core (Rust, 管理员, 独立进程, Named Pipe IPC)             │
│  ├─ NodeManager    节点/订阅/凭据(SQLite+DPAPI)/分享链接        │
│  ├─ CoreManager    sing-box/xray 子进程, Job Object, stats API  │
│  ├─ ModeManager    TUN(process_name 路由)/进程/共享 三模式统一    │
│  ├─ Diagnostics    MTR/路由/NAT类型/连接质量落库                  │
│  └─ Selector        多节点并行探测/自动选路/故障切换(QUIC 优先)   │
├──────────────────────────────────────────────────────────────┤
│  网络栈 (退役 netfilter2.sys + Redirector.bin + aiodns.bin):    │
│  ├─ sing-box TUN + wintun.dll   (进程/系统模式, DNS hijack)      │
│  └─ [兜底] WinDivert 用户态       (TUN 不可用的老旧系统)          │
├──────────────────────────────────────────────────────────────┤
│  代理核心: sing-box 1.13+ (Hysteria2/TUIC/SS/ Trojan...)       │
│            Xray 26.x (VLESS/XTLS-RPRX-Vision...)              │
│  传输: QUIC 优先 (端口跳跃/Brutal), TCP+TLS 兜底                 │
└──────────────────────────────────────────────────────────────┘
```

与现状的差异：删除 netfilter2.sys / Redirector / aiodns / tun2socks.bin 四个组件，新增 launcher、Diagnostics、Selector 三个模块，UI 换成 Tauri，数据落 SQLite。

---

## 六、重构路线图（四阶段，约 9-12 个月业余节奏 / 4-6 个月全职）

### Phase 1 — 验证与收敛（最高优先，约 4-6 周）

1. **验证 sing-box TUN process_name 替代 Redirector 的可行性**：写一个最小配置，对某游戏进程生效/不生效各测一次，确认进程匹配、DNS、UDP（游戏主要是 UDP）三条链路。这是整个重构的支点，**先验证再拆旧**。
2. C# 版标记 archived（保留仓库分支，README 指向 Rust 版），停止新功能。
3. Rust 版补齐功能缺口对齐 C# 全集：流量统计（stats API）、自动更新、NAT/STUN、IP 归属地、测速。
4. 拆掉 npga-rust/src/ui.rs 里残留的 Direct FFI/进程内逻辑，全部经 runtime 层。

### Phase 2 — 网络栈换代（约 6-8 周）

5. ModeManager 统一三模式（进程/TUN/共享）为 sing-box 单核心方案，删除 netfilter2 安装代码路径，Redirector 仅作编译保留一个版本供回退，随后删除。
6. 节点模型扩展：端口跳跃、Brutal/mux、ALPN 进表单（settings 兼容迁移）。
7. Selector v1：启动时并行探测（ICMP + TCP 握手 + HTTP/2 各一层），最低延迟胜出；运行中按游戏目标 IP 复测，劣化超阈值自动切换。

### Phase 3 — 产品化（约 8 周）

8. UI 迁移 Tauri 2.x（前端 React/Solid + 现有 Rust runtime 做 command 层），或 egui 深度打磨二选一，此处是**唯一需要产品决断的岔路**（见七）。
9. Diagnostics 模块：会话质量指标落库 + MTR + 路由表诊断 + 一键 fixnet（注册表/DNS/路由修复）。
10. mode 规则包 + meta.json + 下载更新。
11. SQLite 迁移 + 设置导出/导入。

### Phase 4 — 发布工程（约 4 周）

12. EV 代码签名接入 CI（GitHub Actions 现状已有两套工作流，收敛为一个矩阵：x64/aarch64 × 单文件/安装包）。
13. launcher 产品化（子命令已有雏形）。
14. 完整回归：七协议 × 三模式 + 卸载清理 + 升级迁移测试。

---

## 七、风险与决策点（需要你拍板的）

| # | 决策 | 选项 | 建议 |
| --- | --- | --- | --- |
| 1 | UI 框架 | A. Tauri 2.x　B. egui 继续 | **A**：要产品观感选 A；只有一个人维护且不想写前端选 B |
| 2 | 进程模式实现 | A. sing-box TUN process_name　B. WinDivert 自建 | **A**：先做 Phase 1 验证，失败再退 B |
| 3 | C# 版命运 | A. 冻结归档　B. 双轨并行 | **A**，双轨=慢性失血 |
| 4 | 节点协议取舍 | 全协议 vs QUIC 优先 | 保留全协议节点模型，但**默认配置和 UI 引导 QUIC** |
| 5 | netfilter2 兜底期 | 立即删 / 保留至 Phase 2 末 | Phase 2 末删，之前按「老系统兜底」保留 |

已知风险：

- sing-box TUN 进程匹配依赖连接归属查询，极少数程序（多进程游戏启动器套娃）可能漏匹配，需在 Phase 1 实测清单覆盖；
- Tauri 引入 Web 前端技术栈，如果团队纯系统背景，学习成本真实存在——若否，则 egui 路线把节省的精力投入 Diagnostics 和 Selector（这两项对体验收益更大）；
- 自动选路若设计不谨慎可能「帮用户倒忙」（竞赛类游戏换 IP 可能触发服务器重连/掉线），切换策略必须提供手动覆盖和「仅故障时切换」选项。

---

## 附录 A：本报告引用的代码位置

- `Hatch/Forms/MainForm.cs`（1689 行，33 处 Global 直连）
- `Hatch/Global.cs`（静态 MainForm/Settings/JobObject）
- `Hatch/Servers/SingBox/SingBoxConfigUtils.cs:62`（sing-box outbound 仅 Hysteria2）
- `Hatch/Controllers/NFController.cs:150-253`（netfilter2 驱动安装/卸载/版本比较）
- `Redirector/SocksHelper.cpp`（PROJECT_REVIEW.md P1/P2 问题集中区）
- `npga-rust/src/ui.rs`（2059 行）/ `runtime.rs`（936 行）/ `main.rs`（子命令）
- `npga-rust/MIGRATION.md`（兼容性边界表）
- `build.dependencies.json`（Xray 26.3.27 / sing-box 1.13.14 / wintun 0.13）
- 雷神安装目录：`C:/Program Files (x86)/LeiGod_Acc/resources/leishenSdk/`（NNDivert、nfwfp、msquic、tun2socks、mtrtool 等）

## 附录 B：参考

- sing-box route 规则 process_name/process_path 与 TUN 配置
- sing-box DNS hijack / fakeip
- Hysteria2 端口跳跃与 Brutal 拥塞控制
- Tauri 2.x 文档（WebView2 + 移动端）
- PROJECT_REVIEW.md（本项目历史审查记录）

