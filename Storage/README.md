# Storage

共享运行资源。本目录只保留 Rust 主线仍在使用的内容。

| 路径 | 用途 |
| --- | --- |
| `mode/` | 路由模式定义（97 个 TXT/JSON 文件），由 `Tools/AssembleRuntime.ps1` 复制进运行时目录，客户端在「加速方式」下拉框中读取 |

## 已移除的历史资产

以下文件属于已冻结并归档的 2.0.x C# 客户端（见 `refactor/DECISIONS.md` ADR-001 / ADR-007），
Rust 主线没有引用，已从仓库删除：

| 文件 | 原用途 | Rust 版的替代方案 |
| --- | --- | --- |
| `Storage/tun2socks.bin` | C++ `tun2socks` 隧道 | sing-box TUN（ADR-002） |
| `Storage/aiodns.conf` | aiodns DNS 解析器 | sing-box 内置 DNS + `hijack-dns` |
| `Storage/i18n/ja-JP`、`Storage/i18n/zh-TW` | C# 客户端的本地化资源 | 客户端界面为中文，内置文本 |
| `Storage/stun.txt` | STUN 服务器清单 | `Settings::stun_server()` 读取配置中的 `STUN_Server` |
| `hatch_logo_no_background.png` | 旧客户端图标 | 界面使用矢量绘制的 `logo()` |

代理核心（Xray / sing-box / pcap2socks / wintun）与 GeoLite2 数据库由
`Tools/FetchPinnedCores.ps1` 按 `build.dependencies.json` 的固定版本与 SHA256 下载，
不入库。
