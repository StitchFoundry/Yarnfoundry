# Builds the YarnFoundry Windows installer: dist\YarnFoundry-<version>-windows-setup.exe
# Run on a Windows machine with Rust and Inno Setup 6 (ISCC.exe) installed.
# The result is UNSIGNED; signing is added by the release workflow, not here.
$ErrorActionPreference = 'Stop'
$root = Resolve-Path (Join-Path $PSScriptRoot '..\..')
Set-Location $root

$match = Select-String -Path 'Cargo.toml' -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1
if (-not $match) { throw 'Could not read the workspace version from Cargo.toml' }
$version = $match.Matches[0].Groups[1].Value

cargo build --release -p yarnfoundry --locked
if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }

$iscc = (Get-Command iscc.exe -ErrorAction SilentlyContinue).Source
if (-not $iscc) { $iscc = Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6\ISCC.exe' }
if (-not (Test-Path $iscc)) { throw 'Inno Setup 6 (ISCC.exe) not found' }

& $iscc "/DAppVersion=$version" "/DSourceExe=$root\target\release\yarnfoundry.exe" "$PSScriptRoot\yarnfoundry.iss"
if ($LASTEXITCODE -ne 0) { throw 'Inno Setup failed' }

Write-Warning 'The installer is unsigned. Windows SmartScreen will warn until it is signed.'
Get-ChildItem "$root\dist\*-windows-setup.exe"
