<#
Skhid: Steam shows that you are playing Survarium while you play on Skhid.

    skhid-steam.bat           set it up (double-click)
    skhid-steam-remove.bat    undo it

Survarium (app 355840) installed in Steam: its Play button starts Skhid
(the app's launch options, in userdata\<id>\config\localconfig.vdf).
Not installed: a non-Steam game "Survarium" that starts Skhid is added
(userdata\<id>\config\shortcuts.vdf). Both start skhid-steam-run.bat, which
starts Skhid's launcher and waits until the game has closed.

Steam must be closed while its files change. Only those two files of Steam
are changed, each copied first to <file>.before-skhid. No network.

Options: -Remove, -User <id>|all, -Yes (no questions), -SteamPath <folder>.
Runs on Windows PowerShell 5.1; the tests (test_skhid_steam.py) run it in
PowerShell 7 on Linux against a made-up Steam folder.
#>
[CmdletBinding()]
param(
    [switch]$Remove,
    [string]$User,
    [switch]$Yes,
    [string]$SteamPath,
    # Used by skhid-steam-run.bat: wait until Skhid's game has closed.
    [switch]$WaitForGame,
    [Parameter(DontShow)][string]$SteamProcessName = 'steam',
    [Parameter(DontShow)][double]$PollSeconds = 3
)

$ErrorActionPreference = 'Stop'

$SurvariumAppId = '355840'
$ShortcutName = 'Survarium'
$RunBat = 'skhid-steam-run.bat'
$BackupSuffix = '.before-skhid'
$AppPath = @('UserLocalConfigStore', 'Software', 'Valve', 'Steam', 'apps', $SurvariumAppId)

# Steam's files are UTF-8. They are handled as "byte strings" (one char per
# byte, Latin-1), so that what is not changed is written back byte for byte.
$Latin1 = [Text.Encoding]::GetEncoding(28591)
$Utf8 = New-Object Text.UTF8Encoding $false

function ConvertTo-ByteString([string]$s) { $Latin1.GetString($Utf8.GetBytes($s)) }
function ConvertFrom-ByteString([string]$s) { $Utf8.GetString($Latin1.GetBytes($s)) }
function Read-ByteText([string]$Path) { $Latin1.GetString([IO.File]::ReadAllBytes($Path)) }
function Write-ByteText([string]$Path, [string]$Text) { [IO.File]::WriteAllBytes($Path, $Latin1.GetBytes($Text)) }

# ---------------------------------------------------------------- text VDF

# One token per match, from where the last one ended (\G): 1 a quoted
# string's inside, 2 a brace, 3 an unquoted word; a comment or a condition
# such as [$WIN32] matches with none of them and is kept as it is.
$TokenRegex = New-Object Text.RegularExpressions.Regex ('\G[ \t\r\n]*(?:"((?:[^"\\]|\\[\s\S])*)"|([{}])|//[^\n]*|\[[^\]\n]*\]|([^ \t\r\n{}"]+))')
$Unescape = [Text.RegularExpressions.MatchEvaluator] {
    param($m)
    switch -CaseSensitive ($m.Groups[1].Value) {
        'n' { "`n" } 't' { "`t" } '\' { '\' } '"' { '"' } default { $m.Value }
    }
}

function New-VdfNode([int]$Open) {
    [pscustomobject]@{ Entries = [System.Collections.Generic.List[object]]::new(); Open = $Open; Close = -1 }
}

# Parses text VDF into nodes: Entries (Key, Value = string or node, and
# where key and value are in the text), Open and Close (brace positions).
function Read-TextVdf([string]$Text) {
    $root = New-VdfNode -1
    $stack = New-Object System.Collections.Generic.Stack[object]
    $node = $root
    $key = $null
    $keyStart = 0
    $prev = 0
    if ($Text.StartsWith([string][char]0xEF + [char]0xBB + [char]0xBF, [StringComparison]::Ordinal)) { $prev = 3 }
    $m = $TokenRegex.Match($Text, $prev)
    while ($m.Success) {
        $prev = $m.Index + $m.Length
        $g = $m.Groups
        if ($g[2].Success) {
            $at = $g[2].Index
            if ($g[2].Value -eq '{') {
                if ($null -eq $key) { throw "unexpected { in VDF at byte $at" }
                $child = [pscustomobject]@{ Entries = [System.Collections.Generic.List[object]]::new(); Open = $at; Close = -1 }
                $node.Entries.Add([pscustomobject]@{ Key = $key; Value = $child; KeyStart = $keyStart; ValueStart = $at; ValueEnd = -1 })
                $stack.Push($node)
                $node = $child
                $key = $null
            } else {
                if ($null -ne $key -or $stack.Count -eq 0) { throw "unexpected } in VDF at byte $at" }
                $node.Close = $at
                $node = $stack.Pop()
                $node.Entries[$node.Entries.Count - 1].ValueEnd = $prev
            }
        } elseif ($g[1].Success -or $g[3].Success) {
            if ($g[1].Success) {
                $s = $g[1].Value
                $at = $g[1].Index - 1
                if ($s.IndexOf([char]'\') -ge 0) { $s = [regex]::Replace($s, '\\([\s\S])', $Unescape) }
            } else {
                $s = $g[3].Value
                $at = $g[3].Index
            }
            if ($null -eq $key) {
                $key = $s
                $keyStart = $at
            } else {
                $node.Entries.Add([pscustomobject]@{ Key = $key; Value = $s; KeyStart = $keyStart; ValueStart = $at; ValueEnd = $prev })
                $key = $null
            }
        }
        $m = $m.NextMatch()
    }
    if ($prev -lt $Text.Length -and $Text.Substring($prev) -match '[^ \t\r\n]') { throw "unreadable VDF near byte $prev" }
    if ($null -ne $key -or $stack.Count -ne 0) { throw 'VDF file is cut short' }
    $root.Close = $Text.Length
    return $root
}

function Format-VdfString([string]$s) {
    '"' + $s.Replace('\', '\\').Replace('"', '\"').Replace("`n", '\n').Replace("`t", '\t') + '"'
}

function Find-VdfEntry($Node, [string]$Key) {
    foreach ($e in $Node.Entries) { if ($e.Key -ieq $Key) { return $e } }
    return $null
}

function Find-VdfNode($Node, [string[]]$Path) {
    foreach ($p in $Path) {
        $e = Find-VdfEntry $Node $p
        if ($null -eq $e -or $e.Value -is [string]) { return $null }
        $Node = $e.Value
    }
    return $Node
}

function Get-TextVdfValue($Root, [string[]]$Path, [string]$Key) {
    $node = Find-VdfNode $Root $Path
    if ($null -eq $node) { return $null }
    $e = Find-VdfEntry $node $Key
    if ($null -eq $e -or $e.Value -isnot [string]) { return $null }
    return $e.Value
}

# Sets Path/Key to Value in the text: the value is replaced where it is, or
# a line (and the missing blocks) is added at the end of its block, in the
# tabs Steam writes. The rest of the text is not touched.
function Set-TextVdfValue([string]$Text, [string[]]$Path, [string]$Key, [string]$Value) {
    $root = Read-TextVdf $Text
    $node = $root
    $depth = 0
    while ($depth -lt $Path.Count) {
        $e = Find-VdfEntry $node $Path[$depth]
        if ($null -eq $e) { break }
        if ($e.Value -is [string]) { throw "$($Path[$depth]) is a value, not a block" }
        $node = $e.Value
        $depth++
    }
    if ($depth -eq 0) { throw "this is not the file expected (no $($Path[0]))" }
    if ($depth -eq $Path.Count) {
        $e = Find-VdfEntry $node $Key
        if ($null -ne $e) {
            if ($e.Value -isnot [string]) { throw "$Key is a block, not a value" }
            return $Text.Substring(0, $e.ValueStart) + (Format-VdfString $Value) + $Text.Substring($e.ValueEnd)
        }
    }
    $nl = if ($Text.Contains("`r`n")) { "`r`n" } else { "`n" }
    $add = New-Object Text.StringBuilder
    $indent = $depth
    for ($i = $depth; $i -lt $Path.Count; $i++) {
        $tabs = "`t" * $indent
        [void]$add.Append($tabs + (Format-VdfString $Path[$i]) + $nl + $tabs + '{' + $nl)
        $indent++
    }
    [void]$add.Append(("`t" * $indent) + (Format-VdfString $Key) + "`t`t" + (Format-VdfString $Value) + $nl)
    for ($i = $Path.Count - 1; $i -ge $depth; $i--) {
        $indent--
        [void]$add.Append(("`t" * $indent) + '}' + $nl)
    }
    $at = $node.Close
    $lineStart = if ($at -gt 0) { $Text.LastIndexOf([char]10, $at - 1) + 1 } else { 0 }
    if ($Text.Substring($lineStart, $at - $lineStart) -match '[^ \t\r]') {
        return $Text.Substring(0, $at) + $nl + $add.ToString() + $Text.Substring($at)
    }
    return $Text.Substring(0, $lineStart) + $add.ToString() + $Text.Substring($lineStart)
}

# Removes the line of Path/Key; the text is returned unchanged when it is not there.
function Remove-TextVdfValue([string]$Text, [string[]]$Path, [string]$Key) {
    $node = Find-VdfNode (Read-TextVdf $Text) $Path
    if ($null -eq $node) { return $Text }
    $e = Find-VdfEntry $node $Key
    if ($null -eq $e) { return $Text }
    $start = if ($e.KeyStart -gt 0) { $Text.LastIndexOf([char]10, $e.KeyStart - 1) + 1 } else { 0 }
    $end = $e.ValueEnd
    while ($end -lt $Text.Length -and ($Text[$end] -eq ' ' -or $Text[$end] -eq "`t" -or $Text[$end] -eq "`r")) { $end++ }
    if ($Text.Substring($start, $e.KeyStart - $start) -match '[^ \t]' -or ($end -lt $Text.Length -and $Text[$end] -ne "`n")) {
        return $Text.Substring(0, $e.KeyStart) + $Text.Substring($e.ValueEnd)
    }
    if ($end -lt $Text.Length) { $end++ }
    return $Text.Substring(0, $start) + $Text.Substring($end)
}

# -------------------------------------------------------------- binary VDF
# shortcuts.vdf: entries of <type byte> <key>\0 <value>; a map ends with 08.
# Types: 00 map, 01 string\0, 02 int32, 03 float, 07 uint64 (LE).

function Read-CString($St) {
    $end = [Array]::IndexOf($St.B, [byte]0, $St.P)
    if ($end -lt 0) { throw 'shortcuts.vdf is cut short' }
    $s = $Latin1.GetString($St.B, $St.P, $end - $St.P)
    $St.P = $end + 1
    return $s
}

function Read-BinaryMap($St) {
    $list = New-Object System.Collections.Generic.List[object]
    while ($true) {
        if ($St.P -ge $St.B.Length) { throw 'shortcuts.vdf is cut short' }
        $type = [int]$St.B[$St.P]
        $St.P++
        if ($type -eq 8) { return , $list }
        $key = Read-CString $St
        if ($type -eq 0) {
            $value = Read-BinaryMap $St
        } elseif ($type -eq 1) {
            $value = Read-CString $St
        } else {
            $size = switch ($type) { 2 { 4 } 3 { 4 } 4 { 4 } 6 { 4 } 7 { 8 } 10 { 8 } default { 0 } }
            if ($size -eq 0) { throw "shortcuts.vdf has an unknown field type $type" }
            if ($St.P + $size -gt $St.B.Length) { throw 'shortcuts.vdf is cut short' }
            if ($type -eq 2) {
                $value = [BitConverter]::ToInt32($St.B, $St.P)
            } else {
                $value = New-Object byte[] $size
                [Array]::Copy($St.B, $St.P, $value, 0, $size)
            }
            $St.P += $size
        }
        $list.Add([pscustomobject]@{ Type = $type; Key = $key; Value = $value })
    }
}

function Read-BinaryVdf([byte[]]$Bytes) {
    $st = @{ B = $Bytes; P = 0 }
    $root = Read-BinaryMap $st
    if ($st.P -ne $Bytes.Length) { throw 'shortcuts.vdf has unexpected data at its end' }
    return , $root
}

function Write-CString($Out, [string]$s) {
    $b = $Latin1.GetBytes($s)
    if ([Array]::IndexOf($b, [byte]0) -ge 0) { throw 'a text holds a zero byte' }
    $Out.Write($b, 0, $b.Length)
    $Out.WriteByte(0)
}

function Write-BinaryMap($Out, $List) {
    foreach ($e in $List) {
        $Out.WriteByte([byte]$e.Type)
        Write-CString $Out $e.Key
        if ($e.Type -eq 0) {
            Write-BinaryMap $Out $e.Value
        } elseif ($e.Type -eq 1) {
            Write-CString $Out $e.Value
        } elseif ($e.Type -eq 2) {
            $Out.Write([BitConverter]::GetBytes([int32]$e.Value), 0, 4)
        } else {
            $Out.Write($e.Value, 0, $e.Value.Length)
        }
    }
    $Out.WriteByte(8)
}

function Write-BinaryVdf($Root) {
    $out = New-Object IO.MemoryStream
    Write-BinaryMap $out $Root
    return , $out.ToArray()
}

function Find-MapEntry($List, [string]$Key) {
    foreach ($e in $List) { if ($e.Key -ieq $Key) { return $e } }
    return $null
}

function Get-Crc32([byte[]]$Bytes) {
    $crc = [int64]4294967295
    foreach ($b in $Bytes) {
        $crc = $crc -bxor $b
        for ($k = 0; $k -lt 8; $k++) {
            if ($crc -band 1) { $crc = ($crc -shr 1) -bxor [int64]3988292384 } else { $crc = $crc -shr 1 }
        }
    }
    return $crc -bxor [int64]4294967295
}

# The id Steam itself gives a non-Steam game: crc32(Exe + AppName) with the
# top bit set, stored as a signed int32.
function Get-ShortcutAppId([string]$Exe, [string]$Name) {
    $id = (Get-Crc32 $Latin1.GetBytes($Exe + $Name)) -bor [int64]2147483648
    return [int32]($id - [int64]4294967296)
}

function New-SkhidShortcut([string]$Index, [string]$Exe, [string]$StartDir, [string]$Icon) {
    $fields = New-Object System.Collections.Generic.List[object]
    $add = { param($t, $k, $v) $fields.Add([pscustomobject]@{ Type = $t; Key = $k; Value = $v }) }
    & $add 2 'appid' (Get-ShortcutAppId $Exe $ShortcutName)
    & $add 1 'AppName' $ShortcutName
    & $add 1 'Exe' $Exe
    & $add 1 'StartDir' $StartDir
    & $add 1 'icon' $Icon
    & $add 1 'ShortcutPath' ''
    & $add 1 'LaunchOptions' ''
    & $add 2 'IsHidden' 0
    & $add 2 'AllowDesktopConfig' 1
    & $add 2 'AllowOverlay' 1
    & $add 2 'OpenVR' 0
    & $add 2 'Devkit' 0
    & $add 1 'DevkitGameID' ''
    & $add 2 'DevkitOverrideAppID' 0
    & $add 2 'LastPlayTime' 0
    & $add 1 'FlatpakAppID' ''
    & $add 0 'tags' (New-Object System.Collections.Generic.List[object])
    return [pscustomobject]@{ Type = 0; Key = $Index; Value = $fields }
}

function Test-IsSkhid([string]$s) {
    return ($null -ne $s -and $s.IndexOf($RunBat, [StringComparison]::OrdinalIgnoreCase) -ge 0)
}

function Test-SkhidShortcut($Entry) {
    if ($Entry.Type -ne 0) { return $false }
    $exe = Find-MapEntry $Entry.Value 'Exe'
    return ($null -ne $exe -and $exe.Type -eq 1 -and (Test-IsSkhid $exe.Value))
}

function Get-ShortcutList($Root) {
    $e = Find-MapEntry $Root 'shortcuts'
    if ($null -eq $e -or $e.Type -ne 0) { throw 'shortcuts.vdf has no shortcuts list' }
    return , $e.Value
}

# ------------------------------------------------------------------- Steam

function Find-Steam {
    if ($SteamPath) { $path = $SteamPath } else {
        $path = $null
        foreach ($key in 'HKCU:\Software\Valve\Steam', 'HKLM:\SOFTWARE\WOW6432Node\Valve\Steam', 'HKLM:\SOFTWARE\Valve\Steam') {
            try {
                $item = Get-ItemProperty -Path $key -ErrorAction Stop
                if ($item.SteamPath) { $path = $item.SteamPath } elseif ($item.InstallPath) { $path = $item.InstallPath }
            } catch { }
            if ($path) { break }
        }
        if ($path) { $path = $path.Replace('/', '\') }
    }
    if (-not $path -or -not (Test-Path -LiteralPath ([IO.Path]::Combine($path, 'steamapps')))) {
        throw 'Steam was not found. Run this again with -SteamPath "<Steam folder>".'
    }
    return $path
}

function Test-SurvariumInstalled([string]$Steam) {
    $libraries = New-Object System.Collections.Generic.List[string]
    $libraries.Add($Steam)
    $file = [IO.Path]::Combine($Steam, 'steamapps', 'libraryfolders.vdf')
    if (Test-Path -LiteralPath $file) {
        $root = Read-TextVdf (Read-ByteText $file)
        foreach ($top in $root.Entries) {
            if ($top.Value -is [string]) { continue }
            foreach ($e in $top.Value.Entries) {
                if ($e.Key -notmatch '^\d+$') { continue }
                if ($e.Value -is [string]) { $libraries.Add((ConvertFrom-ByteString $e.Value)) }
                else {
                    $p = Get-TextVdfValue $e.Value @() 'path'
                    if ($p) { $libraries.Add((ConvertFrom-ByteString $p)) }
                }
            }
        }
    }
    foreach ($lib in $libraries) {
        if (Test-Path -LiteralPath ([IO.Path]::Combine($lib, 'steamapps', "appmanifest_$SurvariumAppId.acf"))) { return $true }
    }
    return $false
}

function Get-SteamUsers([string]$Steam) {
    $names = @{}
    $login = [IO.Path]::Combine($Steam, 'config', 'loginusers.vdf')
    if (Test-Path -LiteralPath $login) {
        try {
            $users = Find-VdfNode (Read-TextVdf (Read-ByteText $login)) @('users')
            foreach ($e in $users.Entries) {
                $name = Get-TextVdfValue $e.Value @() 'PersonaName'
                if ($name) { $names[[string]([decimal]$e.Key - [decimal]76561197960265728)] = ConvertFrom-ByteString $name }
            }
        } catch { }
    }
    $dir = [IO.Path]::Combine($Steam, 'userdata')
    $list = @()
    if (Test-Path -LiteralPath $dir) {
        foreach ($d in (Get-ChildItem -LiteralPath $dir -Directory | Sort-Object Name)) {
            if ($d.Name -notmatch '^\d+$' -or $d.Name -eq '0') { continue }
            $label = if ($names.ContainsKey($d.Name)) { "$($names[$d.Name]) ($($d.Name))" } else { $d.Name }
            $list += [pscustomobject]@{ Id = $d.Name; Label = $label; Config = [IO.Path]::Combine($d.FullName, 'config') }
        }
    }
    return , $list
}

function Confirm-Yes([string]$Question) {
    return ((Read-Host "$Question (y/n)").Trim() -match '^y')
}

function Select-SteamUsers($Users) {
    if ($Users.Count -eq 0) { throw 'No Steam user was found. Sign in to Steam once, then run this again.' }
    if ($User) {
        if ($User -eq 'all') { return , $Users }
        $chosen = @($Users | Where-Object { $_.Id -eq $User })
        if ($chosen.Count -eq 0) { throw "Steam user $User was not found." }
        return , $chosen
    }
    if ($Users.Count -eq 1 -or $Yes) { return , $Users }
    Write-Host 'Steam users on this PC:'
    for ($i = 0; $i -lt $Users.Count; $i++) { Write-Host "  $($i + 1)) $($Users[$i].Label)" }
    $answer = (Read-Host 'Which one? Enter its number, or A for all').Trim()
    if ($answer -match '^[aA]') {
        if (-not (Confirm-Yes "Change all $($Users.Count) Steam users?")) { throw 'Nothing was changed.' }
        return , $Users
    }
    $n = 0
    if (-not [int]::TryParse($answer, [ref]$n) -or $n -lt 1 -or $n -gt $Users.Count) { throw 'No such user. Nothing was changed.' }
    return , @($Users[$n - 1])
}

function Wait-SteamClosed {
    if (-not (Get-Process -Name $SteamProcessName -ErrorAction SilentlyContinue)) { return }
    Write-Host 'Steam is running. Close it (Steam menu > Exit); this waits for it...'
    while (Get-Process -Name $SteamProcessName -ErrorAction SilentlyContinue) { Start-Sleep -Milliseconds 500 }
    Start-Sleep -Seconds 1
}

function Copy-Backup([string]$Path) {
    Copy-Item -LiteralPath $Path -Destination ($Path + $BackupSuffix) -Force
}

# ------------------------------------------------------------ the two modes

function Set-LaunchOptions($U, [string]$RunPath) {
    $file = [IO.Path]::Combine($U.Config, 'localconfig.vdf')
    if (-not (Test-Path -LiteralPath $file)) { return "skipped: no localconfig.vdf (sign in to Steam once with this user)" }
    $text = Read-ByteText $file
    $want = ConvertTo-ByteString ('"' + $RunPath + '" %command%')
    $current = Get-TextVdfValue (Read-TextVdf $text) $AppPath 'LaunchOptions'
    if ($current -ceq $want) { return 'already set' }
    if (-not (Test-IsSkhid $current)) { Copy-Backup $file }
    Write-ByteText $file (Set-TextVdfValue $text $AppPath 'LaunchOptions' $want)
    return 'Play on Survarium now starts Skhid'
}

function Remove-LaunchOptions($U) {
    $file = [IO.Path]::Combine($U.Config, 'localconfig.vdf')
    if (-not (Test-Path -LiteralPath $file)) { return $null }
    $text = Read-ByteText $file
    if (-not (Test-IsSkhid (Get-TextVdfValue (Read-TextVdf $text) $AppPath 'LaunchOptions'))) { return $null }
    $before = $null
    if (Test-Path -LiteralPath ($file + $BackupSuffix)) {
        $before = Get-TextVdfValue (Read-TextVdf (Read-ByteText ($file + $BackupSuffix))) $AppPath 'LaunchOptions'
        if (Test-IsSkhid $before) { $before = $null }
    }
    if ($null -eq $before) { $text = Remove-TextVdfValue $text $AppPath 'LaunchOptions' }
    else { $text = Set-TextVdfValue $text $AppPath 'LaunchOptions' $before }
    Write-ByteText $file $text
    return 'Survarium launch options restored'
}

function Add-Shortcut($U, [string]$RunPath, [string]$Icon) {
    $file = [IO.Path]::Combine($U.Config, 'shortcuts.vdf')
    if (Test-Path -LiteralPath $file) {
        $root = Read-BinaryVdf ([IO.File]::ReadAllBytes($file))
    } else {
        if (-not (Test-Path -LiteralPath $U.Config)) { [void](New-Item -ItemType Directory -Path $U.Config) }
        $root = New-Object System.Collections.Generic.List[object]
        $root.Add([pscustomobject]@{ Type = 0; Key = 'shortcuts'; Value = (New-Object System.Collections.Generic.List[object]) })
    }
    $list = Get-ShortcutList $root
    $exe = ConvertTo-ByteString ('"' + $RunPath + '"')
    $dir = ConvertTo-ByteString ('"' + [IO.Path]::GetDirectoryName($RunPath) + [IO.Path]::DirectorySeparatorChar + '"')
    foreach ($e in $list) {
        if (-not (Test-SkhidShortcut $e)) { continue }
        $exeField = Find-MapEntry $e.Value 'Exe'
        if ($exeField.Value -ceq $exe) { return 'already in the library' }
        # The files were moved: point the same shortcut at their new place.
        $exeField.Value = $exe
        $start = Find-MapEntry $e.Value 'StartDir'
        if ($null -ne $start) { $start.Value = $dir }
        [IO.File]::WriteAllBytes($file, (Write-BinaryVdf $root))
        return 'shortcut updated to this folder'
    }
    if (Test-Path -LiteralPath $file) { Copy-Backup $file }
    $index = $list.Count
    while ($null -ne (Find-MapEntry $list ([string]$index))) { $index++ }
    $list.Add((New-SkhidShortcut ([string]$index) $exe $dir (ConvertTo-ByteString $Icon)))
    [IO.File]::WriteAllBytes($file, (Write-BinaryVdf $root))
    return "'$ShortcutName' added to the library (non-Steam game)"
}

function Remove-Shortcut($U) {
    $file = [IO.Path]::Combine($U.Config, 'shortcuts.vdf')
    if (-not (Test-Path -LiteralPath $file)) { return $null }
    $root = Read-BinaryVdf ([IO.File]::ReadAllBytes($file))
    $list = Get-ShortcutList $root
    $kept = @($list | Where-Object { -not (Test-SkhidShortcut $_) })
    if ($kept.Count -eq $list.Count) { return $null }
    $list.Clear()
    $i = 0
    foreach ($e in $kept) {
        if ($e.Key -match '^\d+$') { $e.Key = [string]$i; $i++ }
        $list.Add($e)
    }
    [IO.File]::WriteAllBytes($file, (Write-BinaryVdf $root))
    return "'$ShortcutName' shortcut removed"
}

function Find-GameFolder {
    foreach ($base in $PSScriptRoot, [IO.Path]::GetDirectoryName($PSScriptRoot)) {
        if (-not $base) { continue }
        $x86 = [IO.Path]::Combine($base, 'game', 'binaries', 'x86')
        if (Test-Path -LiteralPath ([IO.Path]::Combine($x86, 'launcher_skhid.exe'))) { return $x86 }
    }
    return $null
}

# Steam shows "playing" while skhid-steam-run.bat runs: it waits here until
# the game has closed (or the launcher and its updater, if the game was not
# started). Two empty looks in a row, so that the launcher's restart after an
# update, or a game started again, does not end it.
function Wait-SkhidGame {
    $seen = $false
    $quiet = 0
    while ($quiet -lt 2) {
        Start-Sleep -Milliseconds ([int]($PollSeconds * 1000))
        if (Get-Process -Name 'survarium_skhid' -ErrorAction SilentlyContinue) {
            $seen = $true
            $quiet = 0
        } elseif (-not $seen -and (Get-Process -Name 'launcher_skhid', 'updater_skhid' -ErrorAction SilentlyContinue)) {
            $quiet = 0
        } else {
            $quiet++
        }
    }
}

function Invoke-Main {
    if ($WaitForGame) { Wait-SkhidGame; return 0 }
    $steam = Find-Steam
    $x86 = $null
    if (-not $Remove) {
        $x86 = Find-GameFolder
        if (-not $x86) { throw "Skhid's game was not found. Put these files in the Skhid game folder, beside Skhid.bat." }
    }
    $runPath = [IO.Path]::Combine($PSScriptRoot, $RunBat)
    if (-not $Remove -and -not (Test-Path -LiteralPath $runPath)) { throw "$RunBat is missing. Keep it beside this script." }
    $users = Select-SteamUsers (Get-SteamUsers $steam)
    Wait-SteamClosed
    $installed = (-not $Remove) -and (Test-SurvariumInstalled $steam)
    $icon = ''
    if ($x86 -and (Test-Path -LiteralPath ([IO.Path]::Combine($x86, 'survarium_skhid.exe')))) { $icon = [IO.Path]::Combine($x86, 'survarium_skhid.exe') }
    foreach ($u in $users) {
        if ($Remove) {
            $done = @((Remove-LaunchOptions $u), (Remove-Shortcut $u)) | Where-Object { $_ }
            if (-not $done) { $done = @('nothing to remove') }
            Write-Host "$($u.Label): $($done -join '; ')."
        } elseif ($installed) {
            Write-Host "$($u.Label): $(Set-LaunchOptions $u $runPath)."
        } else {
            Write-Host "$($u.Label): $(Add-Shortcut $u $runPath $icon)."
        }
    }
    if ($Remove) { Write-Host 'Done. Start Steam.' }
    elseif ($installed) { Write-Host 'Done. Start Steam and press Play on Survarium.' }
    else { Write-Host "Done. Start Steam and play '$ShortcutName' from your library." }
    return 0
}

if ($MyInvocation.InvocationName -ne '.') {
    try {
        exit (Invoke-Main)
    } catch {
        Write-Host "Error: $($_.Exception.Message)" -ForegroundColor Red
        exit 1
    }
}
