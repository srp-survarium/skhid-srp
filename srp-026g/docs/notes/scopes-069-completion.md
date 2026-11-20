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
- reusing an existing weapon, scope or variant ID for a different item;
- duplicate modification IDs across every item config (the client uses one
  global item-modification lookup, including shop no-ops).

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

Implemented packs are `rifle-scopes@18` and `scope-fits@12`, pinned by hash in
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

**Native lobby review started; not released.** The owner tested SVU scopes and
reported an invisible PSO-1 (see the follow-up below). The native-game review
must still exercise aim-in,
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
- `combined-content/`: initial full local candidate, before the native-review
  corrections below, and its per-file audit. Patch SHA-256
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
2560×1440 fullscreen. The owner subsequently authorized launch and operation on 2026-11-15. The
client was launched, the review inventory provisioned, and the owner tested SVU
attachments. That client has since exited; another game session is outside this
review. Keep the corrected build prepared for the next isolated launch.


The deferred native review should cover the M4A1 collimators (including XPS2's
ring), AK-74M/Leupold 3-7, RPD's animated mount/nonzero eye offset, folded sights
on SCAR-H and VHS-2, SVU optics, and the Glock preview. Use `combined-pairs.tsv`
for supported combinations. Attach one purchased scope to multiple weapons,
replace it with an old scope, remove it, and re-login to check persistence;
then inspect stationary aim, aim-in/out, reload, reticle size and native materials.

## Follow-up audit: historical Glock preview crash

The old handoff's crash happened on 2026-11-01 at 08:02:37 with weapons-046@3,
before `1b91ee659` (13:56 that day) changed imported weapons' `preview_item`
from their template's clip to their own idle clip. That fix addressed lobby
crashes in `bone_matrices_computer`, the subsystem named by the Glock report.
This is a plausible connection, not proof that it fixed this particular crash.
The old dump's documented temporary and archive locations no longer exist.

The final candidate archive's Glock config was extracted independently into
`artifacts/scopes-069/glock-audit/`. Its configuration equals the released one
apart from the new `addons` placeholder. Its `preview_item` is
`resources/animations/single/weapons/glock_17/item/stand/3rd_view/idle`.
The DeltaPoint, NcSTAR 4 and RSA-S variants use two copied weapon models;
both preserve the released skeleton and bind-pose bytes. The base and copied
models evaluate with finite vertices at preview times 0, 0.5 and 1 (nine
poses), with all skeletal animation channels belonging to their bind pose.
The separate `object_movement` root-motion channel is excluded from that
bone-name comparison, as defined by resource-porter's
`skeleton_animation::ROOT_MOTION_CHANNEL`.

The audit script and result are `glock-audit/check.py` and `result.log` beside
the extracted config. These checks narrow the historical risk but do not
reproduce or clear the crash. The current client's Attachments-window test
remains unverified; the later authorization to launch superseded the earlier
instruction to keep the game closed.


## Native review follow-up: invisible attachments (2026-11-15)

The owner's `~/screenshots/2026-11-15-18-49-12.png` shows PSO-1 selected on the
SVU but no scope on the weapon. The local account persisted the correct variant
3111 (`72:3111:0:3`), and the native log recorded a preview rebuild at 18:49:07.
Extracting the actual mounted archives reproduced the pack's config bytes.

The cause is a global modification-ID collision, not an SVU exception. Pack 11
used `990000 + variant dict id`; the original shop items use
`991000 + shop dict id`. All 12 variants 3101–3112 therefore collided with the
12 original shop no-op modifications 993101–993112. A globally resolved no-op
leaves both the bare weapon model and its absent scope unchanged. The owner
observed Leupold working, but that variant also collides: relying on which
resource registers first is invalid.

Pack 12 uses `1000000 + variant dict id` for all 212 variants. Variant dictionary
IDs, scope ownership, saved records and compatibility remain unchanged. The
catalog checker now scans every distinct item config, regardless of category,
and rejects duplicate item-modification IDs. Its regression checks both
registration orders. The old composed candidate fails this check; see
`artifacts/scopes-069/modification-regression.log`.

