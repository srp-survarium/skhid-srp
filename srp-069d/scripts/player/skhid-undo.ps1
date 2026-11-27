<#
.SYNOPSIS
Undoes skhid-setup: deletes the start file Skhid.cmd, and nothing else.

.DESCRIPTION
Only that one file is deleted, and only when skhid-setup wrote it (its first line starts with "@rem skhid-setup:").
Nothing else changes; the game folder was never touched.

.PARAMETER Folder
The folder skhid-setup wrote Skhid.cmd to (default: your desktop).
#>
param([string]$Folder)
$ErrorActionPreference = 'Stop'

$marker = '@rem skhid-setup:'
if ($Folder) { $dir = "$Folder".Trim().Trim('"') } else { $dir = [Environment]::GetFolderPath('Desktop') }
$file = Join-Path $dir 'Skhid.cmd'
if (-not (Test-Path -LiteralPath $file -PathType Leaf)) {
    Write-Host "Nothing to undo: $file does not exist."
    exit 0
}
$old = [System.IO.File]::ReadAllText($file)
if (-not $old.StartsWith($marker)) {
    Write-Host "NOT DONE: $file was not written by skhid-setup, so it stays. Delete it yourself if you do not need it." -ForegroundColor Red
    exit 1
}
Remove-Item -LiteralPath $file
Write-Host "Deleted: $file"
