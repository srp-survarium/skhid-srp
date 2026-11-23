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
- **The lens's place** is the real optic's rear glass as the frame draws it (round 4, section 10):
  its round in the idle model's own space, found once per optic from the meshes, placed each frame
  by the idle model's world matrix (a record's +50h, the matrix the renderer draws it with) and the
  drawn scene view's own view and projection (+3ACh, +42Ch, read at `render.begin` of the frame's
  draw: the game's thread runs ahead of the render thread, so a camera kept from the game's thread is
  the next frame's). The round: the aimed model's glass (the record of 61 vertices and 180 indices,
  every 0.69 optic's `scope_lense_1p_s`: its fan read back from the GPU gives its centre, normal and
  radius) gives the optic's axis; of the idle model's surfaces (up to 512 vertices each, read back
  the same way: a staging copy of the res_buffer's part of its pool buffer, once) the flat rounds
  across that axis (a ring of points in one plane, centred on the axis) are the glasses, and the one
  nearest the eye is the rear glass. An idle model with no such round (an optic without its own glass
  mesh): the aimed glass in its place, one log line. A glass not read: today's scope for that optic,
  one log line. The renderer's bound sphere of a flat round is its box's, the radius times the root
  of 2: the round-3 disc, from that sphere, was 41 % too large.
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
- **The picture's size follows the lens:** the smallest of 512, 768 and 1024 pixels square at least
  as wide as the lens on the screen (512 until a lens is measured), each made at its first need and
  kept; the aim and timing lines print the one drawn.
- **The lens's picture** is drawn over the frame after its post-process and before the interface
  (`render.flash_ui`), with its own two shaders (`pip_lens.hlsl`): a quad in the glass's plane
  projected with the frame's projection (an ellipse when the glass is seen at an angle), the picture
  mapped in screen space about the glass's centre (the reticle at the glass's centre), its edge
  alpha-blended by each pixel's share inside the round, its rim darkened over its outer 8 %, every
  Direct3D state it sets put back. Tested against the frame's depth (section 12, 2): what of the
  optic is in front of its rear glass covers the picture.
- **Exposure:** the frame's (section 12, 1): the second view's eye adaptation starts from the frame's
  adapted luminance with a time step of 0.

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

- `srp-map-sur <map> <out.sur> --hold <gun>[:<optic>[:<rounds>]] [--enter <ms>] [--aim <ms>] [--move <ms>]`
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
about that rate; the timing lines of section 6 say where each frame waits; AK-74M + Leupold 3-7,
960 x 540, uncapped, per frame):

```
                                      frame's Present   second view    interval
lens off (SKHID_PIP_PROBE=time)       10.5 ms           -              13.0 ms
lens on, aimed (round 2's draw)        0.02 ms          0.8 ms CPU,    13.4 ms
                                                        then ~10 ms
                                                        in its tail
lens on, aimed (r_gpu_sync skipped)    8.9 ms           1.0 ms CPU     12.5 ms
```

(the render thread between draws 0.04 ms in all three: never idle.)

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

## 9. Round 3 (exe `pip-scopes-3`, 2026-11-23 12:17) and the handoff for `pip-scopes-4`

Standing still the lens works (the owner's `2026-11-23-12-18-49.png`: one real gun and optic, the
zoomed picture with its reticle filling the eyepiece, the world at uro_fov). Round 4 (section 10) took
up each item.

