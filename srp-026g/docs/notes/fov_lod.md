# Models at a high field of view (patch 40)

Why models went out at a wide FOV, and the fix. Code: `fov_projection`, `fov_lod_scale`
and `fov_screen_scale` in `client-patches/late_join_026g.asm`, `FOV_LOD_*` in
`client-patches/patch_026g.py`, README patch 40, `client_patch.md` section 3.37; the
check: `client-patches/check_fov_lod_026g.py`.

- **Report:** "with a high FOV some models disappear" (patch 22's setting, `uro_fov`
  50 to 100, vertical degrees).
- **Not the frustum:** every culling frustum (models, portals, grass, particles, lights,
  HiZ occlusion) is built from the camera's real view-projection
  (renderer_context +5098h), so nothing is cut at the screen's edges.
- **The cause:** the game measures a model by its size on the screen, through the
  projection, and the thresholds were tuned at 65 degrees (the stock game is fixed at
  65 and a zoom only narrows it):
  - `static_render_model_instance::select_lod` (0x16EEC0), a static model with
    `lod_switching.type` 1: half the larger NDC extent of its box; below `param2` it is
    LOD 3, drawn not at all (0x16F46A). 281 static models with LODs are of this type;
    20 have a `param2` you can see (0.01 to 0.2 of the screen):
    - Rudnya: church interior details (0.2), the Kamaz box (0.025), the UAZ-452 (0.01);
    - Cologne: the floor-vegetation corners (0.15), the hoist (0.1);
    - Mamayev Kurgan: base_a_mod_ (0.15), base_b (0.07);
    - School, Vostok and Vector Laboratory: Kamaz trucks (0.025), UAZs (0.01),
      the flagpole (0.01), ladders.
  - **London** (level_09, skhid's patch 14; seen by the owner, 2026-10-20: models about a
    third of the screen lost their floors, bars and walls at a wide FOV, and patch 40 fixed
    it): 110 of its models are type 1 with `param0` 0.2 to 0.9 (mostly 0.4) and `param2`
    0.001. What happens there is the LOD0 -> LOD1 switch at `param0`, not LOD 3: a box under
    40 % of the screen at 65 is under it sooner at 100 (x0.53: boxes of 21 to 40 % of the 100
    degree screen), and London's LOD1 meshes leave out whole surfaces (the Gherkin's parking:
    11 surfaces -> 5, its bars, lamps and doors gone; building 10's indoor pieces: 16 -> 3,
    11 -> 2, 13 -> 6). Only select_lod's correction fixes it (Unicorn, the game's select_lod
    on 81 London placements, 64 800 views: equal to 65 in every one); the screen factor
    plays no part. The pop at 65 (from further away) is London's content, not the FOV.
  - **LOD 3 "drawn not at all" is doubtful (LIKELY):** `get_surfaces` (0x16F3E6) falls back
    from a LOD while its surface count is 0, and LOD 3's count (byte +3 of
    `model_lods_descriptor`) is never written by the cooker (0x124E50..0x125044) in memory
    dlmalloc does not zero: a model at LOD 3 is usually drawn with LOD 2 or 1. The figures
    above (281 / 20) are of the base db alone.
  - `render_surface_instance::calculate_screen_factor` (0x158D80), every surface: the
    share of the screen times 100. It picks the shader LOD and drops small casters from
    the sun's and spot lights' shadows and from rain occlusion.
  - Type 0 (distance, 671 models) does not depend on the angle; nor do characters
    (no LOD culling).
- **How much sooner, without the fix** (the out-distance falls as tan 32.5 / tan(F/2)):

  | uro_fov | k | out at, of the stock distance | a Kamaz side-on, 16:9 |
  |---|---|---|---|
  | 65 or less | 1 | 1 | about 130 m |
  | 80 | 1.317 | 0.76 | about 100 m |
  | 90 | 1.570 | 0.64 | about 85 m |
  | 100 | 1.871 | 0.53 | about 70 m |

- **The fix:** `fov_projection` keeps k = max(1, tan(A/2) / tan(65 z/2)) in the late
  join's state page (+E50h, after the Tab list's +E40h), A being the angle it passes
  and 65 z the stock angle at the same zoom. select_lod's x0.5 (0x16F2A4) and the
  screen factor's x100 (0x1590AF) call two scales that multiply by k after the
  original's constant. A model then changes LOD and goes out at the distance it does
  at 65.
- **Exactly the stock picture at 65 and less:** k is 1.0 there, at every zoom, and
  through a full scope at every setting; x 1.0 changes no bit. Before the first
  projection the page is 0 and the scales leave the original's value.
- **Cost:** at a wide angle the game draws what 65 draws within the same distance, in
  a wider view: more models and shadow casters than the uncompensated build, never more
  per distance than stock. k also applies to the shadow passes' LOD (their own
  matrices), a little more shadow detail there.
- **Who computes it:** each game for its own screen; rendering only. Nothing a match
  computes reads a LOD or a screen factor (collision and hits use the physics shapes), so
  games with and without it play together: no `simulation_changes.tsv` row, no revision
  of its own.
