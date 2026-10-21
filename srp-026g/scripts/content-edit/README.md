# content-edit: game data changes as resources.db patches

`content_edit.py` makes a patch folder by editing the live game configs in place:
only the bytes of the edited values change (a renamed key's table is re-sorted by
name CRC, as the game's lookup needs; new names and strings go after the file's
last byte), and every edited file is read back and compared with the live one
value by value. It refuses a value whose live number or card text is not the one
the edit list expects, so an edit list made for one release cannot silently land
on other data.

## Making the patch

An edit list is live when it has its row in `release/content.tsv` (`edit<TAB><file>.json<TAB>-<TAB><what>`,
after the edit rows already there: they are applied in that order). `release/build-content
<revision>` applies every one to the files as the base patches and the packs leave them (so the
`from` values are those, never a later release's), compares the results with what players have,
and packs only the files that differ into `resources.db.patch_<revision>`, with the server game data
(`release/build-content --help`). Nothing is put together by hand; a retune is a change
of the list, built again from the same base. Any file of the game's archives can be edited, not
only `gameplay/` (the server game data holds only the files srp reads).

`content_edit.py` runs on its own for a check (`--check`) on a folder of files at their game paths
(`srp-game-data get --db <resources.db> --patch <patch>... --out live/ <path>...`).

Then run `weapon_balance_1_changes_only_its_values`
(`crates/survarium/src/v026g/maps.rs`, ignored: it needs `SRP_TEST_RESOURCES_DB`,
`SRP_TEST_SKHID_PATCHES` and `SRP_TEST_PATCH_WEAPON_BALANCE_1`) when a change touches its files. The
server game data carries no localization tables: the test reads the texts in the patches themselves.

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

The patch carries the `_aoac` textures of `../weapon-camo/weapon_camo.py` beside these configs (the
content pack `weapon-camo@1`, `release/content.tsv`). Applied to revision 24's live configs (server
game data): 83 files edited, 0 unexpected differences; in `release/build-content 27` (2026-10-21,
after golden-weapons-1 and the other lists of the manifest): 83 files edited, 0 unexpected
differences.

## weapon-balance-2.json (2026-10-20)

