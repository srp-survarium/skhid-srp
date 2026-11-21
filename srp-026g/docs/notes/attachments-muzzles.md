# Muzzle devices: the items, their stats and their shot effects

**Version scope:** client 0.26g; the muzzles lane of the weapon attachments (`attachments.md` is the
system: the modules table, `resolve`, client patch 161, the servers' fit check). This note is what
that lane made: the pack `muzzle-devices` (`../../scripts/muzzle-devices/`), the texts, the `next`
row of `simulation_changes.tsv`. Tags: PROVEN (read in the data or seen in a game, said which),
LIKELY, OPEN.

## The rules

- The owner, 2026-11-20 and 21: every muzzle device 0.69d0 has except silencers; flash hiders change
  the look only; muzzle brakes and compensators get stats, simplified where 0.69d0's need new exe
  code (no hip, aimed or first-three-shots recoil, no reveal radius).
- A device is fitted through the weapon's kit, slot 4 (`weapon-kit.md`). What makes a weapon fitted
  is the contract of `attachments.md`: the item's modification adds `data/srp_muzzle`, its own dict
  id, to the weapon's config; the table says where it stands and where the shot leaves it.
- **Decided 2026-11-21 by the coordinator, to whom the owner left these calls ("your pick"), final
  unless he says otherwise:** the stats table below as it stands; the two Zenit DTK-4 on M24x1.5 in
  as muzzle brakes (0.69d0's own class, the sound unchanged); duckbills at dispersion -15 %; flash
  hiders the look and the flash alone; prices 6,000 (a brake, a compensator) and 4,500 (a flash
  hider, a choke, a duckbill), seller 0.
- Nothing is typed a device or a gun. A device's family, price, stats and shot effect are read from
  0.69d0's `game.options` and turned by three small tables (`rules.tsv`, `families.tsv`,
  `left_out.tsv`); the result is `devices.tsv`, checked in and refused by the build when stale.

## The devices

54 items, dict ids 2601-2651 and 2656-2658 (`scripts/weapon-modules/ids.tsv`), category 22 as the
scopes, configs `gameplay/items/muzzle_devices/<item>.options`. In the modules table as built
(`weapon-modules@2`, table version 3): 553 gun and device fits on 89 of the 102 guns.

- **A config has a shop scope item's shape**, as the grips' items: `data` (the device's model as
  `idle_model` and `aimed_model`, type 5, the factors 1 / 0.5 / 1, `hide_weapon_on_aim` false) and
  an empty `ui_desc/props_list` beside the modification.
- **`ui_desc/props_list` is required of every item with a `ui_desc`** (PROVEN, 2026-11-21). Without
  it the game crashed at its start, on the sign-in screen, fitted or not: an access violation at
  0xF6BC2, inside `binary_config_value::operator[]` (0xF6B10). That function finds a key by its CRC
  and compares the name of the record it lands on without checking that it landed inside the table,
  so a missing key reads past it. `fill_items_dictionary_impl` (0x286410) runs over every dictionary
  item at the start; for one whose config has `ui_desc` (checked) it reads
  `ui_desc/text_descriptions/name` and `/description`, `ui_desc/icon` and `ui_desc/props_list`
  with that function, and no `data` key. The devices had the first three. `release/build_content.py`
  checks these four keys of every item (#2341, from main). The proof in a game: the same items with
  `props_list` and no `data` signed in and played (exe and server c8b28fb43, 2026-11-21).

| Family | Devices | 0.69d0 at its bought grade (7) | Here |
|---|---|---|---|
| Muzzle brakes and compensators | 17 | recoil -4 %, aim time +3 % | the same |
| M30x1 compensators (Remington 700, SV-98) | 2 | slowdown on a hit +0.08 on the gun's 0.3, aim time +3 % | recoil -4 %, aim time +3 % (a brake's) |
| Shotgun compensators | 3 | pellet spread -16 % on both axes, aimed movement -0.3 % | dispersion -16 % |
| Chokes | 7 | pellet spread -20 % on both axes, aim time +3 % | dispersion -20 %, aim time +3 % |
| Duckbills | 6 | pellets +40 % across, -70 % up and down | dispersion -15 % (the two axes' mean) |
| Flash hiders | 19 | reveal radius -40 % (the AKS-74U's and AKMSU's -24 %), a recoil or aim-time penalty | no number: the look and the shot's flash |

- **The family** is told by 0.69d0's main drop modification, not by its name or group: the reveal
  radius makes a flash hider, whatever 0.69d0 calls it (the Zenit DTK-4 M30x1 "muzzle brake", the
  AKS-74U's own device). The AEK-919K's device is in 0.69d0's group `flash_hider` and rolls recoil:
  a brake here. Chokes and duckbills by their group.
- **The keys** (`rules.tsv`). Each is a share of the weapon's own value (modifier type 2, as the
  stock rolls): recoil on the six kick bounds (`weapon-rolls.md`), `parameters/aim_time`,
  `dispersion/base_dispersion`. The last is the only weapon key that reaches pellet spread
  (`attachments.md`, "Stat keys"), and it scales a slug's spread by the same share.
- **The M30x1 compensators' slowdown is dead here**, so they take a brake's recoil (the
  coordinator, 2026-11-21). 0.69d0's slowdown on a hit is 0.26g's `parameters/stopping_power`, and
  the slow from a hit is -min(the last 0.5 s' stopping power, 0.5) (the stagger 0x2DBCD0,
  `client_patch.md` 3.51). The three guns these two devices fit (Remington 700, its premium, SV-98)
  have 0.6 already: more of it slows nobody more. PROVEN in the data and the exe note, not tried in
  a game. The rule (`rules.tsv`, `as`) takes what 0.69d0 rolls first on its plain brakes, and the
  pack's build checks the reason against every gun that takes the device.
- **Dropped**, no 0.26g key without new exe code: the reveal radius, hip and first-three-shots
  recoil, aimed movement speed, the duckbill's two axes.
- **Prices** (`families.tsv`; silver, seller 0 at no rank by the servers' rule for a module some gun
  takes): a brake or compensator 6,000; a flash hider, a choke, a duckbill 4,500. 0.69d0 prices
  every device alike. The items table (`content-edit/items.tsv`, a row for every dictionary entry)
  has the devices' rows from `items.py`.
- **The card** shows the name and the description; the description says the numbers in the
  dictionary's own words for each stat. `props_list` is empty: how the lobby draws a
  modification's property on an item's card was not tried.

**Left out, by a rule the build checks** (`left_out.tsv`): the four VSSK barrels (ids 2652-2655).
Each is a barrel (group `*_barrel`) of a gun whose own part in that slot is a silencer: the VSSK's
suppressor in another shape. They keep their ids and get no item, so nobody can own or fit one.

**The two Zenit DTK-4 on M24x1.5** are in: 0.69d0 sells them as muzzle brakes with a brake's
numbers. Their shape is a closed can and their shot effect is `pp_suppr`; the weapon's sound does
not change (no device changes a sound in 0.69d0's data either).

**No muzzle fit, as in 0.69d0:** Jackhammer, KS-23, Magnum, OTs-14, both PPSh, SVU-A, VAL, both VSS,
Winchester. **Refused by 0.69d0's own slots** (it links the device and gives the gun no such
thread): the Defcon 1 M16x1 on the G3, both Remington 700 and the SOCOM 16.

## The shot effects

A weapon has one `particles/shoot`; a device's modification sets it (type 0) when 0.69d0 gives the
device an effect, and the flash plays at the device's fire point, which the client takes from the
table. Seven devices name none (the chokes, three duckbills, the KKM brake): the gun's own flash at
the device's fire point. The AKS-74U's and AKMSU's devices name the stock `fx/weapons/ak_74`.

Twelve effects come with the pack, `fx/weapons/attach/`: `gas_small` (the flash hiders'),
`pp_arrest1`, `pp_suppr`, `dtk1_2`, `dtk3_7`, `dtk4_6`, `dtk5`, `dtk8`, `dtk9`, `dtk11`, `dtk12`,
`duckbill`, with 17 materials and 15 textures.

- **The donor is 0.46c2**, for the particles and their materials alike. It has the same effects under
  the same names and its materials are in 0.26g's own parameter vocabulary (0.69d0's are renamed,
  with no known mapping). Which device names which effect is 0.69d0's. Its brightness is not on
  0.26g's scale: "The scale" below.
- **A particle** is 0.46c2's tree written in 0.26g's 24-byte records. One action is taken out: the
  target-colour action of `dtk1_2`, a type 0.26g's loader has no case for (`construct_action`
  0x364F10), which 0.46c2 itself has disabled. An enabled one would stop the build. Every node has
  the properties every live node of its type has (a missing one crashed the 0.21d port).
- **A material** is 0.46c2's own minus the parameters no live 0.26g material of the same base
  material and effect has. Its shader switches must then be those of a live material of that base
  and effect, so the permutation exists.
  - The dropped parameters are at neutral values (tiling 1, scrolling 0, sequences off), with one
    exception the build names: 0.46c2 gave the fire effect (`weapon_fire`) an angle attenuation
    that 0.26g's has not. The eleven fire materials lose it.
  - One material takes a switch from the live ones: `idle_muzzle_hi` is one-sided in 0.46c2, and
    every live material of its other switches is two-sided.
  - A number 0.46c2 writes whole where every live material has a float (the four scroll directions
    of six materials) is written as that float: the content build's value-type check refuses the
    other.
- **A texture** is 0.46c2's `.dds` as it is, its `.options` in 0.26g's records.

### The scale

0.46c2's own stock flashes are dimmer than 0.26g's, so a device effect copied as it is would be
dimmer against the gun's stock flash than 0.46c2 made it. The pack keeps the donor's device-to-stock
ratio (the coordinator, 2026-11-21): `make_pack.py` puts every effect on 0.26g's scale, by rule,
from the two builds' own stock flashes. PROVEN in the files; the result is `effect_scale.tsv`,
checked in and refused by the build when stale.

| Stock flash (family) | Light's peak, 0.26g | 0.46c2 | Ratio | Fire materials both builds have, brightness 0.26g / 0.46c2 |
|---|---|---|---|---|
| `ak_74` | 0.856 | 0.377 | 2.27 | `ak_front` 4 / 1, `ak_side` 4 / 1 |
| `sks` | 0.856 | 0.339 | 2.52 | `rifles_front` 4 / 1, `sks_side` 4 / 1 |
| `fort_17` | 0.521 | 0.091 | 5.70 | `fort_muzle_flash` 4 / 1, `tt_33_front_muzzle_flash` 4 / 1 |
| `shotguns` | 0.856 | 0.101 | 8.50 | none (smoke) |

(The other stock flashes, which no effect has as its family: `rifles` 2.26, `vityaz` 2.20, `ppsh`
2.17, `toz_66` 2.40, `magnum` 3.06, `uzi` 3.61, `tt_33` 5.85; their fire materials 4 / 1 too, but
`rifles_side` 5 / 1.)

- **An effect's family** is the stock flash most of its fits replace (the modules table's fits and
  each gun's own `particles/shoot`); as many fits on two stock flashes stops the build. One effect
  is one file for every gun its devices fit, so it has one family: `dtk1_2`, `dtk4_6`, `dtk5`,
  `pp_suppr` and `gas_small` `ak_74`; `dtk3_7` and `dtk9` `sks`; `pp_arrest1` `fort_17`; `dtk8`,
  `dtk11`, `dtk12` and `duckbill` `shotguns`.
- **The cast light**: every enabled light's intensity curve is multiplied by the family's ratio,
  the light's peak in 0.26g over 0.46c2's. The families' ratios do not agree (2.27 to 8.50), so it
  is the family's own. `dtk1_2` 0.181 becomes 0.41 against the stock AK's 0.856, the 0.48 it had
  against 0.377 in 0.46c2.
- **The fire**: a fire material (base `weapon_fire`) has `constant_emissive_multiplier`. Every
  fire material of the families' stock flashes that both builds have is 4 here and 1 in 0.46c2,
  so the eleven fire materials the pack brings are multiplied by that one factor, 1 to 4. A second
  value stops the build: the pack's materials are shared between families. `idle_muzle_color` (the
  glow that stays at the muzzle, 1 in both) is no measure: the effects name it too and take the
  players' own.