**1. The lens leaves the eyepiece while the player moves** (`12-19-36.png`, AK-74M + Leupold 3-7:
the disc about one radius right of the real optic's ring; its picture, reticle centred, is fine).
The disc is placed from the aimed model's glass record (its world sphere, read at the frame's
`forward.execute_foreground`), a model the lens puts in the scene and moves itself at
`scope_draw.optic` with the weapon's +660h, as weapon::draw moves the idle optic in the same call.
So the two should coincide; they do not while moving. The log of that round has the aimed glass at
`in view (13 -1 87)` and `(15 -1 87)` mm when moving and `(0 0 ...)` when still. Not found yet which
of these it is: the aimed model's sphere a frame late (placed by its own update command, which may
run after the second view's draw and before or after the frame's visibility), or the frame drawing
the first-person models with a camera other than the scene's +0Ch that `FRAME_CAMERA` keeps.
**Next:** place the disc from the real optic as this frame draws it: the idle optic's world matrix
(a static instance's +14Ch) times the aimed glass's model-space centre (the aimed model's own +14Ch
inverted times its sphere's centre: the two models share the frame of the optic's slot), read at the
frame's forward stage; log both models' records in view at the aim's frames 0 and 30 to see which
lags. Then look at it headless while moving: `srp-map-sur --hold` needs a strafe (`sur_convert::of_map`
pushes one input at `aim_ms`, AIM 0x80; add inputs alternating AIM|LEFT 0x4 and AIM|RIGHT 0x8 every
second after it) and capture `eyes` poses during it.

**2. "Circle in circle" on the PSO-1** (`12-20-20.png`): inside the lens a smaller faceted circle,
darker and clearer, with the chevron; the rest of the lens pale. The owner's setup mounts content
test patches `resources.db.patch_74` and `_75` (PR #2558): the idle optics' `scope_lense_3p_s` (front
and rear glass, one surface) on a new blended, see-through material
`weapons/modules_069/scope_lense_3p_glass` (`forward_lighting_no_distort`). The rig ran with
`patch_74` only. LIKELY: that material is on a list the second view's filter does not cover (the
filter covers the G-buffer +84ACh, forward +0C4B8h and forward-lit +44A0h lists), so the second view
looking down the tube draws the optic's own tinted discs; or the frame draws the rear glass over the
disc. **Next:** run the rig with both patches (`--content` each, linked from the setup's game folder,
never written), find the list (dump each scene-view list's first-person owners at the second pass, as
`foreground_seen` does for one), make the filter total for the eyes' weapon, and check the 13 optics.

**3. The frame rate: no display wait found; the render thread is busy.** The owner's timing lines
(`srp_patches.2026-11-23_12-17-35.log`): unaimed `interval 6.4, present 2.0, main cpu 4.3` ms; aimed
(lens drawn) `interval 10.2..13.5, present 0.1..5.4, main cpu 5.9..7.6, second cpu 2.5..4.4` ms, and
`render thread between draws` 0.05..0.1 ms: the render thread never waits for the game (and the game
thread's interval equals the render's: it waits for the render thread). Every aimed window adds up:
main CPU + second CPU + present = the interval. So the cost is the render thread's work:
- the second view's CPU, 2.5 to 4.4 ms: **its picture was the 1024 one**: `second view made, 1024
  (a large lens)` came right after the 512 one, because `LENS_SIZE` starts at 0.45 half heights
  and 0.45 x 1440 = 648 px is over the 563 px threshold before any lens was measured; the aim lines'
  `its picture 512 px square` prints the constant `SCOPE_SIZE`, not the picture used (a log fault).
  The Leupold 3-7's lens is 0.47 half heights, 677 px at 1440p. **Next:** a 768 picture between,
  the threshold measured from the lens itself, and the log printing the picture drawn;
- the frame's own CPU grows 1.6 to 3.3 ms with the lens (not found why: the lens's draw at
  `render.flash_ui` and the list filters are cheap; LIKELY DXVK's command stream backing up behind
  the second view's draw, which now has no GPU wait of its own);
- `present` includes the frame's r_gpu_sync wait (0x13DDCC..0x13DE5F lies between the Present and
  `render.unmove`). A site after the Present returns is in `sites_026g.tsv` (`render.present_done`,
  0x13DD4B, no hook yet): hooked, it splits the Present from the GPU wait.
Some aimed windows sit at exactly 13.34 ms (four in a row), others at 10.2..10.4: when the work is
over a refresh, it lands on the display's 74.96 Hz. The rig cannot show this: its X11 display paces
every frame at about 75 Hz with or without the lens.

**4. The off-centre lens at an aim's first frame** (`lens at (clip) 139 -26`, `154 -31`, against
`0 0` 30 frames later) is the scope's view coming on at the aim's change point (~0.82) while the gun
still moves to the eye: the glass is really there then (seen headless too: `(0.18, -0.10)` at frame
0). With item 1 open, a moving player shows the same offset all the time.

The owner's comment on the match start's orbit camera ("it is just glass you don't see markers",
"nor it is magnifying") is open with the coordinator: nothing done.

## 10. Round 4 (exe `pip-scopes-4`)

Seen headless with `resources.db.patch_74` and `_75` mounted (the rig: `capture.py --hold ... --move-ms`,
`srp-map-sur --move`: from then on the place strafes left and right by turns, forward with each, 700 ms
a strafe, aiming). Pairs: `private/renders/pip-probe/round4/` (README there).

**1. The lens while moving.** Now placed from the real optic as the frame draws it: the idle model's
own world matrix (a record's +50h) and the drawn scene view's own camera (+3ACh view, +42Ch projection,
read at the frame's `render.begin`). Headless, standing and strafing, the disc stays on the real
glass for every kit, and at an aim's first frames (`aim0`) it is where the glass is. Cause of round
3's offset, by elimination: the aimed glass's bound sphere is not late against its matrix (its
model-space centre computed each 30 frames stayed constant while strafing); round 3 took the frame's
camera from the game's thread (`FRAME_CAMERA`, at `game.draw_scene`), which LIKELY runs a frame
ahead of the render thread in the owner's game (the render thread never idles, the game thread's
interval equals it). Headless the game thread does not run ahead (the diagnostic line `the drawn
camera is ... from the one the game's thread queued last` reads 0), so this is not PROVEN; the next
play log's line says.

