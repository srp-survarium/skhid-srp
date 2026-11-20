# Scopes work: handoff (paused 2026-11-02)

The owner paused the scope work on 2026-11-02 ("can we stop scopes work for now"). This note is
for whoever picks it up. It covers the three open PRs (#516, #954, #958), what they fixed, what
the owner found in the last play build, and how to continue. The specs stay where they are:
`weapon_scopes.md` sections 6 and 7 (on the PR branches) and `live-scope-eye.md` (on #958's
branch). Tags as elsewhere: PROVEN (read in code or data, or seen in a game), LIKELY, OPEN.

## 1. Status

- Three content-only PRs are open and unmerged: #516 (rifle scopes from 0.69d0, `rifle-scopes@5`),
  #954 (scope fits on the release-41 guns, `scope-fits@3`, stacked on #516), and #958 (live scope
  variants' reticle at the screen's centre, `live-scope-eye@1`).
- None is on main. Revision 42 is live. The next content build is `release/build-content 43`.
- All three are 357 commits behind main (merge base 6ce96048c). Main since then has the stat tables
  (`weapon-stats.tsv` and the rest) and `dead-keys-1.json`. A trial merge conflicts only in
  `release/content.tsv`.
- #954 also depends on the three weapon batch PRs (#949, #948, #947), which are still open. Its
  pack was built on weapons-031@7, weapons-046@8 and weapons-069@10. The batches are now at 031@7,
  046@10 and 069@11, so the pack must be checked against them again (section 7).
- Fixed and pushed: the black scope metal (four causes), the scope icons, the untextured
  TA01/aperture, a build check for missing textures, a reticle-on-eye check, the release-41 guns'
  flip-up sights folded and fixed posts cut, and the line-of-sight check `occlusion.py`.
- Open (the owner, play43, 2026-11-02): (1) iron sights stay up under optical scopes on many guns,
  live guns included; (2) aim-in with any scope, stock included, first closes on the iron-sight
  eye, then jumps to the scope view with a black ring; (3) the Glock 17 Attachments-window crash
  was never bisected.
- Nothing of #516@2..@5 or #954@2..@3 was checked in a headless game. The owner's own play builds
  (play41b, play41c, play43) are the only in-game checks after @1.
- No uncommitted or unpushed work in any scope worktree (section 2.4).

## 2. PRs, branches, packs, worktrees

### 2.1 PRs

| PR | branch (remote) | head | base | pack | edit list | spec |
|---|---|---|---|---|---|---|
| #516 | `feat/rifle-scopes` | 54fdedf3d | main | `rifle-scopes@5` | `rifle-scopes-1.json` | `weapon_scopes.md` section 6 |
| #954 | `feat/scope-fits-new-guns` | 2593c5376 | `feat/rifle-scopes` | `scope-fits@3` | `scope-fits-1.json` | `weapon_scopes.md` section 7 |
| #958 | `fix/live-scope-eye` | a098f760d | main | `live-scope-eye@1` | `live-scope-eye-1.json` | `live-scope-eye.md` |

Pack sha256 (pack.json), as in each branch's `release/content.tsv` row:

- `rifle-scopes@5`: 7fb462750bab59817f9de8a0aa3630935b50c40a1de9a3f6f3dd8269df737145
- `scope-fits@3`: 2a688ddd1541663f4e1223557fc97a76bd8fff0389ba58b039a38860efaf532e
- `live-scope-eye@1`: bc23455733e74e113ba79a0393a4f0a2142c35f1a302deb9ff880d11dc037fa1

Older PRs on scopes, all merged: #153 (scope shop, buy once and put on any number of weapons),
#155 (patch 32, the weapon in the upgrade window), #168 (release of #153/#155/#159), #217 (Optics in
a weapon's right-click menu), #895 (short slot label, stock scope names), #1004 (double click puts
a scope in its slot), #875 (bots: the scopes' zoom in the aim error). Related open PRs: #973 (camo
bake of up to 16 surfaces, patch 44), #420 (weapon kit), #953 (skins of live guns).

resource-porter:

