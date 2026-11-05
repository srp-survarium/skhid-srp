# New weapon lobby icons

The twenty guns shipped in revision 43 use perpendicular side-profile icons,
with the original retail 0.26 atlas as the lighting and framing reference.
The comparison is [here](../../docs/notes/weapon-profile-icons.png): original
retail 0.26 references above, original donor icons and the new renders below.

The 0.31, 0.46 and 0.69 imported images were taken from those builds' item
icons and levelled. Levelling an image leaves its perspective: the top,
front and thickness of the weapon remain visible. These replacements render
the actual shipped meshes with an orthographic camera. Screen right is model
+Z, screen up is +Y, and the camera looks along +X; there is no yaw, pitch,
roll or perspective division. All muzzles point right.

The rasterizer comes from resource-porter's equipment/weapon icon workflow
(`render-duty-026-icons.py`, `render-mask-026-icons.py` and
`render-weapons-069d-icons.py`). It handles the one- through four-influence
skinned vertex layouts and uses the model's LOD0 surfaces, diffuse,
DXT5nm normal, Fresnel and roughness maps. Loose pump-action shells are
excluded. The normal map uses the mesh tangent frame, with X from alpha
and Y from green. Lighting has an upper-left key and cool metal highlights;
this is an offline icon renderer, not a recovered original retail icon baker.

The style reference images are extracted directly from the stock 0.26
`flash_movies/lobby_menu.swf`, unpremultiplying its ARGB atlas: AK-12 [3,3],
AK-74M [10,2], A-545 [16,2], and pistol cells [5,1], [6,2], [13,2].
The native icons' measured opaque widths are
160–161 pixels for those rifles and 65–76 for those pistols. Output cells
remain 171×83, at the existing positions, with a transparent background and
soft dark shadow. `icons/lighting.json` records the reference SWF hash and
measured luminances. Exposure instead matches **each weapon's own original
imported retail icon**, from the immutable input pack in `sources.tsv`.
This preserves the differences between tan paint, pale metal, wood and
black finishes. The finished icon's mean RGB luma over alpha > 200 must
be within one 8-bit level of that reference. Exposure is measured again
after clipping, downsampling and adding the shadow, rather than assuming
the whole pipeline is a power-law. This does not recover the donor's exact
lighting or per-material contrast; the renderer still uses its own light rig.
`icons/rendered.json` records every model, cell, source PNG hash, reference
and result luminance, exposure and output PNG hash.

Revision 44 incorrectly normalized every rifle to the same dark reference
mean (55.55), and every pistol to 71.51. That suppressed naturally light
finishes, especially SCAR-H, L85A2, Fort-401 and Glock 17. The exposure audit
compares all twenty shipped icons with their own donor references;
the fix changes exposure only and retains the AKMSU and PPSh magazine poses.
See the [three-way visual comparison](../../docs/notes/weapon-icon-exposure-audit.png)
and [all twenty measurements](exposure-audit.tsv). The donor icons are from
0.31, 0.46 and 0.69, not native 0.26 icons of guns that 0.26 did not contain.

Validation against revision 44: all twenty PNG alpha channels are identical;
each packed cell has the expected premultiplied RGB (within byte rounding).
Only those twenty atlas cells differ, with every other pixel and all 4,990
other SWF tags byte-identical. The manifest build changes only the lobby
movie; all 18,211 composed files verify and both item tables are unchanged.

## Reproduce

Python needs NumPy and Pillow. Extract the stock movie without changing it:

```sh
srp-game-data get --db /path/to/stock/resources.db --out /tmp/weapon-icon-reference \
  flash_movies/lobby_menu.swf
python3 render_icons.py --packs /path/to/content-packs \
  --lobby-swf /tmp/weapon-icon-reference/flash_movies/lobby_menu.swf \
  --out /tmp/weapon-icon-renders
```

`--large` additionally renders inspection images at 684×332. `sources.tsv`
pins the source pack versions and their pack.json hashes. The source packs
contain the same meshes/materials/textures players have; no donor model or
texture substitution happens here. `icons/` holds the resulting twenty PNGs
and their metadata so the pack can be rebuilt without rerendering:

```sh
python3 pack_icons.py --packs /path/to/content-packs --icons icons \
  --out /path/to/content-packs
```

The pack builder refuses to replace an existing version. It checks every
PNG, copies the three source packs, replaces only their listed weapon icon
PNGs, and verifies that all other file bytes remain identical. The outputs
are weapons-031@9, weapons-046@12 and weapons-069@14, pinned in
`release/content.tsv`. Ammo and kill-feed images, gameplay configs, models,
textures and animations stay byte-identical. No simulation-change row is
needed.

Build the content through `release/build-content <next revision>`; its
comparison against revision 44 must contain only `flash_movies/lobby_menu.swf`.
Check the assembled movie's atlas against the live movie: only the twenty
weapon cells may differ; every other bitmap pixel and every other SWF tag
must be identical.

The two modular magazines are posed by `magazine_pose.py` before rendering:
AKMSU uses the shipped constant item-idle clip for its magazine bone instead
of its displaced bind position; PPSh's drum and cartridge surface undo the
converted module's quarter-turn about the magazine socket. Both corrections
transform normals and the tangent frame with the vertices. No game model or
animation changes. The other eighteen PNGs retain their previous hashes.
That pose correction shipped as weapons-069@13; comparison with version 12
changed only the two weapon PNGs and pack metadata. The exposure correction
retains both poses.
