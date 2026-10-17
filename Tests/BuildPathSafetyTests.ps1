$ErrorActionPreference = 'Stop'
$workspace = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
. (Join-Path $workspace 'Tools/BuildPathSafety.ps1')
foreach ($relative in @('.', 'Hatch', '../', 'Hatch/bin', '.git', 'release/../Tests')) {
    $rejected = $false
    try { Assert-BuildDirectory $workspace ([IO.Path]::GetFullPath((Join-Path $workspace $relative))) -Output }
    catch { $rejected = $true }
    if (-not $rejected) { throw "Unsafe build output accepted: $relative" }
}
Assert-BuildDirectory $workspace (Join-Path $workspace 'release-net10') -Output
Write-Output 'Build path regression tests passed (6 unsafe paths rejected).'