**2. The disc is the real rear glass.** Its round from the meshes (section 3). Round 3's disc was the
aimed model's bound sphere: a flat round's sphere is its box's, the radius times the root of 2 (41 %
too large: it covered the eyepiece's inner ring). The frame's depth cannot hide what is in front of
the glass: `$user$position` reads about 26-28 mm for every first-person surface whatever its distance
(LIKELY: the first-person models are drawn with a projection of their own depth, 0x13E010 under the
byte 0xA71BC6, near and far from the stage renderer's +A0h/+A4h), so the disc has no depth test. Per kit (the idle model's rear
glass against the aimed glass in the optic's space, mm; the lens at full aim, its radius in half
heights and its distance from the eye):

```
optic (kits)                          rear glass r, z   aimed glass r, z   lens     distance
Leupold 3-7 (AK-74M)                  19, -116          18, -121           0.325    94
PSO-1 (SVD)                           11, -172          13, -230           0.145   125
PSO-1 old (Vityaz)                    16, -151          16, -200           0.200   129
1P69 Hyperon (Rem 700)                17, -168          17, -166           0.309    89
Leupold Mk4 (TOZ-122, Vepr)           14, -128          14, -130           0.378    60
ACOG TA31 / TA01 (AK-74U, SV-98, Uzi, Winchester)
                                      11, -101          10, -101           0.296    59
ELCAN SpecterDR (AKMN, SKS)           16,  -76          15,  -75           0.400    66
PU 3.5x (Mosin)                       15, -108          14, -107           0.194   125
aperture (RPD, Kiparis)               15,  -35          15,  -35           0.225   107
```

