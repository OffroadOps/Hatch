# Rust 重构分析与实现边界

分析日期：2026-10-11。现有 C# 与之前未提交的修改完整保留；Rust 版放在 `npga-rust/`，构建入口是 `build-rust.ps1`。

## 原项目结构

| 层 | 原实现 | Rust 版 |
| --- | --- | --- |
| 桌面界面 | .NET 10 WinForms，事件直接引用 Global/MainForm | egui/eframe 原生窗口，UI 与后台命令/事件分离 |
| 配置和节点 | C# 类型鉴别 JSON、DPAPI、各协议模型 | serde + 保留未知字段，兼容 Type/Hostname/Port 等字段 |
| 分享和订阅 | ShareLink 与 SubscriptionUtil | Rust URI/Base64 解析，事务替换，以 SubscriptionId 关联节点 |
| 核心启动 | Guard / XrayController / SingBoxController | Rust 子进程、配置检查、Job 对象、并发日志读取、取消和退出回收 |
| 进程模式 | NFController → Redirector.bin / nfapi.dll / netfilter2 | Rust 动态 FFI，按原生源码使用 BOOL/i32 与 void 返回类型，复用原生组件 |
| TUN | C# RouteHelper + tun2socks，手动管理路由 | sing-box TUN，复用 Wintun；按原 CIDR 配置 route_address/route_exclude_address |
| 网络共享 | PcapController → pcap2socks / Npcap | Rust 进程控制，原共享模式参数、显式网卡设备名 |
| DNS | aiodns、TUN 配置和 Redirector DNS | TUN 使用 sing-box DNS hijack；进程模式使用 Redirector DNS 配置 |

C# 可直接恢复类型、方法和事件逻辑，是当前逆向门槛较低的主要原因。改为 Rust 原生机器码可以移除托管元数据；第三方核心继续独立分发，也便于校验和升级。此重构没有设计不可靠的客户端加密壳或假定 Rust 不可逆向。

## 实现与验证

- 节点添加、编辑、删除、筛选、选择和分享；常用协议使用表单，所有原字段可通过高级参数编辑。
- 支持 NPGA/Hatch、VMess、VLESS、Trojan、SS、SSR、Hysteria2、SOCKS 分享链接、SS JSON 和 Base64 订阅。
- 导入同一 Windows 账户的 DPAPI 凭据；新增凭据与订阅地址使用 DPAPI 保存，解密失败不静默丢失数据。
- 订阅下载有超时和 4 MiB 上限；HTTP 错误、空响应、混合有效/无效节点均不替换旧节点。
- 以独立 SubscriptionId 归属新同步节点，不因显示分组同名而删除手动节点。旧版节点没有归属 ID，首次同步保留旧节点以避免误删，可能需要手动整理重复项。
- TXT/JSON 模式、递归 include、CIDR 校验与目录边界/循环检查；本机完整加载 97 个模式，无读取错误。
- 启动前调用实际核心检查配置。端口冲突直接报错；不会查找并杀死其他应用。
- 核心日志并发读取，隐藏已知凭据；退出终止受管进程树并移除本次临时配置。
- Windows 系统代理原值包含 PAC URL，正常停止恢复；异常退出保留 DPAPI 恢复记录，下次启动恢复。
- 托盘收起/显示/停止/退出、用户级开机启动、管理员重启、核心组件诊断。
- 回归覆盖迁移、链接、订阅数据保留、DPAPI、模式 include 和端口。实际固定核心验证 SOCKS/SS/VMess/VLESS/Trojan/WireGuard/Hysteria2/TUN 配置。
- 本地模拟 SOCKS 上游通过实际 Xray 验证 SOCKS5 和 HTTP CONNECT 数据往返，验证重复停止、端口释放和临时配置回收。没有连接外部服务器或修改系统代理、驱动、路由。
- 主界面与节点编辑弹窗通过原生渲染截图检查；没有将设计图当作运行截图。

## 兼容性边界

Rust 版是独立客户端，目前不能宣称与原版每一项辅助功能完全等价：

