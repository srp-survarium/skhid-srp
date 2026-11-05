# Sight alignment

Inspect and correct the aim locator for any stock sight or fitted weapon–scope
pair in the 0.26g game dictionary. This is an offline tool: it reads released
resources, poses the weapon and attachment, previews a candidate, and exports
an edit list for the existing content pipeline. It does not change running
clients, ballistics, archives, or the release manifest.

Requires Python 3.10+, NumPy, Pillow, a WebGL browser, and the repository's
`srp-game-data` executable. Run the commands below from this directory with
NumPy and Pillow installed in the same Python environment. The repository's
development shell supplies Python but does not include these two packages.

## Extract and inspect

```sh
python align.py extract \
  --stock /path/to/stock/game/resources.db \
  --out /tmp/sight-assets
python align.py review --live /tmp/sight-assets --out /tmp/sight-review
python -m http.server --bind 127.0.0.1 --directory /tmp/sight-review 8765
```

Open <http://127.0.0.1:8765>. `extract` defaults to this checkout's
`release/patches.tsv` and `~/.cache/skhid-release/patches`. It verifies each
listed patch's hash and size, layers them over stock, and records provenance
in `alignment-sources.json`. Use `--patches`, `--cache`, and `--bin` to override
those defaults. Both output directories must be new. The input is the stock
resources archive plus the listed patches, never `server-game-data.db`.

The catalog includes stock sights and every `srp_scope_fit` pair, applying
the fitted variant's model and configuration modifiers before loading it.
Unsupported geometry remains visible with its failure reason. `--only cobra`
limits preview generation; `--time 0.5` samples another point in the weapon's
item idle animation. The complete extraction includes all item clips.

1. Select a pair. Green marks the projection center; red marks the selected
   geometry point. A single named dot emitter provides a suggested point.
   For irons or textured reticles, choose a surface and pick the intended
   sight point, or pick two opposite rim points to use their midpoint.
2. Use **Center point (rotation)** to change viewing direction while keeping
   the locator position and roll. **Center point (position)** shifts its
   position while preserving rotation. Fine adjustment is available in
   millimeters and degrees. Reset removes that model's candidate.
3. Inspect the before/after views and the listed pairs sharing this model.
   Changes belong to the model, so switching pairs preserves them. Inspection
   zoom does not change the reported pixel displacement.
4. Download the correction plan. Plans are held in browser memory until
   downloaded; reloading the page discards them.

The preview uses a canonical 65-degree projection multiplied by the pair's
scope factor (or divided by its iron-sight zoom factor). Pixel displacement
is reported at 1080 lines. This is an inspection scale, not an inference of
the resolution, crop, or projection of a supplied screenshot. The scope
factor comes from its own configuration. Change the preview projection when
comparing a known full-frame capture; do not scale a correction from an
unknown crop.

## Plan every collimator

```sh
python align.py plan --live /tmp/sight-assets --out /tmp/collimators-plan.json
```

`plan` makes the viewer's plan for every collimator whose dot (one emitter mesh:
the glowing dot, the Walther's point) is further than 1.5 px at 720 lines from
the centre (`TOLERANCE_PX_1080`): one change a model, the eye looking along the
optic's own axis (the gun's posed frame at `scope_point`, turned back where a
stock attachment sits on a half-turned `scope_point`) and moved across it onto
the dot's line, as the reference variants' unturned eyes do. Every pair sharing
the model must be a collimator with a dot that the change centres, else it
stops. It prints each pair's error before and after.

## Export and accept

```sh
python align.py export --live /tmp/sight-assets \
  --plan /path/to/sight-alignment-plan.json --out /tmp/sight-edits.json
```

Export checks the source SHA-256, exact previous locator values, finite
vectors, and shared-model conflicts. It applies the edit in memory and walks
the result to verify that only the aim position/rotation changes. A stale
plan must be regenerated against the new source. The output uses the
existing `content-edit` `set_entry` format.

Validate the candidate in the native game before accepting it. In particular,
check the reticle against screen center while stationary and fully aimed,
then check transitions and each pair sharing the edited model. Copy accepted
edits into `../content-edit/`, add their `edit` row to `release/content.tsv`,
and use `release/build-content` to build and verify content. Never assemble
a patch folder from these inspection extracts. Feature work does not change
the release revision or restart players' services.

## What the preview proves

`model.py` uses engine row-vector matrices and the actual 0.26g aim order:

```text
camera = translate(0, 0, aim_offset) * rotate_y(pi) * locator * bone_pose
alignment = inverse(camera) * Item_bone
```

Retail `weapon::calculate_aim_alignment` at `0x244D40` calls
`calculate_locator` at `0x243BC0`. Runtime binding uses `bone_idx`; the
editor's `bone_name` is not authoritative. The viewer evaluates the item
idle B-splines, skins the weapon using inverse bind poses, attaches the optic
to `scope_point`, and normalizes into the aim bone's frame. The optional
Unicorn check runs the original executable's function for 120 randomized
attached and unbound cases and compares its output with this formula.

The preview does **not** reproduce the entire player animation graph, sway,
recoil, ADS interpolation, lighting, or optical shaders. A mesh's bounding-box
center may not be its visible reticle zero. In particular, chevrons and
textured grids require an explicit point and a game check. Suggested emitter
offsets are review leads, not confirmed bugs or permission to batch-correct
every model.

The reported 9A-91 example matches the Cobra EKP-8 fitted model:
`models/weapons/a9_91/a9_91_cobra_ekp_8.skinned_model`. Its geometry reproduces
the upward displacement (14.5 px at 720 lines).
`../content-edit/sight-alignment-1.json` is `plan` and `export` on the players'
files through patch 47, cut to that one model (the owner's choice; the note
`../../docs/notes/sight-alignment.md` lists the other 23 the plan finds off).
The owner saw its dot centred in the game (2026-11-05). It replaces the first
rotation-only candidate for the
9A-91 (aim rotation x 0.009 -> -0.0101 at the same position, which centres the
dot too but leaves the eye 3.8 mm above the optic's axis).

## Checks

```sh
python -m unittest discover -s .
python check_engine.py /path/to/original/026g/survarium.exe
node --check viewer.js
```

The second command additionally needs Unicorn and refuses executables whose
SHA-256 is absent from `tools/re-kit/v026g/identity.json`. Unit checks cover
independent spline evaluation, handedness, centering, and export isolation.
On the manifest ending at patch 36, all 252 pairs loaded at animation times
0, 0.5, and 1 second; on the one ending at patch 47, all 272 at time 0. Browser checks exercised centering and exported the
candidate back through `content_edit.py --check`. Shared-model persistence,
reset across pairs, and rapid pair switching were also exercised in Firefox.
