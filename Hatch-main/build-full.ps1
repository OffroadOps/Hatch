# Netch 2.0.0 Full Build Script
# Requires: .NET 8 SDK, Visual Studio 2022 (C++), Go, MinGW-w64 (gcc)

param (
    [Parameter()]
    [ValidateSet('Debug', 'Release')]
    [string]
    $Configuration = 'Release',

    [Parameter()]
    [ValidateNotNullOrEmpty()]
    [string]
    $OutputPath = 'release',

    [Parameter()]
    [bool]
    $SelfContained = $True,

    [Parameter()]
    [bool]
    $PublishSingleFile = $True,

    [Parameter()]
    [bool]
    $PublishReadyToRun = $False,

    [Parameter()]
    [bool]
    $UseExistingBin = $True
)

$ErrorActionPreference = "Stop"
Push-Location (Split-Path $MyInvocation.MyCommand.Path -Parent)

# Add MinGW to PATH if not present
if (-not ($env:Path -like "*mingw64*")) {
    $env:Path += ";D:\mingw64\bin"
}

# Add dotnet to PATH if not present
if (-not ($env:Path -like "*dotnet*")) {
    $env:Path += ";C:\Program Files\dotnet"
}

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  Netch 2.0.0 Build Script" -ForegroundColor Cyan
Write-Host "  Using Xray-core as proxy engine" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Check prerequisites
Write-Host "Checking prerequisites..." -ForegroundColor Yellow
$dotnetVersion = dotnet --version
Write-Host "  .NET SDK: $dotnetVersion" -ForegroundColor Green

$goVersion = go version
Write-Host "  Go: $goVersion" -ForegroundColor Green

try {
    $gccVersion = gcc --version | Select-Object -First 1
    Write-Host "  GCC: $gccVersion" -ForegroundColor Green
} catch {
    Write-Host "  GCC: Not found in PATH" -ForegroundColor Red
    exit 1
}

# Clean and create output directory
Write-Host ""
Write-Host "Preparing output directory..." -ForegroundColor Yellow
if (Test-Path -Path $OutputPath) {
    Remove-Item -Recurse -Force $OutputPath
}
New-Item -ItemType Directory -Name $OutputPath | Out-Null
New-Item -ItemType Directory -Path "$OutputPath\bin" | Out-Null

# Copy static files
Write-Host "Copying static files..." -ForegroundColor Yellow
Copy-Item -Recurse -Force '.\Storage\i18n' ".\$OutputPath\"
Copy-Item -Recurse -Force '.\Storage\mode' ".\$OutputPath\"
Copy-Item -Force '.\Storage\stun.txt' ".\$OutputPath\bin\"
Copy-Item -Force '.\Storage\nfdriver.sys' ".\$OutputPath\bin\"
Copy-Item -Force '.\Storage\aiodns.conf' ".\$OutputPath\bin\"
Copy-Item -Force '.\Storage\tun2socks.bin' ".\$OutputPath\bin\"
Copy-Item -Force '.\Storage\README.md' ".\$OutputPath\bin\"

# Download GeoIP database
Write-Host "Downloading GeoIP database..." -ForegroundColor Yellow
try {
    Invoke-WebRequest -Uri 'https://raw.githubusercontent.com/Loyalsoldier/geoip/release/Country.mmdb' -OutFile ".\$OutputPath\bin\GeoLite2-Country.mmdb"
    Write-Host "  GeoIP database downloaded" -ForegroundColor Green
} catch {
    Write-Host "  Failed to download GeoIP, copying from existing..." -ForegroundColor Yellow
    if (Test-Path "..\Netch\bin\GeoLite2-Country.mmdb") {
        Copy-Item "..\Netch\bin\GeoLite2-Country.mmdb" ".\$OutputPath\bin\"
    }
}

# Download/Copy Xray-core
Write-Host ""
Write-Host "Getting Xray-core..." -ForegroundColor Yellow
$xrayPath = ".\$OutputPath\bin\xray.exe"

