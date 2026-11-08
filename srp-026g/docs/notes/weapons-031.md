# Weapons batch 1: OTs-14, SCAR-H, VHS-2, Minimi (0.31a2)

Lobby icon update: [perpendicular profiles matching retail 0.26 references](../../scripts/weapon-icons/README.md). The import icon descriptions below record the earlier versions.

Balance retune: [Zubr +8 comparison and proposed roles](imported_weapon_balance.md).
The balance figures below describe the initial import.

Four guns of Survarium 0.31a2 (2015) for release 41, the content pack `weapons-031@5` (its row in
`release/content.tsv`). The owner's scope (2026-11-01): four batches of ported weapons, this one
from 0.31a2; no premium or skin copies; machine-gun ammunition is the rifle round (no belt items);
stats are the donor's own, scaled by a peer both builds ship, and the guns sit in our live balance
(the SCAR-H's pierce is its round-mate's, the SOCOM 16's: the owner, 2026-11-01).

## The guns

| | dict id | config | lobby cell | kill feed icon | ammo |
|---|---|---|---|---|---|
| OTs-14 "Groza" | 2261 | `wpn_ots14` | [6, 5] | 72 | 9x39 PAB-9, SP-5, SP-6 (300, 301, 302) |
| FN SCAR-H | 2262 | `wpn_scar_h` | [5, 5] | 71 | Dedicated 7.62x51 AP in packs of 20 (2266) |
| VHS-2 | 2263 | `wpn_vhs_2` | [4, 5] | 70 | 5.56x45 (2201) |
| FN Minimi | 2264 | `wpn_fn_minimi` | [8, 5] | 74 | 5.56x45 belt (2265) |

- Configs at `gameplay/items/weapons/wpn_<name>.options`, so the edit lists' `wpn_*` globs (weapon
  details, movement, kill feed icon in the lobby) cover them. The batch's own edit list `weapon-camo-031-1`
  gives them a camouflage and a decal (as weapon-camo-1 and weapon-decals-1 the live guns), and its
  own pack `weapons-031-camo@2` their 20 `_aoac` masks (`scripts/weapons-031/camo.tsv`, the
  suggested parts; the SCAR-H's and VHS-2's sights a mask that paints nothing, `--bare-masks`, so
  no surface of a camouflaged gun fails to load its mask): no shared list or pack changes, as batch 2's `weapons-046-camo`.
- Lobby cell `[row, col]` of `LobbyMenu_WeaponIcons`: the donor's own cell (free in 0.26g); the
  donor's icon (a cell and its 8-pixel gap, 171x91) scaled into a 0.26g cell.
- Kill feed icon: the donor's number; the hud's `CombatLogWeapon<icon>` is 0.31a2's
  `icon_<icon + 1>.png` (`scripts/kill-feed-icons/make_weapon_icons.py`), the lobby's
  `KillFeedWeapon<icon>` made from it (`make_lobby_weapon_icons.py`). Both movies find an icon by its
  class name (batch 2's lookup, #948), so a batch adds bitmaps and classes and edits no list.
- Ids, cells and icons were claimed beside the other batches' (dict 2261-2264).
- Texts: `st_<name>_name` and `_description` in the ten localization tables.
- The card: the stock rifles' eight rows and the magazine (prop 134, after the rate of fire), which
  the live guns have from `weapon-balance-1` (a record of patch 21, so a new pack carries its own).
- Sellers and ranks: srp's rule (`prices.rs`): Renaissance (the donor's faction) at rank
  `item_level - 5`; `base_cost` is the price; `drop_weight` the drops.

## Ammunition

A default kit has three ammo bags.
- **FN Minimi:** the 5.56x45 is 30 rounds a bag, so a kit held 90, under its 100-round box: it
  spawned 90/0 and never reloaded. It takes its own 5.56x45 belt (2265, `ammo_5.56x45_belt`):
  100 rounds in 2 bags, as the PKP's 7.62x54 7N13 and the RPD's 7.62x39 belts; the 5.56's ballistics,
  200 a belt (2 a round, as the 5.56), srp 100; Renaissance, rank 9 (no rank needed for ammo).
