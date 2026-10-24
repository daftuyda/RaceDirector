param([string]$GameDirectory = 'C:\Program Files (x86)\Steam\steamapps\common\UmamusumePrettyDerby')
$ErrorActionPreference = 'Stop'
$game = (Resolve-Path -LiteralPath $GameDirectory).Path
if (Get-Process -Name UmamusumePrettyDerby -ErrorAction SilentlyContinue) { throw 'Close the game first.' }
$plugin = Join-Path $game 'racedirector.dll'
if (Test-Path -LiteralPath $plugin) {
 $disabled = Join-Path $game ('racedirector.dll.disabled-' + [DateTime]::Now.ToString('yyyyMMdd-HHmmss-fff'))
 Move-Item -LiteralPath $plugin -Destination $disabled
 Write-Host "Disabled RaceDirector: $disabled"
} else { Write-Host 'RaceDirector is already disabled.' }
Write-Host 'Launch the game normally through Steam. The loader will skip the absent plugin.'
