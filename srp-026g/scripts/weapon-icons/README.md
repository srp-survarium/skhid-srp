# New weapon lobby icons

The twenty guns shipped in revision 43 use perpendicular side-profile icons,
with the original retail 0.26 atlas as the lighting and framing reference.
The comparison is [here](../../docs/notes/weapon-profile-icons.png): original
retail references above, current imports and the new renders below.

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

The reference images are extracted directly from the stock 0.26
`flash_movies/lobby_menu.swf`, unpremultiplying its ARGB atlas. Rifle exposure
uses AK-12 [3,3], AK-74M [10,2] and A-545 [16,2]; pistols use stock cells
[5,1], [6,2] and [13,2]. The native icons' measured opaque widths are
160–161 pixels for those rifles and 65–76 for those pistols. Output cells
remain 171×83, at the existing positions, with a transparent background and
soft dark shadow. `icons/lighting.json` records the reference SWF hash and
measured luminances. `icons/rendered.json` records every model, cell,
exposure and PNG hash.

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
are weapons-031@8, weapons-046@11 and weapons-069@13, pinned in
`release/content.tsv`. Ammo and kill-feed images, gameplay configs, models,
textures and animations stay byte-identical. No simulation-change row is
needed.

Build the content through `release/build-content <next revision>`; its
comparison against revision 43 must contain only `flash_movies/lobby_menu.swf`.
Check the assembled movie's atlas against the live movie: only the twenty
weapon cells may differ; every other bitmap pixel and every other SWF tag
must be identical.

The two modular magazines are posed by `magazine_pose.py` before rendering:
AKMSU uses the shipped constant item-idle clip for its magazine bone instead
of its displaced bind position; PPSh's drum and cartridge surface undo the
converted module's quarter-turn about the magazine socket. Both corrections
transform normals and the tangent frame with the vertices. No game model or
animation changes. The other eighteen PNGs retain their previous hashes.
For this correction alone, `pack_icons.py --pack weapons-069` builds version
13; comparison with version 12 changes only the two weapon PNGs and pack
metadata.
