# NPGA Rust

NPGA 的 Rust 原生 Windows 客户端。主程序无 .NET / WebView 依赖；UI、配置、订阅、协议参数生成、核心进程管理和 Windows 系统代理控制均由 Rust 实现。网络协议交给项目现有的固定版本 Xray/sing-box；进程过滤复用 C++ Redirector 和签名驱动。

## 使用

解压完整便携包，运行 `NPGA-Rust.exe`。首次启动没有预置节点、延迟或虚构流量。点击「导入节点」，粘贴分享链接；也可在「我的节点」手动添加，或到「订阅管理」添加并更新订阅。

「高级设置 → 导入旧版 settings.json」支持原版 PascalCase 字段及同一 Windows 用户的 DPAPI 凭据。原文件保留，新配置存储在 `data/rust/settings.json`；原版未使用到的字段仍保存在 JSON 中。损坏配置不会静默清空；写入使用临时文件、落盘和原子替换，并保留有效备份及损坏文件副本。

支持连接 SOCKS5、SS（无 SIP003 插件）、VMess、VLESS（TLS / Reality / Vision）、Trojan、WireGuard、Hysteria2。SSR 节点可以导入、保留、编辑和分享，但固定的 Xray 不支持原生 SSR，连接会明确拒绝。旧 h2/quic/XTLS、证书指纹和插件配置会明确报不支持，详情见 MIGRATION.md。

| 加速方式 | 行为 |
| --- | --- |
| 本地代理 | 提供 SOCKS5 和 HTTP 端口，默认 9995 / 2802 |
| 系统代理 | 修改当前用户的 Windows 系统代理，停止时恢复原值；异常退出的恢复记录会在下次启动处理 |
| 进程加速 | 使用原版游戏/进程模式与 C++ Redirector，需要管理员权限及 netfilter2 驱动 |
| 全局 TUN | 使用 sing-box + Wintun，支持原版 CIDR 路由规则，需要管理员权限 |
| 网络共享 | 调用 pcap2socks，需要 Npcap 及正确的设备名 |

「以管理员重启」会调用 Windows UAC。进程过滤的驱动首次安装，在管理员 PowerShell 中运行 `./install-driver.ps1`；此脚本调用 Rust 安装入口，保留已安装的驱动。开发和测试过程不自动安装驱动。

关闭窗口默认收起到系统托盘；托盘可打开主界面、停止加速和退出。退出释放受管核心、过滤器、临时配置和系统代理。Windows Job 对象负责终止受管进程树；程序不会为了释放端口杀死其他应用。

当前实时流量统计来自 Redirector，仅进程模式有真实数据。其他模式显示「—」；延迟是 TCP 连通性，Hysteria2 使用 ICMP，不能等同于 UDP 隧道质量。

## 构建

构建需要 Windows x64、PowerShell 7、Rust 1.99.0 和 Visual C++ Build Tools / Windows SDK。工具链和依赖由 `rust-toolchain.toml`、`Cargo.lock` 固定。

在项目根目录：

```powershell
./build-rust.ps1
# 自定义现有核心目录
./build-rust.ps1 -RuntimePath D:/NPGA
```

默认查找本项目已构建的 `release-npga/NPGA` 或 `release-net10/Hatch`；复制其原生组件，不触发 .NET 构建。输出 `release-rust/NPGA-Rust/NPGA-Rust.exe`、便携 ZIP 和 SHA256 清单。打包严格排除用户配置、日志和临时核心配置。

在 `npga-rust` 目录中单独开发：

```powershell
cargo run -- --runtime-root D:/netch.plus/release-npga/NPGA --data-dir ./data
cargo test
$env:NPGA_RUNTIME_ROOT = 'D:/netch.plus/release-npga/NPGA'
cargo test -- --include-ignored --test-threads=1
cargo clippy --all-targets -- -D warnings
```

本机 Rust 工具链单独位于项目 `.build-cache/rust-toolchain`，没有修改系统 PATH。

CLI 支持 `--diagnostics`、`--runtime-root PATH`、`--data-dir PATH`、`--view connect|nodes|subscriptions|settings|logs|about`。`--screenshot PATH.png` 捕获原生窗口渲染后退出，供布局检查使用。

## 发布与授权

Release 开启优化、Thin LTO、单编译单元和符号剥离，生成原生 PE 文件。Rust 可提高逆向成本，无法保证代码或运行时凭据不可分析。不要把服务端密钥当作客户端编译常量保存。

沿用项目 GPL-3.0 许可及 Netch 来源说明。ToDesk 仅作为蓝白配色、侧栏导航和连接面板的视觉参考，没有使用其名称、商标或资源作为产品标识。
