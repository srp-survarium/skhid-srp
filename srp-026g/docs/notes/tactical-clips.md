# Tactical reload for the one-clip guns

The owner (2026-11-01): a tactical reload for the live guns that reload with one animation, "the
way the live vz.58 and the other 0.29 guns already do it": data and clips only, no exe change.
Packs `tactical-clips-029` (0.29a3's clips, preferred: the patch-19 donor, same rig) and
`tactical-clips-069` (0.69d0's, for guns 0.29a3 has none for); resource-porter
`port-tactical-reload` (`src/weapons_026/tactical.rs`, manifests `ports/tactical-reload-029a3.json`
and `ports/tactical-reload-069d.json`); `scripts/tactical-clips/make_pack.sh`.

## The rules

- **The reload** is 0.26g's two-clip one, state type 314 (`pistol_weapon_core_reload_state`), as
  the vz. 58's: with rounds left in the magazine the tactical clip plays, without them the empty
  one (get_weapon_lexeme 0x30FE90, slot `2 x (ammo_in_magazine == 0) + 4 x crouch`). Hands clips
  `[reload_tactic, reload] x 4`, item clips `[reload_tactic, reload]`.
- **The clips** are one donor build's pair, tactical and empty, so the two keep that build's
  lengths against each other. A donor clip at a path 0.26g has with other bytes is shipped at
  `<path>_029a3` / `<path>_069d0`; the live clip of that name stays for anything else.
- **The times:** the engine plays both clips at one speed, the tactical clip's length over
  `reload_time` (the ctor 0x30FC50). `reload_time` stays: the tactical reload lasts it, as the
  one reload did; the empty one lasts `reload_time` x (empty clip / tactical clip).
- **The sounds:** the donor's reload sound lists (its clips' sound events index them). Every
  converted gun's are the live ones already.
- **Which guns:** a live 303 gun whose donor reloads with two clips, and whose donor clips fit:
  the hands clips drive 0.26g's player rig (70 bones) bone for bone, the item clips the bones the
  gun's live reload drives, and at each clip's start and end every part of the gun sits in the
  hands where 0.26g has it at rest or in its own reload (5 mm, 0.02 rad; the porter checks it).
  Accepted by name in the manifest: the AKs' top round (`patron`, 1.3 cm), the Uzi's bolt
  (`zatvor`, 7 mm at the tactical clip's end).
- **Changes the simulation** (the reload's length): `simulation_changes.tsv`.

## 0.29a3 (pack tactical-clips-029@1)

0.29a3 reloads these with two clips (its configs: state 314, the same sound lists as ours):

| weapon | tactical (= reload_time) | empty, now | clips (frames) |
|---|---|---|---|
| AK-74M, AK-74N (+premium), AKS-74U (+premium), AKMN (+2 premiums), RPK | 3.167 | 4.37 | `ak/reload_tactic` 145, `ak/reload_029a3` 200 |
| Uzi (+2 premiums) | 2.3 | 3.35 | `uzi/reload_tactic` 55, `uzi/reload_029a3` 80 |
| AS Val, VSS (+premium) | 3.167 | 4.47 | `val/reload_tactic` 85, `val/reload` (hands, ours) / `val/item/reload_029a3` 120 |

Before, every reload lasted `reload_time`. The VSS's model has a `magazin2` the Val's clips do not
drive: as its live reload.

## 0.69d0, for the guns 0.29a3 has none for (pack tactical-clips-069@1)

0.69d0 reloads these with two clips (its 313, 0.26g's 314); its reload sound lists are ours:

| weapon | tactical (= reload_time) | empty, now | clips (frames) |
|---|---|---|---|
| AK-12 (+premium) | 2.8 | 3.71 | `ak_12/reload_tactic` 120, `reload_069d0` 159 |
| A-545 | 3.167 | 3.96 | `a_545/reload_tactic` 80, `reload_069d0` 100 |
| ASh-12 | 3.167 | 4.67 | `ash_12/reload_tactic` 122, `reload_069d0` 180 |
| 9A-91 | 3.167 | 4.47 | `a9_91/reload_tactic` 73, `a9_91/reload` 103 |
| VSK-94 (+premium) | 3.367 | 4.75 | the 9A-91's |
| PP-2000 | 2.8 | 3.43 | `pp2000/reload_tactic` 155, `reload_069d0` 190 |
| PP-19 Vityaz | 3.367 | 4.57 | `vityaz/reload_tactic` 140, `reload_069d0` 190 |
| PP-19 Vityaz premium | 3.167 | 4.30 | the Vityaz's |

0.69d0's item clips carry a `shoot` event channel (its fire clips' shot): only the fire state
listens to it (weapon_core_fire_state_base::initialize 0x30E9B3 subscribes, its finalize 0x33AA40
unsubscribes), so in a reload it is heard by nothing. Accepted by name: the Vityaz's fire selector
(`safety`, 10 degrees from our idle), the ASh-12's safety catch (25 degrees at the tactical clip's
end, as our own reload starts it). Reported: the hands' fingers and forearm roll bones apart from
our reload at the clips' ends (the Vityaz's forearm rolls); worth a look in the game.

## Not converted

- 0.69d0 moved the gun in the hands: its hands' `Weapon` bone carries the static placement our
  clips keep in the gun's `offset` bone (the same numbers across the gun root's half turn about y),
  so its reload would slide the gun at the start and end: the SR-2M, AEK-919K, Vepr (+premium)
  (13 cm), UMP-45 (14 cm), PPSh-41 (6 to 9 cm), SR-3M (+premium) (the Val's clips, 5 cm).
- One clip in 0.29a3 and 0.69d0 too: the PKP, RPD, VSSK (+camo), TOZ-122, Magnum (a revolver).
