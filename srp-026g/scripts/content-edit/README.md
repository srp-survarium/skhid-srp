# content-edit: game data changes as resources.db patches

`content_edit.py` makes a patch folder by editing the live game configs in place:
only the bytes of the edited values change (a renamed key's table is re-sorted by
name CRC, as the game's lookup needs; new names and strings go after the file's
last byte), and every edited file is read back and compared with the live one
value by value. It refuses a value whose live number or card text is not the one
the edit list expects, so an edit list made for one release cannot silently land
on other data.

## Making the patch

```bash
# the live files: the server game data the live server runs on holds the gameplay files;
# other files (materials) come from the newest offered patch that has them
dbgrab server-game-data.db get gameplay/items/ live/      # tools/re-kit/common/dbgrab
dbgrab resources.db.patch_19 get collimator_glowing_dot_ x/ && cp x/*.material live/
dbgrab resources.db.patch_20 get localization/ live/
python3 -I content_edit.py weapon-balance-1.json live/ folder/
cp lobby_menu.swf folder/flash_movies/                     # weapon balance 1: the movie with the ninth card row
release/build-patch folder/ <revision>                    # resources.db.patch_<revision>
```

Then build the server data with it (`srp-game-data build`, or `release/release.sh`
with the patch) and run `weapon_balance_1_changes_only_its_values`
(`crates/survarium/src/v026g/maps.rs`, ignored: it needs `SRP_TEST_RESOURCES_DB`,
`SRP_TEST_SKHID_PATCHES` and `SRP_TEST_PATCH_WEAPON_BALANCE_1`). The server game data
carries no localization tables: the test reads the texts in the patches themselves.

## weapon-balance-1.json (2026-10-18)