The content's limit: the two PSO-1s' third-person models have their rear glass 50 to 58 mm deeper
than the first-person eyepiece the aim was made for, at the bottom of a long rubber eyecup, so their
lens is small (0.145 and 0.2 half heights against today's full screen); the eyecups and the
eyepieces of the third-person models are single-sided: while strafing (the eye off the axis) their
insides show the world through back faces, light shards around the PSO-1's lens (`move1`, `move2`).
The aperture sight's glass is its own (radius 15 at 107 mm). Every other optic's rear glass is its
eyepiece's opening within 5 mm of the aimed one.

**3. Brightness.** (a) The centre of the picture (60 px square, standing, the same moment) is 0.71 to
0.87 times as bright as today's scope's (mean grey 99 to 109 against 120 to 147, every kit; `lum.sh`
in the rig folder): the second view adapts its own eye to its own picture and lands darker than the
frame's eye does on today's zoomed view. Not fixed: the picture should take the frame's exposure
(the second view's draw given the frame window's adapted luminance; one try in round 1 came out
plainly off). The
dim green-grey picture was LIKELY the see-through glass of patch 75 (2): drawn in the second view
from the list +494h (not filtered in round 3), over the whole picture. (b) the optic in view is drawn
from the same forward-lit first-person list as at the hip (the scan of the lists, section 6): its
rear face is lit as any surface facing away from a lamp above and ahead.

**4. Circle in circle.** The second view's lists holding the eyes' models were found by a scan of
the drawn scene view (`SKHID_PIP_LOG`): the idle optic's glass (patch 75, `forward_lighting_no_distort`)
is on +494h, a vector beside the G-buffer's (stage_visibility fills it, 0x1B792A;
accumulate_motion_vectors reads both, 0x1B1CE7), which round 3 did not filter. Now filtered at
`gbuffer.models` with the G-buffer list, before any stage draws: in the second view the eyes'
first-person surfaces are on no list but the aimed model's reticle; in the frame the aimed model is
on none. The real glass does not tint the picture: the disc is drawn after the frame.

