# Complete scope coverage

Retired on 2026-11-21 with every scope variant (`../../docs/notes/attachments-scopes.md`,
"The variants retired"): `release/content.tsv` lists `scope-coverage@3`, version 2 less its 640
variants, made by `../scope-fits/retire.py`; `scope-coverage-1.json` and the `scope-coverage` and
`scope-coverage-assets` checks are gone. The rest of this file is how version 2 was made.

`audit.py` compares every weapon dictionary entry, including premium aliases
and `srp_like`, with the 0.69 attachment graph. A fit requires a complete path
through non-optical modules and usable slots. Weapon-specific module exceptions
apply. A piggyback-only link is not a standalone fit. Existing fits remain valid
regardless of what the donor offers.

The fixed rifle-scopes@18/scope-fits@12 input has 565 existing combinations and
640 missing standalone combinations. `variants.tsv` reserves the latter's
stable item IDs, 4000–4639. Modification IDs use a separate, checked range.
The builder refuses a changed registry or prepared input fingerprint.

`make_pack.py` builds scope-coverage@2 and the coverage/centering edit lists.
It transfers full native mount frames through a rigid correspondence of the
weapon meshes, requiring at least half the donor vertices to match within
0.2 mm. Scope and camera use the attachment bone, including animated covers
and hinges. Non-identity module rotations also rotate the shading basis.
The Groza and Valmet handguard replacements remove only matching baked module
triangles; unmatched default geometry stops generation.

The old UZI receiver differs from 0.69. `legacy-mounts.json` records a separately
measured top-cover mount for a medium rail. Its own socket positions the optic.
It does not weaken the mesh-matching rule for other weapons.

`repair_existing.py` retains every old item ID and fit. Existing collimators put
the camera on the emitter axis and the attachment bone. Existing rail sockets are measured from the mounted geometry; misplaced optic bodies and reticles move together onto them. Legacy optical overlays
retain their original idle view on that bone, eliminating animation drift.
The pack adds model copies; `scope-centering-1.json` changes only model modifiers
after preceding content edits. `review/` contains generated review configs,
which are not packed. The registered check tests the actual edit-list output.

## Reproduction

1. Build `extract_donor.rs` as a binary with `anyhow` and the split-aware
   `vostok-resource-porter` library used by `scope-fits/prepare_donor.sh`.
2. Prepare the live archive stack with `prepare_live.sh`, including the fixed
   rifle-scopes@18/scope-fits@12 candidate. Do not extract from server-game-data.db.
3. Run `prepare_donor.py --base <scope-fits donor> --live <live> --archive <0.69 split>
   --extractor <binary> --out <new donor>`. It extracts the listed meshes,
   materials and textures. Materials use a working 0.26 template.
4. Run `make_pack.py --live <live> --donor <donor> --compose <compose-model porter>
   --out <packs>/scope-coverage-2` with NumPy, SciPy, Pillow and ImageMagick.
   `inputs.json` pins the complete prepared trees. The texture encoder must
   support DDS output. The source manifest pins the resulting `pack.json`.
5. Run `tools/check scope-coverage scope-coverage-assets scope-preservation` and
   `release/build-content <local candidate revision> --full --out <new output>`.

Geometry checks are evidence about placement, reticle axes and animation
binding. They do not constitute an in-game play test or a release.
