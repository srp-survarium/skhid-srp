# Muzzle devices (the weapon kit's slot 4)

**Version scope:** client 0.26g, client patch 90 (`../../client-patches/rust/patches/src/muzzle.rs`),
the pack `muzzle-devices` (`../../scripts/muzzle-devices/make_pack.sh` over resource-porter's
`port-muzzles`, its `docs/muzzles-069d-026.md`), the edit list `muzzle-devices-1`, the lobby
movie's weapon window, the server's `modifications::MUZZLE_KIND`. The transport is the weapon kit
(`weapon-kit.md`, patch 68). Tags: PROVEN (read or run), [srp] srp's own choice, [owner] the owner's.

## The rules

- [owner, 2026-10-25/31, 2026-11-01] One slot takes one muzzle device: a flash hider or a muzzle
  brake. **No suppressors, ever.** All we can port, properly: devices where retail puts them, the
  gun's built-in device out of the way.
- [srp, under the owner's mandate of 2026-11-01] **Premium skins take their base gun's devices**
  (0.69d0's fits of the base weapon). **Stats at grade 7** (0.69d0's drop modifications of the
  device at grade 7 of 0..10). **Flash hiders change the shot's look only**: 0.26g has none of their
  keys (unmasking radius, hip and first-shots recoil), so they hide the flash (the gun's shot
  effect becomes the native `fx/weapons/9x39`) and nothing a match computes.

## What players get

- 35 devices (dict ids 2601-2635, category 22 as the scopes, `parameters/type` `srp_muzl`):
  0.69d0's purchasable flash hiders and brakes that fit at least one of our guns. Left out: every
  suppressor (by name, or by a suppressor's shot effect), and Zenit's DTK-4 (a closed can).
  Sold by seller 0 at no rank: a brake 6,000, a flash hider 4,500 [srp].
- 53 guns take them, by 0.69d0's own fits (`module_to_item_links`). Left out, with the reason
  (resource-porter `ports/muzzle-devices-guns.tsv`): fused brakes (AK-12, vz. 58), integral or
  built-in suppressors (VSS, VAL, VSK-94, VSSK, MP7 SD, 9A-91, KRISS Vector), meshes or barrels
  that are not 0.69d0's (Vepr-12 premium, SPAS-12, Benelli M4, Uzi), guns no device fits (the
  break-action shotguns, Magnum, TOZ-66, Winchester).
- **Brakes:** recoil -4 % (the six kick bounds the recoil loader 0x331840 reads, type 2) and aim
  time +3 % (`parameters/aim_time`, type 2). 0.69d0's keys are a player's modifiers
  (`recoil_modifier`, `aim_time_modifier`), which a weapon never registers (`weapon-rolls.md`), so
  the port puts them where weapon-rolls-1 puts the stock rolls. The M30x1 brakes: aim time only
  (their other roll, movement-speed damage, has no 0.26g key); the M22x0.75 shotgun brakes: none
  (0.26g's shotguns lack the buckshot-spread keys): looks only. The Taktika Tula flash hider also
  aim time +3 %. A simulation change: the `next` row of `simulation_changes.tsv`.
- The texts (names, one-line effects, the slot's label `st_srp_muzzle_slot`) in all ten dictionaries (`scripts/muzzle-devices/texts.py`); icons in
  `LobbyMenu_AmmoIcons` columns 5 and 6 (new), rendered from the models.

## How a device is drawn (patch 90; PROVEN in Unicorn, see the AK-74M check below)

- **Placement.** Our gun meshes are 0.69d0's base meshes plus the stock modules, vertex for vertex
  (the muzzle study of 2026-10-31), so 0.69d0's `muzzle_point` is our muzzle. The pack adds it to
  every body of each gun (`export_properties`, on the bone 0.69d0 names, its position written in our
  bone's frame). The patch reads it at `load_weapon` and draws the device at
  `calculate_locator(muzzle_point)` (R_y(pi)·L·B·W): mesh-aligned axes, so 0.69d0's modules, which
  are authored in the gun mesh's orientation around the muzzle, go there as they are. Not at
  `barrel_point`: that is 0.5 to 25 cm off the muzzle (#420's and #766's failure).
- **The built-in device** is cut out of the gun's mesh by the removal rule: our triangles whose
  corners 0.69d0's base mesh lacks, inside the placed stock module, and the seam triangles of the
  device past the muzzle. The cut part, our own triangles with the gun's material and UVs, is the
  gun's **stock device** (`data/srp_muzzle_stock_model`, `_fire_mm`, from the edit list), drawn
  while the kit names none: a gun looks as before until a device goes on.
- **A fitted device** is the kit's slot 4: its modification adds `data/srp_muzzle_model` and
  `data/srp_muzzle_fire_mm` (its `fire_point`) to the weapon's config; the patch asks the weapon's
  cook for that model and draws it instead of the stock one.
- **The flash** (`weapon+620h`, which update_pfx_transform and the shot effects take) moves to the
  device's fire point every draw, before they read it.
- **The scene:** the device goes in and out with the weapon's model (shown, hidden, the aimed scope
  that hides the gun, a culled player, artefact carry's hidden gun).

## Checks (the gates)

1. **G1, the cut:** something is cut on every gun with a built-in device, and the placed stock
   module lies on the cut part (mean distance within 3 mm; the AKS-74U 3.5 mm, its module a
   slightly different version). `port-manifest.json` has each gun's numbers.
2. **G2, nothing left:** no triangle of ours that the base lacks reaches more than 3 mm past the
   muzzle near the bore.
3. **G3, the barrel reaches the muzzle** (within 3 mm), else 0.69d0's barrel is another and a
   device would float.
4. **G4, no suppressor** reaches the pack (names, shot effects, the DTK-4s).
5. **G5, the renders:** each gun's `renders/<gun>.png` (the work folder's bundle): the gun with its
   stock device in orange; its muzzle bare, with the stock device, and with each device it takes.
6. **G6, the exe:** `check_muzzle_026g.py` (Unicorn, with the align probe), `selftest` check 90.
7. **G7, the game:** the AK-74M first, headless: the stock brake exactly where it was, a fitted
   device at the muzzle, the flash at its front, the device leaving with the gun.

## Camouflage

Patch 44 bakes a camouflage into a weapon's own surfaces; a stock device is a model of its own, so
it stays plain under a camouflage, as every fitted device does (`scripts/muzzle-devices/
camo_check.py` prints how much of each stock device a camouflage used to paint: none on the AK-74M,
AK-74N, Vepr, PKP, SV-98, Valmet, Vityaz; most of the SR-3M's and the M4A1's). [srp] Muzzle devices
are bare metal; the owner's rule puts camouflage on parts of a weapon, and the device is now its
own part.

## Not done

- The new guns of the release-41 batches take devices once they record `muzzle_point` on their
  bodies (the 0.69d0 port writes it as `barrel_point`): a row each in `muzzle-devices-guns.tsv`.
- 0.69d0's own shot effects of the devices (`fx/weapons/attach/*`) are not ported.
