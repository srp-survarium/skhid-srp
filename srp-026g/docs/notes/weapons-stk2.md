# The STK2 AK-74s (packs weapons-stk2@2, weapons-stk2-camo@1, weapons-stk2-scopes@1)

STK2 (the cancelled 2012 S.T.A.L.K.E.R. 2, `/mnt/toshiba/surv-builds/STK2.7z`, `xr2-prog`) has one
weapon, a modernised tactical AK-74, as three assembled, textured static models. Two of them are
new guns here. Built by `scripts/weapons-stk2/make_pack.sh` with resource-porter's shared weapon
core (`port-weapons`, donor build `stk2`, `ports/weapons-stk2.json`).

## The owner's rules (2026-11-21)

- Two new weapons: `ak74_4tir2` (long barrel, collapsible AR-style stock, rails, no optic) and
  `ak74_4tir1` (short barrel, side-folding skeleton stock), its built-in red dot cut off: it ships
  without an optic. Not `ak74_4tir3` (tir2 with a scope and a bipod).
- Both take every optic the AK-74M takes, through the scope pipeline in use.
- Under every optic, collimators included, the iron sights fold or disappear, the 0.69 way.
- Animations: the live AK-74M's own 0.26g first- and third-person clips (same family: the grip and
  the magazine are close); where the hands miss this gun, fix it properly offline (the model placed
  on the AK-74M's rig first, then clip corrections on the real skeleton), never by guessing in game.
- STK2's own red dot and scope as new scope items: a possible follow-up, not this work.

## The coordinator's picks (the owner may override)

- Names: "AK-74 Tactical" (tir2) and "AK-74 Tactical Carbine" (tir1), in the eleven localizations.
- Stats, faction, level and price: the AK-74M's for both (Renaissance, level 10, 47,500); the
  carbine's shorter barrel takes the short-barrel spread and fall-off the AKMSU took from the
  AKS-74U (spread 0.2 -> 0.3, full damage to 15 m, falling to 40 m; the AK-74M 20-50 m).
- The AK-74M's sounds and its 5.45 rounds (7N10 and 7N22).

## The items

| Item | Dict id | Config | Lobby cell | Kill feed | STK2 model |
|---|---|---|---|---|---|
| AK-74 Tactical | 2296 | `wpn_ak74_tactical` | weapons [11, 6] | 100 | `ak74_4tir2` |
| AK-74 Tactical Carbine | 2297 | `wpn_ak74_tactical_carbine` | weapons [12, 6] | 101 | `ak74_4tir1` |

- Texts `st_srp_weaponstk2_<name>_name/_desc` in the eleven tables of `client-patches/localization`.
- Lobby icons: right-side profiles of the guns' own models as the other ported guns' (no build drew
  one), their brightness the AK-74M's retail icon's ([10, 2]): `scripts/weapons-stk2/icons.py`,
  in the porter's `ports/icons/stk2/`.
- Kill feed: no build has one either; drawn from the lobby cells as the Colt M1911's
  (`scripts/kill-feed-icons/make_weapon_icons_stk2.py`), then the lobby's tooltips' copies.
- Stats in `content-edit/weapon-stats.tsv` (the AK-74M's row; the carbine's spread and distances),
  price in `items.tsv`; sold by Renaissance at the rank of level 10 as the AK-74M (no shop row).

## The models (`scripts/weapons-stk2/donor.py`, `guns.json`)

- STK2 draws this gun in the AK-74M's own frame: same origin, scale and axes; the receivers,
  magazines, pistol grips and trigger guards coincide within millimetres (checked by overlay). So
  every STK2 part keeps its place, and each mesh island goes on the AK-74M bone that moves it
  (`guns.json` `bones`): the magazine with its rounds (and the carbine's coupled second magazine) on
  `magazin`, the bolt carrier and its handle on `zatvor`, the trigger on `nab`, the magazine catch
  on `safety_catch`, the rest on `offset`. The AK-74M's skeleton, bind pose and hand and finger
  locators as they are.
- The eye (`aim`) on the line from the rear sight's notch to the front post, at the AK-74M's eye's
  place along the bore; the muzzle (`barrel_point`) at STK2's.
- Materials: the AK-74M's metal and polymer ones, their textures STK2's: diffuse, the RGB tangent
  normal map as 0.26g's DXT5nm, and STK2's one specular map as both the fresnel and the roughness
  map, each matched to the AK-74M's own map's distribution.
- The textures in the stock cooked layout (@2): mips down to 4x4, the header's PITCH and
  LINEARSIZE flags with the size of all after the header, and the trailing sRGB byte (1 on the
  diffuse) the loader takes off the end. @1 had ImageMagick's DDS (mips on to 1x1, the top level's
  size, no sRGB byte): ordinary readers draw it right, but the game read its small mips 15 bytes off
  their blocks, and the gun showed lime, pink, blue and purple blotches in the lobby (the owner's
  screenshot, 2026-11-21; `mip_preview.py` draws both readings). Only the 20 textures differ from
  @1; the camouflage masks and the scope fits are unchanged.
- The item config: the stock AK-74M's with the live AK-74M's two-clip reload (pack
  tactical-clips-029's), so the guns play the AK-74M's clips, sounds and reload timing
  (`reload-timings-1` retimes them for each gun from `weapon-stats.tsv` as for the AK-74M).

## The hands (`scripts/weapons-stk2/preview.py`)

- The guns play the AK-74M's clips unchanged: on the AK-74M's rig, STK2's magazine, magazine well,
  charging handle, pistol grip and handguard are where the AK-74M's are, and the AK-74M's clips put
  the hands on the AK-74M's contact bones, which the guns keep.
- Measured offline on the real skeleton (the hud hands posed as the game poses them: the state's
  hands clip additive on the rifles' stance, the gun on `Weapon`), per frame of the idle, aim, draw
  and both reloads: how deep each palm goes into the gun. The live AK-74M's own is 8 to 13 mm (the
  hud fingers close into its furniture); the new guns' within 6 mm of it, the left palm a few mm
  deeper on the wider rail handguard. No frame has a hand off the gun or a part the AK-74M lacks in
  a hand's way (the carbine's light is ahead of the left hand), so no clip is corrected.
- The reload: `reload-timings-1` retimes the guns' two clips into the same bytes as the AK-74M's
  (`srp_reload/wpn_ak74_tactical*/` equal to `srp_reload/wpn_ak_74m/`).

## Scope fits (`scripts/weapons-stk2/scope_fits.py`)

- Each of the AK-74M's 18 variants made again for each gun (36, dict ids 4700 on, `variants.tsv`).
- The optic is the AK-74M's fit of it, less the AK-74M's dovetail adapter with its rail: it stands
  on the gun's own top rail (STK2's side mount), moved with the gun's eye by the two rails' tops
  (4.2 mm down) and centres, so optic, reticle and eye keep their places on each other.
- The two PSO-1s clamp to the AK's dovetail on the AK-74M; the receivers coincide, so they stay
  where they are, and STK2's side mount with its top rail comes off under them.
- Under every optic the iron sights go: the rear sight's leaf and slider, the long gun's flip-up
  front sight, the carbine's front post and ears (its sight block's top made flat).
- `content-edit/weapons-stk2-scopes-1.json`: the guns' `addons` placeholder and the lobby's rows of
  gun and shop optic.

## Not in these packs

- STK2's red dot and scope as optics of their own.
- A chambered round: STK2's mesh has none, so the AK-74M's `patron` bone moves nothing here.
- The materials name the AK-74M's unused `weapons/vityaz/vityaz_spec` slots, as the AK-74M's do
  (no archive has that texture; the port reports it).
- In-game play: the previews (`private/renders/stk2-ak74/`) are offline renders for the owner's
  look first.
