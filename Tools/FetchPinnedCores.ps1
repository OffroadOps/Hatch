#Requires -Version 7.0
<#
.SYNOPSIS
    Downloads the version-pinned proxy cores listed in build.dependencies.json
    into a runtime root, verifying every download against its SHA256.

.DESCRIPTION
    Produces a minimal runtime tree (bin/xray.exe, bin/sing-box.exe,
    bin/wintun.dll, bin/GeoLite2-Country.mmdb) without building the frozen C#
    client. Used by CI to run the real-core integration tests, and available
    locally to assemble a runtime for the Rust client.

    Safety: only creates files under -RuntimeRoot, never deletes an existing
    tree, rejects reparse points, and aborts on hash mismatch. A state file
    records verified hashes so repeat runs skip already-verified downloads.

.PARAMETER RuntimeRoot
    Destination runtime root. Default: .ci-runtime in the workspace root.

.PARAMETER Manifest
    Dependency manifest. Default: build.dependencies.json in the workspace root.

.PARAMETER Force
    Re-download even when the state file records a verified hash.

.EXAMPLE
    pwsh -File .\Tools\FetchPinnedCores.ps1 -RuntimeRoot .ci-runtime
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
if (-not $RuntimeRoot) { $RuntimeRoot = Join-Path $workspace '.ci-runtime' }
if (-not $Manifest) { $Manifest = Join-Path $workspace 'build.dependencies.json' }
$RuntimeRoot = [IO.Path]::GetFullPath($RuntimeRoot)

if (-not (Test-Path -LiteralPath $Manifest)) { throw "Manifest not found: $Manifest" }
if (-not (Test-Path -LiteralPath $RuntimeRoot)) {
    New-Item -ItemType Directory -Path $RuntimeRoot -Force | Out-Null
} else {
    $existing = Get-Item -LiteralPath $RuntimeRoot -Force
    if ($existing.Attributes -band [IO.FileAttributes]::ReparsePoint) {
        throw "RuntimeRoot is a reparse point: $RuntimeRoot"
    }
}

$statePath = Join-Path $RuntimeRoot '.pinned-cores.json'
$state = @{}
if (Test-Path -LiteralPath $statePath) {
    $loaded = Get-Content -LiteralPath $statePath -Raw | ConvertFrom-Json -AsHashtable
    if ($loaded) { $state = $loaded }
}

$dependencies = (Get-Content -LiteralPath $Manifest -Raw | ConvertFrom-Json -AsHashtable)
$downloadRoot = Join-Path ([IO.Path]::GetTempPath()) ('npga-cores-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $downloadRoot -Force | Out-Null

function Get-Sha256([string]$Path) {
    (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

try {
    foreach ($name in ($dependencies.Keys | Sort-Object)) {
        $entry = $dependencies[$name]
        $url = [string]$entry['url']
        $expected = ([string]$entry['sha256']).ToLowerInvariant()
        $destination = [string]$entry['destination']
        $archivePath = $entry['archivePath']
        $archiveEntry = $entry['archiveEntry']
        if (-not $url -or -not $expected -or -not $destination) {
            throw "Manifest entry '$name' is missing url/sha256/destination"
        }
        $target = Join-Path $RuntimeRoot ($destination -replace '/', '\')
        $targetDir = [IO.Path]::GetDirectoryName($target)
        if (-not (Test-Path -LiteralPath $targetDir)) {
            New-Item -ItemType Directory -Path $targetDir -Force | Out-Null
        }
        if (-not $Force -and $state.ContainsKey($name) -and $state[$name] -eq $expected -and
            (Test-Path -LiteralPath $target)) {
            Write-Host "skip  $name (verified $($expected.Substring(0,12))…)"
            continue
        }

        $fileName = [IO.Path]::GetFileName(([Uri]$url).AbsolutePath)
        $download = Join-Path $downloadRoot $fileName
        Write-Host "fetch $name <- $url"
        Invoke-WebRequest -Uri $url -OutFile $download -UseBasicParsing
        $actual = Get-Sha256 $download
        if ($actual -ne $expected) {
            throw "$name SHA256 mismatch: expected $expected, got $actual"
        }

        if ($archivePath -or $archiveEntry) {
            $extract = Join-Path $downloadRoot ($name + '-extract')
            Expand-Archive -LiteralPath $download -DestinationPath $extract -Force
            if ($archivePath) {
                # Exact path inside the archive.
                $source = Join-Path $extract ([string]$archivePath -replace '/', '\')
            } else {
                # Search by file name; the archive layout may add a versioned
                # top-level directory (same semantics as build.ps1).
                $matches = @(Get-ChildItem -LiteralPath $extract -Recurse -File -Filter ([string]$archiveEntry))
                if ($matches.Count -ne 1) {
                    throw "${name}: expected exactly one '$archiveEntry' in $fileName, found $($matches.Count)"
                }
                $source = $matches[0].FullName
            }
            if (-not (Test-Path -LiteralPath $source)) {
                throw "${name}: '$archivePath$archiveEntry' not found inside $fileName"
            }
            Copy-Item -LiteralPath $source -Destination $target -Force
        } else {
            Copy-Item -LiteralPath $download -Destination $target -Force
        }
        $state[$name] = $expected
        Write-Host "ok    $name -> $destination"
    }
} finally {
    if (Test-Path -LiteralPath $downloadRoot) {
        Remove-Item -LiteralPath $downloadRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}

($state | ConvertTo-Json -Depth 4) | Set-Content -LiteralPath $statePath -Encoding utf8
Write-Host "Pinned cores ready in $RuntimeRoot"
