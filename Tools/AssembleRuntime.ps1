#Requires -Version 7.0
<#
.SYNOPSIS
    Assembles a complete NPGA runtime tree for the Rust client without the
    frozen C# build chain.

.DESCRIPTION
    Combines two sources into one runtime root that build-rust.ps1 accepts
    via -RuntimePath:

      1. Version-pinned proxy cores from build.dependencies.json
         (Tools/FetchPinnedCores.ps1, SHA256-verified downloads).
      2. Shared assets tracked in Storage/ (mode/).

    Safety: only writes under -RuntimeRoot, never deletes the tree, and aborts
    on any hash mismatch. The frozen C# tree that used to produce this bundle
    moved to an external archive (refactor/DECISIONS.md ADR-007).

.PARAMETER RuntimeRoot
    Destination runtime root. Default: .release-runtime in the workspace root.

.PARAMETER Manifest
    Dependency manifest. Default: build.dependencies.json in the workspace root.

.PARAMETER Force
    Re-download cores even when the state file records a verified hash.

.EXAMPLE
    pwsh -File .\Tools\AssembleRuntime.ps1 -RuntimeRoot .release-runtime
#>
[CmdletBinding()]
param(
    [string]$RuntimeRoot = '',
    [string]$Manifest = '',
    [switch]$Force
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$workspace = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
if (-not $RuntimeRoot) { $RuntimeRoot = Join-Path $workspace '.release-runtime' }
if (-not $Manifest) { $Manifest = Join-Path $workspace 'build.dependencies.json' }
$RuntimeRoot = [IO.Path]::GetFullPath($RuntimeRoot)

# 1. Pinned proxy cores (xray, sing-box, wintun, pcap2socks, GeoLite2).
& (Join-Path $PSScriptRoot 'FetchPinnedCores.ps1') -RuntimeRoot $RuntimeRoot -Manifest $Manifest -Force:$Force

# 2. Shared assets from Storage/ (kept in the main repository for the Rust client).
Copy-Item -LiteralPath (Join-Path $workspace 'Storage\mode') -Destination (Join-Path $RuntimeRoot 'mode') -Recurse -Force
Write-Host 'storage mode/ copied'

# 3. Verify the exact component list build-rust.ps1 requires.
$required = @(
    'bin\xray.exe', 'bin\sing-box.exe', 'bin\wintun.dll',
    'bin\pcap2socks.exe', 'mode'
)
foreach ($taskRequired in $required) {
    if (-not (Test-Path -LiteralPath (Join-Path $RuntimeRoot $taskRequired))) {
        throw "Runtime component missing after assembly: $taskRequired"
    }
}

Write-Host "Runtime ready: $RuntimeRoot"
