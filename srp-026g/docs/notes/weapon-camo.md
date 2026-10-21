# Camouflage on weapons

The owner's request (2026-10-20): the armour camouflages can be put on the regular weapons, painted
where the premium (camouflaged) weapons have their paint; the tooling to do that in game.

## Rules

- **Which weapons.** Every weapon (all 83 weapon configs, pistols and 0.29a3 ports included):
  91 textures, 111 `_aoac` files with the premium copies (`scripts/weapon-camo/guns.tsv`,
  `scripts/content-edit/weapon-camo-1.json`). Not their barrels, sights, suppressors and
  attachments when those have textures of their own, nor the 12-gauge shell, the SV-98's net or
  the premium-only `_camo` textures.
- **Where the paint goes.** On some parts of a weapon, never the whole: the parts (UV islands)
  `scripts/weapon-camo/parts.tsv` names, chosen by hand in the picker page (all 91 textures,
  2026-10-21: 22 by the owner, the rest after them): the body (receiver, covers, handguard,
  stock, grip, magazine); not barrels, muzzles, sights, rails, bolts, triggers, pins, nor a
  magazine inside a pistol's grip. A weapon without a line there gets the suggestion: the large parts its premium painted (each at least 2 % of the
  texture's parts: receiver, covers, handguard, stock, magazine); with no premium (65 textures),
  the large parts, which can take in a barrel (the TOZ-66's): pick those before a release. Small parts (triggers, bolts,
  pins, sights, screws) stay bare. Most premiums were repainted as a whole, so "where the premium
  painted" alone would cover nearly the whole weapon (97 % of the AK-12's parts).
- **Which camouflage.** Any camouflage item (category 25) that goes on armour; one at a time.
- **Slot.** [srp] A weapon's camouflage is in upgrade slot 2: slot 3 holds its scope, 0 and 1 are
  the decal slots (`WEAPON_CAMO_SLOT`, `modifications::camo_slot`). A scope and a camouflage
  are worn together. The client keeps a record in any slot.
- **Shared cosmetics.** As for armour (PR 149): one camouflage item on any number of items,
  weapons and armour mixed, each with its own record; sold, it comes off all.
- **Tiling.** `data/camo_scale` of the weapon config (4.0 for all, the stock bakes' value): the
  camouflage repeats that many times across the weapon's texture. Per weapon, tunable in data.
- **What a match sees.** Every game builds a weapon from its item's upgrades (0x2F14C0); the
  camouflage is baked on each game's own GPU, as armour's. No simulation change: games with and
  without patch 44 play together (without it the weapon is plain).
- **Where it shows.** Wherever a weapon model is built (weapon_cook): the lobby's character, the
  match (first and third person, one model), replays. The weapon shows plain for a moment and
  takes the camouflage when its bake lands (the lobby's decal editor does the same on armour).
- **Cost.** A bake per weapon surface the first time a camouflage is seen on that weapon (cached in
  `Documents/Survarium/streamed_textures/srpw1_*`, the 1 GiB baked-texture cache armour uses):
  three 2048² textures (diffuse, normal, smoothness), about 11 MB on disk and in video memory.

## The engine (client 0.26g, PROVEN unless tagged)

- `camouflage_decal_texture_type_baker_cook` bakes a camouflage into a surface's textures only
  when `<diffuse>_aoac.dds` loads beside its diffuse (0x12E8E9): c = lerp(diffuse, camo(uv ×
  scale), 1 − aoac.G); overlay(c, aoac.B); c × aoac.R (`bake_decal_composition.ps`). Without it
  the surface stays plain, silently.
- A camouflage item's modification adds `camouflage` to the host config's `data` table (modifier
  type 3, path `data`); a host config without `data` drops it (0x2F1430). Weapon configs have no
  `data`: the data patch adds one (holding `camo_scale`).
- Only the character's parts (0x204560 → 0x129830) and the lobby's decal editor (0x2641A0) ask for
  a bake; weapon_cook never did. Patch 44 (`client-patches/weapon_camo_026g.asm`, client_patch.md
  §3.41) asks for one per weapon surface, as the editor does.
- The lobby (`flash/lobby_menu/src/survarium/upgrade/UpgradeWindow.as`): a weapon's upgrade
  window shows the scope slot (index 3, category 22) when the weapon takes scopes, and the
  camouflage slot (decalSlot2, index 2, category 25) when it has `camo_available`; the Upgrade
  button shows on a weapon with either (`isUpgradeableWeapon`).

## The masks (`scripts/weapon-camo/`)

- Donors: the camouflaged textures of the premium weapons in our data (ported from 0.29a3 in
  patch 19) and in 0.28a2's resources.db, on the same UVs as ours (same base texture, or same
  mesh UVs; checked for the Lancaster's ported model).
- Two donors (Uzi, SKS, AKMN, Lancaster, AK-74N wooden stock): texels both leave alike are bare;
  a part is painted when they differ on most of it and each differs from the plain texture
  (uzi_camo_st only tints the Uzi's grips blue: not paint).
- One donor (the other 21): a part is painted when the premium differs from the plain texture on
  a fifth of it or more (bare parts differ on under 6 %, a dark camouflage over a dark part on
  about 44 %).
- From the donors' parts, the suggestion keeps the large ones (`MIN_PART`); parts.tsv overrides
  it per texture (a UV point on each painted part; a point on no part stops the build).
- The picker (`weapon_camo.py --picker page.html --camos <items>`): each texture with its parts
  outlined and the weapon in 3D beside it, the painted parts under the camouflage as the bake
  makes it, any of the game's 50 camouflage items; a click on either paints a part or leaves it
  bare; the premium texture can be shown beneath; the page gives the parts.tsv lines (kept in the
  browser meanwhile).
- The aoac: G the mask, B the plain texture's fine shading on painted parts (×4, 0.5 elsewhere),
  R its broad shading (to 0.3, 1 elsewhere), so the camouflage keeps the part's wear and edges.
  DXT1 with mips, the stock files' header.

## Checked

- `client-patches/check_weapon_camo_026g.py` (Unicorn): the hook site and the code on made-up
  configs and models, every case (selftest check 44 for the bytes).
- 2026-10-20, a local game (patch revision 25 build, a test patch of this data, a server of this
  branch, headless Xorg dummy at 60 Hz): an account with the City camouflage in slot 2 of an
  AK-12, then an Uzi: the lobby baked `srpw1_character_human_camo_premium_city_camo_weapons_ak_12_ak_12_0{d,n,s}`
  and the Uzi's, and the character's weapon showed the camouflage. The server kept the slot-2
  records at load.

## Not done

- Decals on weapons (the owner, 2026-10-21: "it should be nice if I could put a decal on the
  weapon too"): the decal editor places decals on the character; weapons need it extended (exe).
- A match (first and third person) not yet seen.
- The other 69 textures' picks were made after the owner's 22, by looking (the MP7 black copies the owner's MP7): worth a pass in the picker.
