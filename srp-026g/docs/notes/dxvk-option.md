# Vulkan renderer (DXVK), opt-in

The owner (2026-11-05): let Windows players try DXVK; off by default.

## The option

- **Where:** Settings, Graphics, left column, right under Monitor: a "Renderer" list built as
  Monitor's (its label on its left, the same size), "DirectX 11" (the default) or "Vulkan/DXVK"
  (`st_srp_renderer_option`, `_dx11`, `_dxvk`, ten languages; "Vulkan (DXVK)" does not fit Monitor's
  field). The rows under it move down one of Monitor's steps; the Optimal and Default buttons go
  side by side in their panel (`flash/main_menu/src/SrpRendererChoice.as`, `SrpVideoTab.as` with the
  FOV slider). No restart hint: the tab says none for its other settings either.
- **DirectX 11 by default.** Apply saves it as `srp_dxvk on|off` (on: Vulkan) in
  `Documents\survarium\user.cfg`, as the HUD crosshair list keeps its Hidden choice (exe patch 83's
  saved bools, `hide-hit-markers-scores.md`); Cancel puts the saved choice back.
- **At the next start:** the game imports `d3d11.dll` and `dxgi.dll`, which Windows takes from the
  game's folder first, so only the updater, before PLAY, can switch them. Nothing changes in a
  running game.
- **Linux:** nothing changes; `play_skhid.sh` installs DXVK into the Wine prefix as before.

## The files

- The Skhid ZIP ships DXVK 3.1.1's 32-bit `d3d11.dll` and `dxgi.dll` in `game\binaries\x86\dxvk\`
  (14 MB, about 5.6 MB in the ZIP), unchanged. `make_client_026g.py` takes them from the release
  archive `dxvk-3.1.1.tar.gz` (GitHub, sha256 `40565b4a…`), downloaded once into `~/.cache/skhid`;
  the archive and both DLLs are pinned.
- DXVK 3.x needs a device with **Vulkan 1.4** (DXVK 2.x needed 1.3). 3.1.1 is the version the
  local Wine clients run (winetricks).

## The updater (every run, after the overlay, before the patch step)

- `srp_dxvk on`, and a device that is not the CPU reports Vulkan 1.4 or later: the two DLLs are
  copied from `dxvk\` beside the game (skipped when already the same).
- Otherwise: the two DLLs beside the game are removed, **only when their hash is one of ours**
  (`OURS` in `client/updater/logic/src/dxvk.rs`). A `d3d11.dll` or `dxgi.dll` of the player's own is
  never replaced or removed; with one there, DXVK is not used.
- Both or neither: a copy that fails removes ours again; the two in `dxvk\` must be one release's.
- **Back to DirectX by itself** (the owner, 2026-11-05): the game owns the marker (exe patch 83).
  At `configs.start` (on_configs_loaded, before its renderer is made; d3d11 and dxgi are
  delay-loaded, so nothing of theirs has run), once a process, with a `d3d11.dll` beside it the
  game writes `dxvk.pending`; its login screen deletes it (`login_menu.activated`). The updater
  never writes it: one it finds with ours beside the game is a start with DXVK that never showed a
  screen. It then sets `srp_dxvk off` in user.cfg (that line only, every other byte kept), removes
  ours, and logs it; the list shows DirectX 11, and picking Vulkan again tries once more. It deletes a
  marker it finds in any case. Never starting the game, opening the launcher again, closing at the
  login screen or a server that is down keep DXVK on. One enum and one transition (`State`, `step`
  in `logic/src/dxvk.rs`): off; on and not started; on and confirmed; on and failed (turned off).
- The Vulkan check runs only for a player who asked, in a second run of the updater
  (`updater_skhid.exe vulkan`, 10 s at most), so a driver that crashes or hangs in it cannot stop
  the update.
- Best effort: every outcome is one `dxvk:` line in `updater_skhid.log`; it never fails the update
  or keeps PLAY off.

## Known limits

- A start that fails with DXVK costs one more launcher start: that run turns it off. A start that
  dies before its login screen for another reason (a crash of the game's own, the window closed
  while loading) turns it off too; picking Vulkan again is the way back.
- "Before its renderer is made" is read from the engine's order (user.cfg runs at configs.start,
  the renderer takes its settings from it), not traced in the code.
- Antivirus: DXVK's 3.1.1 release notes say some antivirus programs flag its 32-bit builds. A
  false positive on `dxvk\` stops every Windows player's update ("Game files removed by
  antivirus", `Skhid-defender-setup.cmd`), as for any Skhid file. Check the two hashes on
  VirusTotal before the release. DXVK's release ships no signature: its DLLs are not
  Authenticode-signed and there is no checksum file; GitHub's asset digest (sha256) is the one
  pinned here.
- Not checked on Windows; not played.
