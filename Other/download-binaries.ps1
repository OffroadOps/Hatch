# Download precompiled binaries instead of building from source
# This simplifies GitHub Actions and speeds up the build process

param(
    [string]$OutputPath = "release"
)

Set-Location (Split-Path $MyInvocation.MyCommand.Path -Parent)

# Create release directory if it doesn't exist
if (-Not (Test-Path $OutputPath)) {
    New-Item -ItemType Directory -Path $OutputPath | Out-Null
}

Write-Host "Downloading precompiled binaries..."

# Download Xray
Write-Host "Downloading Xray..."
$xrayUrl = "https://github.com/XTLS/Xray-core/releases/latest/download/Xray-windows-64.zip"
$xrayZip = Join-Path $env:TEMP "xray.zip"
$xrayExtract = Join-Path $env:TEMP "xray-extract"

try {
    Invoke-WebRequest -Uri $xrayUrl -OutFile $xrayZip -UseBasicParsing
    Expand-Archive -Path $xrayZip -DestinationPath $xrayExtract -Force
    Copy-Item -Path (Join-Path $xrayExtract "xray.exe") -Destination (Join-Path $OutputPath "xray.exe") -Force
    Write-Host "✓ Xray downloaded successfully"
} catch {
    Write-Error "Failed to download Xray: $_"
    exit 1
}

# Download pcap2socks
Write-Host "Downloading pcap2socks..."
$pcap2socksUrl = "https://github.com/zhxie/pcap2socks/releases/latest/download/pcap2socks-windows.zip"
$pcap2socksZip = Join-Path $env:TEMP "pcap2socks.zip"
$pcap2socksExtract = Join-Path $env:TEMP "pcap2socks-extract"

try {
    Invoke-WebRequest -Uri $pcap2socksUrl -OutFile $pcap2socksZip -UseBasicParsing
    Expand-Archive -Path $pcap2socksZip -DestinationPath $pcap2socksExtract -Force
    Copy-Item -Path (Join-Path $pcap2socksExtract "pcap2socks.exe") -Destination (Join-Path $OutputPath "pcap2socks.exe") -Force
    Write-Host "✓ pcap2socks downloaded successfully"
} catch {
    Write-Error "Failed to download pcap2socks: $_"
    exit 1
}

# For aiodns, we'll create a simple placeholder or download from a known source
# Since aiodns is custom, we'll need to build it or provide a precompiled version
Write-Host "Note: aiodns.bin needs to be provided separately or built from source"

Write-Host "`nAll binaries downloaded successfully!"
Write-Host "Output directory: $OutputPath"
Get-ChildItem -Path $OutputPath -File | ForEach-Object {
    Write-Host "  - $($_.Name) ($([math]::Round($_.Length / 1MB, 2)) MB)"
}

exit 0
