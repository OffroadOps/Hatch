# 项目审查与清理记录

> **2026-10-17 更新**：Hatch 完整构建链（C#/C++/Go、安装器、`build.ps1`）已移出主仓库至独立归档
> `D:\netch.plus-archive`（见 `refactor/DECISIONS.md` ADR-007）。本文为归档前的历史审查记录，
> 文中 `Hatch/`、`build.ps1`、`Redirector/` 等路径现位于归档目录。

日期：2026-10-08。范围：本地源码、构建配置、测试、缓存目录，以及 sing-box 官方兼容性说明。未进行驱动安装、代理连接或系统路由修改。

## 后续更新：.NET 10 升级

用户随后明确要求升级到 .NET 10，以下升级已经实施。本文其余部分保留首次审查时的发现和环境记录。

- Hatch、Tests、HatchIpProbe、RedirectorTester 全部迁移到 `net10.0-windows`；RedirectorTester 从 .NET Framework 4.8 转为 SDK 风格，移除旧 App.config。
- SDK 和两套 CI 工作流统一固定为本机已安装的 10.0.401；README 和 CHANGELOG 同步更新。
- 重新生成 NuGet 锁文件，删除框架已提供的 CodePages 显式引用；修正日志窗体过时的 WinForms 关闭回调。
- 框架依赖部署的运行时滚动策略设为 LatestPatch，限定在 .NET 10 补丁版本。
- 验证：锁定依赖恢复成功；Release 主程序编译成功；4/4 测试通过；HatchIpProbe 和 RedirectorTester 均为零警告、零错误；win-x64 自包含单文件发布成功，验证输出位于 `.build-cache/net10-verification/`。
- 未运行驱动测试工具、未测试真实代理连接/TUN 路由、未重新编译 C++/Go 或生成安装包；原有业务问题仍待修复。

## 后续更新：NPGA 品牌

用户指定项目简称 **NPGA**，全称 **Netch.Plus Game Accelerator**。已更新窗口/托盘/关于页、程序集元信息、自动启动项、防火墙规则名称、说明文档、CI 产物和安装器。程序为 `NPGA.exe`，安装包为 `NPGASetup.exe`，便携包为 `NPGA.zip`，顶层目录为 `NPGA/`；最终输出位于 `release-npga/`。

分享链接现在导出 `NPGA://`，仍可导入旧 `Hatch://`。现有 GitHub 仓库链接仍指向真实的 `OffroadOps/Hatch`，源目录和命名空间保留以保持代码与资源引用稳定。

## 后续更新：业务修复、工具链和安装包

首次审查中的问题清单是历史记录。此次已修复 UDP 解包边界和重叠复制、SOCKS 分段读写、UDP socket/控制线程所有权、构建输出删除路径保护、sing-box 旧嗅探字段、无效订阅覆盖、Hatch 分享链接端口与派生字段导出、核心日志并发和释放竞争。同时修复异步连接检测异常捕获、进程路径前缀边界判断，以及安装包自定义输入/输出路径。

- 托管测试 10/10 通过，覆盖分享链接、无效订阅保留、配置生成及 2,000 行并发日志完整性和重复停止。
- 原生 SOCKS 测试通过：IPv4/IPv6 短包、合法 payload、RSV/FRAG 验证、逐字节分段握手、20 次控制线程关闭。
- 使用实际 sing-box 1.13.14 的 `check` 验证新生成配置，成功。
- 本机 Inno Setup 6.7.3 已安装。按用户选择，将 Go 1.25.14 和 w64devkit 2.10.0（GCC 16.2.0）放入 `D:\DevTools`；下载分别对照 Go 官方 SHA256 和 GitHub 发布 asset digest 校验成功。
- Go/MinGW 两个 bin 目录已加入系统 PATH；原 PATH 备份位于 `.build-cache/toolchain-environment-result.json.before.json`。代理只用于本次下载/构建进程，未保存为系统代理变量。
- 最终构建目录为 `release-net10/`，Redirector、RouteHelper、aiodns 都从源码构建，包含 .NET 10 自包含主程序和固定版本代理核心。
- 尚未进行真实服务器连接、系统驱动安装与 TUN 路由的端到端验证。订阅归属仍使用 Group/Remark，手动节点与订阅同名分组的隔离建议尚未实施；此次修复的是空或无效订阅不再清空旧节点。

