<#
.SYNOPSIS
Undoes steam-play-setup: deletes %LOCALAPPDATA%\Survarium-Steam\launcher.cfg, and nothing else.

.DESCRIPTION
Only that one file is deleted, and only when steam-play-setup wrote it (it has the line "srp_steam_play 1").
The folder and the game's own files (user.cfg with your game settings) stay.

.PARAMETER Force
Delete the file even though steam-play-setup did not write it.
#>
param([switch]$Force)
$ErrorActionPreference = 'Stop'

$file = Join-Path $env:LOCALAPPDATA 'Survarium-Steam\launcher.cfg'
if (-not (Test-Path -LiteralPath $file -PathType Leaf)) {
    Write-Host "Nothing to undo: $file does not exist."
    exit 0
}
$old = [System.IO.File]::ReadAllText($file, [System.Text.Encoding]::Unicode)
if ($old -notmatch '(?m)^srp_steam_play ' -and -not $Force) {
    Write-Host "NOT DONE: $file was not written by steam-play-setup. Look at it first; run again with -Force to delete it." -ForegroundColor Red
    exit 1
}
Remove-Item -LiteralPath $file
Write-Host "Deleted: $file"
Write-Host 'The Steam Play button starts the game as it came from Steam again.'