- **What gets none**: an effect without a light, a material that is no fire material. `gas_small`
  (the flash hiders') is smoke alone in every build and stays so; `dtk8` and `dtk12` have a light
  and no fire; the duckbill's fan is a `default_particle` material.
- The shotgun family's ratio is large because 0.46c2 cut its stock shotgun light to 0.101: the four
  shotgun effects' lights become 0.79 to 1.56, over 0.26g's stock 0.856, as they were over the
  stock shotgun's in 0.46c2 (0.093 to 0.184 against 0.101).
- 0.69d0's own files, for comparison (not used): the fire billboards are the same sizes as
  0.46c2's; its stock lights are 2.5 to 90, and of the devices' only `dtk1_2`, `dtk11` (and
  `dtk10`, the arrestors) were raised to 2 or 3, the others still 0.09 to 0.18.
- The fire's size is the donor's and is not scaled: `dtk1_2`'s billboards are 0.13 to 0.26 m, the
  stock AK's 0.33 and 0.66 m. A brake's flash is smaller than the bare muzzle's in every build.

## The build

```sh
scripts/muzzle-devices/prepare.sh <vostok-port> <0.69d0 vostok-port> <0.69d0 .split> <0.46c2 .split> \
    <srp-game-data> <inputs> --db <resources.db> --patch <patch> ...
python3 -I scripts/muzzle-devices/devices.py <inputs>/d069/gameplay/game.options     # devices.tsv, or --write
python3 -I scripts/muzzle-devices/texts.py --check                                    # the 11 dictionaries
python3 -I scripts/muzzle-devices/items.py --check                                    # content-edit/items.tsv
python3 scripts/muzzle-devices/render_icons.py --modules <weapon-modules pack> --inputs <inputs> --out <icons>
python3 -I scripts/muzzle-devices/make_pack.py --inputs <inputs> --modules <weapon-modules pack> \
    --record <port-modules' modules-069d.json> --icons <icons> --out $SKHID_PACKS_DIR/muzzle-devices-<version>
```

- `make_pack.py` checks the donor files against `inputs.sha256`, every dict id, name, config path
  and modification id against the players' dictionary, each device's model against the modules
  pack, and by the record that every device has a gun that takes it, that every muzzle fit of the
  table has its item (the four left out aside), and that every key a device's modification touches
  is in the config of every gun that takes it (553 fits). It makes the scale again and refuses a
  stale `effect_scale.tsv` (`--write-scale` writes it), and reads each scaled light back.
- The icons are drawn from the devices' own models by the weapons' icon renderer
  (`scripts/weapon-icons/renderer.py`), into `LobbyMenu_AmmoIcons` columns 5 to 7, 22 a column, in
  the table's order. Column 8 is the grips'. That renderer has no shadows, so `render_icons.py` adds
  the one the icons need: a triangle whose ray to the key light meets the model loses its highlight
  and the key light its normal would take. Without it the inside of a cone or a crown (2629, 2630,
  2636, 2637) took the highlight of a face turned to the light, a pale wedge.
- The texts (`texts.py`, `names.tsv`): a name is the model's name, the kind's word and the thread
  ("Zenit DTK-3 muzzle brake, M24x1.5"); 0.69d0's own strings mislabel kinds and threads and name
  seven devices alike. Keys `st_muzzle_<module>_name` and `_description`.

## The simulation

A device with numbers changes what a match computes: the `next` row of `simulation_changes.tsv`.
A flash hider, and a device on a weapon in the lobby, change nothing a match computes.

## State (2026-11-21)

**Made and checked without a game:** the pack `muzzle-devices@3` (113 files: 54 item configs, 12
particles, 17 materials, 15 textures with their options; 54 dictionary rows; 54 icons), the texts,
the items table's rows, the `next` row. @2 was the same with the effects on 0.46c2's scale (11
particles and 11 fire materials differ, nothing else).

- `vostok-port pack check` passes it against the players' files (the stock db and the listed
  patches): no dict id, name, path or icon cell is taken.
