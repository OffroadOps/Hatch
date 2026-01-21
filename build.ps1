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
	$PublishReadyToRun = $False
)

Push-Location (Split-Path $MyInvocation.MyCommand.Path -Parent)

if ( Test-Path -Path $OutputPath ) {
    rm -Recurse -Force $OutputPath
}
New-Item -ItemType Directory -Name $OutputPath | Out-Null

Push-Location $OutputPath
New-Item -ItemType Directory -Name 'bin'  | Out-Null
cp -Recurse -Force '..\Storage\i18n' '.'  | Out-Null
cp -Recurse -Force '..\Storage\mode' '.'  | Out-Null
cp -Recurse -Force '..\Storage\stun.txt' 'bin'  | Out-Null
cp -Recurse -Force '..\Storage\nfdriver.sys' 'bin'  | Out-Null
cp -Recurse -Force '..\Storage\aiodns.conf' 'bin'  | Out-Null
Invoke-WebRequest -Uri 'https://raw.githubusercontent.com/Loyalsoldier/geoip/release/Country.mmdb' -OutFile 'bin\GeoLite2-Country.mmdb'
#cp -Recurse -Force '..\Storage\GeoLite2-Country.mmdb' 'bin'  | Out-Null
cp -Recurse -Force '..\Storage\tun2socks.bin' 'bin'  | Out-Null
cp -Recurse -Force '..\Storage\README.md' 'bin'  | Out-Null
Pop-Location

if ( -Not ( Test-Path '.\Other\release' ) ) {
	Write-Host "Setting up Other components..."
	New-Item -ItemType Directory -Path '.\Other\release' -Force | Out-Null
	
	# Download Xray
	Write-Host "Downloading Xray..."
	$xrayUrl = "https://github.com/XTLS/Xray-core/releases/latest/download/Xray-windows-64.zip"
	$xrayZip = Join-Path $env:TEMP "xray.zip"
	$xrayExtract = Join-Path $env:TEMP "xray-extract"
	Invoke-WebRequest -Uri $xrayUrl -OutFile $xrayZip -UseBasicParsing
	Expand-Archive -Path $xrayZip -DestinationPath $xrayExtract -Force
	Copy-Item -Path (Join-Path $xrayExtract "xray.exe") -Destination '.\Other\release\xray.exe' -Force
	
	# Download pcap2socks
	Write-Host "Downloading pcap2socks..."
	$pcap2socksUrl = "https://github.com/zhxie/pcap2socks/releases/latest/download/pcap2socks-windows.zip"
	$pcap2socksZip = Join-Path $env:TEMP "pcap2socks.zip"
	$pcap2socksExtract = Join-Path $env:TEMP "pcap2socks-extract"
	Invoke-WebRequest -Uri $pcap2socksUrl -OutFile $pcap2socksZip -UseBasicParsing
	Expand-Archive -Path $pcap2socksZip -DestinationPath $pcap2socksExtract -Force
	Copy-Item -Path (Join-Path $pcap2socksExtract "pcap2socks.exe") -Destination '.\Other\release\pcap2socks.exe' -Force
	
	Write-Host "Other components ready"
}
cp -Force '.\Other\release\*.bin' "$OutputPath\bin" -ErrorAction SilentlyContinue
cp -Force '.\Other\release\*.dll' "$OutputPath\bin" -ErrorAction SilentlyContinue
cp -Force '.\Other\release\*.exe' "$OutputPath\bin"

if ( -Not ( Test-Path ".\Netch\bin\$Configuration" ) ) {
	Write-Host
	Write-Host 'Building Netch'

	dotnet publish `
		-c $Configuration `
		-r 'win-x64' `
		-p:Platform='x64' `
		-p:SelfContained=$SelfContained `
		-p:PublishTrimmed=$PublishReadyToRun `
		-p:PublishSingleFile=$PublishSingleFile `
		-p:PublishReadyToRun=$PublishReadyToRun `
		-p:PublishReadyToRunShowWarnings=$PublishReadyToRun `
		-p:IncludeNativeLibrariesForSelfExtract=$SelfContained `
		-o ".\Netch\bin\$Configuration" `
		'.\Netch\Netch.csproj'
	if ( -Not $? ) { exit $lastExitCode }
}
cp -Force ".\Netch\bin\$Configuration\Netch.exe" $OutputPath

if ( -Not ( Test-Path ".\Redirector\bin\$Configuration" ) ) {
	Write-Host
	Write-Host 'Building Redirector'

	msbuild `
		-property:Configuration=$Configuration `
		-property:Platform=x64 `
		'.\Redirector\Redirector.vcxproj'
	if ( -Not $? ) { exit $lastExitCode }
}
cp -Force ".\Redirector\bin\$Configuration\nfapi.dll"      "$OutputPath\bin"
cp -Force ".\Redirector\bin\$Configuration\Redirector.bin" "$OutputPath\bin"

if ( -Not ( Test-Path ".\RouteHelper\bin\$Configuration" ) ) {
	Write-Host
	Write-Host 'Building RouteHelper'

	msbuild `
		-property:Configuration=$Configuration `
		-property:Platform=x64 `
		'.\RouteHelper\RouteHelper.vcxproj'
	if ( -Not $? ) { exit $lastExitCode }
}
cp -Force ".\RouteHelper\bin\$Configuration\RouteHelper.bin" "$OutputPath\bin"

if ( $Configuration.Equals('Release') ) {
	rm -Force "$OutputPath\*.pdb"
	rm -Force "$OutputPath\*.xml"
}

Pop-Location
exit 0