try {
    $releaseUrl = "https://api.github.com/repos/XTLS/Xray-core/releases/latest"
    $headers = @{ "User-Agent" = "PowerShell" }
    $release = Invoke-RestMethod -Uri $releaseUrl -Headers $headers
    $version = $release.tag_name
    Write-Host "  Latest Xray-core version: $version" -ForegroundColor Green
    
    $asset = $release.assets | Where-Object { $_.name -like "*windows-64*" -and $_.name -like "*.zip" } | Select-Object -First 1
    
    if ($asset) {
        $downloadUrl = $asset.browser_download_url
        Write-Host "  Downloading Xray-core..." -ForegroundColor Yellow
        
        $tempDir = ".\temp_xray"
        if (Test-Path $tempDir) { Remove-Item -Recurse -Force $tempDir }
        New-Item -ItemType Directory -Path $tempDir | Out-Null
        
        Invoke-WebRequest -Uri $downloadUrl -OutFile "$tempDir\xray.zip"
        Expand-Archive -Path "$tempDir\xray.zip" -DestinationPath "$tempDir\extract" -Force
        Copy-Item "$tempDir\extract\xray.exe" $xrayPath -Force
        
        Remove-Item -Recurse -Force $tempDir
        Write-Host "  Xray-core $version installed" -ForegroundColor Green
    }
} catch {
    Write-Host "  Failed to download Xray-core: $_" -ForegroundColor Yellow
    Write-Host "  Copying from existing Netch installation..." -ForegroundColor Yellow
    
    # Try to copy v2ray-sn.exe and rename, or use existing xray.exe
    if (Test-Path "..\Netch\bin\xray.exe") {
        Copy-Item "..\Netch\bin\xray.exe" $xrayPath -Force
    } elseif (Test-Path "..\Netch\bin\v2ray-sn.exe") {
        Write-Host "  Warning: Using v2ray-sn.exe as fallback (may not be fully compatible)" -ForegroundColor Yellow
        Copy-Item "..\Netch\bin\v2ray-sn.exe" $xrayPath -Force
    }
}

# Copy existing bin files if UseExistingBin is true
if ($UseExistingBin -and (Test-Path "..\Netch\bin")) {
    Write-Host ""
    Write-Host "Copying existing bin files..." -ForegroundColor Yellow
    
    $binFiles = @(
        "aiodns.bin",
        "nfapi.dll", 
        "Redirector.bin",
        "RouteHelper.bin",
        "wintun.dll",
        "pcap2socks.exe"
    )
    
    foreach ($file in $binFiles) {
        $srcPath = "..\Netch\bin\$file"
        if (Test-Path $srcPath) {
            Copy-Item $srcPath ".\$OutputPath\bin\" -Force
            Write-Host "  Copied: $file" -ForegroundColor Green
        } else {
            Write-Host "  Not found: $file" -ForegroundColor Yellow
        }
    }
}

# Build Netch main program
Write-Host ""
Write-Host "Building Netch..." -ForegroundColor Yellow

$buildArgs = @(
    "publish",
    "-c", $Configuration,
    "-r", "win-x64",
    "-p:Platform=x64",
    "-p:SelfContained=$SelfContained",
    "-p:PublishSingleFile=$PublishSingleFile",
    "-p:PublishReadyToRun=$PublishReadyToRun",
    "-p:TreatWarningsAsErrors=false",
    "-o", ".\Netch\bin\$Configuration",
    ".\Netch\Netch.csproj"
)

& dotnet @buildArgs
if (-not $?) {
    Write-Host "Failed to build Netch" -ForegroundColor Red
    exit 1
}

Copy-Item ".\Netch\bin\$Configuration\Netch.exe" ".\$OutputPath\" -Force
Write-Host "  Netch.exe built successfully" -ForegroundColor Green

# Clean up
if ($Configuration -eq 'Release') {
    Remove-Item -Force ".\$OutputPath\*.pdb" -ErrorAction SilentlyContinue
    Remove-Item -Force ".\$OutputPath\*.xml" -ErrorAction SilentlyContinue
}

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  Build completed!" -ForegroundColor Green
Write-Host "  Output: $((Get-Item $OutputPath).FullName)" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan

# List output files
Write-Host ""
Write-Host "Output files:" -ForegroundColor Yellow
Get-ChildItem ".\$OutputPath" -Recurse -File | ForEach-Object {
    $relativePath = $_.FullName.Replace((Get-Item ".\$OutputPath").FullName, "")
    Write-Host "  $relativePath" -ForegroundColor Gray
}

Pop-Location
exit 0
