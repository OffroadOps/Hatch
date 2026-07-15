# Changelog

All notable changes to Hatch will be documented in this file.

## [2.0.0] - 2026-07-16

### Release hardening
- Connected the Hatch source tree to the existing Git history and automated Windows build/release workflows.
- Added a reproducible root `build.ps1` that pins external dependency versions and verifies every download with SHA256.
- Added `GeoLite2-Country.mmdb` to release packaging so IP country lookup no longer fails silently because of a missing database.
- Added real update-checker tests and a project reference from `Tests` to `Hatch`.
- Normalized legacy GB18030 C# sources to UTF-8 BOM and repaired the update checksum marker and copyright metadata.
- Made the updater accept `v`-prefixed GitHub tags, reject empty release lists cleanly, and parse only valid SHA256 table rows.
- Moved the Inno Setup definition into the source tree and consolidated the three legacy publish scripts into one build entry point.
- Corrected legacy Netch branding and obsolete dependency guidance in the Simplified Chinese, Traditional Chinese, and Japanese translations.
- Documented that current binaries are unsigned and may trigger SmartScreen or antivirus warnings.

### Added
- **About Dialog**: New "About" menu item in the main menu bar
  - Display Hatch version (v2.0.0)
  - Display Xray-core version with auto-detection
  - Display sing-box version with auto-detection
  - Show installation status for each core (green for installed, red for missing)

- **Update Features**:
  - Check for Hatch software updates from GitHub
  - One-click update for Xray-core
  - One-click update for sing-box
  - Automatic version detection after updates

- **Project Attribution**:
  - Tribute to Netch project: "🥚 Hatched from Netch - Without Netch, no Hatch"
  - Links to GitHub repositories (Hatch and Netch)

- **Core Management**:
  - Version-pinned core dependency manifest (`build.dependencies.json`)
  - SHA256 verification for build-time downloads
  - Version constant in `Constants.cs`

### Changed
- Menu structure: Added "About" before "Exit"
- Enhanced core file management

### Fixed
- sing-box.exe placement in correct directories
- Core version detection and display

### Technical Details
- New files:
  - `Hatch/Forms/AboutForm.cs`
  - `Hatch/Forms/AboutForm.Designer.cs`
  - `build.dependencies.json`
- Modified files:
  - `Hatch/Forms/MainForm.cs`
  - `Hatch/Forms/MainForm.Designer.cs`
  - `Hatch/Constants.cs`

---

## [1.9.7] - Base Version

### Initial Features
- Forked from Netch 1.9.7
- Support for multiple proxy protocols
- Process mode and TUN/TAP mode
- Real-time bandwidth monitoring
- Multi-language support

---

**Note**: This project is a continuation of Netch 1.9.7 with enhancements and new features.

**Repository**: https://github.com/OffroadOps/Hatch
**Original Project**: https://github.com/netchx/netch
