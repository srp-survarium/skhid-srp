# Picture-in-picture scopes (patch 198)

The owner, 2026-11-23: "can we implement this wdyt? true picture-in-picture scopes", then "go all
the way". Today a magnified scope swaps the gun for its aimed model (the eyepiece) and zooms the
whole screen, a black ring around the lens; his standing complaint is "the camera should zoom in on
the scope itself" (`scopes-handoff.md` 5.2). With picture-in-picture the frame keeps the normal
field of view and only the lens shows the zoomed scene, drawn by a second view.

Code: `client-patches/rust/patches/src/pip_scopes.rs` (the logic), `vostok/second_view.rs` (the
engine's side), `pip_lens.hlsl` (the lens's two shaders, compiled by `pip_lens.sh` with the game's
own fxc into `pip_lens.rs`). Sites, fields and functions: `sites_026g.tsv`, `fields_026g.tsv`,
`engine_026g.tsv`, the re-kit.

## 1. The second view (the probe, PROVEN in a game)

- **The engine allows it.** A draw is `game::renderer::draw_scene(scene, scene view, output
  window, viewport)` (0x1F7F0), a command the render thread runs as `renderer::render` (0x13D010).
  Every render target, the depth-stencil and every frame-to-frame history (TAA's
  `$user$previous_present`, the eye adaptation's luminance, SSAO's) belong to the **output window**
  (`render_output_window` +118h, made at its size by `create_targets`); the visible lists, the
  camera, the exposure parameters and the environment belong to the **scene view** (1DE40h bytes).
  0.10b's editor draws one scene into two windows each frame (`editor_world.cpp`, the camera view);
  the game makes one of each.
- **Ours:** a hidden `WS_POPUP` window of the picture's size, its own `render_output_window`
  (constructor 0x1198B0, 30B8h bytes) and `scene_view` (0x15DF00), made once on the render thread,
  each counted once by us and kept for the process (`second_view::make`). The window's swap chain is
  never presented: its back buffer (and its `$user$present` target, +2234h) is the picture.
- **Where in the frame:** `game.draw_scene` (game::tick, 0x1ECD4F) queues, before the frame's own
  draw, `set_view_matrix` / `set_projection_matrix` (0x219D0 / 0x21B50) of our view and its
  `draw_scene`, then puts the scene renderer's matrices back to the frame's. The render thread runs
  ours first, so the lens shows this frame's picture, not the last one's (a frame late, the picture
  would slide against the reticle by the turn of one frame times the magnification).
- **What our draw leaves out or keeps for the frame** (render thread; `renderer::render` is not
  reentrant, so one atomic says which draw runs):
  - the interface: no `render_flash_ui` (a `call` hook at 0x13DB3D): no movies, and no screenshot
    or capture taken of our window;
  - the Present (0x13DD3B, S_OK answered);
  - the scene's moved-models reset (0x13DE6B): the frame's own draw still sees them (its motion
    vectors, its shadow caches);
  - the hi-Z occlusion (render options +12Dh off during our draw): its query and the answers it
    stores on each object are the renderer's alone, from one depth; ours would mix two views'. Our
    draw draws every model in its frustum;
  - the first-person models (the foreground list emptied at `stage_forward::execute_foreground`
    0x1AB750; the lens keeps the optic's aimed model alone, section 3);
  - the weather: the frame's view's environment copied into ours before each draw
    (`environment_properties::operator=` 0x15E470, its resources counted).
- **Not shared, so nothing to do:** render targets, depth, TAA history, eye adaptation and SSAO
  history (each window's), the visible lists and exposure (each view's).
- **Shared and left:** the clock. Each draw ticks the lights and particles with the time since the
  draw before it, so the two draws of a frame share the frame's time; each view's eye adaptation
  then adapts with its own part of the frame (slower, never wrong). The frame limiter `r_max_fps`
  (10000 by default) measures each draw alone.

## 2. Cost (the probe, `SKHID_PIP_PROBE`)

Measured 2026-11-23 with GPU timestamps (D3D11 TIMESTAMP queries around each draw, through DXVK)
and the render thread's clock, in a headless capture (`capture.py`, Fire Station day, the two poses
of `fire-station.json` held 20 s each, 1920 x 1080, every quality setting at its highest, TAA), on
an RTX 3060 Ti that other workers' jobs shared, so each figure is a range over four windows of 240
frames; per frame:

```
                        off (time only)   256          512          1024
the frame's draw, GPU    7.0 .. 11.7      7.1 .. 8.7   7.7 .. 8.4   5.7 .. 8.3   ms
the second view, GPU         -            2.4 .. 2.9   2.5 .. 3.5   3.5 .. 4.6   ms
the second view, CPU         -            1.5 .. 1.7   1.4 .. 1.6   1.3 .. 1.9   ms (render thread)
the frame's draw, CPU    3.4 .. 4.8       3.0 .. 3.6   2.8 .. 3.4   3.3 .. 3.7   ms
```

- The second view costs about 2.5 ms of GPU at 256 and 512 and 4 ms at 1024: most of it is the
  view's own fixed work (the sun's shadow cascades fitted to its frustum, the local lights' shadow
  faces it sees, the culling, the ambient and post-process passes), not its pixels; 1.5 ms of the
  render thread at every size.
