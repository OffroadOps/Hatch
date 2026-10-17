# sing-box TUN + process_name 替代验证（ADR-002）

这是整个重构的**支点验证**：如果 sing-box TUN 的进程路由能在真实环境满足需求，
`Redirector/`（自研 C++ SOCKS 转发）、`netfilter2.sys`（未签名内核驱动）、
`aiodns.bin`、`tun2socks.bin` 四个组件就可以整体退役。

## 运行方式

```powershell
# 必须以管理员身份运行（TUN 需要）
powershell -ExecutionPolicy Bypass -File .\verify.ps1
```

脚本不连接任何真实代理：`proxy` outbound 指向本机 127.0.0.1:18081 的
HTTP CONNECT sink（由脚本启动），因此：

- 不产生外部代理流量；下载测试即使失败也不影响进程匹配断言（连接在 TUN 层即建立）
- 不修改任何持久配置，全部使用临时目录

## 验证项与通过标准

| # | 项 | 通过标准 |
| --- | --- | --- |
| 1 | `sing-box check` | 退出码 0（auto_route / process_name / hijack-dns 字段全部被接受） |
| 2 | TUN 网卡与自动路由 | `NPGA-TEST` 网卡存在且有自动下发路由 |
| 3 | Handle 进程 | `npga-handle-test.exe`（curl 副本）的连接在 clash API 可见，且 chains 含 `proxy` |
| 4 | Bypass 进程 | `npga-bypass-test.exe` 的连接 chains 不含 `proxy` |
| 5 | DNS 劫持与清理 | 日志出现 `hijacked dns query`；停止后网卡与路由全部移除 |

## 记录结果

```
日期/机器：2026-10-17 / NewPC（Windows；管理员提权运行，包装器用 pwsh 7）
sing-box 版本：1.13.14（Tools/AssembleRuntime.ps1 装配）
1 check:      [x] PASS  [ ] FAIL
2 TUN/路由:  [x] PASS  [ ] FAIL
3 Handle:    [x] PASS  [ ] FAIL
4 Bypass:    [x] PASS  [ ] FAIL
5 DNS/清理:  [x] PASS  [ ] FAIL
备注（失败现象、日志位置）：EXITCODE=0，PASS=11/11；输出存档见同目录 run-2026-10-17.log。
首轮 7/11 的 3 个失败项均为脚本自身缺陷，修复记录见下节。
```

已于 2026-10-17 达成：`DECISIONS.md` ADR-002 标记 Accepted、`ProcessEngine` 默认切 `tun`；
Phase 2 删除旧组件待执行（UDP 游戏流量场景建议届时用真实游戏补测）。

## 修复记录（2026-10-17：首轮 7/11 → 复跑 11/11）

首轮实测 3 个失败项均源于脚本自身，产品行为无误：

| 失败项 | 原因 | 修复 |
| --- | --- | --- |
| Handle 连接在 clash API 不可见 | curl 依赖域名解析；查询被 hijack 后经 HTTP CONNECT sink 无法完成（sink 不承载 UDP），curl 从未发起连接 | curl 增加 `--resolve cachefly.cachefly.net:80:93.184.216.34`，测试不依赖 DNS |
| DNS 劫持判定失败 | 断言匹配的 `hijacked dns query` 文案在 sing-box 1.13 不存在（劫持实际已生效，日志可见 `:53` 数据包进入 TUN 与 `dns:` 条目） | 改为以被劫持查询的域名出现在日志为准 |
| Bypass「未走 proxy」 | 进程未连上时该断言恒真（空真） | 增加「连接在 clash API 可见」前置断言 |

另有两处工程修复：

- 脚本原先无法读取 sing-box 输出：sing-box 日志走 stderr，已用 `-RedirectStandardError` 捕获到
  `npga-tun-verify.err.log`，启动/DNS 断言同时检索 stdout 与 stderr。
- 脚本声明 `#Requires -Version 5.1` 但为 UTF-8 无 BOM：PowerShell 5.1 按 GBK 解码中文注释会破坏解析
  （报「字符串缺少终止符」）。已给 `verify.ps1`、`npga-rust/install-driver.ps1`、`Tools/FetchPinnedCores.ps1`
  补 UTF-8 BOM，5.1 / 7 双引擎解析 0 错误。

## 已知边界（需要人工观察）

- **进程匹配依赖连接归属查询**：启动器套娃（游戏通过第三方启动器拉起）可能漏匹配，
  测试时建议额外用一个真实启动器场景复测（如 Steam 启动的游戏）。
- **system 栈 vs mixed 栈**：`mixed` 为默认推荐；若某游戏在 mixed 下异常，
  记录是否需要按进程切换 gVisor（`system`）栈。
- **强制结束（Job 对象终止）不会清理路由**：本脚本第 5 项若在强制 kill 后失败，
  产品侧需要在 Rust runtime 增加终止后的兜底清理（见 ADR-002 后果）。