- #35 `fix/convert-069d-materials` a32d941: the per-pixel texture conversion for 0.69d0 guns.
  Merged into `feat/weapons-069d` (resource-porter #24, still open), not into the default branch.
- #13 `feat/compose-model` 6c56b34 (open, base v0.10b): `vostok-port compose-model`. The scope
  packs need it to be made (not to build content). The binary used:
  `/home/sheep/Projects/survarium/resource-porter-compose/target/release/vostok-port`.

### 2.2 Commits

#516 (no merges): b70be8905 (@1), 54d62a661 (texts in the localization TSVs), 8f9b2ef69 (@2),
18a6e2428 (@3), 3c1b4582a (@4), 54fdedf3d (@5). Merges of main: 91baa6e72, 8a0665dc1.

#954 (own commits): 029eea748, 1d8781940, 579387925, bca2bf028, 4192a7bea, 3a5fa1fa3, 0fd57ced9,
20a51085a (@2), 12677b814 (@3, sights and occlusion), 2fc740f62 (@3 on weapons-069@10), 2593c5376
(@3 rebuilt with rifle-scopes@5). Merges of #516: 3e9618f55, 9bc61ee10, 69de8da05, 4a14e0292
(`feat/rifle-scopes` 54fdedf3d).

#958: 6cab07cd1, a28030ead, 5091781fd, 9493b2ebc, a098f760d.

### 2.3 Packs on disk

In the main checkout, `private/content-packs/` (not in git):

| folder | pack.json sha256 (first 16) | note |
|---|---|---|
| rifle-scopes-1 | 20058f3dbc0546db | @1 |
| rifle-scopes-3 | cfb8222d86714678 | @3 (no @2 folder is left) |
| rifle-scopes-4 | daa7d4ab0f6bcd59 | @4 |
| rifle-scopes-5 | 7fb462750bab5981 | current |
| scope-fits-1 | cc752fb902101d31 | @1 |
| scope-fits-2 | e1c50a066f31759a | @2 |
| scope-fits-3 | 2a688ddd1541663f | current |
| live-scope-eye-1 | bc23455733e74e11 | current |

What they ship:

- `rifle-scopes@5`: 191 scope item configs (`gameplay/items/scopes/`), the optics' models under
  `models/weapons/attach/` and 139 weapon model copies with a moved eye (`models/weapons/<gun>/...`),
  48 textures under `textures/weapons/modules/`, `icons/`. About 140 MB.
- `scope-fits@3`: 213 scope item configs, models, and the gun model copies (folded or cut sights,
  moved or levelled eyes). About 147 MB.
- `live-scope-eye@1`: 150 files under `models/weapons/attach/fit6/`, no configs (the edit list
  points the 15 variants at them). About 1 MB.
- No pack ships a weapon config. Weapon configs are changed only by `scope-fits-1.json` (the
  `addons { mount: 1 }` placeholder) and by the `items_compatibility` rows of all three edit lists.

### 2.4 Worktrees and unpushed work

Checked 2026-11-02 (`git status`, local head against `git ls-remote origin`):

| worktree | branch | head | state |
|---|---|---|---|
| `skhid-srp-wt/rifle-scopes` | `feat/rifle-scopes-41` | 54fdedf3d | clean; equals `origin/feat/rifle-scopes` (push with `git push origin feat/rifle-scopes-41:feat/rifle-scopes`) |
| `skhid-srp-wt/scope-fits` | `feat/scope-fits-new-guns` | 2593c5376 | clean; equals the remote |
| `skhid-srp-wt/live-scope-eye` | `fix/live-scope-eye` | a098f760d | clean; equals the remote |
| `skhid-srp-wt/combo-a` | detached | f5b0e113f | clean; throwaway test tree (main + #516@4 + #954@2 + the batches + #958, then a "test manifest" commit by `tmp`); disposable |
| `skhid-srp-wt/combo-b` | detached | 28fb9d5c5 | clean; main + #516@3 + the batches + #958, without #954 (made for the Glock bisect, never run); disposable |
| `skhid-srp-test40d` | `local/play43` | 830cf10b5 | clean; local only by design (106 commits on top of main, never pushed); the owner's play build |

Stale local branches in the main checkout: `feat/rifle-scopes` at 54d62a661 (older than the remote;
the live branch is `feat/rifle-scopes-41`), `feat/scope-context-menu` a09209cf8 (1 ahead, 1766
behind main; superseded by #217).

All paths are under `/home/sheep/Projects/survarium/`. The fetch refspec of this repo is main only:
`origin/<branch>` refs go stale; fetch a branch by name (`git fetch origin <branch>`) and use
`FETCH_HEAD`, or `git ls-remote`.

### 2.5 The play builds the owner tested

- play41b: main + all weapon PRs + #516@3 + #954 at 9bc61ee10 (final weapon packs 031@7, 046@6,
  069@9). Owner screenshot `~/screenshots/2026-11-01-19-08-38.png`.
- play41c: #516@4 (3c1b4582a) + #954@2 (20a51085a). Screenshots `2026-11-01-20-13-00.png`,
  `2026-11-01-20-18-53.png`, `2026-11-01-20-20-00.png`, `2026-11-01-20-20-22.png`.
- play43: `local/play43` in `skhid-srp-test40d`: main + #949, #948, #947 + #516@5 (20aa935ff) +
  #954@3 (982705957) + #958 (043ad204b) + others. Content in `/mnt/hdd/Projects/play43-content`
  (`resources.db.patch_43`, `server-game-data.db`). Screenshot `2026-11-02-15-03-57.png`.
- play43's merge notes (commits 40032d577, 6474bf495, 67e5c617f, 830cf10b5) are the catch-up
  recipe of section 7.

## 3. How scopes work in 0.26g

### 3.1 Items, variants, configs

- A weapon names its scope in `addons.rifle_scope_dict_id`; `weapon_cook::on_weapon_config_loaded`
  0x24A920 reads it (0x24AA0C..0x24AA33) and `rifle_scope_cook` (`on_config_loaded` 0x258580,
  `on_subresources_loaded` 0x258740) builds the scope from the item's `data` block (PROVEN,
  `weapon_scopes.md` section 2).
- A bought scope reaches a weapon through an upgrade record (slot 3) whose dict id is a **variant**:
  a category-22 item made for one weapon. `item_config_cook::translate_query` 0x2F1750 applies the
  variant's `parameters.modification`. Its modifiers (seen in `fitn_acog_ta01_fn_minimi`):
  - type 3 add `[rifle_scope_dict_id, <variant dict id>]` to `addons` (a weapon without a stock
    scope; it needs a non-empty `addons`, hence the `addons { mount: 1 }` placeholder), or type 0
    set of `addons.rifle_scope_dict_id` (a weapon with a stock scope);
  - type 0 set of `object.model` to the variant's weapon model (a copy with a moved eye or changed
    sights), where one is needed.
  - The modification id is 990000 + the variant's dict id (992987, 993070, ...).
- `data`: `fov_factor` (1 / zoom), `hide_weapon_on_aim`, `idle_model`, `aimed_model`, `type` 5,
  `near_plane_factor`, `change_scope_factor`. Stock values: optical scopes (`scope_pso1`,
  `loc_acog_ak74n`, patch 19's `fit_*`) `hide_weapon_on_aim` true, `change_scope_factor` 0.75 (PSO-1)
  or 0.82, `fov_factor` 0.25; collimators (`eotech_552`, `walther_comp2`) `hide_weapon_on_aim`
  false, `fov_factor` 0.8, the same model as idle and aimed.
- The server reads the pairs from the game data: block `srp_scope_fit { scope, weapons[] }` in each
  variant (`crates/survarium/src/v026g/scopes.rs`, `weapon_scopes.md` section 3). Nothing about
  scopes is in code tables.
- A scope is view-only: no shooting code reads it, bullets go to the screen centre (PROVEN). No
  scope change needs a `next` row in `simulation_changes.tsv`.

Dict ids: patch 19 variants 2001-2084, shop scopes 2101-2112; #516 shop scopes 2113-2119 and
variants 2701-2884; #954 the Walther Competition II 2120 and variants 2901 on (212 made, claimed to
3299). Names: `shop_scope_<optic>` (shop items), `fit_<gun>_<optic>` (patch 19), `fitn_<optic>_<gun>`
(#516 and #954), `loc_*` (stock).

### 3.2 The eye, the aim locator, scope_point

- The weapon draws the scope's idle model at its model's `scope_point` locator, turned 180 degrees
  about Y (x and z mirrored) (PROVEN: patch 19's AK-12 and M4A1 variants sit exactly where 0.69d0's
  locators put the module).
- The aimed view is seen from the weapon model's `aim` locator. A turned `aim` locator tilts the
  view: the AK family's 0.004 rad about x put the TA01's reticle 10 px and the Leupold 3-7's 16 px
  above the centre at 1280x720 (PROVEN in game, 2026-11-01). That is why #516@2 and #954 set models
  whose `aim` is unturned (`<model>_n_scope`, `<model>_n_optic`) and #958 re-places 15 patch-19
  aimed models on the tilted line.
- Collimators (the weapon stays visible): the variant sets a weapon model whose `aim` is on the
  optic's sight line, an existing one within 0.3 mm or a copy with only `aim` moved
  (`<model>_n_<optic>`).
- Optical scopes (`hide_weapon_on_aim`): the variant keeps the gun's own eye, which is its
  iron-sight eye. The aimed model (the eyepiece) is placed in front of that eye as it sits in front
  of the reference variant's eye. The eyepiece is therefore not where the idle scope is on the rail.
  This is the root of open problem 5.2.
- The swap from idle to aimed model (0.10b source `game/sources/weapon.cpp`,
  `weapon::on_skeleton_matrices_changed`; 0.26g `survarium::weapon::add_aimed_rifle_scope_to_scene`
  0x244B60 and `remove_aimed_rifle_scope_from_scene` 0x244C50):
  - `instant_aim_start` (0x243980) starts the FOV transition to the scope's `fov_factor`, 0.3 s in
    0.10b (`s_aim_transition_time`).
  - Each frame, once `(1 - fov) / (1 - scope_fov) >= change_scope_factor`, the idle model is removed,
    the aimed model is added at `scope_point`, and, with `hide_weapon_on_aim`, the weapon model is
    removed from the scene and the user's model visibility changed (`set_model_visible(..., 1, 2)`).
  - Aim end reverses it at the same threshold.

### 3.3 The reticle

- 0.26g draws the aimed model alone. 0.69d0 draws a collimator's reticle as its own model.
- Collimators: patch 21's `collimator_glowing_dot` (an emissive disk with a view-angle falloff) is
  baked into each new collimator's aimed model as a surface, at the optic's eye point 0.6 m along
  the sight line (`vostok-port compose-model`). The XPS2 gets the dot, not 0.69d0's ring.
- Scopes: the eyepiece of our scope variant on that weapon (background, lens), with the new scope's
  reticle grid and dot centred where the variant's are.
- Lens, grid and dot materials are ours: `weapons/loc2/scope_lense_1p`, `scope_lense_3p`,
  `scope_aimgrid`, `scope_reddot` (`scripts/rifle-scopes/materials.tsv`).

### 3.4 Materials and texture slots

A 0.26g material has four texture slots in `material/g_stage/effect/`: `texture_diffuse`,
`texture_normal`, `texture_roughness` (holds the `_smap`, a gloss), `texture_fresnel` (the `_fmap`,
Schlick F0). Read from 0.26g's own shaders (disassembled `gbuffer_pass.ps`,
`point_light_accumulator.ps`) and shared assets (PROVEN):

