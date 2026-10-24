param([string]$GameDirectory = 'C:\Program Files (x86)\Steam\steamapps\common\UmamusumePrettyDerby')
$ErrorActionPreference = 'Stop'
$game = (Resolve-Path -LiteralPath $GameDirectory).Path
if (-not (Test-Path -LiteralPath (Join-Path $game 'UmamusumePrettyDerby.exe'))) { throw 'Select the folder containing UmamusumePrettyDerby.exe.' }
if (Get-Process -Name UmamusumePrettyDerby -ErrorAction SilentlyContinue) { throw 'Close Umamusume before updating. Your paused race has not been interrupted.' }
$source = Join-Path $PSScriptRoot 'racedirector.dll'
if (-not (Test-Path -LiteralPath $source)) { $source = Join-Path $PSScriptRoot 'dist\racedirector.dll' }
if (-not (Test-Path -LiteralPath $source)) { throw 'The rebuilt racedirector.dll was not found.' }
$destination = Join-Path $game 'racedirector.dll'
if (-not (Test-Path -LiteralPath $destination)) {
 $disabledCopies = @(Get-ChildItem -LiteralPath $game -File | Where-Object { $_.Name.StartsWith('racedirector.dll.disabled-') })
 if ($disabledCopies.Count -eq 0 -or -not (Test-Path -LiteralPath (Join-Path $game 'racedirector_version.dll'))) { throw 'No existing or disabled RaceDirector installation found. Use Install-RaceDirector.ps1 instead.' }
}
# Back up the currently installed plugin; leave loader/other mods untouched.
$backup = Join-Path $game ('racedirector.dll.backup-' + [DateTime]::Now.ToString('yyyyMMdd-HHmmss-fff'))
if (Test-Path -LiteralPath $destination) { Copy-Item -LiteralPath $destination -Destination $backup }
Copy-Item -LiteralPath $source -Destination $destination -Force
if ((Get-FileHash -LiteralPath $source).Hash -ne (Get-FileHash -LiteralPath $destination).Hash) { throw 'Updated DLL hash did not match the build.' }
Write-Host "RaceDirector updated. Backup: $backup"
Write-Host 'Launch the game. Insert opens the panel; F6 toggles camera; F10 restores camera/UI.'