- `tools/check muzzle-devices` (the rules on a made-up `game.options`, the table against the ids,
  the texts, the items table's rows) and `tools/check localization` pass.
- `release/build-content` with the full `weapon-modules@1` and this pack went through the packs'
  composition, every edit list and the value-type check (the run that found the materials' floats),
  and no further: the build with that fixed was queued and stopped unrun.
- The content side's record has every device's model and fire point on every gun that takes it
  within 0.5 mm of 0.69d0's (544 fits, 1,394 comparisons, the worst 0 mm). Computed there, not here.

**Seen in a game, 2026-11-21, with the effects on 0.46c2's scale (`muzzle-devices@2`)** (a headless
client on a local server, the lane's rig: the game at 30 frames a second under a 60 a second
recording, so each game frame is in two; first view). On the core's c8b28fb43 (exe and server,
table version 3, `weapon-modules-quick@32` of six guns) and before it on e17ace688 / 6a2623ff2
(version 2, the AK-74M); the last two rows on 98843a3ef with the full content
(`release/build-content 69 --full` of the integrated tree):

| Gun, device | Effect | What the game showed |
|---|---|---|
| AK-74M, Zenit DTK-3 (2635) | `dtk1_2` | the device on the gun in the lobby; at the shot an orange jet out of the device's side and a plume above it, at the device; no crash at the first shot |
| AK-74M, AWS Rapira (2602) | `dtk3_7` | small orange glows at the device |
| AK-74M, Red Heat Shturm (2630) | `dtk5` | an orange jet out of the device's side |
| AK-74M, Dynacomp flash hider (2631) | `gas_small` | the long hider in the lobby and the first view; at the shot almost nothing (the muzzle's warm pixels +12 % over the frame before, the stock flash's +950 %) |
| AK-74M, Zenit DTK-4 (2657) | `pp_suppr` | the closed can on the barrel; almost no flash |
| AK-74M, nothing fitted | the gun's `ak_74` | a large bright fireball: much brighter than any device's effect |
| Saiga-12, custom 01 choke (2640) | none: the gun's own | the gun's own orange flash at the end of the choke |
| Saiga-12, custom 01 duckbill (2639) | `duckbill` | a flat pale fan out of the duckbill's mouth, sideways, then grey smoke |
| Saiga-12, Red Heat ILGYNAGK01 (2627) | `dtk8` | a pale puff of smoke at the device, no bright flash |
| AEK-919K, its own brake (2601) | `pp_arrest1` | an orange ball at the device for each round of the burst |
| PM, the AEK-919K's brake (2601) | `pp_arrest1` | the device on the pistol; an orange glow at its end. A see-through copy of the slide and barrel beside them for about 0.1 s after each shot is the pistol's own: the PM with nothing fitted shows it too |
| PM, Manticore Nightbrake (2625) | `gas_small` | the device on the pistol; no flash |
| SVD, custom 01 clamp brake (2605) | `dtk9` | orange jets to the side at the device |
| HK416, Diamondhead T-brake (2613) | `dtk4_6` | orange jets to both sides at the device |
| Saiga-12, Hartman Kochevnik (2619) | `dtk11` | two orange jets out of the device in a V, up and to both sides |
| Saiga-12, Guns.ru custom (2618) | `dtk12` | no fire: a flat band of grey smoke sideways at the device and one small spark ahead of it |