- diffuse: DXT1, sRGB; the game reads sRGB from the **last byte** of the `.dds` (1 sRGB, 0 linear).
  The `.options` beside a texture is not read by the game.
- normal: DXT5, x in alpha, y in green (flipped); the same layout as 0.69d0's (the AK-12, PPSh,
  CompM4 and ACOG maps are identical in both builds).
- `_smap`: red channel = gloss. The g-buffer reads the slot as
  `constant_roughness_min + (max - min) * texture`; the templates have min 0, max 1. GGX width
  `0.5 s^6 + 0.1 s`, `s = 1 - gloss`.
- `_fmap`: red channel = F0.
- Slot mapping from 0.69d0 (`retexture.py`): `t_albedo` -> `texture_diffuse`, `t_normal` ->
  `texture_normal`, `t_smoothness` -> `texture_roughness`, `t_metalness` -> `texture_fresnel`.
- Templates (`materials.tsv`): NcSTAR 4 and XPS2 on `weapons/loc2/eotech_552.material`, PK-01VI and
  RSA-S on `aimpoint_comp_m4.material`, aperture (body and rubber) and Leupold 3-7 on
  `leupold_mk4.material`; the TA01 body is the players' own `weapons/loc2/acog` (same UV layout as
  0.69d0's, identical normal maps).
- A made material is its template with only the four texture names changed. Nothing else of
  0.69d0's material is carried over.

### 3.5 The PBR conversion (0.69d0 metalness workflow -> 0.26g)

