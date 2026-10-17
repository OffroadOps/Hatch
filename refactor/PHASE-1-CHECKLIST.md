# Phase 1 任务清单 — 收敛与验证

开始日期：2026-10-14。本阶段目标：**只保留一个客户端，并验证 Redirector 退役方案可行**。

## 1.1 冻结 C#（ADR-001）✅

- [x] `Hatch/FROZEN.md` 冻结声明（允许/禁止改动清单 + 迁移映射表）
- [x] 根 `README.md` 主线切换说明（中英文）
- [x] `CHANGELOG.md` 追加冻结与主线条目

## 1.2 Rust 补齐至 C# 功能全集 ✅

- [x] 流量统计：Xray `api`/`stats` + sing-box `clash_api` 轮询，五种路由模式统一显示（ADR-005）
      — 真实核心校验测试 `stats_api_attachments_validate_against_real_cores` 通过
- [x] 自动更新：`update.rs`（GitHub Releases + 版本比较 + 校验和表解析），含单元测试
- [x] NAT 类型：`nat.rs`（SOCKS5 UDP ASSOCIATE + RFC 5389/3489 三测试）
- [x] IP 归属地：`geoip.rs`（纯 Rust mmdb 读取器，与参考实现对拍：IPv4/IPv6/指针/私有段）
- [x] 网络测速：`speedtest.rs`（经本地 HTTP 代理，reqwest 内置代理支持，无新增依赖）
- [x] UI 接线：连接页「连接诊断」区（NAT/归属地/测速）+ 关于页「检查更新」+ 全模式速率显示
- [x] `cargo test` 全绿：4 lib + 18 migration + 4 集成（`--ignored` 用真实核心）
- [x] `cargo check --release --all-targets` 通过，无警告

## 1.3 TUN 进程路由验证（ADR-002）✅（2026-10-17 实测 11/11）

- [x] `verify/tun-process/` 配置模板（`sing-box check` 通过）+ 验证脚本 + 通过标准
- [x] 产品侧实现（ADR-006）：`core_config::tun_process()` + `ProcessEngine` 开关（2026-10-17 起默认 `tun`，UI 可回退 `redirector`）
- [x] 规则转换 `process_names()`：正则→精确进程名，无法转换的显式上报
- [x] 真实核心校验：`process_tun_config_validates_against_real_sing_box` 通过
- [x] 管理员环境实测：进程匹配（Handle 走 proxy / Bypass 直连，clash API `process_path` 匹配）
- [x] 实测：DNS 劫持（hijack-dns 生效；「仅受管进程」作用域由 `core_config::tun_process()` 规则顺序保证并经真实核心校验，本轮未做逐进程行为区分）
- [ ] 实测：UDP 游戏流量（本轮脚本仅覆盖 TCP，建议 Phase 2 前用真实游戏场景补测）
- [x] 实测：停止后清理（网卡/路由）
- [x] 通过后把 `ProcessEngine` 默认值改为 `tun`，标记 ADR-002 Accepted（2026-10-17）
- [ ] 进入 Phase 2 删除 Redirector/netfilter2（归档侧源码与主仓库 `runtime-native/`、`Storage/nfdriver.sys`、`install-driver.ps1` 一并退役）

## 1.4 工程收敛 ✅（Rust 侧完成）

- [x] `build.yml`：Rust 作业为默认门禁（fmt + clippy -D warnings + 全量测试 + release 构建）；
      C# 作业由 `dorny/paths-filter` 判定冻结树变更后运行（push 与 pull_request 均生效），手动触发时始终运行
- [x] `release.yml`：新增 Rust 打包作业，tag 发布同时产出 `NPGA-Rust.zip`
- [x] `Tools/FetchPinnedCores.ps1`：按清单下载 + SHA256 校验组装最小运行时（CI 与本地共用）
- [x] `install-driver.ps1` 标注为遗留组件
- [x] build-rust.ps1 摆脱对 C# 包的依赖（ADR-007，2026-10-17）：C# 构建链移出仓库至独立归档
      `D:\netch.plus-archive`，运行时由 `Tools/AssembleRuntime.ps1` 装配（固定版本核心 + Storage 资产 +
      `runtime-native/` 审计二进制）

## 验证记录（2026-10-17）

