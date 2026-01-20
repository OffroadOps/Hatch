# Hatch

A lightweight and powerful network proxy tool for Windows, forked from Netch with Xray-core integration.

## Features

- 🚀 **High Performance**: Built with .NET 10.0 and optimized for speed
- 🔒 **Multiple Protocols**: Support for Hysteria2, Shadowsocks, VMess, VLESS, Trojan, WireGuard, and more
- 🎯 **Process Mode**: Route specific applications through proxy
- 🌐 **TUN/TAP Mode**: System-wide proxy with advanced routing
- 📊 **Real-time Monitoring**: Live bandwidth and latency display
- 🌍 **Multi-language**: Support for English, Chinese (Simplified & Traditional), and Japanese
- ⚡ **Optimized Build**: 56.8% smaller than standard builds while maintaining full functionality

## System Requirements

- Windows 7 or later (x64)
- .NET 10.0 Runtime (for framework-dependent version) or use the self-contained version

## Installation

1. Download the latest release from [Releases](https://github.com/OffroadOps/Hatch/releases)
2. Extract all files to a folder
3. Run `Hatch.exe`

## Quick Start

1. **Add Server**: Click the server dropdown → Add server → Select protocol
2. **Select Mode**: Choose a mode from the mode dropdown (e.g., "Bypass LAN")
3. **Start**: Click the "Start" button to connect

## Key Improvements

### UI Enhancements
- Streamlined interface with hidden configuration sections
- Color-coded latency display (Green <80ms, Orange 80-200ms, Red >200ms)
- Real-time speed test with IP geolocation
- Automatic delay testing on startup

### Protocol Support
- **Hysteria2**: Optimized with ICMP ping for accurate latency measurement
- **Xray-core**: Latest version integrated for enhanced performance

### Build Optimization
- Single-file deployment with internal compression
- 56.8% size reduction (from 166.55 MB to 72 MB)
- Full multi-language support maintained
- No antivirus false positives

## Configuration

### Server Configuration
Servers are stored in `data/servers.json`. You can:
- Import from clipboard (subscription links supported)
- Add manually through the UI
- Test latency with one click

### Mode Configuration
Modes define routing rules and are stored in the `mode/` folder:
- **Process Mode**: Route specific applications
- **TUN/TAP Mode**: System-wide routing with custom rules
- **Bypass Mode**: Exclude local and China IPs

## Building from Source

### Prerequisites
- .NET 10.0 SDK
- Visual Studio 2022 or later (optional)

### Build Commands

```powershell
# Framework-dependent build (requires .NET runtime)
dotnet build -c Release

# Self-contained optimized build
cd Hatch-main
.\publish-optimized.ps1
```

The optimized build applies:
- ✅ PublishSingleFile - Single executable
- ✅ EnableCompressionInSingleFile - Internal compression
- ✅ Debug symbols removed
- ❌ PublishTrimmed - Not supported for Windows Forms

## Project Structure

```
Hatch-main/
├── Netch/              # Main application source
├── Redirector/         # Network redirector (C++)
├── RouteHelper/        # Routing utilities
├── Storage/            # Default configurations
├── Other/              # Additional tools (aiodns, pcap2socks, xray)
└── publish-trimmed/    # Optimized build output
```

## License

GPL-3.0 License - see [LICENSE](LICENSE) for details

## Credits

- Original project: [Netch](https://github.com/netchx/netch)
- Xray-core: [Xray-project](https://github.com/XTLS/Xray-core)
- Hysteria2: [Hysteria](https://github.com/apernet/hysteria)

## Contributing

Contributions are welcome! Please feel free to submit issues and pull requests.

## Disclaimer

This tool is for educational and research purposes only. Users are responsible for complying with local laws and regulations.

---

**Version**: 2.0.0  
**Copyright**: © 2026 OffroadOps