**5. Cost.** The picture is now the smallest of 512, 768 and 1024 at least as wide as the lens: at
1440p 512 for the Leupold 3-7, the Hyperon, the ACOGs, the PU, the apertures and the PSO-1s, 768 for
the Leupold Mk4 and the ELCAN; never 1024 at 1440p (round 3 drew every lens from the 1024 one, its
first lens guess 648 px). The timing lines split the Present (its call to its return) from
r_gpu_sync's wait after it (`render.present_done`). Headless (1080p, the rig's 60 fps cap and display
pacing, a GPU shared with other jobs, 240 frames a line, aimed, 512): the second view 0.8 to 1.4 ms
of the render thread and 0.45 to 2.3 ms of GPU; the GPU wait after the Present 0.15 to 0.5 ms. At
960 x 540 uncapped the rig's display still paces every frame (Present 10 to 12 ms), so no interval
is measured here. The owner's round-3 log: the frame was GPU-bound while aimed (main GPU 6.8 to 7.5
ms plus the second view's 2.8 to 3.5 ms at 1024 against an interval of 10.2 ms; the frame's CPU grew
with them, LIKELY DXVK's queue blocking its submissions): with 512 his second view should cost about
1.5 ms of GPU (his 512 window of round 3: 0.75 to 1.5). The frame's own GPU growth when aimed (5.9
to 6.8 ms) is not found: LIKELY the real gun, optic and hands now large on the screen in the
forward-lit first-person pass (today's swap draws only the eyepiece). The next play log says.

Polish from the reference (`stalker-dlss-fg-bodycam-pip`, ideas only): not added this round. Fitting
and cheap, for a next round: the exit-pupil shadow (a dark crescent that closes in when the eye is
off the optic's axis; here the eye's offset is known each frame from the glass's place), a softer
field stop at the rim tied to the magnification, and the picture's exposure taken from the frame's.
Left out: their objective-mounted camera and off-axis projection (ours stays the eyes' aim, where
shots go), their near-plane fitting (the second view leaves the first-person models out), their
stamping of TAA and distortion masks (the disc is drawn after the frame's post-process).

**6. Open: the frame's motion blur smears the eyes' weapon while strafing with the lens** (found after
`pip-scopes-4` was built; the round-4 `move1`/`move2` pairs show it: the optic's ring pale and
translucent, the handguard streaked or blocky). The patch's, not the rig's: the same strafe with the
option off and iron sights (`irons-ak74m`, the gun in view) keeps the gun dark and sharp; the lens with
`r_motion_blur_quality 0` (`noblur-leupold`) keeps the gun and ring dark and sharp too. So the
first-person surfaces get wrong motion vectors in the frame while the lens is on (standing they are
right: no motion). Not found which state: candidates are a per-model previous matrix rolled by the
second view's draw as well as the frame's (two renderer::render calls a frame), or the first-person
projection the motion pass uses (0x13E010 copies a projection at +4F58h of the renderer object,
perhaps the zoomed one, against the unzoomed one the lens gives the frame). Next: trace
accumulate_motion_vectors (0x1B1B50) for the first-person lists (+84ACh, +494h) and what previous
matrix and projection it reads; a stopgap is the frame's motion blur off while the lens is drawn
(opt+E4h, not traced how the post-process reads it).

## 11. Round 5 (exes `pip-scopes-5`, `pip-scopes-6`)

The owner's play of `pip-scopes-4` (2026-11-23 14:37, RPD + aperture, an ACOG, another map): still
right; moving, a pale doubled copy of the optic's housing offset by a fifth of its width (his ACOG
crop `14-41-19`), the whole world ghosted while walking (`14-38-40`), the picture in the disc dark grey
(the same shot), far things in the picture flickering. `r_motion_blur_quality 0` took the ghost away.

**1. The smear and the ghost: one post-process stage for both draws (PROVEN, fixed).** The renderer
makes one stage_postprocess (0x1AFD40, into renderer+150h's +5Ch) and every draw runs it. Its +10h is
the previous view: the end of execute copies the drawn view there (0x1B34ED); the motion pass
(accumulate_motion_vectors 0x1B1B50, per surface 0x1B18B0) multiplies each surface's last world matrix,
kept in a map of the same stage (+25374h, keyed by the record, updated with this draw's), by it; the
motion blur passes it too (0x1B2ECF, 0x1B3003). With the lens the second view drew just before the
frame, so the frame's "last frame" camera was this frame's eye: the first-person models, which move
with the camera, got the camera's whole motion (the smear, the doubled housing), the world the
difference between the eye and the bobbing camera (the ghosted world), and the second view had the
frame's camera as its last one (its picture's vectors wrong by the zoom). The second view's motion pass
also wrote this frame's world matrices into the map, so a moving world model looked still to the frame.
Now (`postprocess.start` 0x1B2270, `motion.previous_world` 0x1B19C3): each draw has its own previous
view (the frame's put back, the second view's kept from its draw a frame before, none after a frame
without it: then the frame's), and the second view's motion pass leaves the world's map entries to the
frame (its own first-person ones, the lens's reticle, it keeps). Rig, AK-74M + Leupold 3-7 strafing,
blur on: the gun and ring sharp, the world blurred as for a strafe.

**2. The time step (PROVEN, fixed).** renderer::render gives each draw the time since the draw before
it (renderer+2E8h - +2E4h, 0x13D1CD; stored at the context's +413Ch, 0x13D26B; `last_frame_time` set at
its end, 0x13DE5F). With the second view the frame's own draw got the few ms after the second view's,
and the motion blur's constants (0x1B3035, 0x1B30CB, 0x1B3163) and the eye adaptation (0x1B142D) read
it. Each draw's post-process now takes the whole frame's: its own plus the other draw's before it (the
lights, particles and rain keep their split, which adds up to the frame).

**3. The dark picture while moving.** Reproduced in the round-4 rig (TOZ-122 + Mk4, AK-74U + ACOG and
others: the sunlit wall grey in the lens while today's scope shows it bright at the same moment). With
1 fixed the far wall is right (AK-74U + ACOG strafing: the lens 0.8 of today's scope's brightness, as
standing, section 10, 3a). A near sunlit pillar (1 to 2 m) still came out grey and unlit in the lens
while the frame showed it in the sun (`pip-scopes-5`, TOZ-122 + Mk4, `move1c`, `move2`); with the second
view fitting its own sun cascades it was sunlit (one strafe each, not the same moments: LIKELY, not
PROVEN). So the second view's draw keeps its own cascades again (`pip-scopes-6`): the saving of section
5 (reading the frame's last ones) was not sound for near surfaces while moving. Its cost is section 5's
(about 1.5 to 2.5 ms of GPU at 512 there); a cheaper sound form (fewer cascades for the narrow view:
stage_shadow_direct::execute takes its first cascade as 4 - opt+C4h, 0x172EF4) is not tried.

**4. The game thread runs ahead in his game (PROVEN).** His log: `the drawn camera is (mm) from the one
the game's thread queued last: 0, 6, 15, 26, 0` while moving: round 3's offset of the disc was the
camera taken from the game's thread (section 10, 1).

**5. The picture size's hysteresis.** Each picture (512, 768, 1024) is its own output window with its
own anti-aliasing history; a lens near a size, swaying across it, drew from two stale histories by
turns. Now a smaller picture is taken only once the lens is under 0.85 of it. The flicker of far things
he saw is not seen in the rig (its place looks at a wall); its LIKELY causes were the second view's
TAA reprojecting with the frame's camera (1) and this.

**6. His 75 Hz windows are not the lens's.** His log of `pip-scopes-4`: the 13.3 ms windows with the
Present blocking 5 to 7 ms come and go in the same session, and one of them has no second view drawn at
all (`second cpu 0`, interval 13335, present 6236): the display path (the compositor or the swap
chain's present mode) paces those, whatever the lens does. Unpaced, aimed costs 7.1 to 7.7 ms a frame
against 5.5 unaimed: the frame's GPU 6.2 against 5.3 (the real gun, optic and hands drawn), its render
thread 5.0 against 3.1, the second view 1.7 to 2.0 ms of CPU and 0.7 of GPU at 512.

**7. The rig's numbers with `pip-scopes-6`** (960 x 540, uncapped, AK-74M + Leupold 3-7, the rig's
display still paces every frame, so its interval of 12 to 22 ms and the Present's 10 to 12 ms are the
rig's; a CPU and GPU shared with other jobs; per frame, windows of 240): unaimed and today's scope
(`SKHID_PIP_PROBE=time`) the frame's CPU 2.4 to 3.7 ms, GPU 2.2 to 4.1; with the lens aimed the frame's
CPU 3.6 to 4.9, GPU 2.0 to 2.5, the second view's CPU 2.4 to 4.1 and GPU 0.5 to 1.1 (512, its own sun
cascades: its CPU about 1.5 to 2.5 ms over round 4's without them). The baseline did not hang this time.
Pairs: `private/renders/pip-probe/round5/` (README there).

**Next (handoff).**
- The second view's own cascades cost render-thread time; the owner's aimed frame is CPU-heavy (his
  `pip-scopes-4` log: frame 5.0 + second view 1.7 to 2.0 ms of CPU): try fewer cascades for it, and
  confirm in his next log.
- The exposure (section 10, 3a). The eye adaptation (0x1B1400) is per output window: the previous
  adapted luminance at the window targets' +257Ch (or, on a reset, the frame's own measure +275Ch), the
  adapted one written to +2618h (+261Ch beside it); adapted = lerp(previous, current, dt / T), dt the
  context's +413Ch read at 0x1B142D. A sound way to give the second view the frame's exposure: in its
  draw, at the eye adaptation, its previous adapted luminance the frame window's (the targets' +257Ch
  pointer swapped for the call and put back, the references as the engine counts them) and its dt 0
  for that call only (set at `postprocess.start`, the whole step put back before the motion blur, which
  divides by it); then its tone mapping uses the frame's adapted luminance exactly. Read why round 1's
  copy came out wrong (its log) before.
- What is in front of the glass (section 10, item 3 of the brief): not started.
- Polish (exit-pupil shadow, a softer field stop): not started.
- The light shards around the ACOG's and Leupold's lens while strafing (round 5's `move2`s), as the
  PSO-1's of round 4: LIKELY the see-through glass of patch 75 or single-sided eyecups, content.

## 12. Round 6 (exes `pip-scopes-7`, `pip-scopes-8`)

**1. The picture exposed as the frame (`pip-scopes-7`).** The eye adaptation
(compute_per_pixel_eye_adaptated_luminance 0x1B1400, called by compute_luminance_parameters 0x1B16E3
unless the view's adaptation speed is 0) is per output window: it reads the window's last adapted
luminance (target 59, 1x1, its texture at the targets' +257Ch: window +2694h) and this draw's measure
(target 62, 1x1, the end of a 256 to 1 chain from target 70), adapts by the draw's time step (context
+413Ch, 0x1B142D), writes target 60 and copies it back into 59; the tone mapping reads it. Now, in the
second view's draw only (`eye.adapt` 0x1B1400 to `eye.adapted` 0x1B163C), its window's +2694h is the
frame window's texture and the time step 0, both put back at the end (the motion blur divides by the
step): the second view adapts to exactly the frame's last luminance, so the picture is as bright as
the scene around it, wherever the narrow view looks (indoors under a lamp, the owner's
`2026-11-23-12-21-55.png`, it was the dim view's own exposure).
- Why round 1's copy came out off (commit df7e6fc06): it copied the frame's target 59 into ours at the
  draw's start (sound) but set the time step to 0 for the whole draw at renderer::render (0x13D26B), so
  the motion blur, which divides by it (0x1B3035, 0x1B30CB, 0x1B3163), ran on 0, and the frame's own
  draw had the few ms since ours as its step (round 5's fix 2 came later). Its picture was not the
  frame's exposure but a broken blur.
- Rig, AK-74M + Leupold 3-7, Fire Station day (60 px at the centre, mean grey): standing 103 against
  `pip-scopes-6`'s 92 (and today's scope's 120 to 147 in round 4, which adapts to the zoomed view
  alone, not to the scene around it). A view of its own still adapts when the adaptation speed is 0
  (instant): then each window takes its own measure, as the game does.

**2. What is in front of the glass (`pip-scopes-7`).** The disc is tested against the frame's depth.
- The first-person models are drawn (renderer::foreground_begin 0x13E010, under the byte 0xA71BC6)
  with the frame's projection but their own near and far (the renderer's +A0h, +A4h: 0.03 m and
  far) and a viewport of depth 0 to the renderer's +A8h (0.12): their depth is
  0.12 x (f/(f-n)) (1 - n/z), in front of every world depth. That is why `$user$position`, rebuilt
  with the world's projection, read about 27 mm (the near plane) for every first-person surface.
- The frame's depth is the render output's (backend +29D4h, R24G8, the back buffer's size), whole
  until the post-process and cleared before the interface: the lens copies it at the frame's
  `postprocess.start` (CopyResource into a texture of ours, made once a size) and its pixel shader
  reads the copy (R24_UNORM_X8) and drops a pixel whose frame depth is nearer than the glass's, taken
  2 mm toward the eye (the optic's glass writes depth there: `SKHID_PIP_LOG=depth` shows first-person
  depth over all of the glass but a small hole). The eye's rim, an eyecup's lip or a turret in
  front of the rear glass now cover the picture. Seen headless: the disc drawn 1.6 times too large
  is cut exactly to the eyepiece's opening by the housing (`SKHID_PIP_LOG=depth` draws the two
  depths instead of the picture: red the frame's, green the glass's). A depth not copied, or of another size or format: the
  lens drawn over everything, one log line.
- The light shards (round 4's PSO-1, round 5's ACOG and Leupolds while strafing): not seen in this
  round's rig (patch 76, the dark coated see-through glass, mounted; PSO-1, ACOG, Leupold 3-7 and Mk4
  strafing). LIKELY the round-4 ones were the motion blur smear fixed in round 5, over the eyecup's
  single-sided inside; not PROVEN, as the moments of two runs differ.

**The rig.** `srp-map-sur --hold ... --turn <yaw>[:<pitch>]` (`capture.py --turn`): the place turns
once, a second after it enters (whole degrees, right and up positive), so its view can look across
the map instead of at a wall.