The native log also reported a missing TA31 `acog_light_pipe_s` material. The
new native optical geometry included that surface while its older reference
model did not. Its donor material is now mapped to the existing
`weapons/loc2/acog_light_pipe`. The scope-fits builder previously omitted the
shared material/texture closure check; both builders and the registered
real-asset checker now run it. The old pack fails with the missing material
(`material-regression.log`); pack 12 passes for all 162 referenced materials.
The existing Vepr-12 normal-map omission is still reported as baseline data.

Correction validation: all five catalog unit tests, affected `cargo:survarium`
tests/Clippy, registered scope-geometry and scope-assets pass. The asset check
covers all 396 generated variants at three animation times and their material
closure. These checks do not certify a post-fix native screenshot; the owner
retains the remaining visual/aiming review. No production action was taken.


The corrected `release/build-content 60 --full` output is
`artifacts/scopes-069/content-scope-fix/`. Archive readback, full manifest audit,
server-data generation and item tables passed. Patch SHA-256:
`7f1e458f90740d4c3996a8bf86551ba706d23842b5daac9997e35084a4523554`;
server data SHA-256:
`8ac7b3c141dc5b170ea42cb418767a3d6efbe0ab95b7533aeec5f37f41648e5a`.
The registered `tools/check scope-preservation` also passed on its independent
rebuild (`scope-fix-preservation-check.log`). The corrected composed catalog has
no duplicate item-modification IDs and keeps
169 released fits plus 396 additions (20 scopes, 87 weapons). Its pair report is
`scope-fix-pairs.tsv`. `scope-fix-pack-diff.txt` confirms the same 5,753 file
paths: exactly 212 configs changed only their two modification-ID fields and
10 TA31 model settings changed only the light-pipe material. All other bytes
are identical to pack 11.

The isolated config now points at this corrected patch and server data. Its
idle server was stopped through its own launcher; its account, inventory and
SVU PSO-1 record (`72:3111:0:3`) remain intact. No new window was opened over the
owner's separate game session. The next isolated launch will use pack 12.
Post-fix native rendering and aiming are still for the owner's visual retest.

## Complete standalone coverage and attachment audit (2026-11-15)

The owner requested the missing weapon fits and correct attachment/centering
for every scope. The earlier preservation check did not prove donor coverage.
`scripts/scope-coverage/audit.py` now checks all 103 weapon entries against all
20 sold optics using 0.69's complete slot graph, including default modules,
weapon-specific exceptions, premium aliases and `srp_like`. Optical modules
cannot serve as intermediate mounts for a standalone fit.

The final archive readback has **1,205 combinations on 102 weapons**: all 565
previous combinations retained with their original variant IDs, plus **640**
new combinations (dictionary IDs 4000–4639). There are **zero missing standalone
fits**. The other 66 linked combinations require another scope or lack a usable
standalone slot; 789 are not offered by the donor. The Mauser M.712 is the only
weapon without a standalone fit. New coverage includes the AKS-74U, TOZ-66,
TT-33, UZI, PM, Winchester 1894, PPSh, SKS, APS and Groza families and their
premium copies. The renamed MP7 Silencer entry is included through `srp_like`.

The source is `scripts/scope-coverage/`, pack **scope-coverage@2**, and the
coverage/centering edit lists in `release/content.tsv`. `inputs.json` pins the
15,874-file prepared live tree and 947-file donor tree; `variants.tsv` locks IDs.
No patch revision was changed or reserved. The existing `next` simulation row
records the added attachments and changed aiming models.

Mounts use the full native rigid transform on the appropriate weapon bone.
The older weapon meshes are registered against donor meshes before transferring
locators; weak matches stop generation. The UZI's different receiver has an
explicit measured top-cover rail. Groza and Valmet handguard replacements remove
only exact matching default-module triangles. Scope and camera follow the same
bone, including hinged barrels and moving receiver covers.

The side/top review exposed a second generic issue: static module sockets are
already in mesh coordinates. Applying the weapon's half-turn at that boundary
mirrored longitudinal offsets. The corrected builder applies the half-turn only
at the weapon attachment. `check_mounts.py` independently recovers each rendered
rail's transform from its vertices and compares its socket to the optic body.
It rejects the earlier prototype (first failure: a 1.902 mm mirrored dovetail
offset), even though its camera checks passed. Twelve representative families
were reviewed in `artifacts/scopes-069/coverage-mount-review-2.png`; the Groza and
Valmet optics now sit on their replacement handguard rails.

