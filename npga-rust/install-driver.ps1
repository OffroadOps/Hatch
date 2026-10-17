# 遗留组件说明：本脚本安装 netfilter2.sys（未签名内核驱动），仅用于当前的进程模式。
# 按 refactor/DECISIONS.md ADR-002，进程模式计划改用 sing-box TUN 的 process_name 路由，
# 该驱动与 Redirector.bin 届时一并退役。验证通过前请勿删除本脚本。
[CmdletBinding()]
param([string]$RuntimeRoot = $PSScriptRoot)
$ErrorActionPreference = 'Stop'
$taskIdentity = [Security.Principal.WindowsIdentity]::GetCurrent()
$taskPrincipal = [Security.Principal.WindowsPrincipal]::new($taskIdentity)
if (-not $taskPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { throw 'Open PowerShell as administrator and run this script again.' }
$taskRuntime = (Resolve-Path -LiteralPath $RuntimeRoot).Path
$taskExecutable = Join-Path $PSScriptRoot 'NPGA-Rust.exe'
if (-not (Test-Path -LiteralPath $taskExecutable)) { throw 'Run the script from the NPGA-Rust portable package directory.' }
$taskProcess = Start-Process -FilePath $taskExecutable -ArgumentList @('--install-driver','--runtime-root',('"{0}"' -f $taskRuntime)) -WindowStyle Hidden -Wait -PassThru
if ($taskProcess.ExitCode -ne 0) { throw "Driver installation failed: exit $($taskProcess.ExitCode)" }
Write-Host 'netfilter2 driver registered. Start NPGA-Rust as administrator to use process acceleration.'