Per pixel, then encoded DXT1 again with the source's mips, header and last byte
(`scripts/rifle-scopes/pbr_026.py`; the same in resource-porter `src/weapons_026/donor_069d/convert.rs`
after #35). m = metalness, values 0..1:

- diffuse = 0.76 * albedo * (1 - 0.5 * m) (sRGB values)
- gloss = 0.36 * smoothness
- F0 = 0.08 + 0.09 * sqrt(m)
- normal map unchanged

Fitted on ten texture sets both builds ship with the same layout: A-545, AK-12, AK-74M, AK-74N, AKMN,
ASh-12, PPSh, ACOG, Cobra, CompM4. PPSh check: gloss stock 0.217 against made 0.214; diffuse mean /
contrast / saturation stock 34.5 / 20.9 / 7.0, made 32.2 / 16.4 / 5.4. Rendered mean sRGB stock /
before / after: AK-12 48.3 / 39.8 / 48.1, PPSh 43.7 / 38.6 / 49.3, CompM4 66.0 / 37.6 / 52.8.

Texture names tell the role: `<stem>` albedo, `<stem>_smap` smoothness, `<stem>_fmap` metalness,
`<stem>_nmap` normal (`pbr_026.convert`).

### 3.6 Meshes

0.69d0 packs a static vertex's normal, tangent and bitangent as one float each,
`(x << 16 | y << 8 | z) / 2^24`. 0.26g reads three B8G8R8A8 words (x in the third byte), the
normal's alpha 127, the tangent's and bitangent's 255. `compose()` converts every part
(`scopes_lib.static_026`); on the CompM4 mesh, which both builds ship, the result matches 0.26g's
bytes within 1 (PROVEN). The porter's gun meshes (skinned) were already right.

0.26g clamps a weapon's texture coordinates where 0.69d0 repeats them: the aperture's mesh ran
u to 1.91. `wrap_uv` moves triangles wholly past an edge back by whole textures (#516@3).

### 3.7 Icons and atlas cells

- Shop icons are in `LobbyMenu_AmmoIcons` (the consumables atlas), column 4. Stock scopes rows 1-12;
  #516's seven rows 13-19 (`ui_desc.icon` [row, 4], e.g. the TA01 [18, 4]); the Walther
  Competition II [20, 4] (#954). Atlas cells are claimed in the batches' `claims.tsv` (session
  scratchpad `weapons-rev41/`, temporary).
- Source: 0.69d0's own picture of each optic,
  `flash_movies/textures/ui_icons/modules/module_renderer_<ui_desc.icon_renderer>.png` (the Walther:
  106), extracted by `prepare_donor.sh` to `icons/<module>.png`.
- Framing, measured on the stock icons: 73 px tall from y 8, centred on x 88, the stock soft black
  glow. Contact sheet: `srp-026g/docs/images/rifle-scopes-icons.png` on #516's branch.
- `render_icon.py` (an optic without a picture) shades as 0.26g: the normal map in the vertex
  tangent frame, GGX, Schlick, sRGB, anti-aliased alpha. It raises on a missing material instead of
  drawing grey.
- Texts: names and short descriptions in all ten `client-patches/localization/*.tsv`, which
  `localization-1.json` compiles last. The Walther Competition II reuses the stock Walther's
  `st_walther_competition2_collimator_*`.

### 3.8 The camouflage bake does not reach scopes

Patch 44 bakes only the weapon model's own surfaces: the game's log names
`srpw1_<camo>_<model>_<0..n-1>`, n the weapon model's surface count (the Minimi's 0..6). The scope
is a separate model at `scope_point` and is never painted (#954 comment of 2026-11-01). #973 (up to
16 surfaces) does not change this. The `srpw1_..._fn_minimi_n_optic_*` lines are the copied gun
model's own surfaces. The black-and-white scope in `2026-11-01-19-08-38.png` was the material bug
of section 4.1, not camo.

The log lines `cook of '' [quid n][class 107] failed` are the bake's fresnel queries of surfaces
with no `t_fresnel` (`client_patch.md`, the log table; `baked-textures.md`). They are benign.

### 3.9 The config entry limit

The client rebuilds an upgraded item's config (camouflage, decals, scope) in a 16 KB arena it
never bounds (`create_binary_config_buffer` 0x358AA0; past it, `vostok::configs::free_memory`
0x358A10 calls through overwritten memory). srp refuses game data with an item config over
`tables::MAX_CONFIG_ENTRIES` = 565 entries (`crates/survarium/src/v026g/tables.rs`;
`srp-check-game-data` reports it). The KS-23M of weapons-069@9 (587) crashed every match at load.
`scope-fits-1.json` adds entries to the guns (`addons { mount: 1 }`); a scope record adds one more
at run time. Check the release-41 guns' counts after every edit list. play43's local-only commit
830cf10b5 raises the limit to 566 for #953's Remington 700 "Soot"; that is not for a PR.

## 4. What was fixed (cause -> fix)

### 4.1 Black scope metal (#516@4, @5; #954@2, @3)

Seen: the 0.69d0 optics drew nearly black with light streaks, metal as a mirror
(`2026-11-01-19-08-38.png`, `2026-11-01-20-13-00.png`: the Walther on the SCAR-H, a PK-01VI on the
VHS-2).

1. Meshes (the main cause): the optics' static meshes kept 0.69d0's basis floats (exponent bytes
   0x3A-0x3F where 0.26g expects 127). Fix: `compose()` converts every part
   (`scopes_lib.static_026`). #954 gets it through the merge of #516@5 (the Walther and the rails).
2. Materials: `build-material-patch` had carried `constant_roughness_min/max` 0.2/0.2 over the
   templates' 0/1, so the gloss texture did nothing. Fix: `retexture.py` makes each material as its
   template with only its four textures changed; both `prepare_donor.sh` use it.
3. Textures:
   - @4 applied the curves to the DXT colour endpoints, and blocks whose endpoints the curve made
     equal were left raw: raw metalness as Fresnel in 63 % of the AK-74M's blocks, 38 % of the
     A-545's, 40 % of the aperture's;
   - no Fresnel where metalness is 0 (stock median 21/255);
   - a flat diffuse (`0.514 * (albedo * (1 - 0.8 m)) ^ 0.75`);
   - @4's diffuse lost the `.dds` last byte (the sRGB flag).
   Fix: per pixel, re-encoded, the formulas of section 3.5 (`pbr_026.py`; resource-porter #35 for
   the 0.69d0 guns).
4. Rails (#954@2): their 0.69d0 textures converted the same way; @3 makes their materials from
   templates and their meshes in 0.26g's layout.

### 4.2 Icons (#516@5, #954@3)

Seen: flat black or grey icons with hard borders next to the stock ones (`2026-11-01-20-20-00.png`,
`2026-11-01-20-20-22.png`); the owner: "we don't render icons for scopes consistently". Causes:
diffuse-only shading with a hard alpha edge; the TA01's donor material name not mapped to loc2's
`acog`; the aperture's raw grey metal albedo. Fix: 0.69d0's own pictures framed to the stock spec
(section 3.7); `render_icon.py` rewritten.

### 4.3 TA01 and aperture "untextured" (#516@5)

Grey only in the icon renderer (above). In the game both were textured. Every material of all 184
#516 variants was listed: the seven made materials bind four textures each from the pack (diffuse
DXT1 sRGB byte 1, normal DXT5, gloss and Fresnel DXT1 byte 0); the rails, the TA01 body, eyepiece,
lens, grid and dot materials are the players' own. None was missing.

### 4.4 Build check for textures (#516@5)

`rifle-scopes/make_pack.py` stops when a material of the pack's models names a texture that is
neither shipped nor the players', or is not a DDS 0.26g reads. 88 materials pass; a missing texture
or a dropped sRGB byte fails it.

### 4.5 The aperture's garbled body (#516@2, @3)

@2: made on the Leupold Mk4's template instead of the ELCAN's. @3: `wrap_uv` (u ran to 1.91).

### 4.6 Off-centre reticles (#516@2, #954, #958)

- #516@2: scope variants on weapons with a turned `aim` set the model again with `aim` unturned
  (`<model>_n_scope`).
- #516@2: the NcSTAR 4 on the M4A1 sat under the carry handle: it had taken the stock model, whose
  eye matched. A collimator now never takes a model its mount variant does not set, nor one with a
  turned `aim`.
- #954: a scope gets an unturned-eye copy (`_n_optic`) where the gun's `aim` is turned.
- #958: 15 patch-19 variants (ACOG TA31, PSO-1, ELCAN on the AK-74M, AK-74N and its premium, AKMN and
  its two premiums; ACOG TA31 and ELCAN on the Ash-12; PSO-1 on the Saiga-12 and its premium) get
  their aimed model re-placed on the tilted line of sight (`models/weapons/attach/fit6/<aimed
  model>_<weapon model>`, `replace_string` of `data/aimed_model`). review.py: ACOG and ELCAN +10.9 /
  +9.9 px -> 0.0; PSO-1 chevron +3.1 px above -> its designed 7.9 px below; Saiga 16.6 px left -> 0.2.
  Not changed: the A-91, SR-3M, Val, VSS, Vepr and RPD variants (reticles 1 to 2 cm off `aim` in the
  files, which would be hundreds of px; they must be shown some other way; to be seen in a game
  first). #958 is not checked in a game; the Saiga's turn direction is not seen.
- The owner corrected one reading (2026-11-01): the SCAR-H + Walther in `2026-11-01-19-08-38.png` "is
  aligned"; the screenshot was cropped.

### 4.7 The eye check, `eyes.tsv` (#954@2)

Every variant's reticle (red dot, else grid) must sit in front of its weapon model's eye exactly as
in front of its reference's (within 0.2 mm, `EYE_AXIS`), and that eye must be unturned. All 212
pass. Columns: variant, x, y, reference x, reference y (mm).

### 4.8 Iron sights on the release-41 guns (#954@3)