Scored against the recorded live matches since revision 19 (about 10,000 gun kills):
each victim's real armour, the killer's ammo and the distance, through the damage
chain read in the exe (damage x (1 - clamp(armour - piercing, 0, 1)), both falling
to 60 % between the weapon's two damage distances). "Kill index" = kills with the
weapon / kills the same players get with their other weapons.

| change | why |
|---|---|
| Armour piercing of the 0.29a3 weapons on the 0.26g scale, by calibre: 5.56 down (HK416 0.45 -> 0.34, M4A1 and MSBS 0.42 -> 0.32, FNC 0.40 -> 0.30), MP7 and P90 0.55 -> 0.30, KRISS 0.45 -> 0.25, PKP 0.60 -> 0.45; 7.62x39 up to the AKM's 0.40 (vz. 58, Valmet, SOK-94 0.30); 9x18 up to the Kiparis's 0.20 (Kedr, AEK-919K 0.10) | the port brought damage back to 0.26g's scale but kept 0.29a3's piercing, a different scale per calibre (0.29a3 moved wall penetration into a key 0.26g does not read); piercing also decides wall penetration |
| MP7 damage 0.35 -> 0.40, FNC 0.45 -> 0.50 | keeps two guns that played average or weak (kill index 0.9, 0.75) where they were after the piercing fix |
| KRISS damage 0.45 -> 0.42 | kill index 1.22 (1.09-1.36) |
| SR-2M 900 -> 800 rounds a minute | kill index 1.33, the best accuracy of the automatics |
| SR-3M (and premium) 900 -> 800 rounds a minute, damage 0.55 -> 0.45 | the fastest automatic in the recorded kills, kill index 1.22 |
| AS Val, 9A-91, VSS, VSK-94 (and premiums) damage 0.55 -> 0.50 | the same 9x39 numbers as the SR-3M |
| PP-2000 750 -> 680 rounds a minute | kill index 1.22 |
| Mosin damage 1.0 -> 1.1, piercing 0.35 -> 0.40; TOZ-122 damage 1.1 -> 1.2 | the two beginner rifles played weakest (kill index 0.54, 0.66) |
| SVD magazine 10 -> 5 rounds, and its description in the six localization tables ("10 rounds" -> "5 rounds") | a mild cut for a rifle on the watch list: its rate of fire and damage stay, half the shots between reloads (the Mosin, same ammunition, holds 5); the card has no magazine line; srp's bots read the value from the data |
| `stamina_regenation_speed_modifier` -> `stamina_regeneration_speed_modifier` in every item config and in the modifications' paths (5 modifications, 1 defect) | the exe reads only the correct spelling (name table 0x928448 entry 3); the items' stamina-regeneration modifications and defect did nothing |
| the three glowing-dot materials of the sights ported from 0.51c5 (`material_instances/weapons/loc/collimator_glowing_dot_red`, `loc2/collimator_glowing_dot_red`, `loc2/collimator_glowing_dot_green`): `constant_transparency` 1 -> 0.3 | 0.51c5's value: the round-12 material merge of the scope port dropped it because its kind differed (float in 0.51c5, int 1 in the template), so every ported red-dot and ring reticle (Aimpoint, Cobra, Docter, EOTech, DeltaPoint) glowed 3.3 times brighter than in 0.51c5; the 0.29a3 stock sights are unchanged |

| a magazine line on every weapon card (83): `ui_properties` line 134 "Magazine" (a new text in the six localization tables), a `props_list` entry after the rate of fire with the weapon's `magazine_capacity` | the card did not say how many rounds a magazine holds; it draws eight comparable lines and every weapon had eight, so the patch also carries the lobby movie with a ninth row (`srp-026g/docs/notes/items.md` §7) |

The lobby card shows text kept in each config (`ui_desc/props_list`), not the
values: the list changes that text with each value (`set_prop`).

The movie (`flash_movies/lobby_menu.swf`, 40,009,816 bytes, sha256 `b4c1a4b7…4fe7`) is patch 20's
(sha256 `f21eac03…e117`) with two tags changed and every other tag byte-identical: sprite 1538
(`SrvItemCompareHintG`) places a ninth row `diff_9` (sprite 1537, depth 33, 22.6 px under `diff_8`),
and `survarium.controls.ItemCompare`'s constructor, re-imported as FFDec p-code, adds
`getChildByName("diff_9")` to its rows when there is one.

## backpacks-1.json (2026-10-19)

| change | why |
|---|---|
| The six backpacks (`bm_back_1` Marauder, `scavenger_back_1`, `ren_back_1`, `edge_back_1`, `edge_back_5`, `edge_back_6`) may also roll health regeneration: modification type 21 (+5 % to +15 %, ids 1090-1094), character 20 of `parameters/available_modifications_mask` set (`0000000000000011110000010000100100000000` -> `0000000000000011110010010000100100000000`) | they rolled three types only (weight, carried weight, durability); the owner's choice of a fourth. Only srp reads the masks (the client draws whatever modification the server sends), so the patch changes what the server rolls on upgrades, rerolls and drops. The oxygen tanks and the exo back pieces keep theirs |

Applied to revision 22's live files: 6 files edited, 0 unexpected differences. Weapon balance 1 is in patch 22, so nothing else waits for the same files.

## duty-name-uk-1.json (2026-10-20)

| change | why |
|---|---|
| Ukrainian: `"Обов'язку"` -> `"Долгу"` in the seven Duty exo texts (`st_duty_026_suit_name`, `_head_desc`, `_helmet_name`, `_gloves_name`, `_gloves_desc`, `_boots_name`, `_boots_desc`) | the owner's: the faction is called Долг in Ukrainian too. The plain word обов'язок elsewhere (`st_edge_steal_box_descr`, `st_edge_defend_items_descr`) stays |

Applied to revision 24's live `localization/ukrainian/localization` (from the server game data): 1 file edited, 0 unexpected differences. Text only: no simulation change.

## battery-pickup-1.json (2026-10-20)

| change | why |
|---|---|
| `gameplay/victory_items/default.options` `states[0]/take_time` 0.6 -> 0.5 (an indexed table's item: the `set` path's `name[i]`) | the owner: the battery pick-up animation 20 % faster (1.2 times the speed). The take state time-scales its animation to last `take_time`, so it plays faster, not cut (`docs/notes/battery_pickup.md`); `put_time` stays 0.6 |

Applied to revision 24's server game data (`gameplay/victory_items/`): 1 file edited, 0 unexpected differences.

## weapon-camo-1.json (2026-10-20)

| change | why |
|---|---|
| all 83 weapon configs (every weapon one of whose surfaces has an `_aoac` from `../weapon-camo/guns.tsv`, premium copies included): `parameters/camo_available` true, and a new `data` table holding `camo_scale` 4.0 (the `add_value` edit: a new key in its place by name CRC) | the owner's: armour camouflages on weapons (`docs/notes/weapon-camo.md`). The lobby, the server (a camouflage into slot 2 of a weapon) and exe patch 44 key on `camo_available`; a camouflage's modification writes `data/camouflage` into a `data` table the host config has (none had one); patch 44 bakes with `data/camo_scale` |

The patch carries the `_aoac` textures of `../weapon-camo/weapon_camo.py` beside these configs.
Applied to revision 24's live configs (server game data): 83 files edited, 0 unexpected differences.

## weapon-balance-2.json (2026-10-20)

| change | why |
|---|---|
| AK-12 and premium AK-12 (`wpn_ak_12`, `wpn_ak_12_premium`): `bullet_damage` 0.55 -> 0.5, `bullet_pierce` 0.45 -> 0.35 (f32 kept); the card's damage (prop 1) 55 -> 50 and armour piercing (prop 3) 45 -> 35 | the owner: "0.27a (released 3 months after 0.26a where ak12 was introduced) nerfed ak12 damage 55 » 50 and armour penetration 45 » 40. I think it should be done here"; then, shown the table below: "50 35 for ak-12" (piercing between 0.27d0's 40 and 0.28a2's 30; 0.28a2 rebalanced the whole game, so only the AK-12's own change is taken) |
| premium AK-12: `default_modifications` [1230, 2044, 2094] -> [1230] (a new edit, `remove_item`) | the owner: a premium weapon gives silver and nothing else. 1230 is +10 % silver; 2044 (reload time -15 %) and 2094 (reliability +25 %) go. The defaults are not on the wire: the client and srp both read them from the config |
| Socom16 (`wpn_socom16`): `bullet_damage` 0.8 -> 0.6; the card's damage 80 -> 60 | the owner: "and also Socom16 75 → 50" (0.28a2's cut); then, shown the table below: "seems like socom is nerfed too much, let's make it 60" (0.31a2's value). Live has 0.26g's 0.8 (0.27d0 had cut it to 0.75). Piercing stays 0.45: 0.28a2 also cut it to 0.35, the owner named the damage only |

The two weapons in the builds at hand (damage / piercing; the card shows them x100):

| build | AK-12 | premium AK-12 | Socom16 |
|---|---|---|---|
| 0.26g stock | 0.55 / 0.45 | - | 0.80 / 0.45 |
| live, revision 25 | 0.55 / 0.45 | 0.55 / 0.45 | 0.80 / 0.45 |
| 0.27d0 (no 0.27a at hand) | 0.55 / 0.40 | 0.50 / 0.40 | 0.75 / 0.45 |
| 0.28a2 | 0.50 / 0.30 | 0.45 / 0.30 | 0.50 / 0.35 |
| 0.28d0 | 0.45 / 0.30 | 0.45 / 0.30 | 0.50 / 0.35 |
| 0.31a2 | 0.35 / 0.55 | 0.35 / 0.55 | 0.60 / 0.40 |
| 0.46c2 (`material_pierce` 6, another scale) | 0.26 / - | 0.26 / - | 0.65 / - |
| **here** | 0.50 / 0.35 | 0.50 / 0.35 | 0.60 / 0.45 |

Applied to revision 24's server game data (`gameplay/items/weapons/wpn_ak_12*`,
`wpn_socom16`; patch 25 did not touch weapons): 3 files edited, 0 unexpected differences.

