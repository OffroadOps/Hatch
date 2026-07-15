[CmdletBinding()]
param(
    [ValidateSet('Debug', 'Release')]
    [string]$Configuration = 'Release',

    [string]$OutputPath = 'release',

    [bool]$SelfContained = $true,

    [switch]$SkipNativeBuild,

    [switch]$SkipTests,

    [switch]$SkipInstaller
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$root = Split-Path -Parent $MyInvocation.MyCommand.Path
$outputRoot = [IO.Path]::GetFullPath((Join-Path $root $OutputPath))
$appDir = Join-Path $outputRoot 'Hatch'
$cacheDir = Join-Path $root '.build-cache'
$publishDir = Join-Path $cacheDir "publish-$Configuration"

function Resolve-DotNet {
    $command = Get-Command dotnet -ErrorAction SilentlyContinue
    if ($command) { return $command.Source }

    $localDotNet = Join-Path $env:LOCALAPPDATA 'Microsoft\dotnet\dotnet.exe'
    if (Test-Path -LiteralPath $localDotNet) { return $localDotNet }

    throw 'The .NET SDK required by global.json is not installed. Install .NET SDK 8.0.100.'
}

function Resolve-MSBuild {
    $command = Get-Command msbuild -ErrorAction SilentlyContinue
    if ($command) { return $command.Source }

    $vsWhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (Test-Path -LiteralPath $vsWhere) {
        $installation = & $vsWhere -latest -products * -requires Microsoft.Component.MSBuild -property installationPath
        if ($installation) {
            $candidate = Join-Path $installation 'MSBuild\Current\Bin\MSBuild.exe'
            if (Test-Path -LiteralPath $candidate) { return $candidate }
        }
    }

    throw 'MSBuild with the Visual C++ build tools was not found. Install Visual Studio 2022 Build Tools with the Desktop development with C++ workload.'
}

function Get-VerifiedDownload([string]$Name, $Spec) {
    $extension = [IO.Path]::GetExtension(([Uri]$Spec.url).AbsolutePath)
    if ([string]::IsNullOrWhiteSpace($extension)) { $extension = '.bin' }
    $path = Join-Path $cacheDir "$Name$extension"

    if (Test-Path -LiteralPath $path) {
        $actual = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($actual -ne $Spec.sha256) {
            Remove-Item -LiteralPath $path -Force
        }
    }

    if (-not (Test-Path -LiteralPath $path)) {
        Write-Host "Downloading pinned dependency $Name $($Spec.version)..."
        Invoke-WebRequest -Uri $Spec.url -OutFile $path -UseBasicParsing
    }

    $hash = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($hash -ne $Spec.sha256) {
        throw "SHA256 mismatch for $Name. Expected $($Spec.sha256), got $hash."
    }

    return $path
}

function Install-PinnedDependency([string]$Name, $Spec) {
    $download = Get-VerifiedDownload $Name $Spec
    $destination = Join-Path $appDir ($Spec.destination -replace '/', '\')
    New-Item -ItemType Directory -Force (Split-Path -Parent $destination) | Out-Null

    if ($Spec.PSObject.Properties.Name -contains 'archiveEntry' -or
        $Spec.PSObject.Properties.Name -contains 'archivePath') {
        $extractDir = Join-Path $cacheDir "extract-$Name-$($Spec.sha256.Substring(0, 12))"
        if (-not (Test-Path -LiteralPath $extractDir)) {
            New-Item -ItemType Directory -Force $extractDir | Out-Null
            Expand-Archive -LiteralPath $download -DestinationPath $extractDir -Force
        }

        if ($Spec.PSObject.Properties.Name -contains 'archivePath') {
            $source = Join-Path $extractDir ($Spec.archivePath -replace '/', '\')
        }
        else {
            $matches = @(Get-ChildItem -LiteralPath $extractDir -Recurse -File -Filter $Spec.archiveEntry)
            if ($matches.Count -ne 1) {
                throw "Expected one $($Spec.archiveEntry) in $Name archive, found $($matches.Count)."
            }
            $source = $matches[0].FullName
        }

        if (-not (Test-Path -LiteralPath $source)) {
            throw "Pinned dependency entry not found: $source"
        }
        Copy-Item -LiteralPath $source -Destination $destination -Force
    }
    else {
        Copy-Item -LiteralPath $download -Destination $destination -Force
    }
}

function Copy-RequiredFile([string]$Source, [string]$Destination) {
    if (-not (Test-Path -LiteralPath $Source)) {
        throw "Required build input is missing: $Source"
    }
    New-Item -ItemType Directory -Force (Split-Path -Parent $Destination) | Out-Null
    Copy-Item -LiteralPath $Source -Destination $Destination -Force
}

Push-Location $root
try {
    $dotnet = Resolve-DotNet
    New-Item -ItemType Directory -Force $cacheDir | Out-Null

    if (Test-Path -LiteralPath $outputRoot) {
        Remove-Item -LiteralPath $outputRoot -Recurse -Force
    }
    if (Test-Path -LiteralPath $publishDir) {
        Remove-Item -LiteralPath $publishDir -Recurse -Force
    }
    New-Item -ItemType Directory -Force $appDir, (Join-Path $appDir 'bin'), $publishDir | Out-Null

    Write-Host 'Restoring locked NuGet dependencies...'
    & $dotnet restore '.\Hatch\Hatch.csproj' -p:Configuration=$Configuration -r win-x64 --locked-mode
    if ($LASTEXITCODE -ne 0) { throw "dotnet restore failed with exit code $LASTEXITCODE" }

    if (-not $SkipTests) {
        & $dotnet restore '.\Tests\Tests.csproj' -p:Configuration=$Configuration -r win-x64 --locked-mode
        if ($LASTEXITCODE -ne 0) { throw "test restore failed with exit code $LASTEXITCODE" }
        & $dotnet test '.\Tests\Tests.csproj' -c $Configuration -p:Platform=x64 --no-restore
        if ($LASTEXITCODE -ne 0) { throw "Tests failed with exit code $LASTEXITCODE" }
    }

    if ($SkipNativeBuild) {
        Write-Host 'Using audited prebuilt native files from artifacts for this local build.'
        $prebuilt = Join-Path $root 'artifacts\Hatch'
        Copy-RequiredFile (Join-Path $prebuilt 'Redirector.bin') (Join-Path $appDir 'Redirector.bin')
        Copy-RequiredFile (Join-Path $prebuilt 'RouteHelper.bin') (Join-Path $appDir 'RouteHelper.bin')
        Copy-RequiredFile (Join-Path $prebuilt 'nfapi.dll') (Join-Path $appDir 'nfapi.dll')
        Copy-RequiredFile (Join-Path $prebuilt 'bin\aiodns.bin') (Join-Path $appDir 'bin\aiodns.bin')
    }
    else {
        $msbuild = Resolve-MSBuild
        & $msbuild '.\Redirector\Redirector.vcxproj' /p:Configuration=$Configuration /p:Platform=x64 /m /v:minimal /nologo
        if ($LASTEXITCODE -ne 0) { throw "Redirector build failed with exit code $LASTEXITCODE" }
        & $msbuild '.\RouteHelper\RouteHelper.vcxproj' /p:Configuration=$Configuration /p:Platform=x64 /m /v:minimal /nologo
        if ($LASTEXITCODE -ne 0) { throw "RouteHelper build failed with exit code $LASTEXITCODE" }

        Copy-RequiredFile (Join-Path $root "Redirector\bin\$Configuration\Redirector.bin") (Join-Path $appDir 'Redirector.bin')
        Copy-RequiredFile (Join-Path $root "Redirector\bin\$Configuration\nfapi.dll") (Join-Path $appDir 'nfapi.dll')
        Copy-RequiredFile (Join-Path $root "RouteHelper\bin\$Configuration\RouteHelper.bin") (Join-Path $appDir 'RouteHelper.bin')

        if (-not (Get-Command go -ErrorAction SilentlyContinue)) {
            throw 'Go is required to build aiodns.bin from source.'
        }
        $oldCgo = $env:CGO_ENABLED
        $oldGoOs = $env:GOOS
        $oldGoArch = $env:GOARCH
        try {
            $env:CGO_ENABLED = '1'
            $env:GOOS = 'windows'
            $env:GOARCH = 'amd64'
            Push-Location '.\Other\aiodns'
            & go build -trimpath -buildvcs=false -buildmode=c-shared -ldflags '-s -w' -o (Join-Path $appDir 'bin\aiodns.bin') .
            if ($LASTEXITCODE -ne 0) { throw "aiodns build failed with exit code $LASTEXITCODE" }
            Pop-Location
        }
        finally {
            $env:CGO_ENABLED = $oldCgo
            $env:GOOS = $oldGoOs
            $env:GOARCH = $oldGoArch
            if ((Get-Location).Path -ne $root) { Pop-Location }
        }
    }

    Write-Host 'Publishing Hatch...'
    & $dotnet publish '.\Hatch\Hatch.csproj' -c $Configuration -r win-x64 --self-contained $SelfContained -p:Platform=x64 -p:PublishSingleFile=true --no-restore -o $publishDir
    if ($LASTEXITCODE -ne 0) { throw "dotnet publish failed with exit code $LASTEXITCODE" }
    Copy-Item -Path (Join-Path $publishDir '*') -Destination $appDir -Recurse -Force

    Copy-RequiredFile '.\Storage\nfdriver.sys' (Join-Path $appDir 'bin\nfdriver.sys')
    Copy-RequiredFile '.\Storage\stun.txt' (Join-Path $appDir 'bin\stun.txt')
    Copy-RequiredFile '.\Storage\aiodns.conf' (Join-Path $appDir 'bin\aiodns.conf')
    Copy-RequiredFile '.\Storage\tun2socks.bin' (Join-Path $appDir 'bin\tun2socks.bin')
    Copy-Item '.\Storage\i18n' $appDir -Recurse -Force
    Copy-Item '.\Storage\mode' $appDir -Recurse -Force
    Copy-Item '.\README.md', '.\CHANGELOG.md', '.\LICENSE' $appDir -Force
    New-Item -ItemType Directory -Force (Join-Path $appDir 'data'), (Join-Path $appDir 'logging') | Out-Null

    $dependencies = Get-Content '.\build.dependencies.json' -Raw | ConvertFrom-Json
    foreach ($property in $dependencies.PSObject.Properties) {
        Install-PinnedDependency $property.Name $property.Value
    }

    $zipPath = Join-Path $outputRoot 'Hatch.zip'
    Compress-Archive -LiteralPath $appDir -DestinationPath $zipPath -CompressionLevel Optimal -Force

    if (-not $SkipInstaller) {
        $isccCommand = Get-Command iscc -ErrorAction SilentlyContinue
        $isccPath = if ($isccCommand) { $isccCommand.Source } else { $null }
        if (-not $isccPath) {
            $defaultIscc = Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6\ISCC.exe'
            if (Test-Path -LiteralPath $defaultIscc) { $isccPath = $defaultIscc }
        }
        if (-not $isccPath) {
            throw 'Inno Setup 6 was not found. Use -SkipInstaller for archive-only builds.'
        }
        & $isccPath '/Qp' '.\installer\Hatch.iss'
        if ($LASTEXITCODE -ne 0) { throw "Installer build failed with exit code $LASTEXITCODE" }
    }

    $releaseFiles = @(Get-ChildItem -LiteralPath $outputRoot -File | Where-Object { $_.Name -ne 'SHA256SUMS.txt' })
    $checksumLines = foreach ($file in $releaseFiles) {
        $hash = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
        "$hash  $($file.Name)"
    }
    $checksumLines | Set-Content -LiteralPath (Join-Path $outputRoot 'SHA256SUMS.txt') -Encoding ASCII

    Write-Host "Build completed: $outputRoot"
    $releaseFiles | Select-Object Name, Length
    $checksumLines
}
finally {
    Pop-Location
}
