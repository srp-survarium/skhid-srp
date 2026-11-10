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
  A press on the button opens the panel under it (the button's own click). While the panel is open the
  button is held on its "down" frames: CLIK draws its state on the stage's render (in the game it went
  back to its first frame), so after each render the frames are set again; on closing it shows "over"
  or "up" as the pointer is. A press on the stage while it is open (a stage MOUSE_DOWN listener, the
  stock lists' way) is decided by where it lands only, never by its target: the game's log (build
  6b2fb5d4b) showed every press over the popup with the panel container as its target, the button
  getting none. In the panel's rectangle (its lists open inside it) the panel stays; on the button it
  closes, and that press's own click of the button opens nothing (Ruffle gives the click; the button's
  next MOUSE_DOWN clears it); anywhere else it closes. Esc and a DirectX 11 pick close it too.
- **Presses on the popup:** a child that takes no presses still gives its parent a hit where it is
  drawn (Flash's rule, the game's too), so the popup holds only the panel's rectangle (a Sprite filled
  at alpha 0.01, its one part a press lands on), the shadow and the controls; the popup itself takes
  none. The panel's frame and surface lie in the tab right under the button while open, so nothing of
  them is over the button: a press there is the button's, and over the column rows it is the tab's, an
  outside press. The shadow, in the popup over the tab's rows, lies outside the outline only, never over
  the button.
