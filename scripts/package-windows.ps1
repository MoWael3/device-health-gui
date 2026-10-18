# Build the Windows release zip into dist\:
#   device-health-dashboard-<version>-windows-x86_64.zip
# If WINDOWS_CERT_PFX (path to a .pfx code-signing certificate) and
# WINDOWS_CERT_PASSWORD are set, the .exe is signed with signtool first.
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

$version = (Select-String -Path Cargo.toml -Pattern '^version = "(.*)"' | Select-Object -First 1).Matches.Groups[1].Value
$name = "device-health-dashboard-$version-windows-x86_64"
New-Item -ItemType Directory -Force dist | Out-Null

cargo build --release
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
$exe = "target\release\device-health-dashboard.exe"

if ($env:WINDOWS_CERT_PFX) {
    $signtool = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\signtool.exe" |
        Sort-Object FullName -Descending | Select-Object -First 1
    if (-not $signtool) { throw "signtool.exe not found (install the Windows SDK)" }
    & $signtool.FullName sign /f $env:WINDOWS_CERT_PFX /p $env:WINDOWS_CERT_PASSWORD `
        /fd sha256 /tr http://timestamp.digicert.com /td sha256 $exe
    if ($LASTEXITCODE -ne 0) { throw "signing failed" }
    Write-Host "Signed $exe"
} else {
    Write-Host "WINDOWS_CERT_PFX not set; the .exe is unsigned"
}

$stage = Join-Path ([IO.Path]::GetTempPath()) $name
Remove-Item -Recurse -Force $stage -ErrorAction SilentlyContinue
New-Item -ItemType Directory $stage | Out-Null
Copy-Item $exe $stage
Set-Content (Join-Path $stage "README.txt") @"
Device Health Dashboard $version for Windows (x86_64)

Double-click device-health-dashboard.exe to run it. No installation needed.
For a Start-menu entry, right-click the .exe and choose "Pin to Start".
"@
$zip = "dist\$name.zip"
Remove-Item $zip -ErrorAction SilentlyContinue
Compress-Archive -Path "$stage\*" -DestinationPath $zip
Write-Host "Built $zip"
