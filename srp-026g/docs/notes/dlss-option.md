# DLSS upscaling, an option of the Vulkan renderer (DXVK)

The owner (2026-11-12): "let's do it as DXVK option". Off by default; only with Vulkan/DXVK
([dxvk-option.md](dxvk-option.md)).

## Why this shape

- survarium.exe is 32-bit; NVIDIA ships DLSS (NGX, `nvngx_dlss.dll`) for x64 only. DLSS runs in
  a 64-bit helper process; the game shares its frame's inputs with it through exported Vulkan
  memory and one exported timeline semaphore, NT handles passed with DuplicateHandle.
- The game's Vulkan device is DXVK's: its interop interface (`ID3D11VkInteropDevice`,
  `IDXGIVkInteropSurface`) gives the VkDevice, the queue (locked while we submit) and each D3D11
  texture's VkImage and layout. Hence an option of the Vulkan renderer only.
- The helper and the cross-process path are proven by a separate standalone probe
  (`private/dlss-probe/`, its `PROTOCOL.txt`, final 2026-11-12): under Wine 11.8 on an RTX 3060 Ti,
  a 32-bit producer on DXVK 3.1.1 x32 and the x64 helper (NGX, DLSS 310.9.16) ran end to end; DXVK
  3.1.1 enables `VK_KHR_external_memory_win32` and `_semaphore_win32` on its device on NVIDIA, so the
  game makes the exportable images and the timeline semaphore on DXVK's own VkDevice. Measured: DLSS
  1.1 ms at 1080p Quality, the round trip about 0.1 ms of GPU, 0.4-0.5 ms a frame for the second
  process, about 190 MiB more VRAM at 1080p Quality. Windows untested. The game's whole side of the
  protocol is one module (below, "The helper interface").

## The option

- **A tick, not a list** (the owner on #1648, 2026-11-12): one checkbox in the Vulkan options
  panel (`SrpVulkanOptions.as`), the first checkbox, right under Performance overlay: "NVIDIA DLSS"
  (`st_srp_vulkan_dlss`), its hint in the checkboxes' style, what it does, "+" and "-"
  (`st_srp_vulkan_dlss_hint`), ten languages. On is DLSS at **Quality**; no mode choice: the probe
  measured Performance no cheaper (2.06 against 1.13 ms at 1080p) and taking more VRAM.
- **Kept** in user.cfg as `srp_dxvk_dlss on|off`, off by default, read once at the start with the
  other Vulkan options (`VulkanOptions`). Pending until Apply, Cancel puts the saved one back,
  "Applies after restart." as the other options.
- **Offered only where it can run:** the exe answers the checkbox's running value `-` when this
  process does not offer it; it is then greyed and dimmed (its saved tick shown, an unticked box
  empty; not changeable, no restart note for it). It is offered once the helper said, at this
  start, that DLSS runs here; so the helper is asked at every start with DXVK on an NVIDIA GPU, with
  the tick on or off (with it off the helper is told to quit after its answer). Not offered:
  - DirectX 11 running or picked: the whole panel is greyed already;
  - DXVK's GPU is not NVIDIA's (vendor 0x10DE): the start's Vulkan look's `vendorID` (`DXVK_GPU`),
    never DXGI's adapter (DXVK reports an NVIDIA card to the game as AMD's);
  - the helper's files are not there (`binaries\x86\dlss\dlss_helper.exe`, `nvngx_dlss.dll`);
  - the helper has not answered yet (about a second from the first frame), did not start, or ended
    before it answered;
  - the helper's answer said no (a GPU without DLSS: GTX, no driver NGX): its reason in
    `srp_patches.log`, and a saved tick goes off (as a failed DXVK start turns the Renderer off;
    at the login screen or after: user.cfg has run by then), so the next start sets no mip bias.
  A link that fails after the helper's yes (the helper crashed in a match) leaves the checkbox
  offered: the PC runs DLSS, the next start tries again.
- **`srp_patches.log`** says what the link did: `dlss: the helper is started`, `dlss: offered: ...`
  (the render size for the output), `dlss: running: ...`, and when it does not run, why:
  `dlss: on, not run: <why>` (the tick on), `dlss: not offered: <why>` (the tick off).
