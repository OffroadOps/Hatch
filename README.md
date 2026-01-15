# Hatch

A Windows game accelerator based on Netch, using Xray-core as the proxy engine.

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows-blue.svg)](https://github.com/OffroadOps/Hatch)

## Features

- Game-specific process proxy mode
- Multiple proxy protocols support (VLESS, VMess, Shadowsocks, Trojan, etc.)
- **VLESS + Reality protocol support** (New in 2.0.0)
- **Xray-core v26.1.13** as proxy engine
- TUN/TAP mode for full system proxy
- Subscription support
- Multi-language support (English, Chinese, Japanese)

## What's New in 2.0.0

### Major Changes
- Upgraded proxy engine from SagerNet/v2ray-core to **Xray-core**
- Added **VLESS Reality protocol** support
- Added **xtls-rprx-vision** flow support
- Simplified UI (removed Help menu and version display)
- Updated to .NET 8

### Reality Protocol Support
Now you can use VLESS servers with Reality protocol. The following parameters are supported:
- `pbk` - Reality Public Key
- `sid` - Reality Short ID  
- `fp` - Fingerprint (chrome, firefox, safari, etc.)
- `spx` - SpiderX
- `flow` - Flow control (xtls-rprx-vision)

## Requirements

- Windows 10/11 64-bit
- [.NET 8 Desktop Runtime](https://dotnet.microsoft.com/download/dotnet/8.0)
- Administrator privileges (for driver installation)

## Usage

1. Download the latest release
2. Extract all files to a folder
3. Run `Netch.exe` as Administrator
4. Add your proxy server (via subscription or manual input)
5. Select a mode and click Start

## Supported Protocols

- VLESS (with Reality, XTLS, TLS)
- VMess
- Shadowsocks
- ShadowsocksR
- Trojan
- Socks5
- WireGuard

## Modes

- **Process Mode**: Proxy specific game/application processes
- **TUN Mode**: System-wide proxy using TUN adapter
- **Global Mode**: Route all traffic through proxy

## Building from Source

### Requirements
- .NET 8 SDK
- Visual Studio 2022 (optional)

### Build
```powershell
# Clone the repository
git clone https://github.com/OffroadOps/Hatch.git
cd Hatch

# Build
dotnet publish -c Release -r win-x64 -p:Platform=x64 -p:SelfContained=true -p:PublishSingleFile=true Netch/Netch.csproj
```

## Credits

- [Netch](https://github.com/NetchX/Netch) - Original project
- [Xray-core](https://github.com/XTLS/Xray-core) - Proxy engine
- [WinDivert](https://github.com/basil00/WinDivert) - Network packet capture

## License

MIT License - See [LICENSE](LICENSE) for details.

## Disclaimer

This software is for educational and research purposes only. Users are responsible for complying with local laws and regulations.