**455 existing variant configurations** (473 weapon/scope pairs) receive camera
or mount corrections. This includes **92 existing adapter assemblies** whose
optic bodies and reticles move together onto their physical rail socket.
Collimators align to the emitter axis. Legacy optical overlays retain their
view relative to the optic on the attachment bone, eliminating animation drift.
Existing configurations are edited after earlier content edits; post-edit
snapshots are not packed over their source. Item IDs, scope properties and
compatibility are retained.

Validation of the corrected candidate:

- Registered `scope-coverage-assets` passed: 640 new variants / 1,920 animated
  native-camera checks; 525 rendered adapter socket checks; 455 existing
  configurations / 3,696 idle-and-reload camera checks. It applies the actual
  centering edit list, with zero unexpected differences, before checking it.
- The standalone graph regressions passed. The extraction helper built and
  passed Clippy with `-D warnings` under the project's Nix environment.
- Registered `scope-geometry` and `scope-assets` passed for the shared native
  camera oracle and the earlier 396 generated variants after the checker update
  (`coverage-existing-checks.log`).
- `release/build-content 60 --full` passed, including complete source-manifest
  comparison, patch readback and item tables. The candidate is
  `artifacts/scopes-069/coverage-content-2/`, not the superseded prototype in
  `coverage-content/`.
- The server's preservation checker compared the corrected prior candidate
  with this candidate: **565 preserved, 640 added, 20 scopes on 102 weapons**;
  no global modification-ID collisions. `coverage-pairs.tsv` records the pairs.
- Registered `scope-preservation` also passed its independent source build and
  comparison against the released catalog (`coverage-preservation-check.log`).
- `coverage-final-audit.tsv`, read from the actual archive stack, reports
  1,205 preserved, 66 not standalone, 789 not offered, and no missing rows.

Pack manifest SHA-256:
`daa4e5baed7215d735020663db52014fcd7a11c5dc05e074eaed1dd64520c03e`.
The 838,053,591-byte local patch SHA-256 is
`4c745a23136699718e24bd67520ec8765aedd81cd228cc4e5e64fe04ba45d2c0`;
server-game-data.db is
`b226720e7fd739c86482f79846764f14c905af7de7a6b1415e2659c8823f5df3`.

The stopped isolated Scopes069Review configuration and its patch symlink now
point to `coverage-content-2`. The config was backed up. The account and saved
attachments remain intact. `artifacts/scopes-069/review-scopes.sh` restarts that
setup with one fullscreen 2560×1440 client. No client was opened for this pass.
This work is implemented and statically tested; the corrected candidate has
not been played or released. Native shader appearance and play feel remain
for the owner's review. The asset checker still reports the same two pre-existing
weapon-material references (SV-98 camo roughness and Vepr attach normal); no new
scope material or texture dependency is missing.

## Merge review (2026-11-15)

The owner requested and received one fullscreen 2560×1440 Scopes069Review
client on the isolated server with `coverage-content-2`. The owner subsequently
accepted merging the completed scope work. This is acceptance for merge, not
evidence that every weapon/scope combination was played.

The hover tooltip's long compatible-weapons list was reported as disorienting.
The owner explicitly deferred that UX change to the next task; this branch
does not change the tooltip.

Main `c7fa003dd` was integrated before merging. Both content additions are
preserved, released revision 60's simulation history stays unchanged, and the
scope simulation entry remains under `next`. No release was performed and the
running local review setup was not replaced during integration.

The integrated source passed `release/build-content 61 --full`, including
31,858-file archive readback, comparison against the complete player resource
stack and item-table generation. This is a local build number, not a reserved
release revision. Its output is `artifacts/scopes-069/merge-content-61/`:

- Patch SHA-256: `3c3131781ecefa8074c85eb56164e52d2ffeccd414b5fba30178e754431e1856`.
- Server-data SHA-256: `eba296a2a37b004a527ab920aa9fc5d5664b5328cd7715dfcb39bf132db686ff`.

The registered content build produced the same patch and server-data bytes.
Its item-table comparison includes main's Winchester damage change (100 to 94);
no generated release tables were written into this feature branch.