Seen: the SCAR-H's flip-up sights up round a red dot (`2026-11-01-20-18-53.png`), the VHS-2's front
sight in a PK-01VI (`2026-11-01-20-13-00.png`). Survey `scripts/scope-fits/sights.tsv`, mesh work
`sights.py`:

| gun | sights | under an optic |
|---|---|---|
| SCAR-H | rear and front flip-up, 0.69d0 modules baked whole in `scar_h_sightsg` | folded: 0.69d0's `_off` modules replace the up ones |
| VHS-2 | rear and front flip-up in `vhs_2_sights_s` (the front is 0.31a2's, its post 2.3 mm lower; found by vertex order) | folded |
| Minimi | front post (2 islands, 310 triangles, of `kolya_lp_receiver02sg`); rear on the feed cover (`kolya_lp_receiver01sg`) | front cut; rear **kept** |
| Fort-401 | rear module `scope_fort_401_ironsight_s`; front post and ears in `fort_401_part2_s` | rear left out; post cut (2 islands, 218 triangles); ears lowered (72 vertices) |
| L85A2 | `scope_enfield_l85a2_ironsight_s` (carry handle with the rear sight and front post) | left out |
| MP5, SVU, AKMSU, FAMAS, G3SG/1, KS-23M, PPSh Drum, five pistols | part of the gun's own mesh | **kept** (below every optic's line of sight) |
| Jackhammer | none | - |

`sights.tsv` actions: `keep`, `fold <surface> <up module> <folded>`, `cut <surface>`,
`cut <surface> <box>` (mesh islands wholly inside the box), and lowering.

### 4.9 The line-of-sight check, `occlusion.py` (#954@3)

- Collimator: rays from the variant model's eye in a cone about the red dot (24 directions), out
  to where the optic's own body closes the window.
- Scope: rays along its axis from the eyepiece disk.
- Nothing of the gun model or the rails may be hit inside 0.6 of the window (`CLEAR`).
- make_pack.py runs it on every variant and stops on a release-41 gun's failure; results in
  `occlusion.tsv`. @2 failed 30 (all 15 SCAR-H, 5 VHS-2, 7 Fort-401, 3 Minimi); @3 passes all 207.
  The Walther on five live weapons (M4A1, RPD, TOZ-122, UMP-45, Valmet) is recorded `live`.
- `occlusion.py --live --donor` on the players' variants writes `occlusion-live.tsv`: 34 of 349
  pairs FAIL, 315 pass (section 5.1).

### 4.10 Smaller fixes