## 首次审查时的项目现状

- Hatch 是基于 Netch 延续开发的 Windows x64 网络代理客户端，主程序为 .NET 8 WinForms。
- `Hatch/`：界面、配置、订阅、核心进程控制与路由管理；`Redirector/`：C++ 网络过滤和 SOCKS 转发；`RouteHelper/`：原生路由操作；`Storage/`：驱动、模式及运行资源。
- `Other/aiodns/`、`Tools/HatchIpProbe/` 分别提供 DNS 和诊断工具；`build.ps1` 汇总托管、原生、核心下载和安装包构建。
- 有固定版本、SHA256 校验和 NuGet 锁文件。现有测试仅一个类、4 个方法，覆盖版本比较和更新元数据，未覆盖主要网络路径。
- 初始 Git 工作区干净；没有发现 AGENTS.md。Git 因目录所有者变化拒绝默认操作，审查使用单次 `-c safe.directory`，未修改全局配置。

## 优先处理的问题

以下均为源码审查发现；除官方版本兼容性核对外，未完成运行时复现。

| 优先级 | 位置 | 触发与影响 | 修复建议 |
| --- | --- | --- | --- |
| P1 | `Redirector/SocksHelper.cpp:435–457` | UDP 收包后未验证最小头长度便读取 `buffer[3]`、地址和端口。短 IPv4/IPv6 包会令 `size-10/22` 转为巨大无符号长度，导致越界复制；正常解包还使用重叠区域 `memcpy`，行为未定义。 | 先验证实际包长、RSV、FRAG、ATYP 和对应头长；搬移数据用 `memmove`；加短包、边界包回归测试。 |
| P1 | `build.ps1:21,124–125` | `OutputPath` 未做安全边界检查即递归删除。例如 `-OutputPath .` 指向项目根目录，可能删除源码；指定其他目录也会删除其原内容。 | 删除前拒绝项目根、祖先目录、源码目录及重解析点；建议仅允许明确的构建输出目录。SDK 检查也应在删除前完成。不要用危险参数验证此问题。 |
| P1 | `Hatch/Servers/SingBox/SingBoxConfigUtils.cs:32–33` | 固定依赖是 1.13.14，但配置仍含 `sniff` 和 `sniff_override_destination`。官方说明这些 inbound 字段已在 1.13.0 移除，Hysteria2 启动存在配置不兼容。 | 将嗅探迁移到 route rule actions；针对固定核心执行 `sing-box check`，作为发布前检查。`block` outbound 也应单独检查版本兼容性，官方 changelog 存在恢复支持的说明，不把它单独判定为必现错误。 |
| P1 | `Hatch/Utils/SubscriptionUtil.cs:31,40–41` | 服务端 HTTP 200 返回错误页面或无法识别的订阅时，`ShareLink.ParseText` 会吞解析错误并返回空列表，随后删除该分组全部旧节点并保存。订阅 remark 与手动节点 Group 相同也可能误删。 | 下载、解析、校验成功后再替换；空列表视为失败或显式确认；用稳定订阅 ID 关联节点，避免使用显示名称作为归属键。 |
| P2 | `Hatch/Utils/ShareLink.cs:110` | 自定义分享链接序列化 `ushort Port` 为 JSON 数字，导入却调用 `GetString()`。本程序导出的 `Hatch://` 链接会解析失败并被上层吞掉。 | 数字字段使用 `TryGetUInt16`；如需兼容旧字符串格式显式分支；补导出→导入往返测试。 |
| P2 | `Redirector/SocksHelper.cpp:68,107,237` | 单次 `recv` 被当作完整握手读取，合法 TCP 分段也会被认定失败；发送亦未循环处理部分写入。 | 封装 `ReceiveExact`/`SendAll`，设置握手超时；测试每字节分段和提前断开。 |
| P2 | `Redirector/SocksHelper.cpp:266–299,376` | UDP 对象析构和 detached 线程都关闭相同 socket，线程没有统一生命周期和所有权。连接释放后存在重复关闭、句柄复用误关风险。 | 统一 socket 所有者；停止时唤醒并 join 工作线程；用 RAII 保证只关闭一次。 |
| P2 | `Hatch/Controllers/Guard.cs:79–87,123` | stdout/stderr 两个异步任务同时写一个 StreamWriter，且任务未保存、停止时未等待全部读写结束；并发输出存在写入冲突与释放竞争。子进程静默退出也只能等到 50 秒启动超时。 | 用单一日志消费者或异步锁；保存并等待 reader tasks；监听进程退出并及时标记失败。 |
| P2 | `Hatch/Controllers/MainController.cs:211–226` | 返回异步 Task 时没有 await，catch 捕获不到后续 DNS、SOCKS 检测任务抛出的异常或取消，与代码期望返回 null 的行为不符。 | 改为 async 方法，在 try 内 await；补取消、DNS 失败、连接失败测试。 |
| P2 | `Hatch/Controllers/MainController.cs:191` | 用字符串前缀判断进程是否位于程序目录，例如 `D:\HatchOther` 也匹配 `D:\Hatch`，随后调用 Kill；默认大小写比较还可能漏判 Windows 路径。 | 规范化路径，用带目录分隔符的 OrdinalIgnoreCase 比较；限制到已知核心进程或受管进程 ID。 |

