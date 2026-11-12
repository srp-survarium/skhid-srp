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
  process does not offer it; it is then greyed (its saved tick shown, not changeable, no restart
  note for it), the reason in `srp_patches.log` (`dlss: on, not run: <why>`):
  - DirectX 11 running or picked: the whole panel is greyed already;
  - DXVK's GPU (the start's Vulkan look, `DXVK_GPU`) is not NVIDIA's (vendor 0x10DE);
  - the helper's files are not there (`binaries\x86\dlss\dlss_helper.exe`, `nvngx_dlss.dll`);
  - the helper's answer at this start said no (a GPU without DLSS: GTX, no driver NGX, 32-bit
    Windows): the reason in `srp_patches.log`, the saved tick off (as a failed DXVK start turns
    the Renderer off), so the next start does not try again.
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
- **Mip bias:** `log2(render / output) - 1` (-1.58 at Quality), DXVK's
  `d3d11.samplerLodBias` in `DXVK_CONFIG`, set at the start only when the helper said DLSS runs
  (DXVK reads its config at its first device, before any frame). LIKELY: DXVK applies it to every
  sampler it may bias (not to point-mip or LOD-clamped ones); to be checked in a capture.
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
- Any helper failure (no answer at the start, a setup refused, the pipe broken, the helper process
  gone, its timeline value late): DLSS stops for the process, the line in `srp_patches.log` says
  why, and the next frame's `set_size` (forced) makes every target native again.
- A helper that dies after the game queued its wait: the game signals that timeline value itself
  from the host (a timeline semaphore takes a host signal), so DXVK's queue never waits forever;
  that frame shows the last good output (the shared output keeps it), one repeated frame.

## The helper interface (the game's side, one module)

- What the game gives: DXVK's GPU (device UUID and LUID), the output size, the quality; at a new
  size four images made and exported on DXVK's device (colour, depth, motion at the render size,
  output at the output size) and one timeline semaphore; each frame its number, jitter, motion
  scale and reset.
- What it gets: whether DLSS runs and the render size; whether the setup took; each frame's done
  value on the timeline.
- Where: the helper is started at the process's start (patch 83's start, after the Vulkan look,
  before `DXVK_CONFIG`): its Hello is the "can it run" answer above and gives the render ratio; the
  images and the frames come at the first frame of each size. Files: `binaries\x86\dlss\`
  (`dlss_helper.exe`, x64, and NVIDIA's `nvngx_dlss.dll` beside it), in the Skhid ZIP as DXVK's
  `dxvk\`.
- The game's Vulkan entry points: from `winevulkan.dll` when it is loaded (Wine: DXVK takes them
  there), else `vulkan-1.dll`. DXVK's device and queue from `IDXGIVkInteropDevice`
  (GetVulkanHandles, GetSubmissionQueue1, LockSubmissionQueue/ReleaseSubmissionQueue), each texture's
  VkImage and layout from GetVulkanImageInfo.
- **What "can it run" checks** (the helper's Hello is the final word): Windows needs the driver's
  own `nvapi64.dll` and NGX; Wine needs, beside the helper, dxvk-nvapi x64 (0.9.2) and DXVK's x64
  `dxgi.dll` loaded before NGX (`DXVK_ENABLE_NVAPI=1`, `WINEDLLOVERRIDES dxgi=n,b;nvapi64=n` for the
  helper only: without nvapi64 NGX's init fails with 0xBAD00002), and the prefix the driver's
  `nvngx.dll`, `_nvngx.dll` and the `NGXCore` key (`play_skhid.sh`'s prefix set-up). Any of them
  missing: the Hello fails, the checkbox is greyed, the reason logged.

## Status

- Implemented: this spec; the option (a checkbox, user.cfg, Settings answers, greyed when not
  offered, ten languages); the start's gate (DXVK, an NVIDIA GPU, the helper's answer). Since no
  helper ships yet, the checkbox is greyed on every PC.
- Tested: the patch's Rust tests (the gate, user.cfg's word, the answer) and Clippy; patch 83 in
  Unicorn (`check_hud_options_026g.py`: the checkbox registered and answered, running "-", the
  start's `dlss:` line on an RTX and on an RX); the Settings movie in Ruffle
  (`scripts/hud-options/check.sh`: six rows on one step, about 40 px now, the panel ending under the
  gamma slider; DLSS greyed when not offered, a tick pending until Apply when offered, every
  dictionary's texts inside). Not played, not released.
- Open, on the helper (the probe's protocol, final): starting it from the game, Hello and Setup,
  the per-frame submit and sync, the helper's folder and its Wine packaging (nvapi), Windows.
- Open, in the game: the render size seam (targets, the output's depth, viewports), jitter, the
  motion compute pass, rt 18's format, AA off while DLSS runs, the screen-image swap, the reset
  triggers, the mip bias's reach.