| change | why |
|---|---|
| AK-12 and premium AK-12 (`wpn_ak_12`, `wpn_ak_12_premium`): `bullet_damage` 0.55 -> 0.5, `bullet_pierce` 0.45 -> 0.4 (f32 kept); the card's damage (prop 1) 55 -> 50 and armour piercing (prop 3) 45 -> 40 | the owner: "0.27a (released 3 months after 0.26a where ak12 was introduced) nerfed ak12 damage 55 » 50 and armour penetration 45 » 40. I think it should be done here"; then, shown the table below: "50 35 for ak-12" (piercing between 0.27d0's 40 and 0.28a2's 30; 0.28a2 rebalanced the whole game, so only the AK-12's own change is taken); 2026-10-21: "ak12 to have 40 penet" |
| premium AK-12: `default_modifications` [1230, 2044, 2094] -> [1230] (a new edit, `remove_item`) | the owner: a premium weapon gives silver and nothing else. 1230 is +10 % silver; 2044 (reload time -15 %) and 2094 (reliability +25 %) go. The defaults are not on the wire: the client and srp both read them from the config |
| Socom16 (`wpn_socom16`): `bullet_damage` 0.8 -> 0.7; the card's damage 80 -> 70 | the owner: "and also Socom16 75 → 50" (0.28a2's cut); then, shown the table below: "seems like socom is nerfed too much, let's make it 60" (0.31a2's value); 2026-10-21: "socom16 is fine if it has 70 damage". Live has 0.26g's 0.8 (0.27d0 had cut it to 0.75). Piercing stays 0.45: 0.28a2 also cut it to 0.35, the owner named the damage only |
| A-545 (`wpn_a545`): `bullet_damage` 0.5 -> 0.45, `bullet_pierce` 0.3 -> 0.25, `rounds_per_minute` 900 -> 800; the card's props 1, 3, 4 to match | the owner, 2026-10-21, shown its history: yes to 0.28a's nerf (0.28a2's values; its recoil change not taken) |

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
| **here** | 0.50 / 0.40 | 0.50 / 0.40 | 0.70 / 0.45 |

Applied to revision 24's server game data (`gameplay/items/weapons/wpn_ak_12*`,
`wpn_socom16`; patch 25 did not touch weapons): 3 files edited, 0 unexpected differences.

## grenade-bounce-1.json (2026-10-21)

| change | why |
|---|---|
| The nine grenade bounce samples (`sounds/single/weapons/f1/grenade_hit_{ground,wood,metal}_0{1,2,3}.raw_options`, the grenade material's collision sounds in `game_materials/material.pairs`): `spl_preset` `weapons_reloading` -> `mid_range`; `volume_k` (0.5 / 0.4 / 0.3) and `spl_value` (20) stay stock | the owner: "make grenade bounces louder". They had a reload click's distance curve (silent from 9 m, 0.03 at 5 m on ground); now 0.15 at 5 m, 0.05 at 10 m, silent from 20 m, and within 1 m only +2 to +3 dB. Retuned after the owner's test of revision 27, where the first version (`material_hit`, `volume_k` doubled, `spl_value` 75) was "too loud, and almost like they use different sound". A preset is two distance curves and nothing else, so the sample sounds the same. Only the grenade pairs use these samples (`docs/notes/grenade_bounce.md`) |

Applied to the stock resources.db's nine `.raw_options` (no Skhid patch up to 27 carries them):
9 files edited, 0 unexpected differences. Sound only: no simulation change, nothing in the
server game data.

## golden-weapons-1.json (2026-10-21)

| change | why |
|---|---|
| The 22 golden weapons (`wpn_*_premium`, `wpn_*_premium_2`, `wpn_neostead`: every weapon whose first default modification is 1230, +10 % silver; and `wpn_mp7_silencer_special`, default 2004, the owner: it joins them): every `default_modifications` item taken off (`remove_item`; the table stays, empty, as the empty `additional_model_slots` of other item configs), `parameters/base_rarity_level` 3 -> 0 | the owner: "remove modifiers except for silver from all premium weapons", upgradable "just like any other weapon but with the first golden bonus already set", "modifications on premium weapons can be rolled too (even silver)". srp gives each golden weapon 1230 as a modification of its own, the first ([`../../docs/notes/golden_weapons.md`](../../docs/notes/golden_weapons.md)): the client never offers a default for a reroll, and the lobby's rank is `min(3, base_rarity_level + modifications)`, so a base of 3 forbade every craft |

Applied to revision 26's server game data (`gameplay/items/weapons/`): 22 files edited, 0
unexpected differences; server data built with it and the patches before passes `srp-check-game-data`. srp and
`release/build-server-db` refuse data without it (`golden::check`). The patch also
carries the lobby movie built from the tree (the craft result shows the item upgraded, not the donor).

`wpn_ak_12_premium.options` is also weapon-balance-2's: a patch carries a file whole, so in one
release apply this list to weapon-balance-2's output, not to the live file beside it (checked on
revision 24's files: weapon-balance-2, then this list over its output: 22 files edited, 0
unexpected differences).

## death-poses-1.json (2026-10-21)

| change | why |
|---|---|
| The face-down death pose out of the weapons' death animations (`remove_item` of a string, in `death` and in `death_hud`, the first-person list the game indexes with the same number): `rifles.user_animations` ak `death/on_site_back` and `death/on_site_fwd` (4 -> 2: `on_site_left`, `on_site_right`), `pistols_new.user_animations` tt33 `death/on_site_back` (2 -> 1: `on_site_left`) | the owner, testing the death camera (patch 46): "we need to remove one death pose which makes head look into arms"; the three end in that one pose, the arms in 16 % of the eyes' look cone, every other pose in none ([`../../docs/notes/death-poses.md`](../../docs/notes/death-poses.md)). The pick is the simulation's random over the list (`player_logic_dead_state`, 0x338330): it picks among the rest |

Applied to the stock resources.db's two lists (no Skhid patch up to 27 carries them): 2 files
edited, 0 unexpected differences. Nothing in the server game data (srp reads neither file).

## crouch-steps-1.json (2026-10-21)

| change | why |
|---|---|
| The 80 crouch pairs of `game_materials/material.pairs` (foot materials `actor_crouch` 54, heard by others, and `actor_local_crouch` 57, heard by the player himself; every boots config gives them to crouch walking): `collision_sound` "" -> a copy of what the walking pair of the same ground plays (54 takes `actor` 10's, 57 takes `actor_local` 42's), `collision_sound_type` its kind (258 collection, 257 composite). 67 pairs get a sound; the 13 whose walking pair is silent stay silent | the owner: "when crouched player should still make sound, right now they don't at all, it should be lower volume (don't know by how much) but not 0". The pairs were empty, so `step_manager::on_step` played nothing (`docs/notes/crouch_steps.md`) |
| The copies: every walking step sample (`sounds/single/material/step/<ground>/`, 22 folders, 506 samples) at `sounds/single/crouch/...`, `volume_k` x 0.4 (-8 dB; the distance curve kept); the 43 collections and 16 composites the walking pairs play at `sounds/collections/crouch/...` and `sounds/composite/crouch/...`, their filenames prefixed `crouch/` | a pair has no volume of its own, a sample's options do: the same steps at 40 %. 0.31a2 (which added crouch steps) is at 58 % of its walking at 1 m and about 10 % from 5 m |

New edits for it: `copy` (a live file also at a new path), `scale` (a number of many files times a
factor), `prefix_string` and `set_entry` (an indexed table's entry chosen by its keys, here a
pair by its two materials). Applied to the stock resources.db's files (no Skhid patch up to 26 carries
them): 1,578 files edited (1,577 new, 1,012 of them byte for byte), 0 unexpected differences;
every new name a pair, collection or composite gives resolves to a new file. Sound only: no
simulation change, nothing in the server game data.
## lobby-bench-light-1.json (2026-10-21)

