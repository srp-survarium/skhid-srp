# Skhid on Steam

A script handed to a player who wants Steam to show "Playing Survarium" while
he plays on Skhid. Not in the players' ZIP (see the end).

## Files for the player

Put in the Skhid game folder, beside `Skhid.bat` (or in a folder there):

- `skhid-steam.bat`: double-click to set it up.
- `skhid-steam-remove.bat`: double-click to undo it.
- `skhid-steam-run.bat`: what Steam starts.
- `skhid-steam.ps1`: does the work (both `.bat` files above run it with
  `-ExecutionPolicy Bypass`).
- `README.txt`: the paragraph for the player.

## What it does

Steam is found from `HKCU\Software\Valve\Steam\SteamPath` (or `-SteamPath`).
One Steam user in `userdata`: that one; several: it asks which, or all after
a yes (`-User <id>|all`, `-Yes`). Steam must be closed: it says so and waits.

- Survarium (app 355840) installed in a library of
  `steamapps/libraryfolders.vdf` (`appmanifest_355840.acf`): `LaunchOptions`
  of apps/355840 in `userdata/<id>/config/localconfig.vdf` becomes
  `"<folder>\skhid-steam-run.bat" %command%`, so Play starts Skhid. The value
  is replaced in the text, or its line added; nothing else in the file
  changes. `-Remove` puts back the value of the backup (or removes the line).
- Not installed: a non-Steam game "Survarium" (Exe the same `.bat`, icon
  `survarium_skhid.exe`, appid crc32(Exe + name) | 0x80000000 as Steam
  makes it) is added to `userdata/<id>/config/shortcuts.vdf` (binary VDF,
  read and written back byte for byte). `-Remove` removes it.
- Before a first change each file is copied to `<file>.before-skhid`.
- `skhid-steam-run.bat` starts `launcher_skhid.exe` as `Skhid.bat` does,
  then `skhid-steam.ps1 -WaitForGame`: it returns once
  `survarium_skhid.exe` has closed, or the launcher and updater if the game
  never started (two empty looks 3 s apart, for the launcher's restart).

## Tests

    nix shell nixpkgs#python3 nixpkgs#powershell -c python3 client/steam/test_skhid_steam.py -v

PowerShell 7 on Linux, against a made-up Steam folder: text and binary VDF
against independent readers and writers here (round trips, CRLF, escapes,
UTF-8 names), both modes end to end with `-Remove`, the user question,
waiting for Steam, and `-WaitForGame` against stand-in processes.

Windows only, not tested here: Windows PowerShell 5.1, the registry lookup,
the three `.bat` files, Steam taking the launch options and the shortcut,
and "Playing Survarium" shown for the whole game.

## Into the ZIP later

Add the four script files and the README text at the ZIP's root, beside
`Skhid.bat`, in `srp-026g/client-patches/make_client_026g.py` (the list of
files it writes, and its README listing); `skhid-steam-run.bat` already looks
for `game\binaries\x86` there first.
