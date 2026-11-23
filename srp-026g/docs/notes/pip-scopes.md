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
  - the first-person surfaces, on three lists of the drawn scene view: the G-buffer's (+84ACh,
    filtered at `gbuffer.models` before stage_gbuffer copies it; its render_models draws the
    first-person ones in a call of their own), the forward stage's (+0C4B8h) and the forward-lit
    one (+44A0h, stage_lights' foreground pass), both at `forward.execute_foreground`; a
    first-person surface is one whose owner's +114h is set. The lens keeps its optic's aimed model
    but its glass (section 3);
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

## 3. The lens (the setting, or `SKHID_PIP_SCOPES`)

- **When:** the eyes' weapon (first person, or a replay's followed player) holds a magnified optic
  (`hide_weapon_on_aim`: the PSO-1, ACOG, Leupold and the other 0.69 rifle scopes; not
  collimators, not iron sights), and the setting is on. Nothing is made before the first such aim
  (the window, the view and the lens's shaders are made then, kept after).
- **No swap.** The stock game, at the aim's change point (the weapon's +181Eh, set by weapon::tick
  in the world), swaps a magnified optic: the gun's body out of the scene, the hands hidden, the
  idle optic out and its aimed model (an eyepiece and a reticle) in, the whole screen zoomed. With
  the lens the eyes' weapon draws as an optic that does not hide the gun: weapon::on_show and
  weapon::draw take their other branch at each of their scope tests (`scope_draw.*` in
  `sites_026g.tsv`, one handler question, `lens_takes`): the idle optic shown and updated, the body
  updated, the hands left alone, the game's own aimed model never added (+181Dh stays 0). So the
  real gun, the real optic and the hands stay in the scene from hip to full aim, animated and
  updated by the game itself. +181Eh, the zoom (+94h), the sensitivity and where shots go are the
  game's as today.
  - Turning the setting on while the game's swap is in (+181Dh) takes it out through the game's
    own other branch (remove_aimed_rifle_scope_from_scene) at the next draw; off puts it back.
- **The reticle.** The lens alone puts the optic's aimed model in the scene (`Reticle::In`, at the
  swap site, while the lens takes the weapon and its scope's view is on, never together with the
  game's own: +181Dh 0), moves it with the gun at the optic site (the weapon's +660h, where
  add_aimed_rifle_scope_to_scene puts it) and takes it out when the lens stops or the weapon is put
  away (`attachments.hidden`). The frame leaves its surfaces out; the second view draws only its
  reticle (all its surfaces but its glass).
- **The frame's field of view stays the eyes' own** (uro_fov) while the magnified optic is held:
  the projection apply_camera set is replaced, for the draw and for the scene's copy the interface
  projects with. The zoom field itself (the camera's +94h) is the game's and is never written, so
  the mouse sensitivity, which scales with it, is today's for the same optic.
- **The lens's place** is the aimed model's glass on the frame's screen, each frame: of that model's
  surface records (behind the pointer at +298h of a static instance; at +62D4h of a skeleton one) the
  one whose render surface has 61 vertices and 180 indices (+10h, +14h of a static render surface):
  every 0.69 optic's `scope_lense_1p_s` is a fan of 60 triangles about its centre (the pack's
  meshes, the ten rifle scopes), and no reticle, red dot or body of theirs has those counts. The
  record's bound sphere (world space, +10h; a flat disc's, so its radius the disc's) projected with
  the frame's own view and projection (`second_view::lens_on_screen`, at
  `forward.execute_foreground`) is the disc. A first rule, the smallest sphere of at least 8 mm on
  the eye's axis, took the PSO-1's reticle (10 mm) for its glass (18 mm): seen headless. The glass
  sits in the real optic's eyepiece (the Hyperon's: 18.6 mm of radius, 90 mm from the eye at full
  aim; the PSO-1's 18 mm at 67 mm, inside its long eyecup), so the disc is the eyepiece's opening. A
  record's sphere is placed by the renderer a frame or two after the model enters the scene (before,
  it holds FLT_MAX): no lens those frames. **An aimed model with no such glass placed in 30 frames of
  an aim is refused**: today's scope for that optic for the rest of the run, one log line naming it
  (the log's surface lines give each surface's counts). An aimed model of another class than static
  or skeleton: today's scope, one log line.
