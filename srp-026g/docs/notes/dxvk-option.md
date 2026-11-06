# Vulkan renderer (DXVK), opt-in

The owner (2026-11-05): let Windows players try DXVK; off by default.

## The option

- **Where:** Settings, Graphics, left column, right under Monitor: a "Renderer" list built as
  Monitor's (its label on its left, the same size), "DirectX 11" (the default) or "Vulkan/DXVK"
  (`st_srp_renderer_option`, `_dx11`, `_dxvk`, ten languages; "Vulkan (DXVK)" does not fit Monitor's
  field), and the "Vulkan options" button under it (below). The rows under them move down two of
  Monitor's steps and the hint's room; the preset panel (Optimal, Default side by side) stays where
  it was. The room is the field of view slider's, which went to the Gameplay tab, a group of the
  right column under the HUD group, its "FOV:" heading in the headings' look (the owner, 2026-11-06:
  "we can move fov to game maybe?"; `client-patches/README.md`,
  "Field-of-view slider") (`flash/main_menu/src/SrpRendererChoice.as`, `SrpVulkanOptions.as`,
  `SrpVideoTab.as`).
- **Restart hint:** the Language choice's "Applies after restart." (`st_srp_language_restart`, its
  style) under the Vulkan options' button at the labels' left edge, while a saved value (the choice
  or a Vulkan option) is not what this process runs (the exe's answers say it: DXVK loaded at the
  start or not, the options DXVK was given). Its room is kept whether it shows or not, so nothing
  moves when it appears; the lists' open popups lie over it.
- **DirectX 11 by default.** Apply saves it as `srp_dxvk on|off` (on: Vulkan) in
  `Documents\survarium\user.cfg`, as the HUD crosshair list keeps its Hidden choice (exe patch 83's
  saved bools, `hide-hit-markers-scores.md`); Cancel puts the saved choice back.
- **At the next start:** the game's Direct3D DLLs (d3d9, dxgi, d3d11) are delayed imports, so a
  module of the same name loaded first serves them. Nothing changes in a running game.
- **Linux:** nothing changes; `play_skhid.sh` installs DXVK into the Wine prefix as before.

## Vulkan options

The owner (2026-11-05): all five, clearly integrated; (2026-11-06) one row that opens a panel, the
window keeping its size.