| 项目 | 状态与原因 |
| --- | --- |
| 进程/TUN/网络共享真实连接 | 已接入启动和清理链路，配置已校验；尚需在管理员环境、有效服务器和目标游戏中完成验证。进程模式另有 TUN 后端可选（`ProcessEngine=tun`，见下） |
| SSR | 可以迁移/分享；固定 Xray 不支持原生 SSR，明确拒绝连接 |
| SS SIP003 插件 | 保存参数，明确拒绝连接；需要单独实现插件进程及适配 |
| 旧 h2/quic/XTLS | 固定 Xray 的旧配置不能自动等同于新传输；明确拒绝，不静默忽略 |
| Hysteria2 证书指纹 | 保存参数，明确拒绝连接，尚未映射校验字段 |
| 流量监控 | 五种路由模式统一经核心 stats API 显示（Xray `api`/`stats`、sing-box `clash_api`）；进程模式另有 Redirector 计数兜底 |
| 多语言界面 | 界面目前为简体中文；节点/模式数据与旧配置兼容 |
| 快捷配置、NAT/STUN、IP 归属地、测速下载 | **已迁移**：连接页「连接诊断」提供 NAT 类型（经 SOCKS 的 RFC 5389 三测试）、出口 IP 归属地（内置 mmdb）、网络测速（经本地 HTTP 代理） |
| 自动更新 | **已迁移**：关于页「检查更新」查询 GitHub Releases、比较版本、解析校验和表 |
| TUNTAP 旧地址/网关/掩码 | 保留原字段；新 TUN 采用独立地址，由 sing-box 管理，无需原手动路由工具 |
| 强制结束后的恢复 | Job 会终止受管核心；系统代理在下次启动恢复，不能保证崩溃瞬间恢复注册表 |

以上差异在客户端、说明和错误信息中显式体现。发布包继续包含原生库和固定核心，不能将主 EXE 的 Rust 重写解释为 C++ 驱动或 Go 代理核心也改写成 Rust。

## 2026-10-14：主线收敛与功能补齐

按 `refactor/DECISIONS.md` ADR-001，本目录成为项目唯一主线，C# 客户端冻结。本次补齐的能力与验证：

- 流量统计端口随 SOCKS 端口派生（+3 Xray API、+4 sing-box Clash API），启动前预占校验，冲突时报明确错误。
- 新增模块：`stats.rs`（统计轮询）、`nat.rs`（STUN/NAT 类型）、`geoip.rs`（纯 Rust mmdb 读取器）、
  `speedtest.rs`（代理测速）、`update.rs`（版本检查）。
- 验证：`cargo test` 全绿（4 单元 + 18 迁移 + 4 真实核心集成）；GeoIP 读取器与参考 `maxminddb` 实现对拍一致；
  统计 API 配置经真实 Xray 26.3.27 / sing-box 1.13.14 校验通过。
- 已知未验证项：Xray `api statsquery` 的 gRPC 调用在沙箱环境被本机代理拦截，未做端到端实测；
  实现使用官方 CLI 子进程，失败时速率显示为「—」而不影响连接。

## 2026-10-15：进程模式 TUN 后端（实验）与 CI 收敛

- **进程模式可切到 sing-box TUN**（ADR-006）：高级设置新增「进程模式后端」开关，写入设置项 `ProcessEngine`：
  - `redirector`（默认）：现有 netfilter2 驱动 + Redirector.bin 路径；
  - `tun`：sing-box TUN + `process_name` 路由，DNS 劫持只对受管进程生效，未列进程直连。
- **规则兼容性**：旧模式的 C++ 正则进程规则会自动转换为精确进程名（`^game\.exe$` → `game.exe`）；
  含通配/分组/路径的规则无法在 TUN 后端表达，运行日志会列出条数与被跳过的规则，全部不可转换时直接报错。
- **验证状态**：生成的 TUN 进程配置经真实 sing-box 1.13.14 校验通过；
  端到端（进程匹配、UDP 游戏流量、停止清理）仍需在管理员环境按 `refactor/verify/tun-process/README.md` 实测，
  通过后才会把默认值切到 `tun` 并删除 Redirector/netfilter2。
- **运行时装配**：`Tools/FetchPinnedCores.ps1` 可按 `build.dependencies.json` 下载并校验核心，
  组装最小运行时用于测试（CI 已采用）。`build-rust.ps1` 仍需要 C# 包提供 Redirector.bin/nfapi.dll/nfdriver.sys，
  待 Phase 2 移除这些遗留组件后即可完全脱离 C# 构建。

## 参考

- [ToDesk 官方 Windows 使用界面](https://www.todesk.com/faq/2.html)
- [eframe 0.33.3 App API](https://docs.rs/eframe/0.33.3/eframe/trait.App.html)
- [sing-box TUN 配置](https://sing-box.sagernet.org/configuration/inbound/tun/)
- [sing-box 路由规则](https://sing-box.sagernet.org/configuration/route/rule/)

生成配置同时用项目固定的 sing-box 1.13.14 与 Xray 26.3.27 验证，以实际核心结果为准。
