#Requires -Version 7.0
[CmdletBinding()]
param(
    [string]$RuntimePath = '',
    [string]$OutputPath = 'release-rust',
    [switch]$SkipTests,
    [switch]$SkipArchive
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$taskWorkspace = [IO.Path]::GetFullPath($PSScriptRoot)
$taskCrate = Join-Path $taskWorkspace 'npga-rust'
$taskCargoCommand = Get-Command cargo.exe -ErrorAction SilentlyContinue
if ($taskCargoCommand) { $taskCargo = $taskCargoCommand.Source }
else {
    $taskLocalTools = Join-Path $taskWorkspace '.build-cache\rust-toolchain'
    $taskCargo = Join-Path $taskLocalTools 'cargo\bin\cargo.exe'
    if (Test-Path -LiteralPath $taskCargo) {
        $env:CARGO_HOME = Join-Path $taskLocalTools 'cargo'
        $env:RUSTUP_HOME = Join-Path $taskLocalTools 'rustup'
    }
    else {
        # rustup's default install location; the PATH of build shells often lacks it.
        $taskUserCargo = Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe'
        if (-not (Test-Path -LiteralPath $taskUserCargo)) { throw 'Install Rust 1.99.0 with rustup and Visual C++ Build Tools, then rerun build-rust.ps1.' }
        $taskCargo = $taskUserCargo
    }
}
if (-not $RuntimePath) {
    foreach ($taskCandidate in @('.release-runtime','release-npga\NPGA','release-net10\Hatch','release-fixed\Hatch','release\Hatch')) {
        $taskCandidatePath = Join-Path $taskWorkspace $taskCandidate
        if (Test-Path -LiteralPath (Join-Path $taskCandidatePath 'bin\xray.exe')) { $RuntimePath = $taskCandidatePath; break }
    }
}
if (-not $RuntimePath) { throw 'Pass -RuntimePath pointing to an existing runtime (assemble one with Tools/AssembleRuntime.ps1) or an NPGA package with audited native libraries and pinned proxy cores.' }
$taskRuntime = (Resolve-Path -LiteralPath $RuntimePath).Path
foreach ($taskRequired in @('bin\xray.exe','bin\sing-box.exe','bin\wintun.dll','bin\pcap2socks.exe','mode')) {
    if (-not (Test-Path -LiteralPath (Join-Path $taskRuntime $taskRequired))) { throw "Runtime component missing: $taskRequired" }
}

# Never delete/recreate an output tree. Reject source directories, ancestors and junctions.
$taskOutput = [IO.Path]::GetFullPath((Join-Path $taskWorkspace $OutputPath))
if (-not $taskOutput.StartsWith($taskWorkspace + '\',[StringComparison]::OrdinalIgnoreCase)) { throw 'OutputPath must be a descendant of this workspace.' }
$taskRelativeOutput = [IO.Path]::GetRelativePath($taskWorkspace,$taskOutput)
if ($taskRelativeOutput.Split('\')[0] -notmatch '^release-rust(?:-[A-Za-z0-9_-]+)?$') { throw 'OutputPath must start with release-rust or release-rust-NAME.' }
$taskCheckPath = $taskOutput
while ($taskCheckPath.Length -gt $taskWorkspace.Length) {
    if (Test-Path -LiteralPath $taskCheckPath) {
        $taskInfo = Get-Item -LiteralPath $taskCheckPath -Force
        if ($taskInfo.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "Output path is a reparse point: $taskCheckPath" }
    }
    $taskCheckPath = [IO.Path]::GetDirectoryName($taskCheckPath)
}

$env:NPGA_RUNTIME_ROOT = $taskRuntime
Push-Location $taskCrate
try {
    if (-not $SkipTests) {
        & $taskCargo test --locked -- --include-ignored --test-threads=1
        if ($LASTEXITCODE -ne 0) { throw 'Rust tests failed.' }
        & $taskCargo clippy --locked --all-targets -- -D warnings
        if ($LASTEXITCODE -ne 0) { throw 'Rust lint checks failed.' }
        & $taskCargo fmt --check
        if ($LASTEXITCODE -ne 0) { throw 'Rust formatting check failed.' }
    }
    & $taskCargo build --release --locked
    if ($LASTEXITCODE -ne 0) { throw 'Rust release build failed.' }
} finally { Pop-Location }

$taskBundle = Join-Path $taskOutput 'NPGA-Rust'
if (Test-Path -LiteralPath $taskBundle) {
    $taskExisting = @(Get-Item -LiteralPath $taskBundle -Force) + @(Get-ChildItem -LiteralPath $taskBundle -Recurse -Force)
    if ($taskExisting | Where-Object { $_.Attributes -band [IO.FileAttributes]::ReparsePoint }) { throw 'Existing package contains a reparse point; use a fresh output path.' }
}
New-Item -ItemType Directory -Path (Join-Path $taskBundle 'bin') -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $taskCrate 'target\release\NPGA-Rust.exe') -Destination $taskBundle -Force
foreach ($taskName in @('xray.exe','sing-box.exe','wintun.dll','pcap2socks.exe','GeoLite2-Country.mmdb')) {
    Copy-Item -LiteralPath (Join-Path $taskRuntime "bin\$taskName") -Destination (Join-Path $taskBundle 'bin') -Force
}
Copy-Item -LiteralPath (Join-Path $taskRuntime 'mode') -Destination $taskBundle -Recurse -Force
Copy-Item -LiteralPath (Join-Path $taskWorkspace 'LICENSE') -Destination $taskBundle -Force
foreach ($taskName in @('README.md','MIGRATION.md')) { Copy-Item -LiteralPath (Join-Path $taskCrate $taskName) -Destination $taskBundle -Force }

# Package a whitelist; never include existing settings, credentials, logs or runtime configs.
$taskWhitelist = @('NPGA-Rust.exe','LICENSE','README.md','MIGRATION.md','bin','mode')
$taskFiles = foreach ($taskName in $taskWhitelist) {
    $taskPath = Join-Path $taskBundle $taskName
    if ((Get-Item -LiteralPath $taskPath).PSIsContainer) { Get-ChildItem -LiteralPath $taskPath -File -Recurse }
    else { Get-Item -LiteralPath $taskPath }
}
$taskChecksums = foreach ($taskFile in $taskFiles | Sort-Object FullName) {
    $taskRelative = [IO.Path]::GetRelativePath($taskBundle,$taskFile.FullName).Replace('\','/')
    '{0}  {1}' -f (Get-FileHash -LiteralPath $taskFile.FullName -Algorithm SHA256).Hash.ToLowerInvariant(),$taskRelative
}
$taskChecksums | Set-Content -LiteralPath (Join-Path $taskBundle 'SHA256SUMS.txt') -Encoding utf8
$taskFiles += Get-Item -LiteralPath (Join-Path $taskBundle 'SHA256SUMS.txt')
if (-not $SkipArchive) {
    Add-Type -AssemblyName System.IO.Compression
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $taskZipPath = Join-Path $taskOutput 'NPGA-Rust.zip'
    $taskStream = [IO.File]::Open($taskZipPath,[IO.FileMode]::Create,[IO.FileAccess]::Write)
    $taskArchive = [IO.Compression.ZipArchive]::new($taskStream,[IO.Compression.ZipArchiveMode]::Create)
    try {
        foreach ($taskFile in $taskFiles | Sort-Object FullName) {
            $taskRelative = [IO.Path]::GetRelativePath($taskBundle,$taskFile.FullName).Replace('\','/')
            [IO.Compression.ZipFileExtensions]::CreateEntryFromFile($taskArchive,$taskFile.FullName,"NPGA-Rust/$taskRelative",[IO.Compression.CompressionLevel]::Optimal) | Out-Null
        }
    } finally { $taskArchive.Dispose(); $taskStream.Dispose() }
    $taskZipHash = (Get-FileHash -LiteralPath $taskZipPath -Algorithm SHA256).Hash.ToLowerInvariant()
    "$taskZipHash  NPGA-Rust.zip" | Set-Content -LiteralPath (Join-Path $taskOutput 'SHA256SUMS.txt') -Encoding utf8
}
Write-Host "Rust package: $taskBundle"
Write-Host "Executable: $(Join-Path $taskBundle 'NPGA-Rust.exe')"
