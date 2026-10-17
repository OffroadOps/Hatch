# runtime-native — 审计过的原生组件

Rust 客户端运行时（进程模式）依赖的两个二进制组件。它们由 C++ 源码 `Redirector/`（x64 Release）
构建；随着 Hatch 完整构建链移出主仓库（`refactor/DECISIONS.md` ADR-007），源码现位于归档目录
`D:\netch.plus-archive\Redirector`，二进制以本目录随仓库跟踪，装配时校验 SHA256。

| 文件 | 构建来源 | SHA256 |
| --- | --- | --- |
| `Redirector.bin` | `Redirector/Redirector.vcxproj`（SocksHelper UDP/TCP 修复后） | `830de7b2…946c83` |
| `nfapi.dll` | `Redirector/Redirector.vcxproj` | `f0519b24…e842cc` |

## 装配与更新

- 装配：`Tools/AssembleRuntime.ps1` 将本目录文件（校验 `SHA256SUMS.txt`）连同
  `Storage/` 资产与固定版本核心一起组装成运行时根目录，供 `build-rust.ps1` 打包。
- 更新流程：
  1. 在归档目录 `D:\netch.plus-archive` 执行 `.\build.ps1 -SkipInstaller`
     （需要 Visual Studio C++ 工具链；或对 `Redirector.vcxproj` 单独 MSBuild）;
  2. 从 `release\NPGA\` 取新的 `Redirector.bin` / `nfapi.dll` 覆盖本目录；
  3. 重新生成本文件下方的 `SHA256SUMS.txt` 并提交。
- 退役条件：ADR-002（sing-box TUN 进程路由）实测通过、`ProcessEngine` 默认值切到 `tun` 后，
  本目录随 Redirector 一并退役。

`nfdriver.sys` 不在此目录：它保持随 `Storage/` 跟踪（与 `mode/` 一样由装配脚本复制）。
