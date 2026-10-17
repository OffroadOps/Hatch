#Requires -Version 7.0
function Assert-BuildDirectory([string]$Workspace, [string]$Target, [switch]$Output) {
    $workspaceFull = [IO.Path]::GetFullPath($Workspace).TrimEnd('\', '/')
    $targetFull = [IO.Path]::GetFullPath($Target).TrimEnd('\', '/')
    if (-not $targetFull.StartsWith($workspaceFull + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Build directory must be inside the workspace: $targetFull"
    }
    if ($Output) {
        $relative = [IO.Path]::GetRelativePath($workspaceFull, $targetFull)
        # Build owns only these top-level output directories, never arbitrary paths.
        if ($relative -notmatch '^(release|out)(-[a-zA-Z0-9_-]+)?$') {
            throw "Unsafe OutputPath '$relative'. Use release, out, or release-<name>."
        }
        $tracked = @(git -c "safe.directory=$workspaceFull" -C $workspaceFull ls-files -- $relative)
        if ($LASTEXITCODE -ne 0 -or $tracked.Count -gt 0) {
            throw "Cannot delete an output directory containing tracked files: $targetFull"
        }
    }
    else {
        $cacheRoot = Join-Path $workspaceFull '.build-cache'
        if (-not $targetFull.StartsWith($cacheRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
            throw "Temporary build directory must be inside .build-cache: $targetFull"
        }
    }
    # Validate parents and the full existing subtree before recursive deletion.
    $ancestor = $targetFull
    while ($ancestor -and $ancestor -ne $workspaceFull) {
        if (Test-Path -LiteralPath $ancestor) {
            if ((Get-Item -LiteralPath $ancestor -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) {
                throw "Build directory contains a reparse point: $ancestor"
            }
        }
        $ancestor = [IO.Path]::GetDirectoryName($ancestor)
    }
    if (Test-Path -LiteralPath $targetFull) {
        if (Get-ChildItem -LiteralPath $targetFull -Recurse -Force | Where-Object { $_.Attributes -band [IO.FileAttributes]::ReparsePoint }) {
            throw "Build directory contains a reparse point: $targetFull"
        }
    }
}