- **Where:** a "Vulkan options" button (`st_srp_vulkan_options`) on the row right under the Renderer
  list, at the column's left edge, the tab's own (the Optimal and Default buttons' SetingsButton, at
  their width; wider only for a text that would not fit), with a drop-down's arrow at its right end and
  its text left of it (the owner, 2026-11-06: a button that clearly opens a menu; a drop-down row was
  worse). The arrow is the Renderer box's own art (DropdownMenuSkinnedLabel's shape, masked to its
  triangle) at 1.5 times its size, over the button (beside it in the tab, since a CLIK button sizes
  itself by its children's bounds); it turns up while the panel is open and greys with the button.
  A press opens the panel under it and a press on it again closes it, both in the button's own click
  handler (its pointer stays on: Scaleform routes presses otherwise than Ruffle, and a button without
  its pointer could not be pressed closed in the game, the owner 2026-11-06). While the panel is open
  the button is held on its "down" frames: after each state it draws (roll-over, roll-out, on the
  stage's render) the frames are set again; on closing it shows "over" or "up" as the pointer is. A
  press outside closes the panel: a stage MOUSE_DOWN listener (the stock lists' way), outside meaning
  neither the panel's frame nor the button under the point nor a target in them (Scaleform may make a
  container the target); while a list of the panel is open a press is that list's. The panel's art
  (shadow, frame, surface) takes no presses: the shadow's outline is filled over the button and the
  frame's bands are rings (one fill of two outlines) that a hit test need not take as drawn; a plain
  rectangle of alpha 0.01 over the panel (not the tab) is what a press on the panel lands on.
- **Its log:** every step of the button's path goes to `srp_patches.log` as `srp: menu: vulkan: ...`
  (the movie's `srp_log` call, which patch 83 writes; the movie's trace reaches no log in the game):
  the button's MOUSE_DOWN, MOUSE_UP and CLICK with their target and point, its click (open or not),
  open, close and the button's frame after, the held frames re-set (the first twenty while open), and
  each stage press with its target and whether it lands on the panel or the button.
- **The panel:** a popup over the column, a little wider than it on both sides, and one piece with the
  button, a tab that drops down: the Settings window's own border (its 1 px light line, then its dark
  band) runs round the panel and round the button above it, 4 px out, the band under the button left
  out, so the button opens into the panel. Its surface is the window's own background art
  (SrvSettingsWndow's shape's bitmap, character 273, added to the movie as `SrpSettingsGround`), placed
  where the window draws it, so the texture inside continues what is behind it. A shadow (a knocked-out
  DropShadowFilter of the outline: distance 6 down, blur 16, 0.9 black) lies outside the whole outline,
  so it sits above the tab. The owner, 2026-11-06: the panel a separate floating window, joined to its
  button, everything inside it looking as the same control outside does. Chosen from renders of: the
  border alone, with a shadow, a stronger shadow, a lighter surface, the tab, a notch pointing at the
  button; the menu's message backgrounds (MessageThinBackground and the others) are other surfaces
  (scratched metal, blue), not the tab's, so the window's own art stays. Its controls are the column's components, unchanged (the
  harness compares each with its twin: class, scale, size, colour, filters, frame, parts and text
  formats). Its rows on the column's grid (each list at the Renderer list's place and width, so its label and box
  are at the column's; each checkbox at Fullscreen's x, its text on its box as Fullscreen's), one line
  each, on one step (Monitor's list step, grown as little as
  ending in a gap of the column's rows takes: no row shows half below it), the same padding over
  the first and under the last. A press outside it, on the button again or Esc closes it (Esc closes the
  window only when no panel is open). The button is disabled (greyed as the stock buttons, not hidden)
  while the Renderer list shows DirectX 11, following the list's pick before Apply; picking DirectX 11
  closes an open panel.
- **Hints:** each of the five shows a hint when the pointer rests on it, the game's own tooltip (CLIK's
  ToolTip with the menu's tip, SrvProfileTipRenderer, 300 ms after the pointer comes over it), three
  lines (`st_srp_vulkan_*_hint`, ten languages): what it does, "+" what it gives, "-" what it costs.
  The tip lies 4 px under the option the pointer is on, never over it (above it when the window ends
  first), from 16 px right of the pointer, inside the window, at the menu's own scale (2 at 4K). It is
  opaque (the symbol's look: a 1 px grey edge round a near-black fill; the symbol's own fill let the
  rows under it show through), the text 6 px from each of its four edges. Each line is a field of its
  own in the tip's format; a line's leading "+ " or "- " goes into a column of its own, the sign centred
  in it on its line's baseline, so the texts after "+" and "-" start at one x in every language. The
  checkboxes' labels: Scaleform centres them on their box (CheckBoxMultiline's vertical align), as the
  stock Fullscreen's in the game; Ruffle draws them high, so the harness moves them for its
  screenshots. The main menu's copy of
  ToolTip needed: its delay timer repeated forever, so the tip never came (`Timer(300,1)`, the lobby
  copy's); a tip is raised over the popups (PopUpManager puts its canvas back on top of whatever the
  stage gets, so the tip lay under the panel); the placement above. The menu's tip is used by these
  hints only (every other control of the movie sets no tip renderer).
- **The five** (each kept in user.cfg, `st_srp_vulkan_*` texts in ten languages):
  - Frame rate limit: Off, 60, 120, 144, 165, 240 (`srp_dxvk_frame_limit 0|60|...`) ->
    `dxvk.maxFrameRate = <n>`.
  - Performance overlay: Off, FPS, FPS+graph, Full (`srp_dxvk_overlay off|fps|frametimes|full`) ->
    `dxvk.hud = fps`, `fps,frametimes`, `fps,frametimes,gpuload,compiler,memory`.
  - Reduce input lag, off by default (`srp_dxvk_low_latency`) -> `dxvk.latencySleep = True`.
  - Tear-free with vsync off, off by default (`srp_dxvk_tear_free`) -> `dxvk.tearFree = True`.
  - Prefer the dedicated GPU, on by default: no line in user.cfg is on (`srp_dxvk_dedicated_gpu`) ->
    `dxvk.hideIntegratedGraphics = True`. DXVK 3.1.1 hides the integrated GPUs only when it finds a
    dedicated one too (`dxvk_instance.cpp`: `numDGPU > 0 && numIGPU > 0`), so a PC with an
    integrated GPU alone keeps it; nothing to check on our side. It counts only the devices it can
    use: one under Vulkan 1.4 is skipped before (`dxvk_device_filter.cpp` testAdapter,
    `dxvk_device_info.cpp` checkDeviceCompatibility), so an integrated GPU of 1.4 beside a dedicated one
    of 1.3 is kept too.
  - An option off is DXVK's default, not named. The game's own anisotropic filtering, FXAA and
    vsync settings stay the game's.
- **Settings:** the five and the Renderer list are one change of the tab: a change is pending
  until Apply (each changed option saved, user.cfg at once), Cancel puts the saved ones back.
