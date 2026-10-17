[CmdletBinding()]
param(
    [string]$ToolRoot = 'D:\DevTools',
    [Parameter(Mandatory)][string]$ResultPath
)

$ErrorActionPreference = 'Stop'
try {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = [Security.Principal.WindowsPrincipal]::new($identity)
    if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        throw 'Administrator permission is required to modify the system PATH.'
    }
    $toolRootFull = [IO.Path]::GetFullPath($ToolRoot)
    $goBin = Join-Path $toolRootFull 'go\bin'
    $gccBin = Join-Path $toolRootFull 'w64devkit\bin'
    foreach ($executable in @((Join-Path $goBin 'go.exe'), (Join-Path $gccBin 'gcc.exe'))) {
        if (-not (Test-Path -LiteralPath $executable -PathType Leaf)) { throw "Tool not found: $executable" }
    }
    $original = [Environment]::GetEnvironmentVariable('Path', 'Machine')
    [pscustomobject]@{Path=$original;SavedAt=[DateTimeOffset]::Now.ToString('o')} |
        ConvertTo-Json | Set-Content -LiteralPath ($ResultPath + '.before.json') -Encoding utf8
    $entries = @($original -split ';' | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
    foreach ($entry in @($goBin, $gccBin)) {
        if (-not ($entries | Where-Object { $_.TrimEnd('\') -ieq $entry.TrimEnd('\') })) { $entries += $entry }
    }
    [Environment]::SetEnvironmentVariable('Path', ($entries -join ';'), 'Machine')
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class ToolchainEnvironmentBroadcast {
    [DllImport("user32.dll", CharSet=CharSet.Unicode, SetLastError=true)]
    public static extern IntPtr SendMessageTimeout(IntPtr hwnd, uint message, UIntPtr wparam,
        string lparam, uint flags, uint timeout, out UIntPtr result);
}
'@
    $broadcastResult = [UIntPtr]::Zero
    [void][ToolchainEnvironmentBroadcast]::SendMessageTimeout([IntPtr]0xffff, 0x1a, [UIntPtr]::Zero, 'Environment', 2, 5000, [ref]$broadcastResult)
    [pscustomobject]@{Success=$true;AddedPaths=@($goBin,$gccBin)} |
        ConvertTo-Json | Set-Content -LiteralPath $ResultPath -Encoding utf8
}
catch {
    [pscustomobject]@{Success=$false;Error=$_.Exception.Message} |
        ConvertTo-Json | Set-Content -LiteralPath $ResultPath -Encoding utf8
    exit 1
}
