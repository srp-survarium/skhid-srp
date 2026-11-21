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
  the same names, its materials are in 0.26g's own parameter vocabulary (0.69d0's are renamed, with
  no known mapping), and its lights are on 0.26g's scale (0.69d0's two peaks of 3.0 against 0.18 and
  0.16; the stock AK effect peaks at 0.86). Which device names which effect is 0.69d0's.
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
  pack, and with `--record` that every device has a gun that takes it, that every muzzle fit of the
  table has its item (the four left out aside), and that every key a device's modification touches
  is in the config of every gun that takes it (553 fits).
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

**Made and checked without a game:** the pack `muzzle-devices@2` (113 files: 54 item configs, 12
particles, 17 materials, 15 textures with their options; 54 dictionary rows; 54 icons), the texts,
the items table's rows, the `next` row.

- `vostok-port pack check` passes it against the players' files (the stock db and the listed
  patches): no dict id, name, path or icon cell is taken.
- `tools/check muzzle-devices` (the rules on a made-up `game.options`, the table against the ids,
  the texts, the items table's rows) and `tools/check localization` pass.
- `release/build-content` with the full `weapon-modules@1` and this pack went through the packs'
  composition, every edit list and the value-type check (the run that found the materials' floats),
  and no further: the build with that fixed was queued and stopped unrun.
- The content side's record has every device's model and fire point on every gun that takes it
  within 0.5 mm of 0.69d0's (544 fits, 1,394 comparisons, the worst 0 mm). Computed there, not here.

**Not seen in a game: anything.** No device item has been in a lobby or a match. So LIKELY, each
until a game shows it:

- that an item's modification reaches the weapon's config as the cook sees it (`data/srp_muzzle`
  by a type 3 modifier, as a camouflage adds `data/camouflage`);
- that the re-encoded particles load and look right with their rebuilt materials (brightness, the
  fire materials without 0.46c2's angle attenuation, the small mips of the flash textures);
- that the flash plays at the device's fire point;
- that the lobby's card shows the name and description as written.

**Not done:**

1. The first shot: the AK-74M with the Zenit DTK-3 (2635) through its kit's slot 4, the flash at the
   device. Then the other effects on a sample of guns (`pp_arrest1` on a pistol-thread gun, `dtk9`
   and `dtk4_6` on an M14x1 one, `dtk8`, `dtk11`, `dtk12` and `duckbill` on a shotgun), and a device
   without an effect (a choke).
2. A view from the side. The first view looks along the barrel from behind, where a flash 5 cm
   short of the device's end looks the same as one at it. The test bot fires on its own with
   `--drive`; a second bot on the player's team, or a second game, would give the side view.
3. `release/build-content --full` with the pack, and `tools/check build-content`.
4. A match's recording with a device fitted through the world check (the kit's record with a dict id
   in slot 4 is patch 68's format; nothing new is written).
5. The M30x1 devices fit the three guns that were at the old 16 KB config bound: fit them only with
   an exe that has patch 161's 64 KB arena (`attachments.md`).
