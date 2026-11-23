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

NUMBERS

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
  Direct3D state it sets put back. Its rim darkens over its outer 8 %. Drawn after the TAA, it has
  no ghost of the last frame's picture while turning (the TAA blends half of the last frame where
  nothing moves, and a lens drawn as a surface of the gun would be such a place).
- **Magnification:** the target in the lens is as large as in today's scoped view; around the lens
  the world is at the eyes' field of view.
- **Others** see nothing different: everything is the eyes' own drawing; the world format and what
  a match computes do not change (no `simulation_changes.tsv` row).

## 4. Not done yet

TODO
