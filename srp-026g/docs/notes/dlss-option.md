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
- NGX's quality: MaxQuality (2). Render size: 1 / 1.5 of each side of the output, each rounded to
  an even number (the engine's half targets are shifts): 1280x720 for 1920x1080. The helper's
  answer (NGX's optimal size, the same share cut to whole pixels) is said in the log, not used.

## What the engine has (0.26g, evidence)

- In short: **the engine does make motion vectors** (the probe's note "the engine has no motion
  vectors today" is wrong): per-object ones for skinned and moved models and the first-person
  models, into their own target; the camera's motion is not in that target but computed in the TAA
  shader from depth. Evidence below.
- **Motion vectors** (`stage_postprocess::accumulate_motion_vectors` 0x1B1B50, run first in the
  post-process when `r_use_motion_vectors_in_taa` opt+133h and `r_post_process_quality` opt+DCh):
  - target `$user$object_motion_vectors` (rt 18; 0x1B1B5F: descriptor `18 * A0h + 98h` = BD8h), cleared to 0
    at the frame's start by the stage's `clear_surfaces` (0x1B1E70, its vtable+10h, from
    `perform_surfaces_clearing`) under the same gate; the pass itself clears nothing. PROVEN;
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
  neighbourhood clamp; the shader reads no jitter (`frame_index & 1` is set, unused). It runs on the LDR
  `$user$present` (rt 52) after the tone curve (0x1B2E52..0x1B2F99: present copied to
  albedo, TAA into present, albedo copied to `previous_present` rt 54). PROVEN.
- **The engine does jitter** (an earlier reading here said it did not): with the temporal AA
  (`r_antialiasing_method` 2), the lit view mode and the scene view's post-process, `renderer::render`
  pushes a jittered projection (`temporal_projection_matrix_modifier::push_jittering` 0x1BE4A0, the call
  at 0x13D3E3: +-0.25 pixel of the window, alternating by the frame index, `e20 = 2x/width`,
  `e21 = 2y/height`) before the visibility stage and pops it after the stages (0x13D47B). The temporal AA's
  half blend of two such frames is its 2x. PROVEN ("The frame's inputs" below: this seam is the jitter's).
- **The AA choice** (`stage_postprocess::execute` 0x1B22EB): method 1 is FXAA followed by the 2 % sharpen
  (0x1B2C95, 0x1B2D2E), method 2 the temporal AA, anything else neither: the last pass is then
  `copy_rewrite` (0x1B2D96), which holds the gamma encode. MLAA and SRAA effects are loaded and never
  chosen. PROVEN (the three readers of opt+F0h found: render 0x13D388, clear_surfaces 0x1B1E75, execute).
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

## The render size (what sizes itself from where)

Read in the 0.26g exe (addresses given; the re-kit's `abi_inputs.tsv` rows repeat them), with the
0.10b source for intent. PROVEN unless tagged.

**Follows the targets** (nothing to do):

- Every target of the frame is made by `create_targets` (72 `new_rt`, 2 `new_lt`, no depth
  target), called only from `render_output_window::set_size` (0x119B33), the window's constructor
  (0x1198E9 -> 0x162860) and the memory estimate (0x162D75, makes nothing). `set_size` is called
  only by that constructor (0x119972) and `end_render_options_changing` (0x1181CF, forced): 0.26g
  has no per-frame resize.
- The half and quarter targets (SSAO 20, SSDO 28, bloom 39..51, luminance, particles) are shifts
  of the size `create_targets` was given (0x162DA0..0x163B80).
- The screen-space constants: `screen_res` (size and inverse size) is the targets' size
  (`set_target_context` 0x149BD0, the only reader of it: context +4120h/+4124h, targets
  +2F80h/+2F84h). The stages' own constants take a target's or a texture's width (as 0.10b's
  `stage_postprocess.cpp`, `stage_ambient_occlusion.cpp`). LIKELY for the post shaders'
  uv scales: no reader of the output's size is in any stage (below), the shaders not re-read.
- The stages' viewports: each of the 41 `backend::set_viewports` calls (0x11C080) outside the
  shadow maps, the bakes and the Hi-Z takes a render target's size (render_target +20h/+24h:
  `system_renderer::fill_surface` 0x11DA3D, distortion 0x17B642, forward lighting, particle
  lighting 0x19644D, atmosphere 0x1A3762, SSR 0x1ACFD1, volume fog, motion vectors 0x1B1C90, the
  colour grading) or is the kept viewport put back or re-ranged (`foreground_begin` 0x13E03B,
  `foreground_end`, the restores).
- The scissor: `set_viewports` itself sets the scissor rectangle to the viewport (0x11C149..
  0x11C197), the engine's only `RSSetScissorRects`.

**Does not follow** (sizes itself from the output; how each is handled is in "The frame with
DLSS"):

1. **The frame's base viewport.** `renderer::setup_render_output_window` (0x13BAC0, called at
   0x13D297 before the stages) sets it from `backend::target_width/height`: the render output's
   +90h/+94h (0x13BBB8..0x13BBE8). Every stage saves and restores this one.
2. **The screen image's viewport.** `stage_screen_image::execute` (0x1BD320) sets none: its quad
   covers the frame's base viewport, which stock is the window.
3. **The depth-stencil.** The render output's own (`update_depth_stencil_buffer` 0x3D0040, at the
   output's +90h/+94h): its view (+D4h) is the backend's `m_base_zb` (+29D4h,
   `set_render_output` 0x11B6D0) that every stage's `reset_depth_stencil_target` binds beside the
   colour targets; its texture (+D8h) is `t_depth_render_target` (`stage_gbuffer` 0x177CEE), which
   `copy_depth_rt.ps` samples by uv 0..1 into rt 9, the linear depth of every screen-space pass.
   The same view is Scaleform's stencil on the back buffer (`render_flash_ui` 0x13CF7E) and the
   overlay's after it (0x13DC01), so it cannot simply be made smaller: a depth view smaller than
   the colour target is refused by D3D11 (and by DXVK's `ValidateRenderTargets`). And a forced
   `set_size` at an unchanged window size does not remake it (`res_render_output::resize`
   0x3CFBBE drops the force).
4. **The backend's `target_width/height`** elsewhere: `draw_screen_lines` 0x11D0EE,
   `draw_3D_point` 0x11D379, the statistics 0x15C305, `stage_view_mode` (0x1BBA1B): debug
   drawing on the back buffer after the UI, right at the output's size. Left as they are.
5. **`renderer::render`'s own copy of the output's size** (0x13D0EF) for the texture streaming and
   the TAA jitter (LIKELY, from 0.10b): streaming at the output's size is what DLSS wants (mips
   for the screen), the game's TAA is off under DLSS. Left.

**Scaleform and the HUD** read nothing of the 3D size: the flash renderer is reset with the
window's size (+108h/+10Ch, 0x119BDE), sets its own viewport, and draws on the back buffer with
the output's depth-stencil. Mouse, crosshair and the HUD's world markers are the game's
projection and the movie's stage: no reader of the targets' size exists outside
`set_target_context`.

## The frame with DLSS (design)

- **Render size** (implemented: patch 83's `hud_options/render_size.rs`, the engine's side
  `vostok/render_size.rs`): `set_size(w, h)` keeps the window and back buffer at w x h; its
  `create_targets` takes the render size (the ratio's share of each side, rounded to an even
  number: the half targets are shifts). Every stage, the post-process included, runs at the render
  size into `$user$present`. How the three things that take the output's size are handled ("The
  render size", 1 to 3):
  - **the depth-stencil is one of ours**, not the output's made smaller: the engine's own
    `render_target` with the depth usage (`create_render_target` 0x3BF930: R24G8, its D24S8 view
    and its texture, what the output's own is), made with the targets. Between the frame's
    `setup_render_output_window` and its present stage it is the backend's `m_base_zb`, and the
    G-buffer's `t_depth_render_target` (ECX at 0x177CFA). The output's own stays the window's
    size for Scaleform and the overlay: every bind has colour and depth of one size;
  - **the base viewport** is set to the render size right after `setup_render_output_window`
    (0x13D29C; `set_viewports`, so the scissor follows);
  - **the screen image's viewport** is set to the output's size before the present stage
    (0x13DB16), where `m_base_zb` becomes the output's again.
  One owner holds the scale wanted and the frame as made (native, or scaled with its
  depth-stencil); the five sites are written on demand, at the process's start, only when a scale
  is asked (before the render device exists: nothing runs them), so a game without it runs the
  engine's own code. The link's interface is one function, `render_size::ask(thread, scale)`: a
  ratio at the process's start, native any time. Until the link exists the diagnostics switch
  `SKHID_DLSS_RENDER_SCALE=0.667` asks (`0.667,300`: native again after 300 frames).
- **Where DLSS sits:** in place of the engine's AA, at the end of the post-process: DLSS reads
  `$user$present` (RGBA8, LDR, after the tone curve, LUT, sharpen and gamma), depth and motion, and
  writes a full-size image; `stage_screen_image::execute` is handed that image instead of present.
  Why there and not before the tone curve: present -> screen image is the engine's one seam between
  the frame's size and the window's; the post-process uses the G-buffer's targets (albedo) as its
  scratch, so a post-process at full size would need half the targets twice. DLSS takes LDR input
  (no HDR flag). The cost: bloom and the tone curve at the render size (both are blurs or per pixel).
- **The HUD and menus** (Scaleform, `render_flash_ui`) draw after the screen image, on the back
  buffer: always at full size, never upscaled.
- **AA:** with DLSS running the game's AA is off (DLSS is the AA): the post-process reads
  `r_antialiasing_method` as 0, so no FXAA, no temporal AA, and no sharpen (the engine's 2 % sharpen comes
  only with FXAA; the gamma encode is in the copy that runs instead). Motion blur stays the player's (it
  runs before DLSS). DLSS sharpening 0.
- **Jitter:** a Halton(2,3) sequence, `8 * (output / render)^2` phases (NVIDIA's guide: 18 at
  Quality), offsets in render pixels in [-0.5, 0.5), in the frame's projection at the engine's own
  jitter seam ("The frame's inputs": one place, not the two `set_p` sites first planned; the
  first-person projection is made from the frame's). The same offsets go to DLSS
  (`Jitter_Offset_X/Y`). The engine's motion pass projects current and previous with the same
  (jittered) projection, so its motion holds no jitter. The sign DLSS is given: the probe scored the
  negated sample offset better at Quality (21.9 against 19.2 dB PSNR); on the game's still frame
  DLSS is right with the frame's `Jitter` as it is, the picture's move (x right, y down), and wrong
  with any other sign ("Status"; the convention of the value the link reads is below).
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
  protocol's MV image stays as the probe has it. Built: "The link", the motion pass.
- **First-person weapon and hands:** already in rt 18 (foreground pass, skinned, own projection):
  their motion is what they move on screen (about none when only the camera turns), so DLSS keeps
  them sharp; jittered with the scene (`foreground_begin`'s projection). Their depth is the
  foreground's own depth range; DLSS uses depth for disocclusion only.
- **Not covered by motion** (particles, smoke, transparent and forward surfaces): the depth behind
  them; DLSS may smear them a little. A reactive/transparency mask is a later step.
- **Reset** (DLSS's history thrown away): the first frame, the first frame after the targets were
  made (a size, a mode), another scene or scene view (a map load), a camera cut (spawn, death camera,
  spectator switch: the view not where a frame's motion leaves it). "The frame's inputs" says how each
  is taken.
- **Depth:** the frame's own D24S8 depth-stencil (`render_size`'s `DepthTarget`, the render
  size), its depth aspect, not inverted (the engine's projection is 0 near, 1 far). The probe
  shared a D32 image; here the shared depth image has the depth-stencil's own format (as DXVK made
  it: D24_UNORM_S8_UINT on the RTX 3060 Ti) and the depth aspect is copied into it as it is
  (vkCmdCopyImage; formats that differ cannot be copied, and a depth format takes no compute
  write). The helper reads an image of a depth format by its depth aspect (its PROTOCOL.md,
  section 3), and DLSS took it (seen).

## The frame's inputs (implemented: `hud_options/frame_inputs.rs`, `vostok/frame_inputs.rs`)

Seven lohooks, written with the render scale's sites (only when a scale is asked), each doing nothing
unless its piece is asked and the frame is one of ours: rendered at a render scale, the lit view mode, the
scene view's post-process on (the frames the engine jitters for its own AA). One owner holds the pieces
asked (`Pieces`: jitter, no AA, motion, signed motion) and the frames as rendered. The link asks with
`frame_inputs::ask(thread, Pieces::ALL)` at the process's start, beside `render_size::ask`; until it
exists the diagnostics switch `SKHID_DLSS_INPUTS` does (below).

**The jitter** (`render.jittered` 0x13D3E8, after `push_jittering` answered):

- Where: the frame's projection is pushed on the context's projection stack where the engine pushes its
  own (`push_set_p` 0x1392C0, the jitterer's `need_modify` set so `render` pops it at 0x13D47B); the
  engine's own +-0.25, when the player's AA is the temporal one, is replaced (`set_p`), since nothing
  resolves it with the AA off.
- The value: `Jitter { x, y }` in render pixels, **the picture moved x to the right and y down** from its
  unjittered place (so a pixel's sample point is at its centre minus (x, y) in the unjittered picture,
  y down). In the projection: each row's x gets `+2x/width` of its w and its y `-2y/height` of its w
  (for the scene's perspective: `e20`, `e21`, the two elements the engine's own jitter writes). Which
  sign DLSS wants of it is the link's to settle on a game frame.
- **What the jittered projection reaches** (read in the exe; the re-kit's `abi_inputs.tsv` rows of
  0x13D010, 0x172E30, 0x13E010 repeat it). PROVEN unless tagged:
  - between the push and the pop: the visibility stage (`frustum_culling` 0x1B74E6 culls with the
    context's `m_vp`: the frustum that is rasterized, so the culling is the frame's), `process_streaming`
    (0x13D462), the targets' clearing, `fill_opaque_models`, the 23 stages. After the pop (unjittered):
    the scene's flush, the debug drawing, the present stage, Scaleform;
  - **the sun's shadows do not get it**: `stage_shadow_direct::execute` pushes the renderer's +20h, the
    projection before the jitter (`push_jittering`'s answer, copied there at 0x13D3EA), for its whole run
    (0x172EA3, popped at 0x173E98). The local lights' and the rain's shadow maps have projections of their
    own (`push_set_p` of a matrix they build); the sky's and the fog's are made from the frame's current one
    (they read +4F58h, 0x1A4635 and 0x1AEDAE) and draw into the frame: LIKELY with its jitter, not traced;
  - **the first-person models get it without a second site**: `foreground_begin` (0x13E09B) copies the
    context's current projection and replaces only its depth range (e22, e32);
  - **nothing of the game reads it**: every reader in the exe of the context's projection and what `set_p`
    derives from it (+4F58h, +4F98h, +4FD8h, +5098h, +50D8h, +5118h, +5158h, the near/far +4144h, the eye
    rays +53A8h) is a `vostok::render` function: no HUD, aim or hit code is among them. The scene view's own
    matrices (+3ACh, +42Ch: what `set_scene_view` copies from) are never written: the jittered matrix is a
    copy on the render thread's stack, gone at the pop;
  - **the previous view the motion pass uses holds none**: the post-process keeps the previous view matrix
    only (stage +10h, the context's `m_v` copied at the end of every `execute`, 0x1B34E3), no projection;
    `render_models` (0x1B1A15) gives the shader `previous world * previous view` and the shader projects it
    with the context's projection, the current one, as the current position;
  - no stage keeps a view-projection from frame to frame (the readers of `m_vp` +5098h: culling, grass,
    the lights' and the composition's draws, the forward stage, Hi-Z). LIKELY for the SSAO's temporal filter
    (its `execute` reads the eye rays and near/far only; its shaders not re-read).
- **The eye rays** (`context.eye_rays` 0x14AD23): `update_eye_rays` (0x14AA50, from every `set_p`)
  unprojects one corner and mirrors it for the other three, which a jittered projection is not (off by up
  to twice the jitter at the far corner: a position rebuilt from depth a render pixel off at the right and
  bottom; the engine's own temporal jitter has the same fault at half the size). In a jittered frame of
  ours each corner is unprojected by itself (the view's four and the world's four), so a pass that
  interpolates the four rays (the full-screen quads of `fill_surface`, 0x176940 and 0x17D260, put one ray
  in each vertex) rebuilds the position the jittered frame drew. A pass that takes the first ray alone and
  mirrors it in its shader keeps the engine's fault: which do was not read.

**No AA** (`postprocess.aa_method` 0x1B22F1): `r_antialiasing_method` is read as 0 by the post-process of a
frame of ours. The player's option is not written.

**The object motion pass every frame** (`postprocess.motion_gate` 0x1B235A, `postprocess.motion_quality`
0x1B2365, `postprocess.motion_clear` 0x1B1E75): in a frame of ours the pass runs and its target is cleared
at the frame's start whatever `r_use_motion_vectors_in_taa`, the AA method, the motion blur and
`r_post_process_quality` say (the sites' exits into `accumulate_motion_vectors`' call and the clear's
body). The motion blur's own flag is left as the options make it.

**rt 18 signed** (`targets.motion_format` 0x163433): targets made at a render scale have
`$user$object_motion_vectors` as RGBA16_FLOAT (10) in place of the HDR format.

**What the link reads** (`frame_inputs::frame(thread) -> Option<FrameInputs>`, on the render thread, valid
from a frame's `render.jittered` to the next one's; none while the frame is not ours):

| field | what |
|---|---|
| `render` | the size the frame is rendered at |
| `jitter`, `jitter_before` | this frame's jitter and the last frame's (the convention above) |
| `camera`, `camera_before` | the view (`m_v`) and the projection before any jitter, this frame's and the last frame's: row-vector 4x4, `clip = position * view * projection` |
| `reset` | why DLSS's history is thrown away at this frame, or none |
| `colour` | `$user$present` (rt 52) |
| `depth` | the frame's own depth-stencil (the render scale's `DepthTarget`) |
| `linear_depth` | `$user$position` (rt 9, R16_FLOAT: the view z) |
| `motion`, `motion_signed` | `$user$object_motion_vectors` (rt 18: xy = current uv - previous uv, y down; z = 1 where a model was drawn), and whether it is the signed format |

Each target is its `res_texture`'s ID3D11Resource (+1ACh), what DXVK's interop names a VkImage by
(`Target::resource`, the vostok layer's). `camera_before.view` is the view of the last frame of ours; the
motion pass's previous view is the post-process stage's own copy, the same matrix whenever the post-process
ran in that frame.

**The reset**, and the engine event behind each cause:

| cause | taken from | covers |
|---|---|---|
| `First` | the first frame of ours after any that was not (native, no post-process) | the start, the lobby and menus between matches when they are not ours |
| `Targets` | `render_window.targets_size`, the engine making the frame's targets (`set_size` 0x119B2B) since the last frame; or another render size | a resolution or mode change, an options apply, the fallback and back |
| `Scene` | the context's scene (+416Ch) or scene view (+4170h) is another than the last frame's, at `render.jittered` | a map load, the lobby and a match |
| `Cut` | the frame's view is more than 3 m, or turned more than 60 degrees, from the last frame's, at `render.jittered` | a respawn, the death camera, another player watched, a replay's jump |

The camera's events are the logic thread's (`player::update_camera` 0x1F3480 writes the handler's view;
`game_world::switch_to_spectator_camera` 0x1F8360, `base_network_client::spectate_next_player` 0x242D50,
`base_player::on_player_respawn_end` 0x2D9810: the last three inferred names, not reviewed) and reach the
renderer as the scene view's matrix a frame or more later, with nothing to say which frame; the frame that
shows a cut is the one whose view jumps, so that is where it is taken, on the render thread. A cut smaller
than the thresholds (a player watched who stands beside the last one, looking the same way) is not taken.

**The diagnostics switch** `SKHID_DLSS_INPUTS`, read at the process's start after
`SKHID_DLSS_RENDER_SCALE` (which it needs: the sites are the render scale's), for a game without a helper:
`all`, or of `jitter`, `aa`, `motion`, `signed` between commas; and beside them `hold<frames>` (each offset
of the sequence that many frames), `corners<frames>` (in place of the sequence the four offsets a half pixel
out, each that many frames: two of them are a whole render pixel apart) and `look<frames>` (that often: a
log line of the frame's jitter and reset, the frame, its linear depth and its object motion read back and
written beside the game as `dlss-inputs-<frame>-present.bin`, `-depth.bin`, `-motion.bin`, and a line of how
much of the motion is negative; a line at every reset). A jittered picture without DLSS shimmers: expected.

## Falling back

- No broken frame: a frame that DLSS did not finish shows `$user$present` through the stock screen
  image quad, the same draw as today. OBSERVED (the captures): that quad samples by point (0.10b's
  `present_2rt.ps` reads `t_base` with `s_accumulator`; LIKELY the same blob in 0.26g), so the
  stretch is the nearest pixel's, blocks of 1 and 2 pixels at 0.667, not a bilinear one. Open: a
  linear sampler for that draw if the fallback frame should look better than that.
- Any helper failure (it does not start, no answer, a setup refused, the pipe broken, the helper
  process gone, its timeline value late): DLSS stops for the process, the line in
  `srp_patches.log` says why (`dlss: on, not run: <why>`), and the link asks native
  (`render_size::ask`): at the next frame's start (`render.output_setup`, before
  `setup_render_output_window`) the engine's own forced `set_size` of the window at its size and
  mode makes every target native again and our depth-stencil is let go. `set_size` also puts the
  window's style and place back (SetWindowLong, SetWindowPos), as an options apply does.
- **A helper that is gone after the game queued its wait:** DXVK's queue then waits for a value
  nobody signals, and the game's own thread may already wait for that queue (DXVK's Present waits
  for an earlier frame when the GPU is the slower side), so the game's thread cannot be the one
  to notice. The link starts a **watch** with the helper, a thread of its own that waits for the
  helper's process to end: then it plays the helper's part of the timeline itself, from the host
  (a timeline semaphore takes a host signal), and DXVK's queue goes on; those frames show what
  the shared output last held (a repeated frame). The helper's
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
  answer is the "can it run" answer above. With the tick on, the images are made at the frame's
  own render size and the Setup sent; the frames follow the Setup's answer. A Present of
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
  running (each frame's picture a frame of the link), failed (before the helper's yes) or stopped
  (after it), each with its reason. The Settings' gate (`DlssOffer`) is read from that state.
- **Hooks:** the process's start (`memory.allocate_arenas`: whether anything starts, the mip
  bias, the render size and the frame's inputs asked), each Present (`render_output.present`: the
  helper's start, the Hello, the Setup, a new size), the screen image (`render.screen_image`,
  written on demand with the render size's sites: a frame), the login screen
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
- **The start** (`dlss::at_start`, where the mip bias is set: the tick on, an NVIDIA GPU by the
  Vulkan look, the helper's files): the 3D frame is asked at Quality's share of the window
  (`render_size::ask`, 1 / 1.5 each side: a ratio is taken only then, when the frame's sites are
  written) with the frame's inputs (`frame_inputs::ask`, all of them), so the game renders smaller
  from its first frame, shown by the stock stretch until the helper's Setup is answered (about a
  second). A ratio not taken (a site that is not the game's code) is no DLSS in this process:
  `dlss: on, not run: the 3D frame's render size was not taken`, the checkbox greyed.
- **A link that was to run and stops asks native** (`render_size::ask`): a failure, a refusal, a
  helper that ended. The next frame's start makes the targets native again ("Falling back"). With
  the tick off the link asks nothing of the render size (the diagnostics switch's scale stays).
- **The Setup** carries the frame's own render size, the engine's even one (its depth-stencil's:
  1280x720 for 1920x1080, 854x480 for 1280x720), never the Hello's answer, which is only said in
  the log (NGX's optimal size: 853x480 for 1280x720, seen): the helper makes DLSS's feature from
  the Setup's image sizes. Made with it (`Shared`):
  - the four shared images: colour (the picture's format, R8G8B8A8_UNORM), depth (**the
    depth-stencil's own format**, D24_UNORM_S8_UINT seen: its depth aspect is copied as it is, and
    the helper reads a depth format by its depth aspect; no conversion), motion (R16G16_SFLOAT,
    written by the motion pass), output (the picture's format, the window's size);
  - **the screen's texture**: an engine render target of the window's size, made as
    `$user$present` is (`create_render_target`, R8G8B8A8_UNORM), which the helper's output is
    copied into and the screen image's quad samples;
  - the motion pass's pipeline, its views and its own copy of the object motion (below);
  - **a timeline semaphore of its own**, from 0, and the frames' command buffers.
- **A frame** is taken where its picture is whole and the screen image is next
  (`render.screen_image` 0x1418E5: `renderer::present` with `stage_screen_image::execute`'s
  argument pushed, after the stages, the post-process and `render.present_stage`), not at the
  Present. The link running, with DXVK's rendering flushed (`FlushRenderingCommands`):
  - the helper is told the frame (`FrameMsg`): the jitter (below), the reset (a Setup's first
    frame; the inputs' `First`, `Targets`, `Scene`, `Cut`), motion in render pixels (scale 1);
  - **the copy-in** (one command buffer, signals 2N+1): `$user$present` (the picture after the
    tone curve, the game's AA off) copied into the shared colour, the depth aspect of the
    frame's own depth-stencil into the shared depth, the engine's object motion (rt 18) into our
    copy of it; then **the motion pass** writes the shared motion; DXVK's images are given back
    in the layout DXVK keeps them in, the shared ones to the helper;
  - **the copy-out** (a command buffer of its own that waits for 2N+2, the helper's output): the
    shared output copied into the screen's texture. No wait on the CPU: DXVK's later commands
    (the quad, Scaleform, the present) are behind it on the queue;
  - the word `stage_screen_image::execute` is handed becomes the screen's texture (one reference
    of it for the callee, the picture's let go): **the stage's own quad draws DLSS's output**, at
    the window's size (the viewport `render.present_stage` set), point-sampled one to one, and
    Scaleform draws over it at full size.
  A frame the link does not take is shown as it is, the stock stretch of the picture: before the
  Setup's answer; a frame whose command buffers are still on the GPU (four pairs in turn: the
  timeline has not passed the copy of the frame three before); a picture of another size than
  the set's (the window's size changed and the Present after it has not seen it); a failure in
  the frame (then the link stops, and the next frame is native).
- **A frame that is none of the inputs'** (no post-process, another view mode than lit: no
  jitter, no object motion, no camera kept) is upscaled by itself: reset, jitter 0, no motion.
- **The jitter DLSS is given** is the inputs' `Jitter` negated, both axes (`JITTER_SIGN`): the
  inputs move the picture x right and y down, so the pixel's sample is at its centre minus
  (x, y); DLSS takes the sample's place from the pixel's centre (`Jitter_Offset_X/Y`, x right, y
  down). Settled on the still frame ("Status").
- **The motion pass** (`dlss_motion_026g.comp`, a compute shader on DXVK's device, 8x8 groups,
  in the copy-in after the copies): for every render pixel, where its surface was in the frame
  before, in render pixels, current to previous, without either frame's jitter. Where rt 18's
  z > 0 (the engine's object motion pass drew: skinned, moved and first-person models), `-xy *
  render size`; elsewhere the camera's: the pixel's clip position (its centre, less the frame's
  jitter; its depth from the shared depth) through one matrix the game makes in doubles, the
  inverse of this frame's view-projection times the last frame's (`reproject`; both cameras
  without jitter, the inputs'), the difference of the two clip positions in pixels, y down. On a
  reset there is no frame before: no motion is written. The engine's images are only copied
  from (rt 18 into an RGBA16F image of ours, any format the engine made it in): a view of ours
  on an engine image would outlive a target the engine makes again.
  - **Built in the repo's toolchain:** `dlss_motion_026g.py --write` runs `glslangValidator`
    (the dev shell's, pinned by the flake's lock) and writes the SPIR-V words as
    `dlss_motion_026g.rs`, which `vostok/dlss.rs` includes; `selftest_026g.py` runs its `--check`.
  - **The probe** (`SKHID_DLSS_MOTION_PROBE`, a local diagnostic): the pass is given a made-up
    previous camera, this frame's a centimetre to its right, and no object motion; what it
    writes is copied into rt 18 and, every 300 frames, read back with the frame's linear depth
    (`dlss-inputs-<frame>-probe-motion.bin`, `-probe-depth.bin`): a pixel z metres away must
    have moved `-0.01 * e00 * width / (2 z)` pixels along x and none along y.
- **A new size** (a Present whose back buffer is another size than the set's): the old set is
  retired once DXVK's queue is idle (its frames through), the helper told (`FrameMsg.quit = 2`),
  and a Hello and a Setup follow with a new set, **its own timeline semaphore, its frames from 0
  again**. The helper imports each Setup's semaphore handle into a new semaphore of its own and
  destroys the old one with the old images (`client/dlss-helper/src/gpu.rs`, `import`), so
  nothing of it relies on one semaphore across sizes; one per set keeps the old size's waits
  apart from the new one's and gives every shared thing one owner. (PROTOCOL.md's sentence "the
  same semaphore serves every size ... its values go on" is the other choice, which the helper
  takes as well; that file is PR #1709's and is to say "each Setup carries its semaphore: a new
  one, values from 0, or a new handle of the same one, its values going on".)
- **The helper's frame reports** (`FrameStat`) are read off the pipe at each frame; the last one
  is kept for the progress line (which frame, how long DLSS's evaluate took on the GPU).
- **Seeing it run:** `dlss: the first frame is submitted: the screen image shows the helper's
  output from here on` in every game; a game started with `SKHID_DLSS_LOG` set says every 300th
  frame, the timeline's value and the helper's last report in `srp_patches.log` (a local
  diagnostic; unset, nothing).

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
- The render size: implemented (above). Tested: the crate's tests (the ratio, the even render
  size, the owner's steps) and Clippy; `check_hud_options_026g.py` (every start reads the switch
  last; with it unset nothing more). Seen in the game (2026-11-12, Wine 11.8, DXVK, RTX 3060 Ti,
  two replays on a headless display, no input, fullscreen 1920x1080, SSDO and SSR on): with
  `SKHID_DLSS_RENDER_SCALE=0.667` the log's `render size: the 3D frame 1280x720 of 1920x1080`,
  the frame filling the window with the scene where it is natively, the HUD (minimap, scores,
  timer, objective letters, the world marks over their places) at the same places and size as
  natively, the replay playing on (captures every 3 s for 25 s, each another frame); with
  `0.667,150` the log's `native asked` and the match going on at the native size (the forced
  `set_size` inside a frame). The first try at 1281x720 showed bright specks at edges (an odd
  side), not seen at 1280. Not seen: a played match (input, the first-person weapon, aiming and
  hits), the lobby and menus, a window resize or an options apply while scaled, a windowed game,
  Windows, the sites' install in Unicorn. Not released.
- Found on the way, not this work's: the first replay tried (`rec026g_20261109-180618`) ends the
  game in `inventory_item::deserialize` (exe+0x2EFF87, a read of 10Ch) some seconds into the
  match, natively as well as scaled (seven runs, the same address); the other replay plays.
- The frame's inputs: implemented (above). Tested: the crate's tests (the Halton offsets and their count,
  a jittered projection moving a projected point by the jitter, each corner's eye ray, the frames' steps
  and resets, the switch's text, a half float's value) and Clippy; `check_hud_options_026g.py` (every
  start reads the two switches last; unset, nothing more); `engine_026g.py`, `hooks_026g.py --check`.
  Seen in the game (2026-11-12, the same set-up: one replay whose camera stands still, no input, a
  1920x1080 window, the frame 1280x720, 17 to 24 frames a second beside other workers' builds, with one
  piece or all; the frame, its linear depth and rt 18 read back by the switch's `look`; pictures and
  tables in `private/renders/dlss-inputs/`):
  - **the jitter moves the frame by the offset asked, in the direction the convention says.** With
    `corners20` two frames 10 apart have offsets a whole render pixel apart; the second's linear depth is
    the first's moved by that pixel: bit-equal in 98.7 to 99.8 % of the pixels of five still parts at the
    expected whole-pixel shift, as two frames of one offset are at no shift (98.8 to 100 %), against 5 to
    41 % at no shift; least different at the expected shift in 80 of 80 pairs (two runs), the shifts
    (1,0), (-1,1) and (-1,-1): both signs of both axes. The Halton offsets themselves were seen only by a
    least-squares estimate of the shift (direction and order right, correlation 0.99; its size, 0.6 to
    0.9 of the asked, differs by part and by blur: the estimator on point-sampled frames, no measure);
  - **the lit frame of such a pair is not as alike as its depth.** `$user$present` is least different at
    the expected shift too (every pair, the next-best shift 2 to 4 times worse), but 1.0 to 2.6 times as
    different there as two frames of one offset are (1.7 to 3.7 of 255 against 1.2 to 1.9; with SSAO and
    SSDO on 1.2 to 2.9). Not explained: not the SSAO (off and on alike); with the engine's mirrored eye
    rays in place of each corner's own, the right-hand part read 2.3 against 1.75 and the others the same
    (one run each, so the eye rays' effect in the game is small and not settled). Something of the
    lighting or the post-process does not follow a one-pixel jitter exactly;
  - **the engine's motion holds no jitter:** with the sequence running, still skinned models in rt 18 read
    within 0.002 pixel;
  - **no AA:** with the `aa` piece the frame under `r_antialiasing_method` 2 and under 1 is the same frame
    (0.87 of 255 apart on the still parts, as two frames of one run are), and each differs from its stock
    frame (2.4 from the temporal AA's, 3.2 from FXAA's with its sharpen); edges stepped where the stock
    ones are smoothed;
  - **the object motion pass, signed:** with the AA and the motion blur off in the options (the stock gate
    shut: rt 18 empty in 26 of 26 looks), the `motion` piece fills rt 18 where a player runs through the
    view (7 of 25 looks have something covered), at his place in the frame, with both signs of x and y (to
    -16 and +72 pixels a frame);
  - **the reset:** `first` at frame 0 and `cut` at frame 2 of every run (the replay's camera taking its
    place), none after with the camera standing; after the fallback (`0.667,300`: native asked) no frame
    is ours and the match goes on.
  Not seen: a moving camera (the camera's motion in rt 18, a cut at a respawn, the death camera or
  another player watched, a cut taken falsely in a fast turn), the first-person models (their jitter and
  motion), the `Scene` and `Targets` resets, DLSS taking any of it (so the jitter's sign as DLSS wants
  it), a played match, the lobby, Windows. (The jitter's sign and the sites' install in the Unicorn
  check: done with the frame, below.)
- **The frame joined** (the link, the render size and the frame's inputs in one branch; "The link"):
  implemented: the start's asks, the Setup at the frame's own size and formats, a frame at the screen
  image (the copy-in of `$user$present` and the depth, the motion pass, the copy of the helper's output
  into the screen's texture, the screen image's quad drawing it), the jitter and resets told, native asked
  by a link that stops, one timeline semaphore per Setup.
  Tested: the crate's tests (the link's transition with a picture as its frame, the gate, the cameras'
  reprojection matrix against two projections of a point, a frame that is none of the inputs') and
  Clippy; `check_hud_options_026g.py`, 281 cases (new: a start where DLSS is to run writes the thirteen
  sites of the 3D frame, each a jump into the arena with nops to its span's end, each span made writable
  once and given its protection back; every other start leaves the game's bytes and makes no page
  writable: the sites' install is in the Unicorn check now); `check_engine_026g.py`; `engine_026g.py`,
  `hooks_026g.py`, `dlss_motion_026g.py` and `link_026g.py` `--check`.
  Seen in the game (2026-11-12, Wine 11.8 staging, RTX 3060 Ti, the helper PR #1709's build, the replay
  `rec026g_20261109-195343` whose camera stands still, no input, a headless display, fullscreen
  1920x1080, 17 to 24 frames a second; the test folder and its run script are the worktree's
  `private/dlss-frame/`, the pictures `private/renders/dlss-frame/`):
  - **DLSS's output is the frame shown, the HUD over it at full size.** The log of a run: `render size:
    the 3D frame 1280x720 of 1920x1080`, `dlss: offered: DLSS at Quality renders 1280x720 for 1920x1080
    (the helper's answer: 1280x720); the Setup is sent, 19 MiB shared`, `dlss: the first frame is
    submitted: the screen image shows the helper's output from here on`, and every 300th frame `dlss:
    frame 300: its picture is copied in, the screen image shows the helper's output of it; the timeline
    at 600; the helper's last report: frame 299, DLSS took 1849 us of the GPU` (1.3 to 6.5 ms over the
    runs, beside other workers' builds). The shared depth as D24_UNORM_S8_UINT was taken by the helper
    and by DLSS. `still-side-by-side.png` and six `still-crop-*.png` (3x, nearest pixel): the same still
    frame native, at 0.667 through the stock stretch, and with DLSS: thin branches and the roof's edge
    whole where the stretch breaks them into blocks, the slabs' edges smooth, the grass blades apart;
    the minimap, the objective letters and the countdown text the same pixels in all three;
  - **in numbers** (the root mean square difference of 255 from the native frame, three still parts of
    the picture; two native frames 4 s apart differ by 1.5 to 2.4): the stock stretch 6.5, 8.0 and 11.1;
    DLSS 3.5, 4.3 and 7.4. Two DLSS frames 4 or 8 s apart differ by 0.7 to 1.2: steadier than the native
    frame is with itself, no shimmer and no smear in the still parts;
  - **the jitter's sign, settled:** four builds, DLSS given the frame's jitter with each sign of each
    axis, the same frame against native (the three parts): as it is (+x, +y) 6.7, 3.1 and 4.4; (-x, +y)
    8.9, 4.8, 6.2; (+x, -y) 9.6, 6.4, 8.5; negated (-x, -y) 10.9, 7.0, 9.1, with a weave on flat walls
    and the slabs' edges broken into dashes; two frames of the negated build differ by 1.4 to 2.1 where
    the right one's differ by 0.7 to 1.2. Without jitter and with the game's AA (the join's first build)
    DLSS's picture was soft: 11.1, 5.8, 7.2. So DLSS takes the picture's move, x right and y down, and
    the probe's "negated sample offset" is this same value: the sample is at the centre minus the move;
  - **on the inputs' open point** (the lit frame following a one-pixel jitter less exactly than the
    depth): nothing of it shows in DLSS's still frame; its frames are steadier than native's, so no pass
    is named here;
  - **the helper ended from outside in the match** (kill -9 at second 33): `dlss: the helper is gone
    with frames waited for: the timeline signalled up to 920 from the game`, `dlss: on, not run: the
    helper ended, exit code 1`; the capture a second later and every one after is the native frame
    (`helper-killed-crop-thin-lines.png`), the match going on;
  - **the motion pass's camera math, on a read-back** (`SKHID_DLSS_MOTION_PROBE`, the jitter running):
    the previous camera a centimetre to the right of this frame's, the pass's output and the frame's
    linear depth read back at frames 150, 450 and 750. Of 1280x720 pixels, the 912,000 between 0.3 and
    50 m (the others are the sky) have the motion their depth gives, `-5.651 / z` pixels along x: off by
    0.0002 to 0.0018 pixel on average by depth band, 0.0044 at most (the half float of the linear
    depth), and exactly 0 along y. So the clip position from the depth, the jitter taken out, the
    matrix's rows and the pixel units are right; the previous camera was made up, not a frame's;
  - the tick off with `SKHID_DLSS_RENDER_SCALE=0.667`: the stretch stays (the link asks nothing of the
    render size then).
  Not seen: a camera that moves (the replay's stands: the motion of a turn or a walk as DLSS takes it,
  a cut's reset), rt 18's object motion as DLSS takes it (players run through the view in the replay:
  no smear was looked for on them), the first-person weapon and hands, particles and transparent
  surfaces, a frame that is none of the inputs' (the lobby, a view mode), a new size in the game (the
  Settings: input), the exit through game::exit, a frame whose command buffers were busy, windowed,
  Windows, a GPU where DXVK's depth-stencil is not D24S8. Not played, not released.
- Open: `client/dlss-helper/PROTOCOL.md` (PR #1709) to say a Setup's semaphore may be a new one ("The
  link", a new size); the mip bias's reach (not looked at in a capture); the helper's barriers name a
  D24S8 image's depth aspect alone (a validation layer would ask for both: the driver takes it); the
  screen image's point sampler for the frames the link does not take (the first second, a busy frame);
  of the inputs: which passes take one eye ray and mirror it, a cut under the thresholds; and the played
  match.