## lobby-bench-light-1.json (2026-10-21)

| change | why |
|---|---|
| `projects/lobby_scene/day/client_project` (the lobby's scene): `client_objects[49]` (`ambient_light(3)`, at (4.929, 0.006, 4.347) on the floor under the workbench's end, colour 0xFFB3856B) `intensity` 1 (an int) -> 2.5 (a float, as `ambient_light(23)`'s 2.2), `radius` 3.1 -> 3.6 m | the owner, of patch 45's weapon above the workbench (`../../docs/notes/client_patch.md` section 3.42): "the lamp under the table should give more light, since now it is somewhat dark". It is the one light by the bench: the room's others are the ceiling point light at (1.672, 2.742, -0.643), 4.7 m away, and a blue spot (`back2`) at the far corner; the weapon at (4.25, 1.30, 3.05) is 1.95 m from it, now well inside its radius |

The scene is the stock `resources.db`'s (no patch has it): take it with `dbgrab resources.db get
projects/lobby_scene/day/client_project live/` (flattened names are fine) and
`python3 -I content_edit.py lobby-bench-light-1.json live/ folder/`. Applied to the stock file
(sha256 `e93de4fc…c0f8`): 1 file edited, 0 unexpected differences, 5 bytes changed; the result
sha256 `3fa6be5e…7062`. Rendering only: no server data, no simulation change. Not seen in a game:
how much brighter 2.5 is (the engine's ambient light is a fill light with a radius; the value is
a first guess to tune).

## weapon-decals-1.json (2026-10-21)

| change | why |
|---|---|
| The 83 weapon configs of weapon-camo-1: `parameters/decal_available` true and an empty `data/decals` table | decals on weapons (client patch 46, `../../docs/notes/weapon-camo.md`): the lobby shows a weapon's two decal slots for `decal_available`, the server lets decals into its slots 0 and 1, and a decal upgrade appends its texture and place to `data/decals` (a modifier at a path the config lacks is skipped), which patch 46 bakes |

Applied after weapon-camo-1, to its output: `content_edit.py weapon-camo-1.json live/ camo/`, then
`content_edit.py weapon-decals-1.json camo/ decals/` and `cp -r decals/* camo/` (the second list
writes the 83 configs again). Checked 2026-10-21 on the local game's server data (revision 27's
files): 83 files edited by each list, 0 unexpected differences; the AK-12's `data` holds
`decals[0]` and `camo_scale`, the type and count armour's `data/decals` has.

## weapon-attachments-1.json (2026-10-21)

| change | why |
|---|---|
| A new text `st_srp_attachments` in the six localization tables (Attachments, Anbauteile, Dodatki, Обвес in russian and original, Обвіс in ukrainian), and the lobby movie's key `scope_window_title` mapped to it in `localization/flash_movies_localizations.options` (was patch 19's `st_srp_optics`, "Optics", which nothing else uses) | the owner: the weapon's window "should also be not called optics and be called attachments"; its button and the title of its Attachments page say it (`../../docs/notes/weapon-camo.md`, the weapon's window: two pages). The Appearance page uses armour's `btn_upgrade_label` as it is |

Applied to the latest files the local game mounts (patch 23's english, german, original, polish and
russian tables, patch 25's ukrainian, patch 19's `flash_movies_localizations.options`): 7 files
edited, 0 unexpected differences. Text only; the patch also needs the lobby movie of the same change.