- A left-out surface's `material_settings` entry is removed too (3a5fa1fa3): `vostok-port pack
  patch`'s model contract check refused the L85A2 copies.
- The aperture and Leupold 3-7 have one zoom each (0.26g has one zoom a scope): aperture x3, TA01 x4,
  Leupold 3-7 x6 (as 0.46c2). The CompM4 item and its 25 variants go to x2 (`fov_factor` 0.8 -> 0.5,
  `rifle-scopes-1.json`), as 0.46c2 and 0.69d0 have it.
- The crash with `cook of 'fx/weapons/m4a1' [class 58] failed` on opening the SCAR-H's Attachments
  window (`ingame-fits/crash2`, exe+0x352308 = VA 0x362308 in
  `particle_system_instance_cook::on_sub_resources_loaded` 0x362280, inferred) came from
  weapons-031@3, whose SCAR-H and VHS-2 named `fx/weapons/m4a1`. Fixed in batch 1 (@4 and later).

## 5. Open problems

### 5.1 Iron sights up under optical scopes (all guns, live too)

Symptoms (the owner, play43, 2026-11-02): "ironsights still stay with scopes often (though not with
collimators, most likely), on many guns". Screenshot `2026-11-02-15-03-57.png`: the Minimi with the
TA01, hip view: a tall post on top of the scope, and the rear sight drums behind it.

Evidence:
- The Minimi + TA01 is #954's `fitn_acog_ta01_fn_minimi` (dict 3070, weapon model
  `weapons/fn_minimi/fn_minimi_n_optic`). `occlusion.tsv` passes it: a scope's check casts only
  along the scope's axis, and the weapon is hidden when aimed.
- The tall post is LIKELY the TA01 model's own top sight, not the Minimi's front post (cut in
  `fn_minimi_n_optic`): 0.69d0's own TA01 picture (`icons/acog_ta01_4x32_scope.png` of the donor)
  and the render `scratchpad/sights/minimi-ta01.png` show a tall piece on top of the housing. OPEN:
  confirm on the mesh (`models/weapons/modules/scopes/acog_ta01_4x32_scope`) and decide with the
  owner whether it stays (retail has it).
- The drums are the Minimi's rear sight on the feed cover, `keep` in `sights.tsv`.
- Live guns: no live variant drops or folds a sight. Each sets the gun's own model or a copy with
  only its eye moved. `occlusion-live.tsv` FAILs (sight or rail inside the window) 34 of 349 pairs:
  - M4A1 front post (`m4a1_s`): ACOG TA01, ACOG TA31, ELCAN, CompM4, PK-01VI, EOTech 552, XPS2,
    RSA-S, NcSTAR 4;
  - RPD (`rpd_s`): EOTech 552, PK-01VI, RSA-S, NcSTAR 4;
  - UMP-45 (`ump_45_mat_s`): EOTech 552, PK-01VI, RSA-S;
  - Valmet (`valmetrk62_reciever_s`): PK-01VI, RSA-S;
  - TOZ-122 (`lpsg4`): Docter, PK-01VI;
  - IZh-27 and its premiums (`izh_27_lancastersg1`): Docter, PK-01VI;
  - MP7 and the silenced copies (`hk_mp7_*_s`): Docter;
  - SPAS-12, AEK-919K: Docter; Rem 870: CompM4 (rail), Docter; Magnum: DeltaPoint (rail);
  - Vepr and its premium: Leupold 3-7 (a part 1.5 cm in front of the eyepiece).
  - Eight patch-19 CompM4 and Kobra variants (A-91, SR-3M, AS Val, Vepr, RPD, UMP-45, Rem 870) have
    their window shut from the eye by the optic's own body: the check judges nothing there.
- The owner's rule now: under an optical scope the sights are folded or removed, the retail way,
  whether or not they cross the line of sight. Collimators look right to the owner.

What retail 0.69d0 does (`gameplay/game.options`, `weapon_modules.<module>.attach_params`):
- every optic, collimators included (e.g. `belomo_pk01_collimator`, `aimpoint_compm4_collimator`,
  `acog_ta01_4x32_scope`, `aperture_scope`, `leupold_3_7x4x8_scope`), sets these sight slots to
  param 1 (`slots_to_change_param`): `picatinny_ironsight_rear`, `troy_ironsight_front`,
  `hk416a5_ironsight_front`, `hkmp7_ironsight_front`, `scar_ironsight_front`, `ironsight_rear`,
  `vhs2_ironsight_front`, `ironsight_front`, `m4a1_front_ironsight`;
- param 1 draws a sight module's `model_param1`, its folded model (`<name>_off`). Modules with one:
  `m4a1_front_ironsight`, `hk416a5_front/rear_ironsight`, `hkmp7_front/rear_ironsight`,
  `scarh_*`, `vhs2_*`, `enfieldl85a2_*`, `svu_a_*`, `fort401_ironsight`, `glock17_ironsight`,
  `vepr_ironsight`, `ak_12_ironsight`, `troy_micronattlesight_*`;
- a sight module on the optic's own slot is taken off (`slots_to_disable`);
- scopes also hide slots while aiming (`slots_to_hide_while_aiming`: `forend`, `root`, `decor_01`,
  ...) and set `hide_gloves_while_aiming` true.

Proposed approach:
1. Ask the owner whether collimators follow too (retail folds under them as well). Default: optical
   scopes only, as the owner said.
2. #954: change `sights.tsv`'s `keep` rows to fold (where 0.69d0 has a folded module for that sight)
   or cut, for scope variants; the Minimi's rear too. `sights.py` already folds and cuts.
3. Live guns: a new small PR from main (like #958), its own pack and edit list: for each live
   optical-scope variant, set (`object.model`, type 0 in the variant's modification) a copy of the
   gun's model with its sights folded or cut, keeping the eye it already has. Where 0.69d0 has the
   same gun with a folded module (M4A1, HK416, MP7, AK-12, Vepr, SVU), use its `_off` mesh; else cut.
   Re-check `eyes.tsv` and the entry limit.
4. Change the check (`occlusion.py`): for an optical scope, fail when any iron-sight part (front
   post, rear sight) of the gun model is unfolded, whatever the line of sight. It needs a list of each
   gun's sight surfaces or islands; `sights.tsv` is that list for the release-41 guns. Live guns need
   the same survey.
5. Before/after renders of every changed gun, same camera, labelled, and the live list re-run.

### 5.2 Aim-in with scopes: the iron-sight detour, the jump, the black ring

Symptoms (the owner, play43): "when using scopes, the weapons starts to close in on where ironsight
is, and then jumps to scope (which has all black around it)"; "the camera should zoom in on the
scope itself" (relayed by the coordinator, 2026-11-02). It "seems to be a general problem": stock
scopes too.

What is known:
- The swap mechanism of section 3.2: the weapon and the idle scope stay drawn until the FOV
  transition reaches `change_scope_factor` (0.75 or 0.82 of the way), then the aimed model replaces
  them in one frame and, for optical scopes, the weapon is hidden.
- The camera converges on the weapon model's `aim` locator. For optical scopes every variant of ours
  (patch 19, #516, #954) keeps the gun's iron-sight eye there, so during aim-in the iron sights come
  to the eye while the idle scope sits above the line of view. At the threshold the eyepiece appears
  in front of that eye: the jump.
- 0.69d0 (retail) uses the same threshold (`change_scope_factor` 0.82 on every optic) but every optic
  overrides the weapon's `aim` slot with its own `aim` locator (`attach_params.slots_to_override`:
  `id` aim, `locator` aim; scopes add `aim_2` via `slots_to_add`; `aim_locator_names` [aim, aim_2]).
  So retail's aim-in converges on the optic's own eye. 0.69d0 scopes also have `scope_radius` (0.7 on
  the TA01), LIKELY the radius of the view through the scope (the dark surround), and an
  `aimed_model` per module (`<module>_aim`).
- The black ring: the aimed model's non-lens surfaces fill the screen around the lens (stock
  `weapons/attach/sight_11mm/pso_1_aim`: surfaces `pso_1_sg1`, `body_gd_s`, `pso_1_lens_sg1`; our
  eyepieces' tiled background). LIKELY stock behaviour for every 0.26g optical scope; OPEN: compare a
  stock SVD + PSO-1 aim-in in a game.
- Not found yet: the 0.26g code that moves the weapon so its `aim` locator reaches the camera. 0.10b
  has no `"aim"` locator lookup (`weapon.cpp` reads only `barrel_point` and `scope_point`); 0.21d's
  PDB names the aim animations `aimed_stand_hud`, `aimed_crouch_hud` and
  `weapon_user_animations_selector::is_going_to_aim`. Candidates in 0.26g: `survarium::weapon::tick`
  0x244AB0, `survarium::weapon::draw` 0x244E10, `weapon_core::instant_aim_start` 0x2E1B50.

Proposed approach (data first; report before changing anything):
1. Measure first, headless, one stock pair (SVD + PSO-1, `scope_pso1`) and one of ours (HK416 +
   TA01): frames of the aim-in with the eye position, to confirm the detour is stock.
2. Data fix, the retail way: give each optical-scope variant a weapon model whose `aim` is the
   scope's own eye (on the scope's optical axis, at its eye relief), as collimators already have;
   place the aimed model so its eyepiece is where the idle scope's is (then idle and aimed coincide
   at the swap). This changes `eyes.tsv`'s rule for scopes (reticle on the axis of the new eye) and
   every scope variant's model; #958's re-placed aimed models follow the same rule.
3. Then judge the remaining jump: raise `change_scope_factor` toward 1 so the swap happens when the
   eyepiece fills the view, and/or make the aimed models' surround match the idle eyepiece. Keep
   stock values unless the owner wants stock scopes changed too (he said it is general: ask).
4. Only if data cannot do it: an exe change as a Rust client patch (AGENTS.md: the `.srpx` arena,
   hooks by symbol, re-kit rows for every function found). Add the aim-pose function to
   `tools/re-kit/v026g/` when it is identified.
5. Watch `near_plane_factor` (0.5 or 0.6): with the eye at the eyepiece the near plane can clip the
   scope body.

### 5.3 The Glock 17 Attachments-window crash (never bisected)

- Seen once, round 2 (2026-11-01 08:02:37), combo build 955ccffba (main + #516@2 + #954 + batches at
  031@3, 046@3, 069@5 + #958), exe `--revision=41` (sha256 53d1311074f5...): opening the Glock 17's
  Attachments window with no scope on it, right after the MP5 + EOTech 552 was attached.
- ACCESS_VIOLATION, read of 0x77C1E320 at exe+0x3A336C (VA 0x3B336C). Stack exe+0x3A31AB,
  0x3A217A, 0x3A2097, 0x3A2082, 0x3A206E, 0x3A23ED, 0x3A23BE, 0x3A27BC. Nearest re-kit labels:
  `vostok::animation::bone_matrices_computer::convert_to_object_matrices` 0x3B2C90 (inferred-high),
  `n_ary_tree_transition_tree_constructor::*` near 0x3A27BC. Last log line 20 ms before: "lobby
  character's player made in 700 ms". The class-107 lines before it are benign (section 3.8).
- Not the entry-limit crash (that one is in `free_memory` 0x358A10 at match load).
- Dump and game log: `scratchpad/ingame-fits/crash1` (temporary).
- Round 3 was to run combo A (with #954) and combo B (without) on the final weapon packs; the
  in-game worker hit its API limit before the first client. The data folders
  `/mnt/hdd/claude-scratch-archive/combo-{a,b}-41` are gone.
- To do: two builds on current main + the batches (+ #516, #958), one with #954; give a Glock 17
  (dict 2283), open its Attachments window twice in a fresh lobby and once right after attaching
  something to another gun. If B crashes too, it is batch 2's (the Glock's model or animations), not
  scopes'.

### 5.4 Other loose ends

- #958's left-alone variants (A-91, SR-3M, Val, VSS, Vepr, RPD): see them in a game before any
  change. The Saiga's turn direction: not seen.
- The lobby (round 2): the MP5 stands upright on the workbench; the shop name reads "Competition II
  collimator" without "Walther" (the stock Walther's text).
- Round 1 in-game items not yet re-checked after @2..@5: PK-01VI and RSA-S were never bought; the
  Leupold 3-7's eyepiece far back on the AKs; the outside look on the AKs' side rail.
- `review.py` renders are approximate (a skinned gun's vertices mirrored in x and z into the
  locators' frame); they are not in-game proof.

## 6. Tools and checks

Python: numpy and Pillow, run with `-I`. A python env with a gcroot was kept at
`scratchpad/sights/pyenv/bin/python3` (the shared nix env was garbage-collected mid-run once).
ImageMagick 7 (`magick`) must work from the cwd (pbr_026 runs it with relative names; cwd /tmp is
fine). `P=/home/sheep/Projects/survarium/skhid-srp/private/content-packs`.

Inputs:
- 0.69d0 data: `/mnt/hdd/Projects/weapon-survey-20261016/work/full069.split` (split list), porter
  `/mnt/hdd/Projects/weapon-survey-20261016/work/porter`, its `target/release/vostok-port` (reads
  split 16-byte archives). Retail files and exe: `/mnt/hdd/survarium-offload/0.69d0`
  (`Survarium.zip`). A dump of 0.69d0's `gameplay/game.options`:
  `vostok-port dump-db-config --db <split> --path gameplay/game.options --leaves`.
- Template patch: `~/.cache/skhid-release/patches/resources.db.patch_19`.
- Players' files: `prepare_live.sh <srp-game-data> <out> --db <resources.db> --patch <patch> ...`
  (writes `paths.txt`, the have-list).

`scripts/rifle-scopes/` (#516):
- `prepare_donor.sh <0.69d0 split list> <patch 19> <resource-porter checkout> <players' paths> <out>`:
  optics' models and aimed models (configs widened to 0.26g's layout by `widen_config.rs`),
  template-made materials (`retexture.py`), missing textures, icons. Check against `donor.sha256`.
