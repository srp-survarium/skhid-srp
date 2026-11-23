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
- **The default from these numbers:** 512 (the lens is the eyepiece on the screen, a disc of about
  half the height: 512 covers it at 1080p and nearly at 1440p). The lens is drawn only while a magnified optic is aimed, so
  the cost is that time's alone.

## 3. The lens (the setting, or `SKHID_PIP_SCOPES`; unplayed)

- **When:** the eyes' weapon (first person, or a spectator's eyes) holds a magnified optic
  (`hide_weapon_on_aim`: the PSO-1, ACOG, Leupold and the other 0.69 rifle scopes; not
  collimators, not iron sights). Nothing is made before the first such aim with the setting on (the
  window, the view and the lens's shaders are made then, kept after), and nothing is drawn while
  no magnified optic is held.
- **The frame's field of view stays the eyes' own** (uro_fov) for the whole aim: the projection
  apply_camera set is replaced, for the draw and for the scene's copy the interface projects with.
  The zoom field itself (the camera's +94h) is the game's and is never written, so the mouse
  sensitivity, which scales with it, is today's for the same optic.
- **The body, the hands and the idle optic stay in the scene** at the swap
  (`add_aimed_rifle_scope_to_scene` 0x244B60 / `remove_aimed_rifle_scope_from_scene` 0x244C50:
  four lohooks skip the idle model's removal and the body's hiding for the eyes' weapon). The aimed
  model is still added: the frame's draw leaves its surfaces out of its first-person list, the
  second view keeps only them. So the aim-in is the camera closing on the optic itself (its own
  eye since the 0.69 scope port), and at the swap the lens opens.
- **The aimed model hangs on the gun**, not on the eye: add_model puts it at the weapon's world
  matrix (weapon+660h, 0x244BA9), and weapon::draw moves the idle and aimed models by the gun's
  scope locator (weapon+744h) every frame. So it follows the gun's sway and kick.
- **The lens's place** is that model's eyepiece on the frame's screen, each frame: of the aimed
  model's surfaces (a static instance's records, +298h, their world spheres at +10h) the smallest of
  at least 8 mm radius that the frame's view axis passes through (the glass or the reticle's plane,
  not the body, not a dot), projected with the frame's own view and projection
  (`second_view::lens_on_screen`, at `forward.execute_foreground`, the frame's draw). The disc is
  that sphere's: its centre and its radius on screen. Before the swap there is no lens (the idle
  optic's glass as today). Not done: what is in front of the eyepiece hiding it (at the full aim
  nothing is; the disc is drawn without a depth test).
- **The second view's camera is the aim itself**, the eyes' transform (the player's +44Ch, bone
  Item), read at update_camera, not the frame's camera: a bullet takes its start and its direction
  from it (weapon_core::instant_fire copies its 16 dwords at 0x2E2440..0x2E2453 and hands them to
  get_dispersed_bullet_direction 0x2E1D50 at 0x2E245C). The
  two are the same today: update_camera hands that transform to the view (`camera.eye`, 0x1F3643)
  and apply_camera copies the view into the scene in the same tick, so no frame lies between them.
  With a view that trails the aim (the camera-trail branch), the frame's camera moves and the
  scope's picture does not: the reticle's centre stays where a shot goes.
- **Magnification:** the second view's field of view follows from the lens's size on the screen
  and the optic's zoom, so the world in the lens is as large as in today's scoped view of that
  optic: tan(half its field) = r x tan(half the optic's full-zoom angle), r the lens's radius in the
  frame's half heights, the optic's angle patch 22's at its fov_factor (65 x 0.25 = 16.25 degrees
  for every full scope, whatever uro_fov). A direction at angle t is then at
  tan t / tan 8.125 deg half heights in the lens and at tan t / tan(uro_fov / 2) in the frame: the
  lens magnifies tan(uro_fov / 2) / tan 8.125 deg, the same as today's scoped view over today's
  unscoped one. **For a full scope (PSO-1, ACOG, fov_factor 0.25): 4.46x at uro_fov 65, 7.0x at
  uro_fov 90** (on screen, against the world around the lens). The radius r is the last frame's
  lens's (one frame behind while the eyepiece grows during the aim-in).
  - The first lens probe (`SKHID_PIP_PROBE=disc`, the earlier captures) squeezed a quarter of the
    frame's field into a disc of 0.45 half heights: 1.8x on screen, wrong. It now uses the same rule
    (a full scope's angle, r = 0.45): MAGNIFICATION_CAPTURE.
- **The reticle** is the aimed model's own, drawn in the second view: today's reticle at today's
  angular size, cut to the lens; its centre the second view's centre, the shot's direction.
- **The lens's picture** is drawn over the frame as a disc, after the frame's post-process and
  before the interface (`render.flash_ui`), with its own two shaders (`pip_lens.hlsl`), every
  Direct3D state it sets put back; its rim darkens over its outer 8 %. Drawn after the TAA, it has
  no ghost of the last frame's picture while turning (the TAA blends half of the last frame where
  nothing moves, and a lens drawn as a surface of the gun would be such a place). The draw is PROVEN
  in a game: `SKHID_PIP_PROBE=disc` draws the probe's picture this way
  (`private/renders/pip-probe/fire-station-*-lens-disc*.png`; thousands of frames, the interface
  drawn after it each frame, no fault).
- **Exposure:** the second view adapts its own eye to its picture, as any view does. Measured (the
  second view's picture against the same world region of the frame, mean of 255): 72.9 against 81.0
  and 66.9 against 74.1 at the two poses, about 10 % darker; the owner saw no issue in the captures,
  so it stays. A try at giving it the frame's exposure (the frame window's adapted luminance, its
  target 59, copied in, its time step 0) came out plainly off (63 and 104 for 81 and 74) and is out.
  Its draw leaves the eye-adaptation reset flag (0xAC914C, one for every view) to the frame's draw.
  The frame's own exposure is not touched: the same region left of the centre measures the same with
  the second view as without (71.1 of 255; 68.1 and 68.4).
- **For other patches:** `pip_scopes::frame_field_of_view(thread)`, the frame's vertical field
  of view this frame in degrees (uro_fov while the lens is on, the game's zoomed one otherwise),
  for what scales with the eyes' view; the camera's +94h stays the scope's zoom either way.
- **Others** see nothing different: everything is the eyes' own drawing; the world format and what
  a match computes do not change (no `simulation_changes.tsv` row).

## 4. The setting

- "Picture-in-picture scopes" on the Graphics tab, one row under Distance detail (Settings.as:
  an `SrpCheckOption` on the video frame, Apply and Cancel as the other checkboxes of ours), the
  exe's `srp_pip_scopes on|off` in user.cfg (patch 83's console variable, `hud_options`), off by
  default, read each frame: off is today's behaviour exactly (no second view made or drawn, no hook
  acts). Its label `st_srp_pip_scopes_option` is in the twelve dictionaries. The movie builds
  (`flash_tree.py build`); its place on the tab is not seen yet (no Ruffle run).