- **FN SCAR-H:** `scar-ammo-1.json` gives it only dedicated 7.62x51 AP in packs
  of 20 (2266, `ammo_7.62x51_scar_ap`), copied from shared AP (52). `ammo.tsv`
  sets 20 rounds per bag and 0.5 kg; `items.tsv` sets 200 silver per pack,
  preserving shared AP's price and weight per round. Renaissance sells it at
  rank 10 (ammo needs no rank), and its icon is the shared AP icon [7, 1].
  Three bags hold 60 AP rounds: 20 loaded and 40 spare. The weapon's magazine
  and ballistics stay as they were. The [balance audit](imported_weapon_balance.md)
  used AP for SCAR's combat figures; this changes supply endurance only.
  SCAR's three old compatibility pairs (51, 52, 2254) are removed. G3SG/1 keeps
  standard 20-round packs (2254, Black Market rank 5), whose description now
  names only G3; SCAR's Renaissance listing moves to the new AP pack. Shared
  standard/AP ammo (51/52) retains its size, price and other compatibility.
  Existing inventory stacks are not converted or deleted: players must select
  the new AP ammo for SCAR. Id 2266 was used only by unreleased weapons-031@4
  and freed in @5; no released inventory item is reassigned.
- The cards: a new round on a native round's config shows its own bullets in a pack (prop 16) and
  weight (prop 15), written by the port from its `clip_size` and `clip_weight` (the belt 100 / 1.0,
  the clip 20 / 0.5).
- With the default three bags, the Minimi gets one belt and no reserve (100 rounds take 2 bags), the
  same as the live PKP: accepted (the coordinator, 2026-11-01).
- Their rounds' sellers are `shop.tsv` rows, their prices `items.tsv`'s `base_cost`.

## Fixed after the first in-game look (2026-11-01)

- **Muzzle flashes:** the SCAR-H and VHS-2 named 0.31a2's `fx/weapons/m4a1`, in no 0.26g archive:
  the lobby crashed drawing them (exe+0x352308 after the cook of 'fx/weapons/m4a1' failed). They take
  `fx/weapons/ak_74` (the M4A1's, HK416's and AK-12's), and so does the Groza (the UZI's flash it
  had was barely visible; the Val's `fx/weapons/9x39` is a suppressed puff, the Groza is not
  suppressed). The port now refuses a config naming a resource no archive has (resource-porter
  `resources.rs`).