- `survey.py --donor <d> --live <l> --survey <weapon survey's survey.json>`: `fit.tsv`.
- `make_pack.py --donor <d> --live <l> --compose <vostok-port> --out $P/rifle-scopes-<n>` (VERSION
  in the script; 5 now). Inputs: `scopes.tsv`, `mounts.tsv`, `fit.tsv`, `materials.tsv`,
  `texts.tsv`, `references.tsv`. Writes `variants.tsv` and `../content-edit/rifle-scopes-1.json`.
  Runs the texture check.
- `review.py --pack <pack> --live <l> --donor <d> --out <png>`: approximate outside/aimed sheet.
- `render_icon.py`, `pbr_026.py`, `retexture.py <template> <donor leaves> <out>`, `scopes_lib.py`.

`scripts/scope-fits/` (#954):
- `prepare_donor.sh` (same arguments): modules of `modules.tsv` (8 sight modules included), rail
  materials from templates, 0.69d0 gun locators, `game.options`, icons. Check `donor.sha256`.
- `survey.py --donor <d> --guns $P/weapons-031-7/files --guns $P/weapons-046-8/files --guns
  $P/weapons-069-10/files`: `mounts.tsv`, `fit.tsv`, `guns.sha256`.
- `make_pack.py --donor <d> --live <l> --guns ... --compose <vostok-port> --out $P/scope-fits-<n>`
  (VERSION 3): writes `variants.tsv`, `eyes.tsv`, `occlusion.tsv`, `../content-edit/scope-fits-1.json`.
  Stops on a failed eye or occlusion check.
- `occlusion.py --pack <pack> --live <l> --guns ... [--out <tsv>]`; `occlusion.py --live <l> --donor
  <d> [--out <tsv>]` for the players' own variants.
- `review.py --pack ... --live ... --donor ... --guns ... --out <png> [--only <gun>] [--per-gun 2]`.
- `sights.py` (called by make_pack.py), `sights.tsv`, `guns.tsv`, `optics.tsv`.

`scripts/live-scope-eye/` (#958): `prepare_live.sh`, `make_pack.py --live <l> --compose <vostok-port>
--out $P/live-scope-eye-<n>`, `review.py` (needs #516's `render_icon.py`), `aimed.tsv`, `variants.tsv`.

Checks for a scope PR (lean, as the owner wants; not the whole selftest):
- `release/build-content 43 --out <dir>` on a test tree; it must list only the expected changes.
  `/mnt/hdd` is nearly full (14 GB free on 2026-11-02; a 1.1 GB patch failed there once): put `--out`
  where there is room and delete it after.
- `srp-check-game-data` on the built `server-game-data.db` ("srp starts on it"); it also enforces the
  entry limit. Expect "weapon scopes: <n> pair(s)" with none left out.
- `check_localization_026g.py` when texts change; the lobby Ruffle harness
  `scripts/localization-check/check.sh` on the built `lobby_menu.swf` (10 languages; 515 texts with
  #516, 529 with #954).
- `pack check` of the scope packs with the weapon packs (no conflicts).
- make_pack.py's own checks (textures, eyes, occlusion).
- A combined test tree: `scratchpad/scopes/tools/mkcombo.sh <name> <base branch>` merges the three
  batches and #958 into `skhid-srp-wt/combo-<name>`, de-duplicates `content.tsv` and puts
  `localization-1.json` last (temporary; copy it before the scratchpad goes).
- In a game: only with the owner's go, one game at a time, under the shared game lock; the owner's
  own play builds have been the check.

## 7. Catching up with main

Order: #516 first, then #958, then #954 (it is stacked on #516 and needs the batches).

1. Fetch by name, merge `origin/main` into each branch (a merge commit, no force).
2. `release/content.tsv`: keep every row. Put the three scope edit rows after `weapon-boxes-1.json`
   and before `weapon-stats.tsv` (as play43, 40032d577); `localization-1.json` stays last. The scope
   packs' rows after the weapon packs they build on.
3. Localization TSVs auto-merge, but unions append rows: re-sort each table in key order (play43
   6474bf495) and run `check_localization_026g.py`.
4. Stat tables: every weapon config needs its row in `weapon-stats.tsv` (play43 67e5c617f added the
   16 skins' rows). The scope packs add no weapon config; variants are category 22 and need no row.
   The batches carry their guns' rows. `dead-keys-1.json` removes `parameters/` copies (e.g.
   `parameters/base_dispersion`) from weapon configs; nothing in the scope packs reads those keys,
   but re-run the build so the table check sees the gun configs after `scope-fits-1.json`.
5. #954: rebuild if the weapon packs moved. Run `survey.py` on the batches' current packs (now
   weapons-031@7, weapons-046@10, weapons-069@11; camo 031@2, 046@2, 069@3) and compare `guns.sha256`. If any gun model or config changed, bump
   VERSION and rebuild (the model copies are of the packs' gun models). The pack.json sha alone does
   not show a content change (section 9).
6. Build content for 43 (revision 42 is live), `srp-check-game-data`, the lobby harness.
7. PR comments: what changed, the checks, the pack sha. Merges only on the owner's word for that PR;
   #954 must be retargeted to main after #516 merges.

## 8. Owner decisions and rules

- 2026-10-26: "let's do those 8" (the rifle scopes). The prices, the CompM4 at x2 and the one-zoom
  picks are the coordinator's, open to the owner.
- 2026-11-01: "Properly port all weapons. And their attachments (all we can)." No suppressors, ever.
  Not ported: the EOTech G33 magnifier (no scope on its own), the Leupold 3-7 "summer17" (a skin),
  lasers.
- Follow retail (0.69d0) where it has a way ("how retail did it? we just follow it", 2026-11-02).
- Under an optical scope the iron sights are folded or removed, the retail way, on live guns too
  (2026-11-02). Collimators: fine as they are so far.
- Aim-in: "the camera should zoom in on the scope itself".
- Every render or capture shown to the owner is a labelled before/after pair from the same camera,
  at one fixed exposure.
- No hacks: fix structurally at the source (the pack's scripts, the porter's core), edit the original
  rather than adding side layers; keep PRs small.
- Content rules (AGENTS.md): a content change is a change of its source; packs and edit lists in
  `release/content.tsv`; never assemble a patch by hand; the release builds the patch.
- PR merges and anything players feel need the owner's word at that moment. No game windows without
  the owner's approval, never while the owner is playing.

## 9. Gotchas

- `pack.json` lists file paths, not contents: a pack rebuilt under the same version with changed files
  keeps the same sha256 (scope-fits@3 did, before and after rifle-scopes@5). Always bump VERSION and
  make a new folder.
- The texture's sRGB flag is the `.dds` last byte; an encoder round trip drops it. `pbr_026.Dds`
  refuses a file without it.
- 0.26g clamps weapon UVs; 0.69d0 repeats them (`wrap_uv`).
- A type-3 modifier adds only to a table that already has a child: a gun without `addons` needs the
  placeholder. A type-3 add where a stock scope exists hits the client's duplicate-name assert; use
  type 0 there.
- A turned `aim` locator (AK family 0.004 rad, Ash-12, Saiga, VSS, Val up to 0.008) moves the
  reticle; every new scope model must use an unturned eye or compensate (#958).
- Gun meshes from the porter are 0.69d0's vertex for vertex, so 0.69d0 locators hold, each in its
  bone's frame (the Minimi's rail hangs on its feed-cover bone).
- Our Kobra, DeltaPoint and old PSO-1 meshes are 0.69d0's shapes in another vertex order (found by
  bounding box); our Leupold Mk4's turret is bigger.
- The OTs-14 Groza takes no scope (0.69d0 mounts optics only on a tactical handguard that replaces
  the 0.31a2 one baked in our mesh); the Mauser M712 has none in 0.69d0; the Docter fits none of the
  new guns.
- `review.py` mirrors a skinned gun's vertices in x and z into the locators' frame: approximate.
- `/mnt/hdd` is nearly full; builds died there with "No space left on device".
- Never `git stash` in these worktrees (the stash is shared); never search from `/`.
- The scratchpad and its files (donors, renders, crash dumps) are temporary. The donor folders used
  for @5/@3 were `scratchpad/scopes/donor516b` and `scratchpad/sights/donor5`; rebuild them with
  `prepare_donor.sh` and check `donor.sha256`.

## 10. Temporary renders and logs (may be gone)

Under `/tmp/claude-1000/-home-sheep-Projects-survarium-skhid-srp/b0897dee-f169-44a9-aec9-267272d23bc4/scratchpad/`:

- `materials/sheets/`: `scope_materials_before_after.png`, `scope_icons_before_after.png`,
  `scope_icons_contact.png` (= `docs/images/rifle-scopes-icons.png`), `scope_icons_rendered_vs_069.png`,
  `material_ak12_stock_vs_before_vs_after.png`, `material_ppsh_stock_vs_before_vs_after.png`,
  `material_compm4_stock_vs_converted.png`; `materials/materials_list.md` (every material of the 184
  variants).
- `sights/review/`: `aimed-before-after.png` (SCAR-H CompM4 and PK-01VI, VHS-2 PK-01VI and DeltaPoint,
  Minimi PK-01VI, Fort-401 EOTech 552), `before.png` (@2), `after.png` (@3), `aim-before-*.png`,
  `aim-after-*.png`; `sights/minimi-ta01.png`; `sights/occlusion-live.tsv`.
- `scopes/`: `review1.png` .. `review6.png` (#516 review sheets), `lse-review.png`,
  `lse-review-top.png` (#958 before/after), `tools/` (`mkcombo.sh`, `resolve.py`, `mounts069.py`).
- `ingame-516/`: round 1 (#516@1) shop, lobby and aimed shots, e.g. `41-ak74m-leupold37-aimed.png`,
  `26-lobby-m4a1-ncstar4-carryhandle.png`, `29bz-lobby-hk416-aperture-zoom.png`.
- `ingame-fits/`: round 2 lobby shots and `crash1/`, `crash2/` (reports, dumps, game logs).
- `fitsgame/`: the headless runner (`runner.sh` holds the game lock while the client lives), config.

Owner screenshots (`/home/sheep/screenshots/`): `2026-11-01-19-08-38.png` (SCAR-H + Walther aimed,
black-and-white metal), `2026-11-01-20-13-00.png` (VHS-2 + PK-01VI aimed, black metal, front sight in
view), `2026-11-01-20-18-53.png` (SCAR-H in the lobby, flip-up sights up), `2026-11-01-20-20-00.png`
and `2026-11-01-20-20-22.png` (shop icons before @5), `2026-11-02-15-03-57.png` (Minimi + TA01, hip
view, sights up).
