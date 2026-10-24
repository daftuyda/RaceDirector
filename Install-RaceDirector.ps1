param([Parameter(Mandatory=$true)][string]$GameDirectory)
$ErrorActionPreference = 'Stop'
$game = (Resolve-Path -LiteralPath $GameDirectory).Path
if (-not (Test-Path -LiteralPath (Join-Path $game 'UmamusumePrettyDerby.exe'))) { throw 'Select the folder containing UmamusumePrettyDerby.exe.' }
if (Get-Process -Name UmamusumePrettyDerby -ErrorAction SilentlyContinue) { throw 'Close the game before installing.' }
$source = $PSScriptRoot
if (-not (Test-Path -LiteralPath (Join-Path $source 'racedirector.dll'))) { $source = Join-Path $source 'dist' }
foreach ($file in @('racedirector.dll','version.dll')) { if (-not (Test-Path -LiteralPath (Join-Path $source $file))) { throw "Missing $file. Run Build-RaceDirector.ps1 first." } }
foreach ($file in @('version.dll','racedirector.dll','racedirector_version.dll')) { if (Test-Path -LiteralPath (Join-Path $game $file)) { throw "Existing $file found. Installation stopped; nothing was overwritten. Move the existing mod files out of the game folder first." } }
# The genuine forwarding DLL is copied from this computer's Windows installation.
Copy-Item -LiteralPath (Join-Path ([Environment]::GetFolderPath('System')) 'version.dll') -Destination (Join-Path $game 'racedirector_version.dll')
Copy-Item -LiteralPath (Join-Path $source 'racedirector.dll') -Destination $game
# Install the entry point last, after all dependencies are present.
Copy-Item -LiteralPath (Join-Path $source 'version.dll') -Destination $game
Write-Host 'Installed. Launch the game and press Insert. F10 restores the camera and selected UI.'