- **The second view's camera is the aim itself**, the eyes' transform (the player's +44Ch, bone
  Item), read at update_camera, not the frame's camera: a bullet takes its start and its direction
  from it (weapon_core::instant_fire copies its 16 dwords at 0x2E2440..0x2E2453 and hands them to
  get_dispersed_bullet_direction 0x2E1D50 at 0x2E245C). The two are the same today; with a view that
  trails the aim (the camera-trail branch), the frame's camera moves and the scope's picture does
  not: the reticle's centre stays where a shot goes.
- **What the second view draws:** the world, and of the first-person surfaces only the aimed model's
  reticle, red dot and body (not its glass). First-person surfaces are on three lists of the drawn
  scene view (section 1): the gun's and hands' and the optic's opaque ones on the G-buffer's, the
  glass and the emissive ones on the forward-lit and forward lists. Each list keeps its world
  entries. The frame leaves the aimed model out of all three. Seen headless (section 6): with only
  the forward lists filtered the picture was black, the eye inside the kept optic's own tube.
- **Magnification:** the second view's field of view follows from the lens's size on the screen and
  the optic's zoom, so the world in the lens is as large as in today's scoped view of that optic:
  tan(half its field) = r x tan(half the optic's full-zoom angle), r the lens's radius in the frame's
  half heights, the optic's angle patch 22's at its fov_factor (65 x 0.25 = 16.25 degrees for every
  full scope, whatever uro_fov). The lens magnifies tan(uro_fov / 2) / tan 8.125 deg against the
  world around it: **4.46x at uro_fov 65, 7.0x at uro_fov 90** for a full scope, as today's scoped
  view over today's unscoped one. The radius r is the last frame's lens's.
- **Zoom steps:** none in 0.26g. An optic has one `fov_factor` (the exe's only optic zoom key; no
  step, range or zoom key of an optic is read: its strings are `fov_factor`, `aim_zoom_factor` and
  the free camera's `cam_zoom_in/out`); the Leupold 3-7's item has 0.25 like every full scope.
- **The picture's size follows the lens:** 512 pixels square while the lens is up to about 560
  pixels across on the screen, else a 1024 one (made at the first such lens, kept after).
- **The lens's picture** is drawn over the frame as a disc, after the frame's post-process and before
  the interface (`render.flash_ui`), with its own two shaders (`pip_lens.hlsl`), every Direct3D state
  it sets put back; its edge alpha-blended by each pixel's share inside the circle (no stair-steps),
  its rim darkened over its outer 8 %. Not depth-tested: at full aim nothing is in front of the
  eyepiece's glass.
- **Exposure:** the second view adapts its own eye to its picture, as any view does (section 1).

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
  the frame's; the right is brighter from an exposure try since dropped: the frame window's adapted luminance copied in came out plainly off). With 512 the lens costs
  about 1 ms of GPU and 1 to 1.4 ms of the render thread while a magnified optic is aimed, nothing
  otherwise.

## 6. Seeing an aimed optic headless (diagnostics)

- `srp-map-sur <map> <out.sur> --hold <gun>[:<optic>[:<rounds>]] [--enter <ms>] [--aim <ms>]`
  (`sur_convert::of_map` with a `Holder`): the map's replay with its one place entering at 1 s with
  one kit, the gun in slot 7 with the optic's record as the lobby writes it
  (`{optic item, its dict id, 0, 3}`), its rounds in slot 8, and the aim key (0x80) held from 30 s to
  the end. The game simulates the place from those inputs as any replay's.
- Patch 176's script: `follow <place>` (the replay client's follow_place_first_person, 0x26DF70,
  asked once a second until the place is followed; a live match never) and `eyes <name> <settle ms>`
  (a picture of the game's own camera: the followed eyes, first person).
- `capture.py --hold 14:2104:51 --aim-ms 30000 --env SKHID_PIP_SCOPES=on --env SKHID_PIP_LOG=on`
  with a poses file of `{"name": ..., "eyes": true, "settle_ms": ...}` and `"follow": 0` in its
  defaults; `--uncapped` drops the rig's 60 fps cap. Nothing of it runs without its switch or script.
- `SKHID_PIP_LOG=on` (off by default, nothing when off): at each aim the optic's fov_factor, its
  aimed and idle models, uro_fov, the lens's last radius and the second view's field of view; at the
  aim's first and thirtieth frames the aimed model's surfaces (radius, centre, in the frame's view)
  and the frame's first-person list, then the lens's place or why there is none. `=picture` also
  copies the second view's picture into the frame's top right corner.
- Each timing line of the log (every 240 frames once the second view is made) has the frame
  interval, the Present's own time (its call to renderer::render's end: a wait for the display
  shows there) and the game's thread's interval, beside the CPU and GPU times of both draws.

## 7. Not done

- **Depth:** the disc is not depth-tested against what is in front of the eyepiece (at full aim
  nothing is).
- Local lights' shadow faces and the remaining passes run at full quality in the second view.
- **Memory:** the second view's window holds a full set of targets at its size (512: about 30 MB,
  1024: about 120 MB of video memory, LIKELY, from `create_targets` formats), made at the first aim
  that needs it, kept after.