- `SKHID_PIP_SCOPES=on|256|512|1024` turns it on whatever the setting says (local tests).

## 5. Cheaper second view

- Off in the second view's draw, put back after it: the hi-Z occlusion (section 1), SSAO/SSDO
  (render options +10Eh), the screen-space reflections (+113h), and the sun's shadow cascades
  (+109h): its shadow mask reads the cascades the frame's last draw fitted from the same eye, which
  cover the narrower frustum (a frame old).
- Measured the same way as section 2 (Fire Station, 1080p, highest settings, the shared GPU; per
  frame, four windows of 240 frames):

```
                               256          512          1024
all passes (section 2)         2.4 .. 2.9   2.5 .. 3.5   3.5 .. 4.6   ms GPU
AO and SSR off                 2.4 .. 2.8   2.5 .. 4.4   5.0 .. 6.4   ms GPU (a busier GPU that hour)
and no cascades of its own     0.5 .. 0.8   0.9 .. 1.1       -        ms GPU
                               0.8 .. 1.1   1.0 .. 1.4       -        ms render thread CPU
```

- The sun's cascades were most of the cost (each draw refits and redraws them for its own
  frustum); AO and SSR at these sizes almost nothing. Without its own cascades the second view's
  shadows are the same as with them (`fire-station-s-high-512-cascades.png`: left its own, right
  the frame's; the right is brighter from the exposure try of section 3). With 512 the lens costs
  about 1 ms of GPU and 1 to 1.4 ms of the render thread while a magnified optic is aimed, nothing
  otherwise.

## 6. Not done

- **Unplayed.** Sections 3 to 5 have never run with an optic aimed: a static capture has nobody to
  aim. The replay archive (`~/games/skhid-replays`, three replays) could show a recorded player's
  aim, but the capture rig plays only its own empty-map replay and holds the camera at its poses
  (patch 176 writes the camera at apply_camera); showing a recorded scoped aim needs a capture that
  follows a player's eyes to a given moment: not made.
- What is in front of the eyepiece is not tested against the disc (no depth test).
- Local lights' shadow faces and the remaining passes run at full quality in the second view.
- **Memory:** the second view's window holds a full set of targets at its size (at 512: about 30
  MB of video memory, LIKELY, from `create_targets` formats), made at the first aim, kept after.
- **Spectators** with the setting see through the spectated player's optic the same way (the eyes'
  camera is theirs).
