# Completing the 0.69 scope port

Owner request, 2026-11-15: more scopes on more weapons, retaining all existing
scopes and pairings, with 0.69 as the reference for how they work and look.

Work is on `feat/scopes-069-complete`, based on main `4fd451b76`, incorporating
the previously unmerged `feat/scope-fits-new-guns` (`ed4cd272a`) and its
`feat/rifle-scopes-41` parent. The old November 2 handoff predates the subsequent
eye-relief, posed-eye and retail iron-sight fixes; those fixes are retained.

## Preservation

`srp-check-scope-fits <released data> <candidate data>` uses `ScopeFits::derive`,
the server's actual parser, independently for both game-data files. It refuses:

- rejected variants and pairs absent from the lobby compatibility table;
- any missing released pair or a changed variant ID (saved attachments refer
  to that variant);
- reusing an existing weapon, scope or variant ID for a different item.

Its stdout is a TSV of every preserved and added pair. Catalog size alone is
not proof: a larger catalog can still lose individual combinations.

## Optical aiming

The previous builders kept the gun's iron-sight eye for magnified scopes and
moved a synthetic eyepiece in front of it. This made the aim-in approach the
iron sights and then switch to a scope elsewhere. The donor's actual optical
`aim` locator and its idle/aimed models now form one assembly: both models use
the same mount translation and the weapon receives that optic's eye.

The conversion in `rifle-scopes/make_pack.py::optical_eye` uses the
scope locator in its attachment bone and compensates the weapon's `aim_offset`. It inverts
the native 0.26 order `translate(offset) * rotate_y(pi) * locator`; zeroing a
rotation or translating only world Z is insufficient on animated mounts.
Each optical variant has its own weapon model, since different optics have
different eyes. Weapons with stock optics need the set-form scope modifier
plus an explicit weapon-model modifier; their previous optical templates
did not include that second modifier.

The optical models retain 0.69's own body, lens and reticle instead of borrowing
another scope's eyepiece. Existing pairs are preserved. Collimator placement,
the stock-template materials, normal/tangent conversion, UV wrapping and the
retail iron-sight folding from the previous port remain in the builders.

## Validation and remaining review

Implemented packs are `rifle-scopes@18` and `scope-fits@11`, pinned by hash in
`release/content.tsv`. Eight shop optics and 396 fits are added. The combined
server parser reports **169 released pairs preserved, 396 added, 20 scope items
on 87 weapons**. Every old variant ID survives, and every resulting pair is in
the lobby compatibility table. Neither the patch revision nor the release list
is changed.

Passed on 2026-11-15:

- `release/build-content 60 --full`: the complete manifest, every offered patch,
  archive readback, server game data and item tables. This is a local candidate;
  60 is the test revision immediately above the listed patches, not a release.
- `cargo:survarium`: tests and Clippy with warnings denied, including the new
  preservation checker cases (growth, lost pair despite growth, changed ID).
- Camera tests: nonzero offsets, 120 randomized mounts, non-rigid frame rejection,
  and an aim locator moved from the item bone to an animated attachment bone.
- Original 0.26 executable in Unicorn: 120 attached/unbound aim-alignment cases.
- Actual generated geometry: 184 rifle variants plus 212 imported-weapon/Walther
  variants, each at animation times 0, 0.5 and 1; **1,188 camera checks**. Optical
  idle/aimed meshes share the donor assembly's translation, and the camera follows
  the attachment bone. Collimator emitters stay on the viewing axis.
- The prepared donor files match both committed `donor.sha256` manifests.
- Weapon details Ruffle harness, content-edit and localization data checks.

The affected `tools/check` run completed: **25 of 29 checks pass** after the
interrupted/environment checks were rerun. Four failures remain outside the
scope code (no runtime or map-source changes are included here):

- Localization Ruffle: Calibri lacks `∞` in `st_srp_inv_pocket_units_whole`;
  the key/value is unchanged on base main `4fd451b76`.
- `cargo:srp`: the PvB end-to-end test at `v026g_end_to_end.rs:1749` expects 25
  map entries but gets 17. Eight other end-to-end cases pass. Its runtime code,
  test and bundled game-data fixture are unchanged by this branch (the only
  Rust addition is the separate preservation-check binary).