- **The panel:** a popup over the column, a little wider than it on both sides, and one piece with the
  button, a tab that drops down: the Settings window's own border (its 1 px light line, then its dark
  band) runs round the panel and round the button above it, the band under the button left out, so the
  button opens into the panel. Round the button the border's inner edge keeps the same gap off its
  drawn art on the left, the right and the top: the left is the panel's own (its edge, the band's width
  in), the right and the top the same gap off the button (its art fills its bounds: the harness draws
  it and scans the frame's pixels, the gaps within half a pixel). Its surface is the window's own
  background art (SrvSettingsWndow's shape's bitmap, character 273, added to the movie as
  `SrpSettingsGround`), placed where the window draws it, so the texture inside continues what is
  behind it. A drawn soft shadow (ten one-pixel bands of black, 0.6 fading to nothing, fainter over the
  top edges; rectangles, no filter: the game showed no DropShadowFilter knock-out) lies outside
  the whole outline, so the panel sits above the tab.
  The owner, 2026-11-06: the panel a separate floating window, joined to its
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
  ToolTip with the menu's tip, SrvProfileTipRenderer, 300 ms after the pointer comes over it): the
  lists' one line (what it does), the checkboxes' three (`st_srp_vulkan_*_hint`, ten languages): what it
  does, "+" what it gives, "-" what it costs (the owner: no pros and cons for the two lists).
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
  - Vulkan/DXVK picked, the four it loads there (not d3d8: below), no marker: `dxvk.pending` written first, then the Vulkan
    look: vulkan-1.dll loaded, an instance made, the devices' `apiVersion` read (the CPU's not
    counted), the instance destroyed, the DLL let go. A device of 1.4 or later: `DXVK_CONFIG` set
    from the Vulkan options (above), then DXVK's DLLs loaded by their full paths, dxgi, d3d11,
    d3d10core, d3d9 (each one's imports by name are of those before it), never let go; not
    d3d8.dll, which is shipped but which the game never imports (1.6 MiB of address space); one that fails stops the loads, logged, and the marker covers
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

## The fullscreen window

**The report** (2026-11-08 23:42 +03, rev53; AMD Radeon RX 6900 XT, AMD's Vulkan driver 2.0.353,
Windows 10.0.26100, the game started through Steam; user.cfg `r_fullscreen on`, `r_resolution
2560x1440`, `srp_dxvk on`): DXVK loaded (`srp_patches.log`, "another dxgi.dll is loaded already"),
its device made, and at the game's first Present its d3d11 log says `Presenter: Failed to create
Vulkan swapchain: -13` (VK_ERROR_UNKNOWN); DXVK's Present then answers E_FAIL
(`D3D11SwapChain::PresentImage`: a negative acquire), and the game's `renderer::render` asserts on a
failed Present that is no device loss (`renderer.cpp` line 2175, the int3 at 0x13DDBD, after
`res_render_output::present` 0x3CF7F0), before the login screen (the marker then makes the next
start DirectX 11). DXVK's dxgi log has one display mode change and no restore before it: one
fullscreen entry. The minidump has the rest: the Steam overlay's own log line,
`Sun Nov 08 20:42:23 2026 UTC - g_IDXGIFactory2_CreateSwapChainForHWND failed, hres=887a0001`
(DXGI_ERROR_INVALID_CALL; the dump is 20:42:24), and in the crashed thread's stack memory below the
crash's ESP stale return addresses (a scan, not a walked chain) of amdvlk32, D3D12, D3D12Core,
gameoverlayrenderer (Steam's DXGI hook), Windows' `System32\dxgi.dll` and amdxc32 (AMD's D3D12
driver): AMD's Windows Vulkan driver makes a DXGI flip-model swap chain on the game's window for its
own presentation, and DXGI refused that window. The window's styles at that moment are not in the
dump (no heap, no DXVK window state): they are inferred from the code below.

**DXGI's rule** (its debug layer: "The specified output window has a style that is incompatible with
DXGI", which AMD quoted in doitsujin/dxvk#5258 and named the cause of the same `-13` in
GPUOpen-Drivers/AMD-Gfx-Drivers#57, Fallout New Vegas under DXVK, where AMD confirmed that appending
WS_POPUP works; #57 is closed as fixed in the driver, still seen with Adrenalin 26.6.4 in July 2026):
the window needs one of WS_POPUP, WS_BORDER, WS_CAPTION, WS_SYSMENU, WS_MAXIMIZEBOX, WS_MINIMIZEBOX,
WS_THICKFRAME, WS_EX_CLIENTEDGE, WS_EX_CONTEXTHELP, WS_EX_DLGMODALFRAME, WS_EX_TOOLWINDOW,
WS_EX_WINDOWEDGE, or must not be WS_EX_TOPMOST.

**The game's window, every way** (the exe's only GWL_STYLE writes are set_size's two; DXVK's own):

- made `CreateWindowExA(0, .., WS_CAPTION | WS_SYSMENU)` (`new_window` 0x184F0, the call 0x186FF):
  taken;
- a start in fullscreen (`r_fullscreen on`, the report's): the render output is made fullscreen
  (`render_output_window_cook::translate_query` 0x138030, `res_render_output` +EDh) and set_size
  writes no style (its first call has nothing to change); `show` 0x3CF600 calls
  SetFullscreenState(TRUE), and DXVK 3.1.1's `enterFullscreenMode` (wsi_window_win32.cpp) takes
  away WS_OVERLAPPEDWINDOW and WS_EX_OVERLAPPEDWINDOW and makes it HWND_TOPMOST: WS_VISIBLE |
  WS_CLIPSIBLINGS, WS_EX_TOPMOST, refused;
- Settings to fullscreen in a running game: `set_size` 0x3CF980 sets GWL_STYLE to its +E0h, 0
  (0x3CFA09), then SetWindowPos(HWND_TOPMOST, SWP_SHOWWINDOW): refused too;
- Alt+Tab: the window's deactivation (message_processor 0x18307, engine_world::on_alttab 0x14020,
  game vtable +2Ch 0x1ED450) calls res_render_output::minimize 0x3CFE80 (SetFullscreenState(FALSE,
  the monitor's output), then ShowWindow(SW_FORCEMINIMIZE) if that succeeded; DXGI and DXVK refuse
  FALSE with an output, DXGI_ERROR_INVALID_CALL); DXVK's GetFullscreenState and Present leave it for a
  window not foreground for 100 ms, and present minimizes it (0x3CF8C5). Back: each engine tick
  (0x13D58, game vtable +30h 0x1ED430) calls res_render_output::goto_fullscreen 0x3CFE20, which
  enters fullscreen again when the window has the focus and is not minimized: the same strip,
  refused but for a WS_POPUP already there, which the strip keeps;
- windowed (a start, or Settings to windowed: set_size writes +DCh, WS_CAPTION | WS_SYSMENU): taken;
- DirectX 11: the game makes its swap chain windowed (desc Windowed 1, one buffer, DISCARD) while
  the window has its caption, and never another; Windows' DXGI keeps that swap chain through its
  fullscreen: the rule is never met.

**Where the Vulkan swap chain is made:** DXVK 3.1.1 makes it in `Presenter::recreateSwapChain` alone,
from `acquireNextImage` (`D3D11SwapChain::PresentImage`) and `checkSwapChainStatus` (a Present with
DXGI_PRESENT_TEST, which the game never passes): inside the game's IDXGISwapChain::Present, at its
first one and again after a resize, a mode change or a minimize. The game presents in
`res_render_output::present` 0x3CF7F0 alone (vtable +20h at 0x3CF90E; its callers 0x117EDF,
0x13CA02, 0x13CB73, 0x13DD46). The report's stack has it on the game's main thread, the window's
own: engine_world::run 0x18B30 loops over process_messages 0x18AC0 (PeekMessage, Translate,
Dispatch) and the engine tick, which runs the render commands (set_size, `show`), the alt-tab
re-entry above and then the render with its Present. So every style write and fullscreen entry
named here and every Present are one thread, in order, and SetWindowLongA there is a same-thread
call (its WM_STYLECHANGING/WM_STYLECHANGED go to the window procedure within the call).

**The fix** (patch 83, `render_output.present`, a lohook at 0x3CF8FF right before that Present):
with DXVK loaded at the start (`Renderer::Dxvk`), the window's two styles are read, and a window
DXGI refuses gets WS_POPUP (SetWindowLongA, on the window's thread, as set_size and DXVK set it),
said in `srp_patches.log` (`dxvk: the window DXGI refuses (style .., extended ..) made a popup:
style .., DXGI takes it`), once per refused state: styles that stay refused (a set Windows did not
take) are not set or said again until a Present finds a window DXGI takes. DXVK's later fullscreen
entries keep WS_POPUP (they take away WS_OVERLAPPEDWINDOW's styles only), so each later Present
only reads. A window DXGI takes (any windowed one) is never changed; with DirectX 11 nothing is
read. After DXVK leaves fullscreen by
itself (Alt+Tab) it puts back its saved styles only when the window's are what it left, so the
window stays a popup without its caption until the next fullscreen entry: the game minimizes it
then, unless it has the focus at that check (0x3CF8A6), when it stays a borderless popup at its last
size; Settings to windowed writes the caption back itself (set_size). This holds for every GPU under
DXVK (Wine too), not only AMD's. The renderer's state is a `Lock` now, by way of defence: no second
thread reading it is shown (the start, the login screen and every Present are the main thread's).

## The address space (SRP-3)

**The reports** (2026-11-08/09, rev53 and rev54; RX 7800 XT twice, RX 6900 XT; AMD's Windows Vulkan driver 2.0.353 to
2.0.416): with Vulkan on, crashes at every lobby reconnect at `d3d11.dll+0x74E17` (DXVK's, a read of 0x20), in
`amdvlk32.dll`, and in the game's own code. The game is 32-bit: 4 GiB of address space, and DXVK on AMD fills it.

- **What crashes** (PROVEN: DXVK 3.1.1's binary and source, reproduced). `0x74E17` is `DxvkBuffer::DxvkBuffer`
  (`dxvk_buffer.cpp:34`, `assignStorage(allocateStorage())`), reached from `CreateTexture2D` making a texture with CPU
  access its mapped buffer: `DxvkMemoryAllocator::createBufferResource` logs "Memory allocation failed" and answers
  null when no memory can be had, and the constructor reads through it. A 32-bit program filling its address space and
  then making 4 MiB staging textures (a test program, not in the repo) crashes at the same
  instruction with the same read, under DXVK on an AMD iGPU and on an RTX alike; Windows' own D3D11 refuses the texture
  instead. The live game on a PC of an RTX 5090 and an enabled Radeon iGPU, Vulkan on: 4073 MiB used 14 s after its
  start, then that crash, half a second after `atmosphere recalculated (... window_resized:true)` as in the report.
- **Where it goes** (measured, a device made and presented once, nothing else): Windows' D3D11 317 MiB on the RTX, 363
  on the AMD iGPU; DXVK 600 on the RTX, 811 to 921 on the AMD iGPU. On AMD the first Present alone takes about 213
  MiB: AMD's Vulkan driver presents through DXGI and loads its own D3D12 driver for it (`amdxc32.dll`, 69 MiB, and the
  D3D12 runtime). And the Vulkan loader loads every driver it knows, so a PC of two vendors' GPUs maps both (on
  riad5's, two AMD driver packages: 94 and 85 MiB). The players' minidumps: 451 to 606 MiB of modules with DXVK, 321
  to 330 without; BigRUS's two rev53 crashes peaked at 4062 and 4073 MiB of virtual size.

**What the start does now** (patch 83, `hud_options.rs` `look_for_dxvk`):

- **One vendor's drivers.** The Vulkan look asks the loader for one vendor's drivers at a time
  (`VK_LOADER_DRIVERS_SELECT`, a glob of their manifests' names: `amd*`, `nv*`, `igvk*`), AMD's, NVIDIA's, Intel's,
  until one has a discrete GPU that DXVK takes (`renderer.rs` `dxvk_pick`: DXVK 3.1.1's choice, discrete first, then
  integrated, at least Vulkan 1.4, the player's `DXVK_FILTER_DEVICE_NAME` honoured); the variable stays set to that
  vendor's, so DXVK's instance loads those alone. A look that finds another vendor's device means the loader took no
  such variable (it ignores it in an elevated process): then every driver, as before. A player's own
  `VK_LOADER_DRIVERS_SELECT` or `_DISABLE` is kept. `srp_patches.log`: `dxvk: Vulkan drivers: "nv*" only
  (VK_LOADER_DRIVERS_SELECT), DXVK's GPU NVIDIA GeForce RTX 5090`, or `all (...)` with why. Measured at the user's own
  level on that PC: AMD's D3D12 driver no longer loads with DXVK on the RTX, and the start that crashed signed in
  (peak 3621 MiB); the excluded vendor's main driver DLL is still mapped by the loader (85 MiB for AMD's), not its
  dependencies.