sing-box 官方依据：[移除字段说明](https://sing-box.sagernet.org/deprecated/)、[迁移示例](https://sing-box.sagernet.org/migration/)、[变更记录](https://sing-box.sagernet.org/changelog/)。

## 清理范围与状态

`clean.ps1` 默认只预览；运行 `./clean.ps1 -Apply` 才删除。范围使用明确白名单，验证绝对路径在项目内、没有 Git 跟踪文件和重解析点。

| 目录 | 文件数 | MiB |
| --- | ---: | ---: |
| Hatch/bin | 540 | 179.99 |
| Hatch/obj | 31 | 13.04 |
| Tests/bin | 245 | 28.26 |
| Tests/obj | 20 | 0.48 |
| .build-cache/publish-Release | 1 | 73.06 |
| .build-cache/extract-pcap2socks-59b3a2e5054c | 3 | 2.72 |
| .build-cache/extract-singBox-f580782c6dd1 | 3 | 52.25 |
| .build-cache/extract-wintun-34afe7d0de1f | 7 | 3.00 |
| .build-cache/extract-xray-d004c39288ce | 9 | 63.26 |

共 859 个文件，按实际字节合计约 416.07 MiB。保留 `release/` 发布包、`artifacts/` 原生构建输入、下载的 ZIP/数据库缓存、源码和素材。清理后需要重新生成托管输出与解压缓存。

**实际删除未执行成功**：自动审批检查拒绝清理命令，返回 `blocked by policy`，未给出更具体原因。没有释放磁盘空间，脚本供审阅和本机执行。

## 验证限制与后续建议

- 尝试 `dotnet test Tests/Tests.csproj --no-restore -p:Platform=x64`，因 `global.json` 要求 8.0.100 且禁用 rollForward，而本机只有 SDK 10.0.401、运行时 10.0.12，命令在编译前失败。不能将此视作测试通过或源码编译失败。
- 使用本地 sing-box 执行配置检查的命令同样被自动审批检查拦截；该问题依据固定版本和官方字段移除说明判断，尚无本地执行日志。
- 优先修复 UDP 内存边界、构建删除路径保护、sing-box 配置迁移、订阅替换保护，再处理分享链接和异步生命周期。
- 增加有实际价值的测试：分享链接往返、无效订阅保留旧节点、SOCKS 分段读写、UDP 短包、核心生成配置检查、启动失败后的清理。将可纯函数测试的解析和配置生成与全局 UI 状态分开。
- 统一核心启动/停止的资源所有权与失败回滚；缩小大量全局警告抑制，逐步恢复未等待任务和空引用诊断。
- 修正 README 的服务器存储路径：实现使用 `data/settings.json` 的 Server 列表，文档目前写 `data/servers.json`。清理 sing-box 文件中的乱码注释，统一术语和品牌命名，但先确认资源引用，避免仅因名字含 Netch 就删除。
- 如调整 SDK 策略，应在 CI 验证后明确选择 .NET 8 SDK 滚动策略；不要为了当前机器编译成功直接把项目改为 .NET 10。

此次新增审查报告和清理脚本，未修改业务源码或全局 Git 设置。
