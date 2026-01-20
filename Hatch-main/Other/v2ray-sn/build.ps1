# Download latest Xray-core release
Set-Location (Split-Path $MyInvocation.MyCommand.Path -Parent)

# Get latest release info from GitHub API
$releaseUrl = "https://api.github.com/repos/XTLS/Xray-core/releases/latest"
$headers = @{ "User-Agent" = "PowerShell" }

try {
    $release = Invoke-RestMethod -Uri $releaseUrl -Headers $headers
    $version = $release.tag_name
    Write-Host "Latest Xray-core version: $version"
    
    # Find Windows x64 asset
    $asset = $release.assets | Where-Object { $_.name -like "*windows-64*" -and $_.name -like "*.zip" } | Select-Object -First 1
    
    if (-not $asset) {
        Write-Host "Could not find Windows x64 release asset"
        exit 1
    }
    
    $downloadUrl = $asset.browser_download_url
    $fileName = $asset.name
    
    Write-Host "Downloading: $fileName"
    
    # Create temp directory
    if (Test-Path "temp") { Remove-Item -Recurse -Force "temp" }
    New-Item -ItemType Directory -Name "temp" | Out-Null
    
    # Download
    Invoke-WebRequest -Uri $downloadUrl -OutFile "temp\$fileName"
    
    # Extract
    Expand-Archive -Path "temp\$fileName" -DestinationPath "temp\extract" -Force
    
    # Create release directory
    if (-not (Test-Path "..\release")) {
        New-Item -ItemType Directory -Path "..\release" | Out-Null
    }
    
    # Copy xray.exe to release
    Copy-Item "temp\extract\xray.exe" "..\release\xray.exe" -Force
    
    # Cleanup
    Remove-Item -Recurse -Force "temp"
    
    Write-Host "Xray-core $version downloaded successfully"
    exit 0
}
catch {
    Write-Host "Failed to download Xray-core: $_"
    Write-Host "Trying to use local Xray from Netch folder..."
    
    # Fallback: copy from existing Netch installation
    $localXray = "..\..\..\..\Netch\bin\xray.exe"
    if (Test-Path $localXray) {
        if (-not (Test-Path "..\release")) {
            New-Item -ItemType Directory -Path "..\release" | Out-Null
        }
        Copy-Item $localXray "..\release\xray.exe" -Force
        Write-Host "Copied local xray.exe"
        exit 0
    }
    
    exit 1
}
