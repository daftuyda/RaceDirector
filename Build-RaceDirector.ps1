param([switch]$DebugBuild)
$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot
$cargoExe = (Get-Command cargo -ErrorAction SilentlyContinue).Source
if (-not $cargoExe) { $cargoExe = Join-Path ([Environment]::GetFolderPath('UserProfile')) '.cargo\bin\cargo.exe' }
if (-not (Test-Path -LiteralPath $cargoExe)) { throw 'Install Rust stable with the MSVC target and Visual Studio C++ Build Tools.' }
& $cargoExe test --workspace --locked
if ($LASTEXITCODE -ne 0) { throw 'Tests failed.' }
$profile = 'release'
if ($DebugBuild) { $profile = 'debug'; & $cargoExe build --workspace --locked } else { & $cargoExe build --workspace --release --locked }
if ($LASTEXITCODE -ne 0) { throw 'Build failed.' }
$dist = Join-Path $PSScriptRoot 'dist'
New-Item -ItemType Directory -Path $dist -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $PSScriptRoot "target\$profile\racedirector.dll") -Destination $dist
Copy-Item -LiteralPath (Join-Path $PSScriptRoot "target\$profile\version.dll") -Destination $dist
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'Install-RaceDirector.ps1') -Destination $dist
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'Update-RaceDirector.ps1') -Destination $dist
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'Disable-RaceDirector.ps1') -Destination $dist
New-Item -ItemType Directory -Path (Join-Path $dist 'docs') -Force | Out-Null
Copy-Item -Path (Join-Path $PSScriptRoot 'docs\*.md') -Destination (Join-Path $dist 'docs') -Force
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'README.md') -Destination $dist
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'LICENSE') -Destination $dist
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'THIRD_PARTY_NOTICES.md') -Destination $dist
$licenses = Join-Path $dist 'licenses'
New-Item -ItemType Directory -Path $licenses -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'native\vendor\hudhook-0.6.5\LICENSE') -Destination (Join-Path $licenses 'hudhook-LICENSE')
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'native\vendor\hudhook-0.6.5\vendor\minhook\LICENSE.txt') -Destination (Join-Path $licenses 'minhook-LICENSE.txt')
Write-Host "Built RaceDirector in $dist. Use Install-RaceDirector.ps1 with the game closed."