| change | why |
|---|---|
| `projects/lobby_scene/day/client_project` (the lobby's scene): `client_objects[49]` (`ambient_light(3)`, at (4.929, 0.006, 4.347) on the floor under the workbench's end, colour 0xFFB3856B) `intensity` 1 (an int) -> 2.5 (a float, as `ambient_light(23)`'s 2.2), `radius` 3.1 -> 3.6 m | the owner, of patch 45's weapon above the workbench (`../../docs/notes/client_patch.md` section 3.42): "the lamp under the table should give more light, since now it is somewhat dark". It is the one light by the bench: the room's others are the ceiling point light at (1.672, 2.742, -0.643), 4.7 m away, and a blue spot (`back2`) at the far corner; the weapon at (4.25, 1.30, 3.05) is 1.95 m from it, now well inside its radius |

The scene is the stock `resources.db`'s (no patch has it). Applied to the stock file
(sha256 `e93de4fc…c0f8`): 1 file edited, 0 unexpected differences, 5 bytes changed; the result
sha256 `3fa6be5e…7062` (`release/build-content 27`, 2026-10-21: the same). Rendering only: no server
data, no simulation change. Not seen in a game:
how much brighter 2.5 is (the engine's ambient light is a fill light with a radius; the value is
a first guess to tune).

## weapon-decals-1.json (2026-10-21)

| change | why |
|---|---|
| The 83 weapon configs of weapon-camo-1: `parameters/decal_available` true and an empty `data/decals` table | decals on weapons (client patch 47, `../../docs/notes/weapon-camo.md`): the lobby shows a weapon's one decal slot for `decal_available` (the client keeps three upgrades an item: scope, camouflage, decal), the server lets a decal into its slot 0, and a decal upgrade appends its texture and place to `data/decals` (a modifier at a path the config lacks is skipped), which patch 47 bakes |

Its manifest row follows weapon-camo-1's: it edits that list's output (it needs the `data` table).
Checked 2026-10-21 on the local game's server data: 83 files edited by each list, 0 unexpected
differences; the AK-12's `data` holds `decals[0]` and `camo_scale`, the type and count armour's
`data/decals` has. In `release/build-content 27` (2026-10-21): 83 files edited, 0 unexpected
differences.

## weapon-attachments-1.json (2026-10-21)

| change | why |
|---|---|
| A new text `st_srp_attachments` in the six localization tables (Attachments, Anbauteile, Dodatki, Обвес in russian and original, Обвіс in ukrainian), and the lobby movie's key `scope_window_title` mapped to it in `localization/flash_movies_localizations.options` (was patch 19's `st_srp_optics`, "Optics", which nothing else uses) | the owner: the weapon's window "should also be not called optics and be called attachments"; its button and the title of its Attachments page say it (`../../docs/notes/weapon-camo.md`, the weapon's window: two pages). The Appearance page uses armour's `btn_upgrade_label` as it is |

Applied to the latest files the local game mounts (patch 23's english, german, original, polish and
russian tables, patch 25's ukrainian, patch 19's `flash_movies_localizations.options`): 7 files
edited, 0 unexpected differences; in `release/build-content 27` (2026-10-21, after the packs'
tables and duty-name-uk-1): the same. Text only; the lobby movie of the same change is built from
its tree.
