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
日期/机器：____
sing-box 版本：____
1 check:      [ ] PASS  [ ] FAIL
2 TUN/路由:  [ ] PASS  [ ] FAIL
3 Handle:    [ ] PASS  [ ] FAIL
4 Bypass:    [ ] PASS  [ ] FAIL
5 DNS/清理:  [ ] PASS  [ ] FAIL
备注（失败现象、日志位置）：____
```

全部 PASS 后：`DECISIONS.md` ADR-002 标记 Accepted，进入 Phase 2 删除旧组件。

## 已知边界（需要人工观察）

- **进程匹配依赖连接归属查询**：启动器套娃（游戏通过第三方启动器拉起）可能漏匹配，
  测试时建议额外用一个真实启动器场景复测（如 Steam 启动的游戏）。
- **system 栈 vs mixed 栈**：`mixed` 为默认推荐；若某游戏在 mixed 下异常，
  记录是否需要按进程切换 gVisor（`system`）栈。
- **强制结束（Job 对象终止）不会清理路由**：本脚本第 5 项若在强制 kill 后失败，
  产品侧需要在 Rust runtime 增加终止后的兜底清理（见 ADR-002 后果）。