- **No descriptor heap on AMD.** When DXVK's GPU is AMD's, `DXVK_CONFIG` gets `dxvk.enableDescriptorHeap = False`, as
  DXVK decides by itself after 3.1.1 (doitsujin/dxvk 076922bd2, its #5837: DXVK 3 crashed at start on an RX 7900 XTX
  with it). Logged on its own line (`DXVK_CONFIG adds ...`); DXVK's log says `Binding model: Legacy`.
- **AMD's look first.** NVIDIA's driver (`nvoglv32.dll` and `nvgpucomp32.dll`, 111 MiB) stays mapped after its look's
  instance is destroyed and the loader let go; AMD's goes (measured with a test program and in the game's DLL load
  trace: NVIDIA's two load at the first look and stay to the end). With NVIDIA's look first, DXVK on a Radeon beside an
  RTX kept NVIDIA's 111 MiB for nothing. AMD's look comes first now: an RTX beside a Radeon iGPU costs AMD's look only,
  a Radeon dGPU stops the look before NVIDIA's driver is ever loaded, and an Optimus laptop (Intel iGPU, RTX) stops at
  the RTX before Intel's. Two discrete GPUs of AMD and NVIDIA: AMD's.
- **A vendor without a driver has no say** (review of #1596). A look whose glob matches no driver makes no instance
  (`VK_ERROR_INCOMPATIBLE_DRIVER`: the real loader leaves lavapipe's manifest out too), and only the looks that made
  one are compared: on a PC without NVIDIA's drivers (an RX alone, an Intel iGPU with a Radeon) AMD's look decides.
  Before the fix the comparison read the empty slot of NVIDIA's failed look, and such a PC loaded every driver.
- **The other vendors' layers off.** A driver manifest can declare an implicit layer in the same DLL: AMD's
  `VK_LAYER_AMD_switchable_graphics` (in `amdvlk32.dll`), NVIDIA's `VK_LAYER_NV_optimus` and `VK_LAYER_NV_present` (in
  `nvoglv32.dll`). The loader loads implicit layers whatever drivers `VK_LOADER_DRIVERS_SELECT` (or `VK_DRIVER_FILES`)
  lets it take, so the driver filter left AMD's 85 MiB DLL mapped beside an RTX. Each vendor's look, and DXVK's
  instance after, has the other vendors' layers turned off by their manifests' own switches
  (`DISABLE_LAYER_AMD_SWITCHABLE_GRAPHICS_1`, `DISABLE_LAYER_NV_OPTIMUS_1`, `DISABLE_LAYER_NV_PRESENT_1` = 1), so the
  look sees what DXVK will; a switch the player set is kept. Measured with the loader's own log, unelevated, RTX 5090
  and Radeon iGPU: `nv*` alone maps `amdvlk32.dll` 85 MiB, with AMD's switch nothing of AMD's; `amd*` alone maps
  `nvoglv32.dll` 25 MiB, with NVIDIA's two switches nothing of NVIDIA's. Elevated, the loader ignores both (every
  driver loads anyway, as before). `srp_patches.log`: `dxvk: other vendors' Vulkan layers off: ...`.
- **One driver of DXVK's GPU** (`one_driver`). One player's AMD PC mapped two AMD Vulkan drivers, 32.0.21045
  (`u0204589.inf`, 85 MiB) and 32.0.32015 (`amdvlk.inf`, 93 MiB), both of the same RX: both manifests match `amd*`,
  and the loader (1.4.350 there) keeps both drivers (it drops a second layer of the same name, and a registry-key
  driver whose manifest's name it has already, not a second driver package). When the winning vendor's look lists
  DXVK's GPU more than once, the manifests beside each driver DLL its instance loaded (`K32EnumProcessModules`, a DLL
  exporting `vk_icdNegotiateLoaderICDInterfaceVersion`; `FindFirstFileA` `*.json` in its folder) are tried one by one
  alone (`VK_DRIVER_FILES`, the glob deleted); the first that gives the GPU exactly as DXVK picked it (its driver
  version: the driver DXVK uses now) is kept, with every vendor's layers off (the loader keeps one of two same-named
  layers, which may be in the other driver's DLL), after a last look with exactly that must give DXVK the same GPU.
  Anything less certain leaves the glob and the layers as they were and says why: a manifest that gives the GPU twice
  (two such GPUs, or an elevated game where the loader ignores `VK_DRIVER_FILES`), no manifest that gives it alone, a
  last look that does not. A player's own `VK_DRIVER_FILES` or `VK_ICD_FILENAMES` is kept like their
  `VK_LOADER_DRIVERS_SELECT` (every driver they allow looked at). `srp_patches.log`: `dxvk: DXVK's GPU has 2 drivers:
  <manifest> only (VK_DRIVER_FILES), its layers off`, or `..., all kept (<why>)`.
- **The video-memory probe's DirectDraw object** (with either renderer, `on_ddraw_probed`). The engine's VRAM check
  (`get_local_video_memory_size` 0x106300, through 0x1060F0, to `GetVideoMemoryViaDirectDraw` 0x105DB0, the DirectX
  SDK sample's) makes a DirectDraw object and releases only the IDirectDraw7 it asks it for. DirectDrawCreate loads
  the GPU's own Direct3D driver, and the leaked object keeps it mapped for the whole game, also under DXVK, which never
  draws with it: an image-load trace of the game's start shows `ddraw.dll`, then `nvd3dum.dll`. The object is now
  released at 0x105E8E, before the probe releases its IDirectDraw7 and lets ddraw.dll go (after that FreeLibrary the
  object's code is unmapped); the probe's number is the same. `srp_patches.log`: `video memory: the DirectDraw object
  of the engine's probe released, references left 0`. Measured at the login screen on an RTX 5090: DirectX 11 3017 ->
  2964 MiB used, DXVK 3366 -> 3293 MiB, `nvd3dum.dll` (43 MiB) gone from both. On AMD the driver is `amdxn32.dll`,
  34 to 35 MiB in every player's dump, DXVK and DirectX 11 alike (not measured on AMD here: the Radeon iGPU of the
  test PC drives no display, and DirectDraw takes the display's GPU).

**What stays:** a PC of one AMD GPU gains nothing from the drivers' limit, only the probe's driver (about 35
MiB), and AMD's presentation through D3D12 is AMD's. The game peaks near 3.9 GB on DirectX 11 already (the arenas reserved at its start); patches 124 and 129
(rev54) reserve more. DXVK on such a PC can still run out: no setting is switched for the player after a crash (no
fallback that would hide which renderer failed).

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
- The fullscreen window: Unicorn runs the patched `res_render_output::present` with the report's
  window (`check_hud_options_026g.py`): WS_POPUP set before the Present, which then sees a window
  DXGI's rule takes. That AMD's driver then makes its swap chain is AMD's word (#57, #5258), not
  seen on this game: no Windows PC with an AMD GPU was run.
- Not checked on Windows. Played locally under Wine (wayland driver, DXVK, fullscreen
  2560x1440) on 2026-11-09: the window made a popup at the start, the game fine.
