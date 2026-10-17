[CmdletBinding(SupportsShouldProcess)]
param([switch]$Apply)

$ErrorActionPreference = 'Stop'
$workspaceRoot = [IO.Path]::GetFullPath($PSScriptRoot)
# Explicit allowlist: preserve release packages, native build inputs and downloads.
$relativeTargets = @(
    'npga-rust/target',
    '.ci-runtime', '.release-runtime',
    '.build-cache/publish-Release',
    '.build-cache/extract-pcap2socks-59b3a2e5054c',
    '.build-cache/extract-singBox-f580782c6dd1',
    '.build-cache/extract-wintun-34afe7d0de1f',
    '.build-cache/extract-xray-d004c39288ce'
)

$plans = foreach ($relative in $relativeTargets) {
    $target = [IO.Path]::GetFullPath((Join-Path $workspaceRoot $relative))
    if (-not $target.StartsWith($workspaceRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Unsafe cleanup target: $target"
    }
    if (-not (Test-Path -LiteralPath $target)) { continue }
    $entries = @(Get-Item -LiteralPath $target -Force) + @(Get-ChildItem -LiteralPath $target -Recurse -Force)
    if ($entries | Where-Object { $_.Attributes -band [IO.FileAttributes]::ReparsePoint }) {
        throw "Cleanup target contains a reparse point: $target"
    }
    $tracked = @(git -c "safe.directory=$workspaceRoot" -C $workspaceRoot ls-files -- $relative)
    if ($LASTEXITCODE -ne 0) { throw 'Unable to verify tracked files.' }
    if ($tracked.Count -gt 0) { throw "Cleanup target contains tracked files: $target" }
    $files = @($entries | Where-Object { -not $_.PSIsContainer })
    [pscustomobject]@{ Path=$target; Files=$files.Count; Bytes=($files | Measure-Object Length -Sum).Sum }
}

$plans | Select-Object Path, Files, @{Name='MiB';Expression={[math]::Round($_.Bytes / 1MB, 2)}} | Format-Table -AutoSize
if (-not $Apply) {
    Write-Host 'Preview only. Run .\clean.ps1 -Apply to remove these generated files.'
    return
}
foreach ($plan in $plans) {
    if ($PSCmdlet.ShouldProcess($plan.Path, 'Remove generated files')) {
        Remove-Item -LiteralPath $plan.Path -Recurse -Force
    }
}
