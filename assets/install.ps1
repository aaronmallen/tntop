param(
    [string]$Version = "latest",
    [string]$Directory = (Join-Path $env:LOCALAPPDATA "tntop\bin")
)

$ErrorActionPreference = "Stop"
if (-not [System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform(
    [System.Runtime.InteropServices.OSPlatform]::Windows
) -or [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture -ne "X64") {
    throw "Unsupported platform. This installer requires Windows x86-64."
}

$base = "https://aaronmallen.dev/aaron/tntop/releases/download"
if ($Version -eq "latest") {
    $Version = (Invoke-RestMethod "https://aaronmallen.dev/api/v1/repos/aaron/tntop/releases/latest").tag_name
}
if ($Version -notmatch '^\d+\.\d+\.\d+$') {
    throw "Invalid release version: $Version"
}
$asset = "tntop-$Version-x86_64-pc-windows-gnu.zip"
$temporary = Join-Path ([System.IO.Path]::GetTempPath()) ([System.Guid]::NewGuid().ToString())
$staged = $null
New-Item -ItemType Directory -Path $temporary | Out-Null
try {
    $archive = Join-Path $temporary $asset
    Invoke-WebRequest "$base/$Version/$asset" -OutFile $archive
    $checksums = (Invoke-WebRequest "$base/$Version/SHA256SUMS").Content
    if ($checksums -is [byte[]]) {
        $checksums = [System.Text.Encoding]::UTF8.GetString($checksums)
    }
    $entries = @($checksums -split '\r?\n' | Where-Object {
        $_ -match ('^([a-fA-F0-9]{64})\s+\*?' + [regex]::Escape($asset) + '$')
    })
    if ($entries.Count -ne 1) {
        throw "Missing or duplicate checksum for $asset"
    }
    $expected = ($entries[0] -split '\s+')[0]
    if ((Get-FileHash -Algorithm SHA256 $archive).Hash -ne $expected) {
        throw "Checksum mismatch for $asset"
    }
    Expand-Archive -LiteralPath $archive -DestinationPath $temporary
    $binary = Join-Path $temporary ($asset.Replace(".zip", "") + "\tntop.exe")
    if (-not (Test-Path -LiteralPath $binary -PathType Leaf)) {
        throw "Archive has no tntop.exe binary"
    }
    New-Item -ItemType Directory -Force -Path $Directory | Out-Null
    $staged = Join-Path $Directory (".tntop-" + [System.Guid]::NewGuid() + ".exe")
    Copy-Item -LiteralPath $binary -Destination $staged
    $destination = Join-Path $Directory "tntop.exe"
    if (Test-Path -LiteralPath $destination) {
        [System.IO.File]::Replace($staged, $destination, $null)
    } else {
        [System.IO.File]::Move($staged, $destination)
    }
    $staged = $null
    Write-Output "Installed tntop $Version to $destination"
    if (($env:PATH -split ';') -notcontains $Directory) {
        Write-Output "Add $Directory to your user PATH in Windows Environment Variables, then open a new terminal."
    }
} finally {
    Remove-Item -LiteralPath $temporary -Recurse -Force
    if ($staged -and (Test-Path -LiteralPath $staged)) {
        Remove-Item -LiteralPath $staged -Force
    }
}