- The item's modification reaches the weapon's config as the cook sees it (`data/srp_muzzle`, the
  device drawn); the kit's slot 4 record survives the account's load. The stats were not measured
  in a game.
- Every effect named here loads with its rebuilt materials, and no game crashed at a shot.
- **On that scale the devices' flashes were far dimmer than the stock flash**, more than 0.46c2
  made them: its own stock flashes are dimmer than 0.26g's ("The scale"). `muzzle-devices@3` has
  them on 0.26g's scale.
- **From the side** (a teammate bot that stands and fires, `srp-bot --drive=shots`; Cologne's
  covered platform, plain walls; about 3 m beside him, square to the barrel; the AK-74M with the
  Zenit DTK-3 as the item in the kit's slot 4, on every account): the device on the end of his
  barrel, behind the front sight, its ports visible; at each shot a cone of fire forward from the
  device's front face and a flat jet across its ports, a glow on the wall round it. Nothing starts
  short of the device or ahead of it. This is also the third view of a fitted device on another
  player.
- The world check: a recording of a four-player match with the device fitted on every gun (118
  worlds, map 132, revision 69), `srp-world-check` of the tree with the content's game data: all
  passed. Not added to the world corpus (main's game data has no device items).

**The shop and the Attachments window, seen 2026-11-21** (exe and server c8df2f38d, a quick content
build of the integrated tree with `muzzle-devices@3`; the AK-74M with nothing fitted, 30,000 silver):

- The shop's Upgrades lists the devices; the search "DTK-3" leaves the Zenit DTK-3 muzzle brake
  (6,000) and the Zenit DTK-4 flash hider, M30x1 (4,500). The DTK-3's card: its name, "Weapon recoil
  -4 %. Aim time +3 %." and "Can be installed on:" with the guns that take it, the AK-74M first and
  highlighted (the gun in the kit).
- Buy: the dialog names it at 6,000; the account then has 24,000 and the item (dict 2635).
- The equipment's AK-74M, right click, Attachments: three slots, Optics, Muzzle device, Grip. The
  DTK-3 dragged from the inventory onto the muzzle slot stays there, and the window's gun wears it
  at the muzzle. Confirm: the gun's record of the account is `<the device's item id>:2635:0:4`.
- Not seen: a device dropped onto the slot of a gun that does not take it (the lead saw such a
  device greyed in the inventory).

**Seen in a game on 0.26g's scale (`muzzle-devices@3`), 2026-11-21:** the 22 files that differ
from @2, as a local test patch mounted over the full content (the game mounts every
`resources.db.patch_<n>` it finds; a test only). First view, from the rig's reference spawn point
and view on Mamayev Kurgan (the same place and view for every gun, within 0.2 px), aimed (the
owner, 2026-11-21: no shots from the hip in what is shown):

| Gun, device | Effect | What the game showed, aimed |
|---|---|---|
| AK-74M, Zenit DTK-3 (2635) | `dtk1_2`, light x2.27, fire x4 | orange jets to both sides of the front sight and a small flame over it, one game frame, plain in daylight; before the scale the same jets, faint |
| Saiga-12, Hartman Kochevnik (2619) | `dtk11`, light x8.50, fire x4 | a large bright orange V of fire that covers the middle of the aimed view for one game frame; before the scale a dim wide V |
| Saiga-12, custom 01 choke (2640) | none: the gun's own | a bright compact flash at the choke's end, over the front sight (the scale changes nothing of it) |
| AK-74M, Dynacomp flash hider (2631) | `gas_small` | no fire, as made (the scale changes nothing of it) |

**A capture at 30 frames a second shows only some shots' flashes** (counted 2026-11-21: the
Saiga-12 with nothing fitted, aimed single shots 1.6 s apart in a dark hall, a shot told by its
kick and a flash by the red the picture gains). With a flash: release 68's main (590cffcdf) 17 of
46 shots; current main (d6998ef7d) 9 of 23; the attachments build with the Saiga on its live model
23 of 45 and on its remodel body 20 of 41; first, second and third lives alike. So a single shot
without a flash proves nothing, on any build; a capture compares flashes only over a series of
shots. The lane's rig caps the game at 30 frames a second so that a one-frame flash is in two
recorded frames, and the stock flash's fire lives 0.05 s: LIKELY the cause, not yet counted at a
higher rate.

**Not done:**

1. `release/build-content --full` with `muzzle-devices@3` (the lead's one full build), and
   `tools/check build-content`.
2. The M30x1 devices fit the three guns that were at the old 16 KB config bound: fit them only with
   an exe that has patch 161's 64 KB arena (`attachments.md`).
