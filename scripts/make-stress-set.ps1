# Generates the stress-test image set and (optionally) runs the headless stress run.
#
#   .\scripts\make-stress-set.ps1 -Out $env:TEMP\jxl-stress -Count 1000
#   .\scripts\make-stress-set.ps1 -Out $env:TEMP\jxl-stress -Run -Workers 6
#
# -Run converts the set with the real queue into "<Out>-out" (a fake recycle
# bin is used, originals stay). Delete both folders yourself afterwards.
param(
    [Parameter(Mandatory)][string]$Out,
    [int]$Count = 1000,
    [switch]$Run,
    [int]$Workers = 0
)
$ErrorActionPreference = 'Stop'
$Manifest = Join-Path $PSScriptRoot '..\src-tauri\Cargo.toml'

cargo run --release --manifest-path $Manifest --example make_stress_set -- $Out $Count
if ($LASTEXITCODE -ne 0) { throw 'make_stress_set failed' }

if ($Run) {
    $OutDir = "$Out-out"
    $Extra = @()
    if ($Workers -gt 0) { $Extra = @($Workers) }
    cargo run --release --manifest-path $Manifest --example stress_run -- $Out $OutDir @Extra
    if ($LASTEXITCODE -ne 0) { throw 'stress_run failed' }
}
