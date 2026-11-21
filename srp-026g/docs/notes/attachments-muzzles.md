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
- Nothing is typed a device or a gun. A device's family, price, stats and shot effect are read from
  0.69d0's `game.options` and turned by three small tables (`rules.tsv`, `families.tsv`,
  `left_out.tsv`); the result is `devices.tsv`, checked in and refused by the build when stale.

## The devices

54 items, dict ids 2601-2651 and 2656-2658 (`scripts/weapon-modules/ids.tsv`), category 22 as the
scopes, configs `gameplay/items/muzzle_devices/<item>.options`. In the modules table as built
(`weapon-modules@1`): 544 gun and device fits on 88 of the 102 guns.

| Family | Devices | 0.69d0 at its bought grade (7) | Here |
|---|---|---|---|
| Muzzle brakes and compensators | 17 | recoil -4 %, aim time +3 % | the same |
| M30x1 compensators (Remington 700, SV-98) | 2 | slowdown on a hit +0.08 on the gun's 0.3, aim time +3 % | stopping power +27 %, aim time +3 % |
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
  `parameters/stopping_power` (0.69d0 adds 0.08 to a value that is 0.3 on both guns it fits: the
  share 0.2667), `dispersion/base_dispersion`. The last is the only weapon key that reaches pellet
  spread (`attachments.md`, "Stat keys"), and it scales a slug's spread by the same share.
- **Dropped**, no 0.26g key without new exe code: the reveal radius, hip and first-three-shots
  recoil, aimed movement speed, the duckbill's two axes.
- **Prices** (`families.tsv`; silver, seller 0 at no rank by the servers' rule for a module some gun
  takes): a brake or compensator 6,000; a flash hider, a choke, a duckbill 4,500. 0.69d0 prices
  every device alike.
- **The card** shows the name and the description; the description says the numbers in the
  dictionary's own words for each stat. No `props_list`: how the lobby draws a modification's
  property on an item's card was not tried.

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
- **A texture** is 0.46c2's `.dds` as it is, its `.options` in 0.26g's records.

## The build

```sh
scripts/muzzle-devices/prepare.sh <vostok-port> <0.69d0 vostok-port> <0.69d0 .split> <0.46c2 .split> \
    <srp-game-data> <inputs> --db <resources.db> --patch <patch> ...
python3 -I scripts/muzzle-devices/devices.py <inputs>/d069/gameplay/game.options     # devices.tsv, or --write
python3 -I scripts/muzzle-devices/texts.py --check                                    # the 11 dictionaries
python3 scripts/muzzle-devices/render_icons.py --modules <weapon-modules pack> --inputs <inputs> --out <icons>
python3 -I scripts/muzzle-devices/make_pack.py --inputs <inputs> --modules <weapon-modules pack> \
    --record <port-modules' modules-069d.json> --icons <icons> --out $SKHID_PACKS_DIR/muzzle-devices-<version>
```

- `make_pack.py` checks the donor files against `inputs.sha256`, every dict id, name, config path
  and modification id against the players' dictionary, each device's model against the modules
  pack, and with `--record` that every device has a gun that takes it and every muzzle fit of the
  table has its item (the four left out aside).
- The icons are drawn from the devices' own models by the weapons' icon renderer
  (`scripts/weapon-icons/renderer.py`), into `LobbyMenu_AmmoIcons` columns 5 to 7, 22 a column, in
  the table's order. Column 8 is the grips'.
- The texts (`texts.py`, `names.tsv`): a name is the model's name, the kind's word and the thread
  ("Zenit DTK-3 muzzle brake, M24x1.5"); 0.69d0's own strings mislabel kinds and threads and name
  seven devices alike. Keys `st_muzzle_<module>_name` and `_description`.

## The simulation

A device with numbers changes what a match computes: the `next` row of `simulation_changes.tsv`.
A flash hider, and a device on a weapon in the lobby, change nothing a match computes.
