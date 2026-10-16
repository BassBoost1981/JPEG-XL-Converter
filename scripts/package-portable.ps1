# Builds the app without installer and assembles the portable folder:
#   dist-portable\JPEG XL Konverter\
#     JPEG XL Konverter.exe, cjxl.exe, djxl.exe, LICENSES\
# Needs the sidecars from scripts\fetch-libjxl.ps1 and `npm install` done.

$ErrorActionPreference = 'Stop'
$Root = Resolve-Path (Join-Path $PSScriptRoot '..')
Set-Location $Root

npm run tauri build -- --no-bundle
if ($LASTEXITCODE -ne 0) { throw 'tauri build failed' }

$Release = Join-Path $Root 'src-tauri\target\release'
$Bin = Join-Path $Root 'src-tauri\binaries'
$Out = Join-Path $Root 'dist-portable\JPEG XL Konverter'
if (Test-Path $Out) { Remove-Item $Out -Recurse -Force }
New-Item -ItemType Directory -Path (Join-Path $Out 'LICENSES') -Force | Out-Null

# The cargo package is called jpeg-xl-konverter; the portable exe gets the product name.
Copy-Item (Join-Path $Release 'jpeg-xl-konverter.exe') (Join-Path $Out 'JPEG XL Konverter.exe')
Copy-Item (Join-Path $Bin 'cjxl-x86_64-pc-windows-msvc.exe') (Join-Path $Out 'cjxl.exe')
Copy-Item (Join-Path $Bin 'djxl-x86_64-pc-windows-msvc.exe') (Join-Path $Out 'djxl.exe')

$Lic = Join-Path $Out 'LICENSES'
Copy-Item (Join-Path $Bin 'LICENSE-libjxl') (Join-Path $Lic 'libjxl.txt')
Copy-Item (Join-Path $Bin 'licenses-libjxl\*') $Lic
Copy-Item (Join-Path $Root 'node_modules\@fontsource-variable\lexend\LICENSE') (Join-Path $Lic 'Lexend-OFL.txt')
$WebpSys = Get-ChildItem (Join-Path $env:USERPROFILE '.cargo\registry\src\*\libwebp-sys-*\vendor\COPYING') |
    Sort-Object FullName | Select-Object -Last 1
if ($null -eq $WebpSys) { throw 'libwebp COPYING not found in the cargo registry (run cargo fetch)' }
Copy-Item $WebpSys.FullName (Join-Path $Lic 'libwebp.txt')

Write-Host "Portable folder: $Out"
Get-ChildItem $Out -Recurse -File | ForEach-Object { '{0,12:N0}  {1}' -f $_.Length, $_.FullName.Substring($Out.Length + 1) }
