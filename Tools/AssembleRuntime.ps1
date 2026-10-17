#Requires -Version 7.0
<#
.SYNOPSIS
    Assembles a complete NPGA runtime tree for the Rust client without the
    frozen C# build chain.

.DESCRIPTION
    Combines three sources into one runtime root that build-rust.ps1 accepts
    via -RuntimePath:

      1. Version-pinned proxy cores from build.dependencies.json
         (Tools/FetchPinnedCores.ps1, SHA256-verified downloads).
      2. Shared assets tracked in Storage/ (nfdriver.sys, mode/).
      3. Audited native components tracked in runtime-native/
         (Redirector.bin, nfapi.dll; verified against runtime-native/SHA256SUMS.txt).

    Safety: only writes under -RuntimeRoot, never deletes the tree, and aborts
    on any hash mismatch. The frozen C# tree that used to produce this bundle
    moved to an external archive (refactor/DECISIONS.md ADR-007).

.PARAMETER RuntimeRoot
    Destination runtime root. Default: .release-runtime in the workspace root.

.PARAMETER Manifest
    Dependency manifest. Default: build.dependencies.json in the workspace root.

.PARAMETER NativeRoot
    Folder with the audited native components and SHA256SUMS.txt.
    Default: runtime-native in the workspace root.

.PARAMETER Force
    Re-download cores even when the state file records a verified hash.

.EXAMPLE
    pwsh -File .\Tools\AssembleRuntime.ps1 -RuntimeRoot .release-runtime
#>
[CmdletBinding()]
param(
    [string]$RuntimeRoot = '',
    [string]$Manifest = '',
    [string]$NativeRoot = '',
    [switch]$Force
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$workspace = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
if (-not $RuntimeRoot) { $RuntimeRoot = Join-Path $workspace '.release-runtime' }
if (-not $Manifest) { $Manifest = Join-Path $workspace 'build.dependencies.json' }
if (-not $NativeRoot) { $NativeRoot = Join-Path $workspace 'runtime-native' }
$RuntimeRoot = [IO.Path]::GetFullPath($RuntimeRoot)
$NativeRoot = [IO.Path]::GetFullPath($NativeRoot)

if (-not $NativeRoot.StartsWith($workspace + '\', [StringComparison]::OrdinalIgnoreCase)) {
    throw "NativeRoot must be a descendant of the workspace: $NativeRoot"
}
if (-not (Test-Path -LiteralPath (Join-Path $NativeRoot 'SHA256SUMS.txt'))) {
    throw "Native component checksum file not found: $NativeRoot\SHA256SUMS.txt"
}

# 1. Pinned proxy cores (xray, sing-box, wintun, pcap2socks, GeoLite2).
& (Join-Path $PSScriptRoot 'FetchPinnedCores.ps1') -RuntimeRoot $RuntimeRoot -Manifest $Manifest -Force:$Force

# 2. Shared assets from Storage/ (kept in the main repository for the Rust client).
Copy-Item -LiteralPath (Join-Path $workspace 'Storage\nfdriver.sys') -Destination (Join-Path $RuntimeRoot 'bin\nfdriver.sys') -Force
Copy-Item -LiteralPath (Join-Path $workspace 'Storage\mode') -Destination (Join-Path $RuntimeRoot 'mode') -Recurse -Force
Write-Host 'storage nfdriver.sys, mode/ copied'

# 3. Audited native components, verified against the checked-in checksum list.
foreach ($line in (Get-Content -LiteralPath (Join-Path $NativeRoot 'SHA256SUMS.txt'))) {
    if ($line -notmatch '^([0-9a-f]{64})\s+(.+)$') { continue }
    $expected = $Matches[1]
    $relative = $Matches[2].Trim() -replace '/', '\'
    $source = Join-Path $NativeRoot $relative
    if (-not (Test-Path -LiteralPath $source)) { throw "Native component missing: $source" }
    $actual = (Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -ne $expected) {
        throw "Native component SHA256 mismatch for ${relative}: expected $expected, got $actual"
    }
    Copy-Item -LiteralPath $source -Destination (Join-Path $RuntimeRoot $relative) -Force
    Write-Host "native $relative ok"
}

# 4. Verify the exact component list build-rust.ps1 requires.
$required = @(
    'bin\xray.exe', 'bin\sing-box.exe', 'bin\wintun.dll', 'bin\nfdriver.sys',
    'bin\pcap2socks.exe', 'Redirector.bin', 'nfapi.dll', 'mode'
)
foreach ($taskRequired in $required) {
    if (-not (Test-Path -LiteralPath (Join-Path $RuntimeRoot $taskRequired))) {
        throw "Runtime component missing after assembly: $taskRequired"
    }
}

Write-Host "Runtime ready: $RuntimeRoot"
