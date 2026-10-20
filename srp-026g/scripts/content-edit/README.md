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