- **Spectators** with the setting see through the spectated player's optic the same way (the eyes'
  camera is theirs).

## 8. The play tests

**Round 1 (2026-11-23, exe `pip-scopes`, AK-74M + Leupold 3-7): two rings, no zoom.** weapon::draw
updates one optic model a frame, the aimed one once the scope's view is on: the idle optic, kept in
the scene by the hooks of that design, stayed where the swap left it (the second ring), and the lens
was never placed (below).

**Round 2 (exe `pip-scopes-2`): "white screen when scoping", then a dark disc with the glass's
pattern, an AK lying at a stale place ("the last gun is still there"), and the SVD's lens nearly
screen-wide, showing the scope's own insides.** Causes, each PROVEN headless:
- `fields_026g.tsv` had a static model instance's surface records inline at +298h; they are behind a
  pointer there (get_surfaces 0x16F436: `add eax,[edi+298h]`). The lens read the instance's own
  bytes as bound spheres: discs of any size and place, a screen-wide one sampling one texel of the
  picture (white with the first glass material).
- Even read right, a record's sphere is FLT_MAX until the renderer places it, a frame or two after
  the model enters the scene: an upper bound of 25 cm now keeps those out.
- The second view filtered only the forward stage's first-person list; the guns, hands and optics
  are drawn from the forward-lit list (+44A0h): it looked into the kept idle optic's own glass from
  inside it.
- The stale gun: the swap design kept the body in the scene, but weapon::draw does not update the
  body while the scope's view is on for a hide_weapon_on_aim optic (0x244F00..0x244F25).

The owner then asked why it has to be "a transition from one fake gun to another": the design
changed to no swap (section 3).

**The frame rate while a magnified optic was held (rounds 1 and 2):** 13.3 ms a frame against 5.3;
the owner's display runs at 74.96 Hz (niri, HDMI-A-1), so those frames waited for the display. Found
headless (an X11 dummy display whose Present itself waits for its display, so every run there is at
about that rate; the timing lines of section 6 say where each frame waits):



- Every renderer::render ends, after its Present, with a wait for the GPU to finish its draw
  (r_gpu_sync, a console byte at 0xA71BF7, on by default: event_query::wait 0x3CF4B0 at 0x13DE1F).
  The second view's draw is a renderer::render too: its wait made a second CPU-GPU sync each frame,
  and in the rig the frame's wait for the display moved there (the frame's Present stopped waiting,
  the second view's tail took the 10 ms). The game's thread and our own hook wait for nothing
  (2 microseconds a frame in the hook; the logic's own 10 ms sleep, game+4E38h, never on).
- **Fixed:** the second view's draw turns r_gpu_sync off and puts it back at its end
  (`render.unmove`); the frame's own draw keeps its wait, so a frame ends synchronised as today
  (r_gpu_sync 0 is a setting the engine supports). Headless, the wait is the frame's Present's again
  with the lens. Whether this was all of the owner's 75 Hz is for the next play test's timing lines.