- NGX's quality: MaxQuality (2). Render size: the helper's answer (NGX's optimal size for the
  output at Quality, about 0.667 of each side).

## What the engine has (0.26g, evidence)

- In short: **the engine does make motion vectors** (the probe's note "the engine has no motion
  vectors today" is wrong): per-object ones for skinned and moved models and the first-person
  models, into their own target; the camera's motion is not in that target but computed in the TAA
  shader from depth. Evidence below.
- **Motion vectors** (`stage_postprocess::accumulate_motion_vectors` 0x1B1B50, run first in the
  post-process when `r_use_motion_vectors_in_taa` opt+133h and `r_post_process_quality` opt+DCh):
  - target `$user$object_motion_vectors` (rt 18), cleared to 0 (PROVEN, 0x1B1B5F: descriptor
    `18 * A0h + 98h` = BD8h);
  - drawn: every visible opaque **skinned** model (scene_view+84ACh, the predicate 0x1B1840 keeps
    vertex types 8/10h/20h/40h: 4..1-bone skeletal) and every visible opaque **moved** rigid model
    (scene_view+494h, the same predicate inverted), with the G-buffer effect's motion technique
    (`effect+5620h = 7`, 0x1B1952). PROVEN (the code; the same as 0.10b's
    `stage_postprocess.cpp`);
  - per vertex: the previous position from the previous world matrix (a map keyed by the
    instance, context+25374h, 0x1B1968..0x1B19FE) and, for skinned models, the previous bones
    (`prev_bones_matrices`, a VS constant; the skinned PS reads its `prev_position` input):
    so **animation and per-object motion of players, bots and first-person models are covered**;
  - two passes (`render_models` 0x1B18B0, its flag): the world's models with the scene's
    projection, then the **first-person (foreground) models** after `renderer::foreground_begin`
    (0x13E010: their own projection and viewport); a model is foreground by its material
    (+114h) or its instance (+63h). PROVEN;
  - the shader (`motion_vectors_accumulation.ps`, the 0.26g blobs disassembled with vkd3d 2.0):
    `xy = current uv - previous uv` (uv: 0..1, y down), `z = 1` marking a covered pixel. PROVEN;
  - **not** drawn: static (unmoved) rigid models, terrain, grass, sky, particles, decals,
    forward/transparent surfaces: 0 there.
- **Camera motion is not in that target:** the TAA shader (`temporal_antialiasing.ps`, 0.26g blob)
  reprojects every pixel from its linear depth (`$user$position`, rt 9), the eye-ray corner and the
  previous view matrix; where the object target's `z > 0` (here or at the reprojected place) it
  uses `-object.xy` instead. Blend weight 0.5 scaled down with motion length (x100), no
  neighbourhood clamp, **no jitter** (`frame_index & 1` is set, unused). It runs on the LDR
  `$user$present` (rt 52) after the tone curve and sharpen (0x1B2E52..0x1B2F99: present copied to
  albedo, TAA into present, albedo copied to `previous_present` rt 54). PROVEN.
- **Stock fault:** rt 18 is made with the HDR format (`create_targets` 0x163432 pushes EBP, 26
  R11G11B10_FLOAT at feature level 10+, as DXVK runs): an unsigned float format, so negative
  object motion is stored as 0 (D3D's float conversion clamps a format without sign to 0). PROVEN
  (the format); the clamp is D3D's rule, not seen in a capture.
- **Seams:**
  - all render targets come from `render_output_window::set_size` (0x119AB0) ->
    `create_targets(w, h)` (0x162DA0); the scene's depth-stencil is the render output's own
    (`res_render_output::update_depth_stencil_buffer` 0x3D0040: R24G8_TYPELESS, bind
    DEPTH_STENCIL|SHADER_RESOURCE, at the output's +90h/+94h size). PROVEN;
  - the frame reaches the window in one draw: `renderer::render` (0x13DB36 -> 0x1418C0) hands
    `get_t(52)` (present) to `stage_screen_image::execute` (0x1BD320), a full-window quad sampling
    it (uv 0..1) into the back buffer; then `renderer::render_flash_ui` (0x13CED0, Scaleform) and
    `res_render_output::present`. PROVEN (code; 0.10b source alike);
  - the scene's projection: `renderer_context::set_scene_view` (0x149A90) -> `set_p` (0x149AF1,
    the scene view's +42Ch); the first-person one: `foreground_begin`'s `set_p` (0x13E103). Shadow,
    light and rain maps have their own `set_p` calls. PROVEN.

## The frame with DLSS (design)

- **Render size:** `set_size(w, h)` keeps the window and back buffer at w x h; `create_targets` and
  the output's depth-stencil get the render size (the helper's answer). Every stage, the
  post-process included, runs at the render size into `$user$present`. Viewports: RE open (which
  stages set theirs from the output's size rather than a target's).
- **Where DLSS sits:** in place of the engine's AA, at the end of the post-process: DLSS reads
  `$user$present` (RGBA8, LDR, after the tone curve, LUT, sharpen and gamma), depth and motion, and
  writes a full-size image; `stage_screen_image::execute` is handed that image instead of present.
  Why there and not before the tone curve: present -> screen image is the engine's one seam between
  the frame's size and the window's; the post-process uses the G-buffer's targets (albedo) as its
  scratch, so a post-process at full size would need half the targets twice. DLSS takes LDR input
  (no HDR flag). The cost: bloom and the tone curve at the render size (both are blurs or per pixel).
- **The HUD and menus** (Scaleform, `render_flash_ui`) draw after the screen image, on the back
  buffer: always at full size, never upscaled.
- **AA:** with DLSS running the game's TAA, FXAA, MLAA and SRAA are off (DLSS is the AA); motion blur
  stays the player's (it runs before DLSS). DLSS sharpening 0 (the game's own 2 % sharpen stays: it
  holds the gamma encode).
- **Jitter:** a Halton(2,3) sequence, `8 * (output / render)^2` phases (NVIDIA's guide: 18 at
  Quality), offsets in render pixels in [-0.5, 0.5], added to the scene's
  projection as a clip-space translation at `set_scene_view`'s `set_p` and at `foreground_begin`'s;
  never to shadow, light or rain projections. The same offsets go to DLSS (`Jitter_Offset_X/Y`). The
  engine's motion shaders project current and previous with the same (jittered) `m_P`, so their
  motion holds no jitter. The sign DLSS is given: the probe scored the negated sample offset better
  at Quality (21.9 against 19.2 dB PSNR); to be checked on a game frame.
- **Mip bias:** `log2(render / output) - 1` (-1.585 at Quality, the render size 1 / 1.5 of the
  output's each way), DXVK's `d3d11.samplerLodBias` in `DXVK_CONFIG`. DXVK reads its options at
  its first device, before any frame, and the helper is asked at the first frame (below), so the
  start sets it without the helper's answer (the owner, 2026-11-12): when the tick is on, DXVK's
  GPU is NVIDIA's and the helper's files are there; its value depends on the mode alone. If the
  helper then fails, that is logged and the game goes on with the bias (sharper mips at the
  native size) until the next start; a helper that says no turns the tick off, so only that one
  start has it. LIKELY: DXVK applies it to every sampler it may bias (not to point-mip or
  LOD-clamped ones); to be checked in a capture.
- **Motion vectors for DLSS** (every pixel, render pixels, `previous = current + mv`): built by the
  game in its copy-in command buffer on DXVK's device, a small compute pass (SPIR-V in the patch)
  instead of a copy: where rt 18's `z > 0`, `-xy * render size`; elsewhere the camera's motion from
  the depth and the current and previous view-projection (the TAA shader's own math). rt 18 is made
  RGBA16_FLOAT while DLSS runs (`new_rt`'s format for index 18), so negative motion survives. The
  protocol's MV image stays as the probe has it.
- **First-person weapon and hands:** already in rt 18 (foreground pass, skinned, own projection):
  their motion is what they move on screen (about none when only the camera turns), so DLSS keeps
  them sharp; jittered with the scene (`foreground_begin`'s projection). Their depth is the
  foreground's own depth range; DLSS uses depth for disocclusion only.
- **Not covered by motion** (particles, smoke, transparent and forward surfaces): the depth behind
  them; DLSS may smear them a little. A reactive/transparency mask is a later step.
- **Reset** (DLSS's history thrown away): the first frame of a size, a map load, a camera cut
  (spawn, death camera, spectator switch: the previous view matrix far from the current one).
- **Depth:** the output's D24S8 depth, its depth aspect, not inverted (the engine's projection is
  0 near, 1 far). The probe shared a D32 image; a D24S8 one is copied into a shared image of its own
  format (depth aspect only), or written by the same compute pass as the motion (R32_SFLOAT): to be
  settled with the helper.

## Falling back

- No broken frame: a frame that DLSS did not finish shows `$user$present` through the stock screen
  image quad (a plain bilinear stretch of the render size), the same draw as today.
- Any helper failure (it does not start, no answer, a setup refused, the pipe broken, the helper
  process gone, its timeline value late): DLSS stops for the process, the line in
  `srp_patches.log` says why (`dlss: on, not run: <why>`), and the next frame's `set_size`
  (forced) makes every target native again. The link's part of this is done (below, "The link");
  the targets' part comes with the render size.
- **A helper that is gone after the game queued its wait:** DXVK's queue then waits for a value
  nobody signals, and the game's own thread may already wait for that queue (DXVK's Present waits
  for an earlier frame when the GPU is the slower side), so the game's thread cannot be the one
  to notice. The link starts a **watch** with the helper, a thread of its own that waits for the
  helper's process to end: then it plays the helper's part of the timeline itself, from the host
  (a timeline semaphore takes a host signal), and DXVK's queue goes on; those frames show what
  the shared output last held (a repeated frame, once the output is shown at all). The helper's
  values are signalled **one at a time**, each right after the game's copy before it: the game's
  own copies of later frames are still on DXVK's queue and signal the values in between, so a
  signal straight to the last wait's value was undone by the next of them (the timeline went
  back, that copy's wait was never met: seen, below). A stop of the link on the game's thread
  does the same before it retires what was shared.
- **The GPU's driver gives up on a queue that waits about 11 s** (seen, NVIDIA 615.78 under
  Wine: Xid 31 on the game's channel, the device lost, the game's own "device removed" end): the
  reason the watch's timer is short.
- **Timers**, each because a helper that hangs gives no event (its end and a broken pipe are
  events, and are acted on at once):
  - 30 s for the helper to open the pipe, to answer the Hello (NGX's start, about 1 s measured)
    and to answer the Setup: counted at the game's Presents;
  - 3 s (six looks of the watch, 500 ms apart) in which the helper's turn on the timeline (an
    odd value: the game's copy of a frame done, the helper's output of it not) does not move: the
    watch ends the helper (TerminateProcess), which is then the case above. An even value behind
    a wait is the game's own copy not run yet (DXVK's queue is the slow one): not counted.

## The helper interface (the game's side, one module)

- What the game gives: DXVK's GPU (device UUID and LUID), the output size, the quality; at a new
  size four images made and exported on DXVK's device (colour, depth, motion at the render size,
  output at the output size) and one timeline semaphore; each frame its number, jitter, motion
  scale and reset.
- What it gets: whether DLSS runs and the render size; whether the setup took; each frame's done
  value on the timeline.
- Where and when (the owner, 2026-11-12): the helper is started at **the first Present**, when the
  output window and its swap chain exist, so its Hello carries the output's real size; the Hello's
  answer is the "can it run" answer above and gives the render size. With the tick on, the images
  are made for that answer and the Setup sent; the frames follow the Setup's answer. A Present of
  **another output size** tells the helper a new size (`FrameMsg.quit = 2`, the helper keeps its
  device and NGX), retires what was shared and says Hello and Setup again. Nothing waits: each step
  is taken at a Present when the helper's answer has come (the pipe is only read when a whole
  record is there). Files: `binaries\x86\dlss\` (`dlss_helper.exe`, x64, and NVIDIA's
  `nvngx_dlss.dll` beside it; `client/dlss-helper/README.md`), in the Skhid ZIP as DXVK's `dxvk\`.
  The helper is started in that folder with the game's own environment and no window; what it
  needs of Wine it sets up itself.
- The game's Vulkan entry points: from `winevulkan.dll` when it is loaded (Wine: DXVK takes them
  there), else `vulkan-1.dll`. DXVK's device and queue from `IDXGIVkInteropDevice`
  (GetVulkanHandles, GetSubmissionQueue1, LockSubmissionQueue/ReleaseSubmissionQueue), each texture's
  VkImage and layout from GetVulkanImageInfo.
- **What "can it run" checks** (the helper's Hello is the final word; `client/dlss-helper/README.md`,
  "What it needs at run time"): Windows needs the driver's own `nvapi64.dll` and NGX; under Wine the
  helper loads dxvk-nvapi and DXVK's x64 `dxgi.dll` from `dlss\wine\` itself and finds the driver's
  NGX core. Any of them missing: the Hello's answer is no with its reason, the checkbox is greyed,
  the reason logged.

## The link (patch 83, `hud_options::dlss` over `vostok::dlss`)

- **One owner, one state** (`Link`, one transition `Link::step`): not started (nothing will
  start: `Off`; or waiting for the first Present), the helper starting, asked (the Hello sent),
  offered (the helper said yes, the tick off: nothing runs), sharing and set up (the Setup sent),
  running (each Present a frame), failed (before the helper's yes) or stopped (after it), each
  with its reason. The Settings' gate (`DlssOffer`) is read from that state.
- **Hooks:** the process's start (`memory.allocate_arenas`: whether anything starts, the mip
  bias), each Present (`render_output.present`: the link's step), the login screen
  (`login_menu.activated`: a refused tick goes off), the game's exit from its menus
  (`game.exit_start`, game::exit 0x1EC730, which the Exit buttons call: the helper told to quit,
  given 3 s, everything of the link ended while DXVK's device lives), and the watch's thread
  (`dlss.helper_watch`, a thread start routine). An exit that does not pass game::exit (the
  window closed, a crash) closes the pipe with the process: the helper ends by itself then, and
  the watch releases the timeline if DXVK's queue still waits.
- **DXVK's device:** from the game's swap chain (`res_render_output` +0CCh):
  `IDXGISwapChain::GetDevice(IDXGIVkInteropDevice1)` gives the interop device (the VkDevice, the
  queue and its family, Flush/Lock/ReleaseSubmissionQueue), `GetBuffer(0, IDXGIVkInteropSurface)`
  the back buffer's VkImage, layout, format and size. The Vulkan functions come from
  `winevulkan.dll` when it is loaded, else `vulkan-1.dll` (vkGetInstanceProcAddr, then
  vkGetDeviceProcAddr on DXVK's device: a missing `vkGetMemoryWin32HandleKHR` is a DXVK device
  without the extension, a failure with that name).
- **What a frame does now:** the helper is told the frame (`FrameMsg`, jitter 0, reset on a
  Setup's first frame), then, with DXVK's queue locked, two submits: the copy-in (the back buffer,
  the frame as the window gets it, scaled into the shared colour image; a Setup's first frame
  also clears the shared depth to far and motion to none, which nothing fills yet) that signals
  2N+1, and a wait for 2N+2 in a command buffer of its own, empty yet (the copy of the output
  goes there), so the wait is a batch of its own on the queue, which the queue's idle covers
  before the semaphore is retired (a submit that only waits may be left by a driver for the
  queue's next submit). The shared colour and output take the back buffer's format; depth is D32_SFLOAT, motion
  R16G16_SFLOAT. Four pairs of command buffers in turn: a frame whose pair is still on the GPU
  (the timeline has not passed the copy of the frame three before) is no frame of the link.
  The scene at the render size, the real depth and motion, and the output's way to the screen
  are the next pieces.
- **A new size** makes a new timeline semaphore with the new images (the helper imports the
  Setup's semaphore each time); its frames count from 0 again.
- **The helper's frame reports** (`FrameStat`) are read off the pipe at each Present and dropped.
- **Seeing it run:** a game started with `SKHID_DLSS_LOG` set says every 300th frame and the
  timeline's value in `srp_patches.log` (a local diagnostic; unset, nothing).
- **With the render size** (PR #1717, `hud_options::render_size`; not joined here):
  - the start asks the ratio (`render_size::ask`, 1 / 1.5) where it sets the mip bias (the tick
    on, an NVIDIA GPU, the helper's files), since a ratio is taken only then; every stop of the
    link (a failure, a refusal) asks native;
  - the Setup's images take the frame's own render size, not the Hello's answer: NGX's optimal
    size may be odd (853x480 for 1280x720, seen) where the engine's is even (854x480), and the
    helper makes DLSS's feature from the Setup's image sizes, so the two need not be compared;
  - the copy-in reads `$user$present` and the frame's own depth-stencil (both at the render
    size) in place of the back buffer, and the link then runs before the screen image, not at
    the Present.

## Status

- Implemented: this spec; the option (a checkbox, user.cfg, Settings answers, greyed and empty
  when not offered and unticked, ten languages); the link to the helper (its start at the first
  Present, Hello, Setup, each frame's submit and timeline sync, a new size, every failure's stop,
  the watch, the exit) and the gate from the helper's answer; the mip bias at the start. The
  helper itself is `client/dlss-helper` (PR #1709); the render size PR #1717.
- Tested, 2026-11-12, Wine 11.8 staging, RTX 3060 Ti (driver 615.78.08), the game on a headless
  display with no input, the helper PR #1709's build:
  - the patch's Rust tests (the link's transition over every state, the gate, the watch's count,
    the frame's values) and Clippy; patch 83 in Unicorn (`check_hud_options_026g.py`: the start's
    look for the helper's files, the mip bias with the tick on, the `dlss:` lines);
    `check_hooks_026g.py` and `check_engine_026g.py` (the new rows, the stdcall callback entry);
    the Settings movie in Ruffle (`scripts/hud-options/check.sh`: a greyed unticked DLSS box on
    its resting frame, empty);
  - **the game, at its login screen (1280x720):** the helper started, its yes logged (853x480 for
    1280x720, about 1 s; 6 s with two other games on the GPU), the Setup taken (8 MiB shared), and
    frames submitted for as long as the run (1800 in 40 s), the timeline one frame behind the
    game's count at each 300th (the helper's signals arrive); the picture as without the link;
  - **in a replay's match** the same (300 frames and more, five runs) until the replay's own
    crash 18 s in (exe+0x2EFF87: the same with the tick off and no helper, and in the render
    size's builds). Once, a crash in the arena's code 0.8 s into a replay's start, before the
    first Present (exe+0xA489291): not the link's, but patch 97's `game.tick` handler
    (`packet_recovery::on_frame`), which reads a replay's network client as a live one (its
    +1F1C48h, past the replay client's 5AB18h bytes: whatever the heap holds there, mostly 0);
  - the tick off: `offered`, the helper gone after its answer; no `dlss\`: `on, not run: the
    helper's files are not in dlss\`;
  - the helper without NvAPI (its refusal): the reason logged from its answer, the saved tick off
    in user.cfg; a program that ends at once: `the helper ended, exit code 1`; one that never
    opens the pipe: `it did not open the pipe in 30 s`, ended;
  - the helper ended from outside while frames ran (taskkill): `on, not run: the helper ended,
    exit code 1`, the game went on; the helper stopped where it was (SIGSTOP: it runs, signals
    nothing) with two frames queued: the watch ended it after 3 s (`did not signal 1652 (the
    timeline at 1649) in 3 s: ended`), the timeline was signalled up to 1652 from the game, and
    the game went on drawing. Before (the timeline signalled straight to the last value; a turn
    with frames queued behind it not counted by the watch), both cases lost the device 11 s later.
- Not tested: Windows; a new output size in the game (it takes the Settings: input); the exit
  through game::exit (the Exit button: input; a close message ends the game without it, and the
  helper went with the pipe); a helper that hangs before its answer other than by never opening
  the pipe; an output of another format than the login screen's. Not played, not released.
- Open, in the game: joining the render size (above), jitter, the motion compute pass, rt 18's
  format, AA off while DLSS runs, the screen-image swap (the copy of the output, in the wait's
  command buffer), the reset triggers, the mip bias's reach.
