# content-edit: game data changes as resources.db patches

Native dictionary text is maintained in
[`client-patches/localization/*.tsv`](../../client-patches/localization/README.md),
one complete file per language. `localization-1.json`, the final manifest edit,
compiles them all. The text-only edit lists described historically below have
been folded into those TSVs; add new item names and descriptions there in every
language, then run `python3 srp-026g/client-patches/check_localization_026g.py`
from the repository root. Gameplay values and UI key mappings remain edit lists; srp's own
menu texts (`st_srp_*`) need no mapping: the compile maps each for the lobby and login movies.

`content_edit.py` makes a patch folder by editing the live game configs in place:
only the bytes of the edited values change (a renamed key's table is re-sorted by
name CRC, as the game's lookup needs; new names and strings go after the file's
last byte), and every edited file is read back and compared with the live one
value by value. It refuses a value whose live number or card text is not the one
the edit list expects, so an edit list made for one release cannot silently land
on other data.

## Making the patch

An edit list is live when it has its row in `release/content.tsv` (`edit<TAB><file>.json<TAB>-<TAB><what>`,
after the other relevant edit rows, but before the final localization catalog edit:
they are applied in that order). `release/build-content
<revision>` applies every one to the files as the base patches and the packs leave them (so the
`from` values are those, never a later release's), compares the results with what players have,
and packs only the files that differ into `resources.db.patch_<revision>`, with the server game data
(`release/build-content --help`). Nothing is put together by hand; a retune is a change
of the list, built again from the same base. Any file of the game's archives can be edited, not
only `gameplay/` (the server game data holds only the files srp reads).

`content_edit.py` runs on its own for a check (`--check`) on a folder of files at their game paths
(`srp-game-data get --db <resources.db> --patch <patch>... --out live/ <path>...`).

### Headwear render assets

`headwear-1.json` uses `append_headwear_bind` (`file`) and `rigid_headwear`
(`file`, expected `vertices`). `headwear.py` checks the native bind order,
skin layout, weights and index bounds. `require_headwear_bind` checks the
actual `body_parts/bind_pose` reference in `gameplay/bodyparts/default`
against the binds being extended, so the assembled character gets the
extra bone. It appends a Head-equivalent render
bone and rebinds only the approved helmet surfaces; all geometric and
texture data stays exact. The Biker's three blended neck vertices become
rigid. The seven helmets without inner faces and the level-2 ported beanie also
set their existing material’s
`two_sided` flag, using the original textures on the reverse side; Keeper
already has an interior and keeps its material. These edits require the matching client hooks; build and ship them
together through the normal release. See `docs/notes/ragdoll.md` section 15.

### The value-type check

`release/build-content` also runs `release/content_type_check.py` over every file a pack or an edit
makes, value by value (types: a float2/3/4, an indexed table, an int, a float, a string, a named
table):

- a value at a key path the previous version of the file has keeps the type it had there; a list
  entry is itself in that path, by its `game_object_type` and its own `full_name` or `name`
  (`client_objects[light:bulb_2]/intensity`), else by its index, so an entry an edit moved or took
  out is never compared with the one now at its index;
- a value at a path new to the file, where 0.26g already uses that structural path (any base
  config: items, materials, model settings, projects; and the file's previous version), has one of
  the types used there; a path 0.26g has no precedent for is free;
- a file at a config path (`.options`, `.raw_options`, `.material`, `/settings`,
  `/export_properties`, `/hit_targets`, `/client_project`) that the reader refuses (a table out of
  CRC order, a broken record) stops the build; it is never skipped. The same for every base config.
  (A root that is not a table is a config there: base patch 19's ported textures' `.options` are a
  lone `true`.)

A new key that has to take another type than 0.26g's goes in `build_content.py`'s `TYPE_ALLOWLIST`
as (game path, structural path) with the reason; a row never lets a key that exists change its
type. It is empty: today's content needs none. The check fails with the file, the path, the old type
and the new type, and no patch is made. It reuses this folder's `content_edit.py` as the reader
(`walk`) and names structural paths as Research's pack builder does
(`scripts/research-mode/make_pack.py`): a list entry by its `game_object_type`
(`client_objects[respawn_point]/position`), otherwise `[]`, a colour ramp's `key0`, `key1`, ... as
`key#`. So it is make_pack.py's pack-time check applied at release time to every pack and edit
list, whoever built it.

This catches the two Research pack bugs (#297) that got past every other check: the builder wrote
every vector (float2/3/4) as an indexed table through a `json.loads(json.dumps(...))` round-trip, so
the game, which reads vectors raw, loaded scrambled levels. A vector rewritten as a list is exactly
what it refuses (`release/test.sh` has the negative test; today's content passes it unchanged).

Then run `weapon_balance_1_changes_only_its_values`
(`crates/survarium/src/v026g/maps.rs`, ignored: it needs `SRP_TEST_RESOURCES_DB`,
`SRP_TEST_SKHID_PATCHES` and `SRP_TEST_PATCH_WEAPON_BALANCE_1`) when a change touches its files. The
server game data carries no localization tables: the test reads the texts in the patches themselves.

### Map edit lists

The map editor (`~/Projects/survarium/map-editor`) saves one edit list per map: moves and turns
of a project's entries (`set_entry` with vectors and nested keys such as `spawned_item/position`),
copies (`add_entry`, with nested keys such as `collision_geometries[0]/name`), an anomaly's
artefact containers (`add_item`, `remove_item`) and deletions (`remove_entry`). Its `map_editor`
table is the editor's own; `content_edit.py` ignores it. The editor's tests apply its lists with
this script and compare the projects with its own (`crates/map-core/tests/real_maps.rs`).
A map's list may also edit its Research mode configs (`gameplay/game_modes/<level>_<weather>_5.options`,
the capture units' `capture_zones`). Moving spawns or capture units, or changing what collision
stops (`collision_objects` `cgroup`/`cmask`), changes the simulation: the release that ships such a
list needs the `next` row of `crates/session/src/v026g/simulation_changes.tsv`.

## Stats tables

A table, `<table>.tsv` (`weapon-stats.tsv`): a row per config (or per level's projects, or per entry
of a list), a column per stat; or `key  value`, a row per key of one config, which the build makes
when no archive has it (`gameplay/srp/<name>.options`). An empty cell means no such key. To retune,
edit the cell. `content_edit.py` `STATS` says what a row is and maps each column to its key path
(`[*]` every entry of a list, `[field=cell]` the matching ones); the build writes the cells and the
cards, refuses a config that differs from its row, and a changed match column without the `next` row.
`content_edit.py --dump-stats <table> <folder>` prints a table from configs; values that differ within a
row are refused there (a cell unifies them).

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

Removed: its text is in `client-patches/localization/*.tsv`.

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

## crouch-steps-2.json (2026-10-23)

| change | why |
|---|---|
| The 16 moving crouch clips (`animations/single/weapons/ak/player/crouch/3rd_view/`: `forward`, `backward`, `left`, `right`, the four diagonals and their `aimed_` versions; every weapon's crouch body plays them): a second event channel `player_sound_events` (type 2), knots 2 and 17 of the 30 frames, each with the domain value (0 or 1, the foot) of the `anim_intervals` domain it starts in. The bone tracks byte for byte | T27, the owner: "for some reason we added sounds but the movement is still silent when crouching". A step is an animation event: only clips with this channel call `on_foot_step`, and the crouch clips had only `anim_intervals`, so crouch-steps-1's pairs never played (`docs/notes/crouch_steps.md`). Run and sprint have it the same way (2 frames after each interval knot, its domain value), and so do 0.31a2's crouch clips (2 and 17) |

New edit for it: `add_event_channel` (an animation clip, not a config: its live channels checked,
one appended; `clip_parse` reads all 777 clips of the live game back byte for byte). Applied to the
stock clips (no Skhid patch carries them): 16 files edited, 48 bytes longer each (a 16-byte header,
32 bytes of knots, domains and name). Sound only: no simulation change, nothing in the server game
data.
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

Withdrawn on 2026-10-22 by lobby-scene-2 below (the owner saw the fill light as a fake white light):
the list and its manifest row are removed, so the scene is built from the stock file again.

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

## battery-respawn-1.json (2026-10-21)

| change | why |
|---|---|
| `mode/parameters/respawn_time` 8 -> 10 (an int, kept) in the 56 configs of `gameplay/game_modes/` whose `mode/mode` is 2 or 3 ("Protective device", "Protective device: chargers"): levels 1 to 8 in every weather (level_08's night too), London, the 0.21d and v0.11e maps | the owner: "in batteries mode respawns should take 10 seconds". The respawn rule reads it from the config (0x316700, in ms), the base the kit's modifiers scale (the radio's -20 %: 8 s); the HUD's countdown is the rule's ([`../../docs/notes/battery_respawn.md`](../../docs/notes/battery_respawn.md)). Team Deathmatch (mode 4) and the test maps keep theirs |

Applied to the live configs (the stock resources.db and patches 12 to 27; the game modes are the
stock db's and patches 14, 20 and 22's): 56 files edited, 0 unexpected differences, one byte each.
The simulation changes (the `next` row of `simulation_changes.tsv`).

## v011e-spawn-protection-1.json (2026-10-21)

| change | why |
|---|---|
| `server_objects/respawn_points[i]/invulnerability_time` 2 -> 6 (an int, kept) on every respawn point of `projects/v011e_level_03/{day_2,day_4}/client_project` (43 each) and `projects/v011e_level_04/{day_2,day_4}/client_project` (44 each): 174 values | the owner: the v0.11 maps' spawn protection drops off too fast, "just make it 6 s and that's it". The respawn point's loader reads it as an int in seconds (0x2F4580, kept in ms); 0.26g's maps author 6, the 0.11e source 2 s ([`../../docs/notes/v011e_spawn_protection.md`](../../docs/notes/v011e_spawn_protection.md)). No base zone, no check radii |

Applied to the live projects (`v011e_level_03`'s from patch 23, `v011e_level_04`'s from patch 22;
no later patch has them): 4 files edited, 174 values, 0 unexpected differences, one byte a point;
in `release/build-content 28` (2026-10-22): the same, the four projects packed and the server game
data with 6 on all 174 points.
The simulation changes (its words in the `next` row of `simulation_changes.tsv`).

## premium-armour-1.json (2026-10-22)

| change | why |
|---|---|
| The four premium armour pieces (`blackmarket_torso_4_premium`, `blackmarket_legs_4_premium`, `scavenger_torso_4_premium`, `scavenger_legs_4_premium`: every armour config with a base rarity): 1240 (+5 % experience) and 1250 / 1260 (+5 % reputation with the Scavengers / the Black Market) taken off `default_modifications` (`remove_item`; the plain piece's one default, 1030, 1050 or 1160, stays), `parameters/base_rarity_level` 3 -> 0 | the owner: the golden weapons' treatment for the premium armour, so that the Modification button works on it. srp gives each piece both bonuses as modifications of its own, the first two ([`../../docs/notes/premium_armour.md`](../../docs/notes/premium_armour.md)): the client never offers a default for a reroll, and a base of 3 forbade every craft |

Applied in `release/build-content 28` (2026-10-22, a dry run after v011e-spawn-protection-1): 4
files edited, 0 unexpected differences (the four configs are patch 21's); the server game data built
with it passes `srp-check-game-data`. srp and `release/build-server-db` refuse data without it
(`golden::check`). The patch also carries the lobby movie built from the tree (the pieces' bonuses on
shop and bundle entries, gold background and name, the plain piece as a craft donor). Not a
simulation change.

## lobby-scene-2.json (2026-10-22)

| change | why |
|---|---|
| `projects/lobby_scene/day/client_project` (the lobby's scene): the New Year tree in the corner by the door taken out (a new edit, `remove_entry`: an indexed table's entry matched by its keys, here the name and the position): `render_visuals[0..17]`, the fir `flora/elka_green_05` at (-2.043, 0.561, 4.355) in a chemical barrel at (-2.035, -0.003, 4.409); its ornaments, the `fx/stars` garland (the `new_year_stars` model), 4 Zarya and 2 RGO grenades, 4 artefact coils; the gifts under it, 2 vodka bottles, a toy Niva (scale 0.15), a trap and a givokost artefact; and their 26 `collision_objects[0..25]` | the owner: "can you remove the christmas tree and gifts from the lobby" |
| `client_objects[41]` (`ambient_light(23)`, at (-2.092, 3.294, 4.439), 0.16 m above the tree's top, intensity 2.2, radius 1.1, colour 0xFFF28653) taken out | the warm glow of the tree's top: nothing else is within its 1.1 m |
| `client_objects[66]` (`sounds/new_year`, the collection `scapes/lobby/new_year`: `christmas_sounds_01..04`) taken out | the owner: "and christmas sounds". The lobby's other sounds stay: `background`, `lobby_room_tone`, `fire`, `lamp_drone`, `lamp_bzz`. The lobby has no music setting: its sound is these scene objects |
| a new light, `point_light(7)` (`add_entry`, after the ceiling lamp's `point_light`; `client_objects[60]` in the result): the ceiling lamp's entry copied, every field kept (colour (1, 0.824, 0.702), `attenuation_power` 7.7, shadows) but `name`, `full_name`, `position` (4.81, 3.02, 2.91), `intensity` 3.28 -> 3 and `range` 8 -> 5 m (an int, as the original's) | the owner: "make the light above crafting table stronger by just making the close by lampe stronger, (same yellow light), not by just adding a new fake white light"; then, of the first version (the ceiling lamp 3.28 -> 4, range 8 -> 10): "the lamp above the table is still not bright". The bulb above the workbench (baked into `electric1.model`: a bare bulb in its socket, 0.21 m, the shape of `props/light/lamp`, on an arm from the wall, with the `lamp_bzz` sound) had no light of its own; the ceiling lamp is the same bulb, its light 0.16 m under it, so this one is too. The weapon at (4.25, 1.30, 3.05) is 1.81 m from it. Intensity 4 in the second version; 3 now that the second lamp lights the weapon too |
| a second lamp over the artefact table, made as the bench one: `wire_set(2)` (a copy of the scene's `wire_set`, the plafond's cable: 7 points, 5 fragments, as the v0.11e maps' short wires; `wire_width` 0.02, the thinnest of the game's wires) from the `electric1` wire's centre at (3.40, 3.505, 4.225) down to (3.40, 3.310, 4.049); `level/lobby/bench_bulb` (the content pack `lobby-bulb@1`, below) hanging from it, its origin (the socket's top) there, `rotation` y 1.571 (`render_visuals[67]` in the result: the entry of the scene's `lamp_on` copied, its model, rotation and scale changed); `point_light(8)`, the same copy as `point_light(7)`, 2.5 cm in front of the bulb's tip along its axis at (3.40, 3.17, 3.88), `intensity` 1.15, `range` 4 m, `attenuation_power` 2; `lamp_bzz(2)`, a copy of `sounds/lamp_bzz` at the bulb (3.40, 3.24, 3.96) | the owner, of the second version: "almost amazing, one small problem is that light is behind the weapon, which drops shadows on it when looking, we can fix that by adding a second lamp + light source above the artifacts list its textures says таблица артефактов, there can be the same lamp going from the same wire original one did"; "i specifically chose that location, because wire is above the artefact table"; of the glowing `lamp_on` first put there: "spot is exact, but the lamp used not, it is also very bright, it should be the same lamp as above the table with wire"; of `props/light/lamp` hanging straight: "see how lamp on the right has wire ... and is at an angle"; of it tilted, its light 0.18 m under the tip: "good, but light is misaligned now", "also lamp is not transparent, like the one above the table"; of the bench bulb model, its light at the tip with the bench light's values (3, 5 m, 7.7): "the lamp is correct, only the light still doesn't look right because the artifacts sheet is too bright", "maybe light direction is wrong?". See below |

The bench bulb, measured in `electric1.model` (its surfaces `electric_s`, the socket, material
`level/lobby/electric`, and `wp_locker_s`, the glass, `level/lobby/glass_clone`, which draws see-through;
the drop wire is in `conc_low_s`, vertices 471..485, radius about 0.003): a drop wire leaves the main
wire's centre at (5.096, 3.515, 2.921), bends at (5.024, 3.424, 2.921) and enters the socket's top at
(4.920, 3.320, 2.921): 0.26 m, all in the plane z 2.92, towards the room (-x), 38 then 48 degrees from the
vertical. The bulb's axis (the principal axis of its vertices) is (-0.774, -0.634, -0.003): 50.7 degrees
from the vertical towards -x, 0.192 m from the socket's top to the tip at (4.771, 3.198, 2.919) (the
ceiling bulb, measured the same way, is 0.7 degrees off the vertical, 0.190 m).

No model of the db is that bulb: `props/light/lamp` (and `lamp_on`) is the same shape in one surface with
one opaque material, and no lamp model has a glass surface of its own with a see-through material; a
scene's `render_visuals` entry has no material override (every entry of the 84 projects holds
`lib_name`, `position`, `rotation`, `scale`, `visual_name` and at most `sectors`). So the content pack
`lobby-bulb@1` (`../lobby-bulb/make_pack.py`, from the stock resources.db) adds
`models/level/lobby/bench_bulb.model`: `electric1`'s two surfaces' triangles of the bench bulb alone (36
vertices and 34 triangles of the socket, 41 and 65 of the glass), the vertices as they are (normals,
tangents, UVs into the lobby's atlases) moved by -(4.920, 3.320, 2.921), so its origin is the socket's
top and it keeps the bench bulb's tilt; the two surfaces' materials are `electric1`'s; settings and
export properties are the stock ones' (`props/light/lamp`'s settings: no collision, LOD0 only) with the
counts and bounds of the new geometry, written by the script's config writer (each written file read
back as the tree it was made from, and the writer checked against the seven stock configs it reads).

The poster is `props/signboard/posters_014` at (3.583, 1.972, 4.627), scale 1.317: the "ТАБЛИЦА
АРТЕФАКТОВ" strip of `props/signboard/posters_01` (its UVs u 0.89..1.0, v 0.02..0.33), x 3.12..3.68, y
1.88..3.55 on the back wall (z 4.60). The `electric1` wire runs over it along x at z 4.16..4.25 (rings of
6 vertices, radius 0.03; over x 3.40 its centre is at y 3.505, z 4.225). The new lamp is the bench one
turned from the right wall to the back wall: its drop and tilt go towards -z (the room) instead of -x.
The scene's angles are the negative of the right-handed ones (`posters_014`, rotation 0, faces -z, its
packed normal; `military_04` faces the room, -x, at y -1.571, and `military_06` at (1.571, 0.012,
1.571); the lobby camera's yaw turns the same way, `../../docs/notes/client_patch.md` section 3.42), so y
1.571 turns the bulb's -x tilt to -z: the placed model's vertices give the axis (0.003, -0.634, -0.774),
50.7 degrees, the socket's top at the wire's end (3.40, 3.310, 4.049) and the tip at (3.40, 3.188,
3.900). The game run of the version before this one showed that tilt (the bulb seen end-on from the
lobby's camera, the drop wire towards it). Its light was then 0.18 m under the tip, the bench light's
offset (the bench light, (4.81, 3.02, 2.91), is 0.18 m under its tip and 4 cm towards the wall: left as
it is, the owner's "almost amazing"); now it is 2.7 cm from the tip along the axis, 2.22 m from the
weapon, on the camera's side (the camera at about (3.09, 1.60, 3.05) looking along +x).

Its values. A `point_light` has no direction: `light_type` 0 is `light_type_point` (the engine's
`light_props.h`; 1 is the spot, as `back2`), and the point light's shader (`shaders/sm_4_0/point_light.h`,
`point_light_accumulator.ps`, the 0.26 sources under `private/editor-026`) reads only its position, range
and `attenuation_power`; its `rotation` and spot angles, copied from the ceiling lamp, are unused. A
surface gets intensity x max(|1 - saturate(d^2 / range^2)|, 0.001)^attenuation_power (times the BRDF's
cos of the incidence: the poster faces the light square on). With the bench light's 3, 5 m, 7.7 the
poster, 0.7..1.5 m away, got 2.55..1.45 and the weapon, 2.22 m, 0.56: the curve is flat near the light
and steep past 2 m. 1.15, 4 m, 2 give the weapon 0.55 (99 %), the poster 1.08..0.85 (42..59 %), 0.6 m
41 %, and 3.5 m 0.06 (0.02 before): the poster about half as bright, the weapon as it was.

`add_entry` takes a list of numbers for a vector (the position) and a list of vectors for an indexed
table of them (the wire's points), and refuses a fraction for an int; an entry it adds before one added
earlier moves that one's checked paths too. It runs before `remove_entry` (no other part of the scene,
nor the exe, refers to `client_objects` or `render_visuals` by index). The counts: `client_objects`
72 + 4 - 2 = 74, `render_visuals` 90 + 1 - 18 = 73. The first
bulb is part of the `level/lobby/electric1` model, whose scene entry has no emissive or other material
setting: it is not changed (the model files are shared). **To tune** after a game run: both lights'
intensity and range, and `attenuation_power` (the copy's 7.7).

Applied to the stock file (sha256 `e93de4fc…c0f8`; patch 27's copy is lobby-bench-light-1's, no longer
in the manifest): 1 file edited, 0 unexpected differences (`client_objects` 72 -> 74, `render_visuals`
90 -> 73, `collision_objects` 92 -> 66, the new entries); the result sha256 `9d809203…2a8b8`;
in `release/build-content 28` (2026-10-22, a dry run, after every other list): the same, every list
at 0 unexpected differences, the scene packed over patch 27's. Rendering and
sound only: no server data, no simulation change. `client_project.lua` (the editor's text of the
scene, which the game does not read) is left as it is.

## ban-texts-1.json (2026-10-22)

Removed: its text is in `client-patches/localization/*.tsv`.

## third-mod-1.json (2026-10-23)

| change | why |
|---|---|
| The 15 masks and respirators (the stock nine, `bandit_026` 1810, the 0.69d masks 1812-1816, `army_026` 1823) may roll type 13 Armor (1000-1004: +2, +3, +4, +5, +8; a mask has only its face values), character 12 of `parameters/available_modifications_mask` set (`0000000000000000010000000000010100000000` -> `0000000000001000010000000000010100000000`) | reliability (type 32) no longer rolls (wear is off since #35, `docs/notes/no-reliability-roll.md`), which left them weight and anomaly filters only; the owner's pick (2026-10-23) |
| The four oxygen packs (`scavenger_oxygen_1`, `edge_back_3`, `ren_back_3`, `ren_back_5`) may roll type 24 Weight limit (1120-1124: +1..+5 kg) and type 28 Energy costs (1160-1164: −2..−10 %), characters 23 and 27 (`0000000000000000010000000000001100000000` -> `0000000000000000010000010001001100000000`) | the same: weight and oxygen were left; the owner's pick |
| The Pilgrim container (`edge_back_6`) may roll type 28 Energy costs, character 27 (`backpacks-1.json`'s `0000000000000011110010010000100100000000` -> `0000000000000011110010010001100100000000`) | its default Regeneration (1094) takes the type `backpacks-1.json` added, so it was left with weight and weight limit; the owner's pick |

Only srp reads the masks: the client shows whatever modification the server sends, from its own
`modifications.options`. Applied to revision 29's live files (from the stock db and patches 12 to
29, after `backpacks-1.json`): 20 files edited, 0 unexpected differences. With it every item that
rolls has three types or more besides reliability
(`modifications::tests::every_item_of_the_built_game_data_rolls_three_types` on the built server
game data). No simulation change: a match applies only the modifications an item has.

## artefact-texts-1.json (2026-10-23)

Removed: its text is in `client-patches/localization/*.tsv`.

## protective-mask-1.json (2026-10-23)

| change | why |
|---|---|
| The Protective mask (1816, `face_069d_protective_mask`, the face-069d pack) no longer covers the helmet slot: `helmet` out of its `data/additional_model_slots` (`remove_item` of a string; `['helmet']` -> `[]`, as the other ported masks have it) | the owner, 2026-10-23 (Skhid T28): "the hunter mask should just be a mask"; players asked to wear a helmet with it. The client's paperdoll locks the cells of the slots an item covers, and srp reads them from the same config (`item_params::covered_slots`), so a helmet now goes on with it |

It stays in the mask slot (its dictionary category fits no other), so the kits that wear it need
no repair: their helmet slot is empty and can now be filled. Its level, weight, price, armour and
modifications are unchanged (`item_params::tests::the_protective_mask_of_the_built_game_data_is_just_a_mask`
on the built server game data). Its model is the pack's, fitted over the bare head: with a helmet
it shows through most helmets (`docs/notes/masks-pack.md`). Applied to revision 30's live files: 1
file edited, 0 unexpected differences. No simulation change: the slot rule is the lobby's.

## match-chat-off-1.json (2026-10-23)

Removed: its text is in `client-patches/localization/*.tsv`.

## hide-interface-1.json (2026-10-24)

Removed: its text is in `client-patches/localization/*.tsv`.

## bandages-1.json (2026-10-24)

| change | why |
|---|---|
| `gameplay/items/drugs/bandages.options` `data/influences`: hands and legs `amount` 0.3 -> 1 (per second), and three new entries made like `right_leg`'s, after it: `body` 0.35, `head` 0.14, `pain` 0.35 | the owner: bandages heal legs and hands fully, and pain, body and head by 35 %. An influence is the pool's health added per second for `activity_time_sec` (1 s; `medkit::tick` 0x33D790). 1/s fills a 0.5 limb pool, as the medkit's 1/s does. 0.35, 0.14 and 0.35 are 35 % of the body (1.0), head (0.4) and pain (1.0) pools ([`../../docs/notes/bandages.md`](../../docs/notes/bandages.md)) |
| `st_base_bandages_description` in the six localization tables: "Fully heals arms and legs. Restores 35% of body, head and pain." (Heilt Arme und Beine vollständig. Stellt 35% von Körper, Kopf und Schmerz wieder her. / W pełni leczy ręce i nogi. Przywraca 35% zdrowia tułowia, głowy i bólu. / Полностью лечит руки и ноги. Восстанавливает 35% здоровья тела, головы и боли. / Повністю лікує руки і ноги. Відновлює 35% здоров'я тіла, голови та болю.) | the owner, 2026-10-25: the description says what a bandage now heals. Each text uses only characters its table already has |
| the card's "Heal amount" line taken out: `ui_desc/props_list` entry `prop_id` 12 ("30") | the owner, 2026-10-25: one number can't describe full and 35 %, so the line would contradict the text. The card keeps use time, active time and weight |

Applied to the live files: 7 files edited, 0 unexpected differences. The simulation changes (the
`next` row of `simulation_changes.tsv`).

## network-texts-1.json (2026-10-25)

Removed: its text is in `client-patches/localization/*.tsv`.

## kill-feed-weapons-1.json (2026-10-26)

| change | why |
|---|---|
| `ui_desc/combat_log_icon` of the 21 configs of base patch 19's weapons: Saiga-12K (and premium) 31 -> 53, SOK-94 29 -> 54, PKP 38 -> 55, Vepr-12 (and premium) 31 -> 56, AEK-919K 3 -> 57, Kedr 22 -> 58, Neostead (both) 12 -> 59, FNC 31 -> 60, MP7 1 -> 61, MP7 silenced and Special 1 -> 62, HK416 31 -> 63, P90 47 -> 64, KRISS 47 -> 65, MSBS 31 -> 66, Valmet 31 -> 67, vz. 58 29 -> 68, M4A1 31 -> 21 | the owner: "new weapons that were added have incorrect icons" in the kill feed. The icon is frame (icon + 1) of the hud's weapon clip; the port gave these weapons stand-ins of stock weapons' icons, the clip ending at 52. Their builds' own icons (0.27d0 to 0.31a2 alike) are now in the hud movie (`CombatLogWeapon53` to `68`, [`../../docs/notes/kill-feed-icons.md`](../../docs/notes/kill-feed-icons.md)); the M4A1's 21 is the clip's own M4 |

## london-1.json (2026-10-26)

London has no anomalies in any of its four modes (`projects/level_09/day_2` to `day_5`:
batteries, batteries with chargers, Team Deathmatch and Research). Each of the seven anomalies
loses its core, artefact container, damage zone and the two collision geometries: 35 entries per
project. The damage zones own their idle sound and particles. The decorative rock piles stay.
The battery modes also lose the two old outdoor spawn-protection zones and their boxes; each
spawn point keeps its own 6 seconds of invulnerability.

The 20 spawn points come from the owner's map-editor placement. `set_entry` changes only their
positions and rotations, retaining their names, order, teams and other properties. All four
projects use the same placements:

| team in the project (server team) | placement | walking distance to its battery container |
|---|---|---|
| team1 (0, west) | building_06_1's first floor, y = 9.25; 10 points placed to match team2 | 44–65 m |
| team2 (1, east) | building_10's first floor, y = 9.52–9.56; the owner's eight points plus two beside them | 68–74 m |

Team1's points are 13–26.5 m on foot from the door with stairs to the street. A point moved
without being turned keeps its original facing. These placements replace the earlier proposed
spawns beside the ground-floor doors. The owner's accidental prop moves are excluded.

Checks compare the built projects with the live projects and the editor's placements: 10 + 10
points per mode, every position and rotation as placed, no remaining anomaly objects or old
spawn zones, and every unrelated value unchanged. `release/build-content` also checks the
complete manifest against the live patches. The simulation change is recorded by the `next`
row in `simulation_changes.tsv`; the release resolves it to its revision.

The server reads the updated spawn table from the generated `server-game-data.db`. For bots,
regenerate all four `level_09_day_*.mapdata` files with `mapdata-dump` over the release's complete
patch list and new content patch. These files then contain the same 20 points and no anomaly
zones; they are generated game data, not repository sources. The Maps tab is rebuilt from the
same content by `release/build-site`.

`feat/dynamic-anomalies` (not shipped) removes the same seven anomalies for timed ones, and
PR #369's `sponge/spots.tsv` selects their containers. Both must drop their London entries
before they can ship on top of this list.
