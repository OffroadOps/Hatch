# Hatch Changelog

## v2.0.0 (2026-01-15)

### Major Changes
- **Renamed project from Netch to Hatch**
- **Upgraded proxy engine from SagerNet/v2ray-core to Xray-core v26.1.13**
- **Added VLESS Reality protocol support**
  - Support for `pbk` (Public Key), `sid` (Short ID), `fp` (Fingerprint), `spx` (SpiderX)
  - Support for `xtls-rprx-vision` flow
- Updated to .NET 8 SDK
- Simplified UI
  - Removed Help menu
  - Removed About button
  - Removed version display in menu bar

### Technical Changes
- Modified `V2rayController.cs` to use `xray.exe` instead of `v2ray-sn.exe`
- Added `RealitySettings` class for Reality protocol configuration
- Updated `VLESSServer.cs` with Reality-related properties
- Updated `V2rayConfigUtils.cs` to generate Reality configuration
- Updated `V2rayUtils.cs` to parse Reality URI parameters
- Changed default STUN server to `stun.miwifi.com`

### Fixed
- Fixed NuGet package compatibility issues with .NET 8
- Fixed `RouteHelper.cs` ADDRESS_FAMILY type conversion
- Fixed `PortHelper.cs` ReadOnlyItemRef access

---

Based on [Netch](https://github.com/NetchX/Netch) v1.9.7
