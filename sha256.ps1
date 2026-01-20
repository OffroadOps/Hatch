param (
    [Parameter(Mandatory=$true)]
    [string]$Path
)

function Get-DirectoryHash {
    param (
        [string]$Directory
    )
    
    $files = Get-ChildItem -Path $Directory -Recurse -File | Sort-Object FullName
    $hashString = ""
    
    foreach ($file in $files) {
        $hash = (Get-FileHash -Path $file.FullName -Algorithm SHA256).Hash
        $relativePath = $file.FullName.Substring($Directory.Length)
        $hashString += "$relativePath`:$hash`n"
    }
    
    $bytes = [System.Text.Encoding]::UTF8.GetBytes($hashString)
    $sha256 = [System.Security.Cryptography.SHA256]::Create()
    $finalHash = $sha256.ComputeHash($bytes)
    
    return [System.BitConverter]::ToString($finalHash).Replace("-", "").ToLower()
}

if (Test-Path $Path -PathType Container) {
    Get-DirectoryHash -Directory $Path
} else {
    (Get-FileHash -Path $Path -Algorithm SHA256).Hash.ToLower()
}