- **At the next start, only with Vulkan/DXVK** (Windows and Linux alike): patch 83's start reads
  them with the Renderer from user.cfg once (`Choice::Vulkan(VulkanOptions)`: an unknown word is the
  list's first choice, a checkbox's its default), and the start that loads DXVK sets the process's
  `DXVK_CONFIG` from them (SetEnvironmentVariableA, the exe's import) right before DXVK's DLLs load.
  DXVK reads it with GetEnvironmentVariableW, the same Win32 environment, when the game makes its
  first device; a value of 260 characters or more reads as none (all five: 158). The variable is
  set (empty with every option off) whatever the player's own environment has; a `dxvk.conf` in
  `binaries\x86` and `DXVK_HUD` still add to it, as DXVK reads them.
- `srp_patches.log` says what was given: `dxvk: DXVK_CONFIG "<value>"` (an option is written whole or
  not at all; one left out for want of room, none ever with these five, is counted on the line). Nothing changes in a running
  game; DirectX 11 runs ignore the five.

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
    counted), the instance destroyed, the DLL let go. A device of 1.4 or later: `DXVK_CONFIG` set
    from the Vulkan options (above), then DXVK's DLLs loaded by their full paths, dxgi, d3d11,
    d3d10core, d3d9, d3d8 (each one's imports by name are of
    those before it), never let go; one that fails stops the loads, logged, and the marker covers
    a crash. As each loads, the game's delayed import of its name (dxgi, d3d11, d3d9) is bound to
    it: its module slot (`survarium::image` `dxgi_module`, `d3d11_module`, `d3d9_module`) holds
    the DLL, and `__delayLoadHelper2` (0xE48E0) loads nothing for a slot already set (below).
    Otherwise Windows' own, the marker deleted.
  - Vulkan/DXVK picked and a marker left: the last start with DXVK never showed its login screen
    (a driver that crashed in the look, in d3d9's VRAM check, the factory or the device): Windows'
    own, the marker deleted, and at the login screen (user.cfg has run there, the console's value
    is the file's) `srp_dxvk` off and user.cfg saved: the list shows DirectX 11; picking Vulkan
    again tries once more.
  - The login screen (`login_menu.activated`) deletes the marker in every case.
- Every outcome but plain DirectX writes its `dxvk:` lines in `srp_patches.log` (a start with DXVK
  the `DXVK_CONFIG` it gave, then the loads).
- **Why the imports are bound, not left to the name** (the first Windows report, 2026-11-05, rev50,
  RTX 5070 Ti, Windows 10 19045): the Vulkan look itself loads Windows' `System32\dxgi.dll`
  (the Khronos loader sorts its devices through DXGI; the minidump lists it right before
  NVIDIA's `nvoglv32.dll`), and it stays after vulkan-1.dll is let go. The process then has two
  modules named dxgi.dll, and the game's plain-name load got Windows' one: its factory, DXVK's
  d3d11 device (`D3D11InternalCreateDevice: Adapter is not a DXVK adapter`), and
  `IDXGIFactory::CreateSwapChain` (vtable +28h) failing into the engine's assert,
  `res_render_output::initialize_swap_chain`'s int3 at 0x3CF765, before the login screen, at
  every try. Under Wine the prefix's own dxgi is DXVK's, so it never showed there.
  Reproduced under Wine with stand-ins, a `vulkan-1.dll` beside the exe that loads a
  `fake\dxgi.dll` (Wine's own dxgi, not DXVK's) and keeps it, then passes every call to Wine's
  vulkan-1: rev50's code crashes at the same 0x3CF765 with the same stack; with the slots bound
  the game's factory is DXVK's and it reaches the login screen.
  The game's VRAM probe `GetVideoMemoryViaDXGI` (0x106030, the DirectX SDK sample's) still loads
  `dxgi.dll` by its plain name (LoadLibraryW, then CreateDXGIFactory1): whichever dxgi it gets
  answers that, Windows' own included. DXVK's own d3d11 imports `CreateDXGIFactory1` from
  `dxgi.dll` by name, so on Windows it may be bound to Windows' one; DXVK calls it only for a
  device asked without an adapter, and the game's one `D3D11CreateDevice` (0x3BB289) always
  passes its factory's adapter (driver type 0).
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
- The Vulkan options: DXVK 3.1.1's taking of `DXVK_CONFIG` set in-process was checked under Wine
  11.8 with `../../scripts/dxvk-loader/run.sh <x32> <work> "<value>"` (all five set: DXVK's log has
  `Found config env: ...`, then each of them in `Effective configuration`), not with the game
  itself; the panel and its hints in Ruffle only (Scaleform's presses, Esc, the tip's place and
  font metrics are the game's to show).
- Not checked on Windows; not played.