| 项 | 结果 |
| --- | --- |
| TUN 进程路由端到端（管理员，sing-box 1.13.14） | **11/11 全过**（EXITCODE=0）：进程匹配 Handle 走 proxy / Bypass 直连、DNS 劫持、停止清理；记录与脚本修复见 `verify/tun-process/README.md`，输出存档 `verify/tun-process/run-2026-10-17.log` |
| `ProcessEngine` 默认值切换（model.rs） | 完成；ADR-002 标记 Accepted |
| 切换后 `cargo fmt --check` / `clippy -D warnings` | 通过 |
| 切换后 `cargo test --locked -- --include-ignored --test-threads=1`（`NPGA_RUNTIME_ROOT=.release-runtime`） | 38 passed（14 单元 + 18 迁移 + 6 真实核心） |

## 验证记录（2026-10-16）

上一轮改动遗留的失败项已修复；以下为本机全量复验结果。

| 项 | 结果 |
| --- | --- |
| `cargo fmt --check` | 通过（修复前 `core_integration.rs` 失败，CI 格式门禁必然报错） |
| `cargo clippy --locked --all-targets -- -D warnings` | 无警告 |
| `cargo test --locked -- --include-ignored --test-threads=1` | 38 passed（14 单元 + 18 迁移 + 6 真实核心集成） |
| `auto_select_skips_failed_candidates` | 通过（修复前必然超时：用例漏做 SOCKS5 握手，xray 按非法版本丢弃连接） |
| `dotnet test Tests` | 14 passed（新增 4 个核心下载校验用例） |
| `NativeSocksTests.exe` | 通过 |
| `Tests/BuildPathSafetyTests.ps1` | 通过（pwsh 7；5.1 现给出明确的版本要求错误） |
| `build.ps1 -OutputPath release-verify` 端到端 | 成功，含原生构建、`sing-box check`、`NPGA.zip` + `NPGASetup.exe` |
| `build-rust.ps1` 端到端（用上一步产物作运行时） | 成功，产出 `NPGA-Rust.zip` |
| `build.yml` / `release.yml` YAML 语法 | 解析通过 |
| TUN 进程路由端到端 | **未实测（当前会话无管理员权限）** |

本机 Rust 工具链此前不完整（`~/.cargo/bin` 下 `cargo.exe` 缺失、若干 rustup 代理为 0 字节），
已重建代理硬链接；`build-rust.ps1` 在缺少 `cargo` 时会以明确错误退出，属预期行为。

## 验证记录（2026-10-15 补充）

| 项 | 结果 |
| --- | --- |
| `cargo test --locked -- --include-ignored --test-threads=1` | 30 passed（7 单元 + 18 迁移 + 5 真实核心集成） |
| 用 `FetchPinnedCores.ps1` 组装的运行时跑真实核心测试 | 5 passed（**未使用 C# 构建产物**） |
| `cargo clippy --all-targets -D warnings` | 无警告（本机 clippy 组件缺失，用 clippy-driver 直接校验） |
| `cargo fmt --check` | 通过 |
| `build-rust.ps1` 端到端（`-SkipTests`） | 成功，产出 `NPGA-Rust.zip`（35 MB）+ SHA256SUMS.txt |
| `build.yml` / `release.yml` YAML 语法 | 解析通过（jobs: rust/legacy、runtime/rust-package/publish） |
| TUN 进程路由端到端 | **未实测（当前会话无管理员权限）** |

## 验证记录（2026-10-14）

| 项 | 结果 |
| --- | --- |
| `cargo test`（lib 单元） | 4 passed |
| `cargo test`（migration） | 18 passed |
| `cargo test --test core_integration -- --ignored`（真实 Xray 26.3.27 / sing-box 1.13.14） | 4 passed |
| `cargo check --release --all-targets` | 通过，无警告 |
| sing-box clash API `GET /traffic` 实测 | `{"up":N,"down":N}` HTTP 200 |
| mmdb 读取器对拍（参考 maxminddb 库） | 7/7 IP 一致（含 IPv6） |
| `refactor/verify/tun-process/tun-process.json` | `sing-box check` 通过 |
| TUN 进程路由端到端 | **未实测（需管理员环境）** |
| Xray `api statsquery` gRPC 路径 | 本机受沙箱代理拦截，无法实测；实现走官方 CLI，失败时速率显示为「—」 |

## 验收标准

Rust 客户端功能 ⊆ C# 全集（按 `npga-rust/MIGRATION.md` 边界表逐项销项）；
TUN 验证 5 项全过；`cargo test` 绿。