- The frame interval (50 to 58 ms in every run, with the second view or without) is the rig's, not a
  measure: neither draw was its limit.
- **Broken: nothing seen.** The frame's own picture with the second view (the inset masked out)
  differs from the picture without it as one run differs from another (mean 0.9 of 255), but for a
  few thin sunlit edges in one pose (0.6 % of the pixels, specular glints on wires and poles;
  LIKELY the sun's shadow cascades, which each draw fits to its own frustum). The second view's
  picture: the same weather, fog and exposure, sharp, no interface, no first-person model. Not
  seen in a static capture: particles (none at these poses), a moving object's TAA history, the
  texture streaming at a narrow field of view (each draw asks its own mips).
- Pictures: `private/renders/pip-probe/fire-station-<pose>-<off|256|512|1024>.png` (the second view
  in the top right corner), each run's log and `timings.txt` beside them.
- **The default from these numbers:** 512 (the lens's picture is a disc of 0.45 of the half height:
  486 px across at 1080p, 648 at 1440p). The lens is drawn only while a magnified optic is aimed, so
  the cost is that time's alone.

## 3. The lens (`SKHID_PIP_SCOPES`, unplayed)

- **When:** the eyes' weapon (first person, or a spectator's eyes) holds a magnified optic
  (`hide_weapon_on_aim`: the PSO-1, ACOG, Leupold and the other 0.69 rifle scopes; not
  collimators, not iron sights).
- **The frame's field of view stays the eyes' own** (uro_fov) for the whole aim: the projection
  apply_camera set is replaced, for the draw and for the scene's copy the interface projects with.
  The zoom field itself (the camera's +94h) is the game's, so the mouse sensitivity, which scales
  with it, is today's for the same optic.
- **The body, the hands and the idle optic stay in the scene** at the swap
  (`add_aimed_rifle_scope_to_scene` 0x244B60 / `remove_aimed_rifle_scope_from_scene` 0x244C50:
  four lohooks skip the idle model's removal and the body's hiding for the eyes' weapon). The aimed
  model is still added: the frame's draw leaves its surfaces out of its first-person list, the
  second view keeps only them. So the aim-in is the camera closing on the optic itself (its own
  eye since the 0.69 scope port), and at the swap the lens opens.
- **The second view** is the scoped view of today: the same camera, the optic's full zoom (patch
  22's angle at its fov_factor), the aimed model in front of the eye with its reticle, narrowed to
  the lens (a radius of 0.45 of the half height) and square. Where a bullet goes is the screen's
  centre, the camera's axis: the second view's centre, so the reticle's centre is the shot's.
- **The lens** is that picture drawn over the frame as a disc at the screen's centre, after the
  frame's post-process and before the interface (`render.flash_ui`), with its own two shaders, every
  Direct3D state it sets put back. Its rim darkens over its outer 8 %. The draw is PROVEN in a game without a scope:
  `SKHID_PIP_PROBE=disc` draws the probe's picture this way (`private/renders/pip-probe/fire-station-*-lens-disc.png`;
  a thousand frames, the interface drawn after it each frame, no fault). Drawn after the TAA, it has
  no ghost of the last frame's picture while turning (the TAA blends half of the last frame where
  nothing moves, and a lens drawn as a surface of the gun would be such a place).
- **Magnification:** the target in the lens is as large as in today's scoped view; around the lens
  the world is at the eyes' field of view.
- **Others** see nothing different: everything is the eyes' own drawing; the world format and what
  a match computes do not change (no `simulation_changes.tsv` row).

## 4. The setting

- `srp_pip_scopes on|off` in user.cfg, off by default (patch 83's console variable, `hud_options`),
  read each frame: off is today's behaviour exactly (no second view drawn, no hook acts).
  `SKHID_PIP_SCOPES=on|256|512|1024` turns it on whatever the setting says (local tests).
- The label `st_srp_pip_scopes_option` is in the twelve dictionaries. **Not made:** its checkbox in
  the Graphics tab (Settings.as, `SrpVideoTab`); until then the console sets it.

## 5. Not done, and what a real use still needs

- **Unplayed.** The lens part (section 3) has never run: a static capture has nobody to aim, and
  nothing here aims. Everything in it is from the code read; the owner's test is the proof.
- **The lens's size and place** are a constant disc at the screen's centre (0.45 of the half
  height). The optic's eyepiece, seen at the eyes' field of view from its own eye, has a size of its
  own; the disc should be the eyepiece's (its lens surface's bounds projected, per optic, as X-Ray's
  fork measures it) and follow it through the aim-in and the gun's sway, not only at the full aim.
- **The reticle** is the aimed model's own, drawn in the second view: today's reticle, at today's
  angular size, cut to the lens. A second-focal-plane look; nothing per optic.
- **Exposure:** each view adapts its own eye (the second view to the scope's picture); the frame's
  exposure is not imposed on the lens.
- **Cost cuts not made:** a lower shadow, AO or SSR quality for the second view (the stages' enable
  bytes, opt+109h..+11Eh, set around its draw as the hi-Z switch is), and a second view sized to the
  lens on screen.
- **Memory:** the second view's window holds a full set of targets at its size (at 512: about 30
  MB of video memory, LIKELY, from `create_targets` formats), made the first time it is wanted and
  kept for the process.
- **Spectators** with the setting see through the spectated player's optic the same way (the eyes'
  camera is theirs).