- `world-check`: 44,286 checked, 44,271 passed and 15 refused, none unread or
  skipped. All refusals belong to local Training Camp recording `2533279d`,
  made on revision 56. Running that recording against the released baseline
  and the candidate produces identical 15 refusals (`world-refusals-*.log`).
- `vostok-layout`: `day_2: an existing object changed or disappeared` in
  `check_composed.py:72`. The Vostok source and checker are unchanged; this
  map-preservation failure has not been repaired in the scope branch.

The interrupted registered content build was rerun successfully; its patch is
byte-identical to the full-audit candidate. The registered scope-preservation check also passed (169 retained, 396 added).
The separate `srp` Clippy step passed with warnings denied after its unrelated
end-to-end assertion prevented the combined crate check from reaching lint.
The STALKER map check passed when rerun inside `nix develop` (its first run
lacked `cc` on PATH). The final scope wrappers use a Git flake reference to
exclude Cargo outputs from Nix source snapshots; the native camera check and
all 396 generated variants pass with those wrappers. `check-results.tsv` records
the final result for every selected check. No broad failing rule was loosened.

**Not yet played or released.** The native-game review must still exercise aim-in,
reticle shaders, attachment/replacement/removal, re-login, and the Glock's optic
(the earlier port recorded an unresolved preview crash). Numerical geometry and
the approximate review sheet cannot certify these behaviors. In particular, the
XPS2 uses the working native EOTech ring-and-dot emitter; its exact apparent size
has not yet been compared in the game with 0.69.

The existing port's fixed zoom choices remain: 0.26g has one zoom per scope,
so this does not add 0.69's variable zoom or switchable magnifier behavior.
For example, aperture is 3×, TA01 is 4× and Leupold 3-7 is 6×; the complete
choices and donor values are recorded in `scripts/rifle-scopes/scopes.tsv`.

## Reproduction and local evidence

Inputs and logs live in `artifacts/scopes-069/` in the main checkout:

- `live/`: stock plus every listed patch through 59; baseline game data is made
  from those same archives. `donor-rifles-2/` and `donor-fits-2/` are the prepared
  `full069.split` donors. Both preparation scripts accept `WIDEN_CONFIG` and
  `PYTHON` so the pinned conversion tools can be reused.
- `live-rifles/`: archive extraction after the rifle-only intermediate pipeline
  build. The intermediate source snapshot excludes the second pack and its edit
  list/item reservations. It is only input preparation; the final build uses the
  complete branch manifest, never that snapshot.
- `combined-content/`: full local candidate and its per-file audit. Patch SHA-256
  `0732b747e99170a9663ee18f1bae8e62d28e7132f822cf44a357d0d53035f217`;
  server data SHA-256
  `26ab5cf59b81c9114e65f0c3e0b2627647cf69b0d71c8a09fd9ba28e329a1961`.
- `combined-pairs.tsv`, `rifle-geometry.tsv`, `fits-geometry.tsv` and
  `native-camera-check.log`: measured preservation and camera evidence.
- `geometry-review.png`: posed geometry review; illustrative shading and a green
  screen-center mark, not a native-game screenshot.

`tools/check scope-geometry scope-assets scope-preservation` runs the registered
camera oracle, real-asset and catalog comparisons. The real-asset check names
its prepared private inputs explicitly in `tools/checks.tsv`, so changes to a
pack or input invalidate the cache. Build packs with their `make_pack.py`
commands; the second pack takes `--guns <live/> --live <live-rifles/>`.

The isolated native test configuration is
`private/local-clients-scopes069/config.json`: this branch's client/server,
full candidate content, account `Scopes069Review`, loopback ports 29690–29693,
2560×1440 fullscreen. It has been prepared but not launched. The owner chose “Keep it prepared; do not
open yet” on 2026-11-15. Gameplay review remains pending until requested.


The deferred native review should cover the M4A1 collimators (including XPS2's
ring), AK-74M/Leupold 3-7, RPD's animated mount/nonzero eye offset, folded sights
on SCAR-H and VHS-2, SVU optics, and the Glock preview. Use `combined-pairs.tsv`
for supported combinations. Attach one purchased scope to multiple weapons,
replace it with an old scope, remove it, and re-login to check persistence;
then inspect stationary aim, aim-in/out, reload, reticle size and native materials.
