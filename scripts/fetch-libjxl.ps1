# Downloads libjxl v0.12.0 (official GitHub release, Windows x64 static build)
# and places cjxl/djxl as Tauri sidecars plus the licenses in src-tauri/binaries.
#
# Trust on first use: while $ExpectedSha256 is empty the script only prints
# the SHA-256 of the download and stops. Put that value into $ExpectedSha256
# and run the script again.

$ErrorActionPreference = 'Stop'

$Url = 'https://github.com/libjxl/libjxl/releases/download/v0.12.0/jxl-x64-windows-static.zip'
$ExpectedSha256 = '3025d7e308390796d20492322e606bc92decaee7b6bc99d3f7547870ae5db7de'

$BinDir = Join-Path $PSScriptRoot '..\src-tauri\binaries'
$WorkDir = Join-Path $env:TEMP ('libjxl-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $WorkDir | Out-Null

try {
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
    $Zip = Join-Path $WorkDir 'jxl-x64-windows-static.zip'
    Write-Host "Downloading $Url"
    Invoke-WebRequest -Uri $Url -OutFile $Zip -UseBasicParsing

    $Hash = (Get-FileHash -Path $Zip -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($ExpectedSha256 -eq '') {
        Write-Host "SHA-256: $Hash"
        Write-Host 'Put this value into $ExpectedSha256 in this script and run it again.'
        exit 1
    }
    if ($Hash -ne $ExpectedSha256.ToLowerInvariant()) {
        throw "SHA-256 mismatch: expected $ExpectedSha256, got $Hash"
    }

    # The archive uses Deflate64, which neither Expand-Archive (.NET Framework)
    # nor tar.exe can read. The Windows shell zip folder can, so the needed
    # entries are extracted through it.
    $Extracted = Join-Path $WorkDir 'extracted'
    New-Item -ItemType Directory -Path $Extracted | Out-Null
    $Shell = New-Object -ComObject Shell.Application
    $Dest = $Shell.NameSpace($Extracted)

    function Extract-One([string]$Folder, [string]$Name) {
        $item = $Shell.NameSpace("$Zip\$Folder").ParseName($Name)
        if ($null -eq $item) { throw "$Folder/$Name not found in the archive" }
        $target = Join-Path $Extracted $Name
        # 4 no progress UI, 16 yes to all, 512 no new-folder prompt, 1024 no error UI
        $Dest.CopyHere($item, 4 + 16 + 512 + 1024)
        $deadline = (Get-Date).AddSeconds(60)
        while (-not ((Test-Path $target) -and (Get-Item $target).Length -eq $item.Size)) {
            if ((Get-Date) -gt $deadline) { throw "Extracting $Name timed out" }
            Start-Sleep -Milliseconds 200
        }
        return $target
    }

    New-Item -ItemType Directory -Path $BinDir -Force | Out-Null
    Copy-Item (Extract-One 'x64-windows-static\bin' 'cjxl.exe') (Join-Path $BinDir 'cjxl-x86_64-pc-windows-msvc.exe') -Force
    Copy-Item (Extract-One 'x64-windows-static\bin' 'djxl.exe') (Join-Path $BinDir 'djxl-x86_64-pc-windows-msvc.exe') -Force
    Copy-Item (Extract-One 'x64-windows-static\licenses' 'LICENSE.libjxl') (Join-Path $BinDir 'LICENSE-libjxl') -Force

    # The static tools also contain brotli, giflib, highway, libjpeg-turbo,
    # libpng, libwebp, sjpeg, skcms and zlib: ship every license of the archive.
    $LicenseDir = Join-Path $BinDir 'licenses-libjxl'
    New-Item -ItemType Directory -Path $LicenseDir -Force | Out-Null
    foreach ($item in $Shell.NameSpace("$Zip\x64-windows-static\licenses").Items()) {
        if ($item.IsFolder) { continue }
        $name = Split-Path $item.Path -Leaf
        Copy-Item (Extract-One 'x64-windows-static\licenses' $name) (Join-Path $LicenseDir $name) -Force
    }

    Write-Host "Done. Files in $((Resolve-Path $BinDir).Path):"
    Get-ChildItem $BinDir -Recurse -File | ForEach-Object { Write-Host "  $($_.FullName.Substring($_.FullName.IndexOf('binaries')))" }
}
finally {
    Remove-Item -Path $WorkDir -Recurse -Force -ErrorAction SilentlyContinue
}
