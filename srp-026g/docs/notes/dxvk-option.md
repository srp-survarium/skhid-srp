# Vulkan renderer (DXVK), opt-in

The owner (2026-11-05): let Windows players try DXVK; off by default.

## The option

- **Where:** Settings, Graphics, left column, right under Monitor: a "Renderer" list built as
  Monitor's (its label on its left, the same size), "DirectX 11" (the default) or "Vulkan/DXVK"
  (`st_srp_renderer_option`, `_dx11`, `_dxvk`, ten languages; "Vulkan (DXVK)" does not fit Monitor's
  field). The rows under it move down one of Monitor's steps and the hint's room; the Optimal and Default buttons go
  side by side in their panel (`flash/main_menu/src/SrpRendererChoice.as`, `SrpVideoTab.as` with the
  FOV slider).
- **Restart hint:** the Language choice's "Applies after restart." (`st_srp_language_restart`, its
  style) under the list at its label's left edge, while the saved choice is not the renderer this
  process runs (the exe's answer says it: DXVK loaded at the start or not). Its room is kept
  whether it shows or not: the rows under the list are one step and the note's room lower, so
  nothing moves when it appears; both lists' open popups lie over it.
- **DirectX 11 by default.** Apply saves it as `srp_dxvk on|off` (on: Vulkan) in
  `Documents\survarium\user.cfg`, as the HUD crosshair list keeps its Hidden choice (exe patch 83's
  saved bools, `hide-hit-markers-scores.md`); Cancel puts the saved choice back.
- **At the next start:** the game's Direct3D DLLs (d3d9, dxgi, d3d11) are delayed imports, so a
  module of the same name loaded first serves them. Nothing changes in a running game.
- **Linux:** nothing changes; `play_skhid.sh` installs DXVK into the Wine prefix as before.

## The files

- The Skhid ZIP ships DXVK 3.1.1's 32-bit DLLs in `game\binaries\x86\dxvk\`, ordinary overlay
  files: the set `winetricks dxvk` installs into a Wine prefix (the setup the Linux clients run),
  `d3d8.dll`, `d3d9.dll`, `d3d10core.dll`, `d3d11.dll`, `dxgi.dll` (24 MB, about 9.4 MB in the ZIP),
  unchanged. `make_client_026g.py` takes them from the release archive `dxvk-3.1.1.tar.gz` (GitHub,
  sha256 `40565b4a…`), downloaded once into `~/.cache/skhid`; the archive and every DLL are pinned.
  The updater installs and checks them as any Skhid file; it knows nothing of DXVK.
- DXVK 3.x needs a device with **Vulkan 1.4** (DXVK 2.x needed 1.3).

## The game's start (exe patch 83, `memory.allocate_arenas` 0xE9580)

- The site: `vostok::memory::platform::allocate_arenas`' start, once, from
  `memory::allocate_region`: before the VRAM check's `Direct3DCreate9` (0xE9C3F) and the render
  device (`CreateDXGIFactory` 0x3BA7D3, `D3D11CreateDevice` 0x3BB289), before the engine's memory
  and console. So patch 83 reads user.cfg itself: `SHGetFolderPathA(CSIDL_PERSONAL)` as the engine
  finds its user folder (`client_patch.md`, "The user folder"), `\survarium\user.cfg`, into a
  static 64 KiB buffer; its first `srp_dxvk` line.
- One state and one transition (`Renderer`, `step` in `hud_options.rs`):
  - DirectX 11 picked, or a file of `dxvk\` missing: Windows' own Direct3D; a marker left goes.
  - Vulkan/DXVK picked, all five there, no marker: `dxvk.pending` written first, then the Vulkan
    look: vulkan-1.dll loaded, an instance made, the devices' `apiVersion` read (the CPU's not
    counted), the instance destroyed, the DLL let go. A device of 1.4 or later: DXVK's DLLs loaded
    by their full paths, dxgi, d3d11, d3d10core, d3d9, d3d8 (each one's imports by name are of
    those before it), never let go; one that fails stops the loads, logged, and the marker covers
    a crash. Otherwise Windows' own, the marker deleted.
  - Vulkan/DXVK picked and a marker left: the last start with DXVK never showed its login screen
    (a driver that crashed in the look, in d3d9's VRAM check, the factory or the device): Windows'
    own, the marker deleted, and at the login screen (user.cfg has run there, the console's value
    is the file's) `srp_dxvk` off and user.cfg saved: the list shows DirectX 11; picking Vulkan
    again tries once more.
  - The login screen (`login_menu.activated`) deletes the marker in every case.
- Every outcome but plain DirectX writes one `dxvk:` line in `srp_patches.log`.
- **Why it works** (`../../scripts/dxvk-loader/run.sh`, Wine 11.8): a program that preloads the five
  from `dxvk\` by full path and then asks for each plain name gets the preloaded module, one module
  of each name in the process (DXVK's d3d11 bound to its dxgi), and its delay-loaded
  `D3D11CreateDevice` runs DXVK's (DXVK 3.1.1's log, a device made). Windows documents the same:
  "If a DLL with the same module name is already loaded in memory, the system checks only for
  redirection and a manifest before resolving to the loaded DLL, no matter which directory it is
  in" (Dynamic-link library search order, learn.microsoft.com). None of the five is a KnownDLL
  (Windows 10/11's list, the same in Wine's `Session Manager\KnownDLLs`: advapi32, gdi32, kernel32,
  ole32, shell32, user32, ...; no Direct3D DLL).

## Known limits

- A start that fails with DXVK costs one start; the next is DirectX 11 again by itself.
- DXVK's DLLs import the Universal C Runtime (`api-ms-win-crt-*`): Windows 10 and 11 have it, a
  Windows 7 or 8.1 without its update fails at the start with DXVK (and then falls back, above).
- The Vulkan look runs in the game's process: a Vulkan layer that hangs there hangs the start
  (one that crashes leaves the marker).
- Antivirus: DXVK's 3.1.1 release notes say some antivirus programs flag its 32-bit builds. A
  false positive on `dxvk\` stops every Windows player's update ("Game files removed by
  antivirus", `Skhid-defender-setup.cmd`), as for any Skhid file. Check the five hashes on
  VirusTotal before the release. DXVK's release has no signature: its DLLs are not
  Authenticode-signed and it ships no checksum file; GitHub's asset digest (sha256) is the one
  pinned here.
- Not checked on Windows; not played.