- **Groza's sights (@12, shipped as @13):** it took the Val's `aim_offset` (0.028) from its template; the donor has
  none, so the port leaves it out. The camera sits at the `aim` locator and looks along the gun
  (the in-game view's proportions match a render from it). One sights the post through the rear
  plate's top notch; the ring below it is a boss, as in 0.69d0. The aim locator is the donor's own,
  (0, 0.0868, 0.178), unturned, at the front post tip's height: the post stands 16 px (of 960) in
  the notch with its tip at the screen's centre, as 0.69d0 shows it. @6 had moved it 3 mm down
  and turned it -0.0123 rad (0.7 degrees) up through the manifest's `file_values`. The eye then ran
  along the notch floor to the post tip, and the plate covered the post (1 px showed; a player's
  report: "front sight not drawn"). The port now has no `file_values` for it. 0.69d0 ships the
  same model with the locator 13.4 cm further back (0, 0.0868, 0.3117). At 0.26g's field of view
  that made the gun too small (@10 and @11, never released), so the eye stays where 0.31a2 put it.
  Renders in `private/renders/groza-sights/`. Not yet seen in a game.

- **Screen shake (@7):** `peer_scale.py` had scaled the shake keys too, by the M4A1, whose shake
  0.31a2 cut to 0.2/0.4 against our 0.8/1.0. That gave the SCAR-H 2.4/3.0 (3 times any live gun)
  and the VHS-2 0.6/1.2. Shake is now never scaled. The port takes the template's, which is the
  class's live value (resource-porter `config::SHAKE_PATHS`), so all four guns have the live
  automatics' 0.8/1.0 x8.

## Where the numbers live

The pack carries the guns' assets. Their stats live in `scripts/content-edit/weapon-stats.tsv`
(rows `wpn_ots14`, `wpn_scar_h`, `wpn_vhs_2`, `wpn_fn_minimi`), and the two rounds' in `ammo.tsv`
(`ammo_5.56x45_belt`, `ammo_7.62x51_20`). The rows were dumped from `weapons-031@7`'s configs
(`content_edit.py --dump-stats`), so they change nothing the pack set. To retune a gun, edit its
cells, not the port manifest or the pack. The build writes the cells and computes the cards. The
cards keep their nine rows and gain the detail rows every live gun has (reload, distances, kicks,
spread, sway, ...).

## Balance

Each gun's values are its 0.31a2 config's times the live peer's over the peer's in 0.31a2
(resource-porter `scripts/weapons/peer_scale.py`), so it differs from the peer as it did in its
build. Recoil kicks scale in min/max pairs; durability, repair and melee are 0.26g's own. The
balance is the manifest's `set` (resource-porter `ports/weapons-031a2-batch1.json`).

| | damage | pierce | rpm | mag | level | faction (rank) | price |
|---|---|---|---|---|---|---|---|
| **OTs-14 "Groza"** | 0.50 | 0.30 | 700 | 20 | 13 | Renaissance (8) | 80,000 |
| AS "Val" (peer) | 0.50 | 0.30 | 800 | 20 | 13 | Renaissance (8) | 76,000 |
| 9A-91 | 0.50 | 0.30 | 700 | 20 | 13 | Renaissance (8) | 84,000 |
| **FN SCAR-H** | 0.59 | 0.40 | 625 | 20 | 15 | Renaissance (10) | 125,000 |
| M4A1 (peer) | 0.50 | 0.32 | 750 | 30 | 12 | Black Market (5) | 75,000 |
| SOCOM 16 (7.62x51) | 0.60 | 0.40 | 300 | 10 | 15 | none | 75,000 |
| AK-12 | 0.50 | 0.40 | 650 | 30 | 15 | Renaissance (10) | 115,000 |
| **VHS-2** | 0.50 | 0.34 | 850 | 30 | 14 | Renaissance (9) | 115,000 |
| M4A1 (peer) | 0.50 | 0.32 | 750 | 30 | 12 | Black Market (5) | 75,000 |
| HK416 | 0.55 | 0.34 | 850 | 30 | 14 | Black Market (5) | 120,000 |
| **FN Minimi** | 0.41 | 0.36 | 800 | 100 | 14 | Renaissance (9) | 120,000 |
| PKP "Pecheneg" (peer) | 0.45 | 0.45 | 650 | 100 | 15 | Renaissance (10) | 125,000 |
| RPK | 0.50 | 0.30 | 600 | 45 | 11 | Edge (6) | 69,300 |

- **Groza:** the Val's damage and pierce (the same 9x39 in both builds), 100 rpm slower, a faster
  aim (0.2 s) and a tighter spread (0.2); the Val's level and drops (30), priced between the Val and
  the 9A-91.
- **SCAR-H:** the heavy round: damage and pierce as the SOCOM 16 on the same 7.62x51 (0.6 and 0.4:
  one round, one penetration, the owner, 2026-11-01), automatic at 625 with 20 rounds; the donor put it two levels over its M4A1, so the top
  level (15) beside the AK-12 and the PKP, drops 40 as the level-15 rifles.
  Retune (the owner, 2026-11-08: nerf; 0.49 "a bit too far"): damage 0.59 -> 0.5, the AK-12's. With its AP
  the torso takes 3 hits at 10 m against armour 60 (was 2, 96 ms), 5 at 50 m (was 4); against armour 80
  still 3 at 10 m, 6 at 50 m (was 5).
- **VHS-2:** a 5.56 rifle at the HK416's rate and pierce with less damage (0.5 against 0.55) and a
  tighter spread (0.1 against 0.15): the HK416's level (14), just under its price; drops 60.
- **Minimi:** the light machine gun of the 5.56: more rounds a minute than the PKP (800 against 650),
  less damage and pierce, a faster reload (8 s against 10), the PKP's slowdown (-5 % movement); one
  level under the PKP; drops 60 as the PKP.
- Reload 3.17 s (Groza), 3.5 s (SCAR-H), 3.0 s (VHS-2), 8 s (Minimi); weights 2.7, 3.7, 3.3, 6.5 kg
  (the donor's: the peers weigh the same in both builds).

## How it was made

resource-porter's weapon port (branch `feat/weapons-port`, `docs/weapons-026.md`):

```sh
vostok-port port-weapons --manifest ports/weapons-031a2-batch1.json --donor v031a2.split \
  --target-db <stock resources.db> --target-patch <every listed patch> --output <bundle>
vostok-port pack from-bundle --base-db <stock> --base-patch <every listed patch> \
  --bundle <bundle> --output $SKHID_PACKS_DIR/weapons-031-5 --id weapons-031 --version 5
```

- The art is 0.31a2's: meshes (0.31's statistics chunk 110 dropped), textures, sounds as they are;
  clips with their event headers widened from 12 to 16 bytes; materials and sound options on
  native keys; the firing sounds' reverb layer an outdoor collection, as patch 19's weapons.
- The configs are the donor's on a native template (Val, AK-12, AK-12, RPD): 0.31's state types
  mapped to 0.26g's (reload 313 is 314, grenade 315 is 316, melee 320 is 321), the grenade, melee
  and recoil parts that play 0.31's shared `weapons/base` clips the template's (0.26g's
  `weapons/ak`), 0.26g's melee and durability.
- The same tool, run on the 0.29a3 donor, gives patch 19's 33 weapons: no unexplained difference
  (`vostok-port compare-weapons`; resource-porter `ports/weapons-029a3-patch19.known.tsv` says why
  each one differs and which side is right).

## For the muzzle step

The guns' muzzle as retail 0.69d0 places devices (`muzzle-devices-problems`): its `muzzle_point`
(model space, metres) and whether our 0.31a2 mesh has the device built in (its triangles are what
the 0.69d0 base mesh lacks, matched against 0.69d0's default module).

| | 0.69d `muzzle_point` | built-in device in our mesh |
|---|---|---|
| OTs-14 | (0, 0.0041, -0.0930) | none: 0.69d's muzzle slot (`groza_lock_muzzle`) has no default device; the mesh's extra parts are 0.69d's handguard module |
| SCAR-H | (0, 0.0671, -0.5691) | yes: `scarh_i05x28_muzzlebrake` (458 vertices, 6.9 cm); also the sights modules |
| VHS-2 | (0, 0.0147, -0.3225) | yes: `vhs2_i05x28_muzzlebrake` (192 vertices, 5.2 cm); also the sights modules |
| FN Minimi | (0, 0.0507, -0.5143) | yes: `fn_m249minimi_i05x28_muzzlebrake` (204 vertices, 8.1 cm), all of the extra vertices |

## Not in this batch

- Scope fit variants (`fitn_*`, `weapon_scopes.md`): a variant needs a scoped copy of the gun's
  model placed by its rail; none of the four has a variant yet, so they take no scope. The
  attachments track (scopes, muzzle devices, grips) places modules at the gun's locators.
- The weapon kit (#893): when it merges, the configs take its kit data as the live guns do.
- Not seen in a game yet: first and third person, reload, fire, the lobby icon and kill feed.
