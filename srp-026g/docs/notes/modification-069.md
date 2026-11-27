# 0.69d0's weapon modification, prototyped on the AK-74M (markers over the gun)

The owner, 2026-11-27, after testing the scope places (#2717): "maybe we should port another weapon
attachment system to try it out, essentially fully the one in 0.69?", "with ui over gun?", then
"let's" to: one gun (the AK-74M, dict id 3), 0.69d0's own UI (the gun big in 3D, a marker over each
slot on the gun, a click opens the slot's list, a module brings slots of its own), the tree in the
lobby only, matches keeping today's weapon kit, no stats. If it works it later replaces the slot
pages, the second sight's Left/Right buttons and #2717's Receiver/Handguard buttons; they stay for
now. Client 0.26g; 0.69d0 the reference. Tags: PROVEN (read in the code or data named), [srp] our
choice, OPEN.

## What 0.69d0 does (PROVEN, read statically)

Sources: 0.69d0's lobby movie (`artifacts/range-069/donor-lobby.swf`, decompiled with FFDec 25:
`survarium.craft.ViewWeaponWindow`, `WeaponModuleSlot`, `SlotPoint`, `ModuleSelector`, `LobbyMenu`),
its x64 exe (`Survarium.zip`, sha256 `1e1ae04d...5448`, `tools/re-kit/v069d`) and its
`gameplay/game.options` (`vostok-port dump-db-config --db <src069>/resources.00.db --path
gameplay/game.options --leaves`; the full split list no longer reads whole, resources.22.db holds a
node kind the porter does not read).

- **The markers are the exe's.** Each frame of the workshop, `weapon_workshop::update_slots_positions`
  (0x2002745b0) walks the weapon's slots (`weapon_slot`, 0xE0 bytes; the root, the gun's `slots`,
  and every module's `slots_to_add`, `construct_weapon_parts_hierarchy` 0x2003870a0) and, for each
  with its byte +0x60 set (LIKELY `accept_module`: the gun's `fire_point`, `aim` and `body_aim`
  have it false and no marker), takes the slot's frame with `weapon_core::get_slot_transform`
  (0x20027526c, the same transform the module models are drawn at) and projects its place with
  `base_game_scene::point_to_screen` (0x2001f7b00, NaN when off the screen). A layout,
  `slots_locations_calculator` (0x2002cbcd0, animated over time, inside a screen region of
  fractions at 0x200ab7f38..44), then places each slot's icon box. The movie is told
  `root.weapon_workshop_set_slots_positions([{name, line_start_x, line_start_y, x, y,
  is_recently_added}])`: `line_start` the slot's point on the gun, `x, y` its box.
- **The movie draws them** (`ViewWeaponWindow.setSlotsPositions`): a `SlotPoint` at the slot's point
  (empty, filled, or critical for an `is_critical` slot), a `WorkshopSlotLine` (1.4 px, white at
  0.65, a blurred black shadow) from the point to the box's edge, and a `WeaponModuleSlot` box
  (64 x 64: the installed module's icon, the slot's name over or under it). New slots grow in
  (0.5 s, Back ease, 0.09 s apart: `is_recently_added`).
- **A click opens the slot's list.** The box's click calls `weapon_workshop_slot_selected(slot)`;
  the exe answers `root.weapon_workshop_set_slot_info({name, proposal: [modules]})`, and the
  `ModuleSelector` opens beside the box (right of it, else left at the stage's edge). A choice calls
  `weapon_workshop_attach_module(slot, item, dict, module)` or `..._detach_module`; the exe rebuilds
  the weapon and gives `root.weapon_workshop_set_slots` again: a module's `slots_to_add` are new
  slots with markers of their own (the AK-74M's Midwest forend brings `picatinny_grip`,
  `picatinny_scope_handguard`, `picatinny_tactical_l/r`; the dovetail rail adapter brings
  `picatinny_scope_medium`). Hovering a box tells the exe `weapon_module_icon_hover` (it lights the
  module), and the camera focuses a module by its `workshop/min_focus_distance`,
  `max_focus_distance`.
- **The AK-74M's own slots** (game.options `weapons.ak_74m.slots`): `charging_handle`,
  `scope_dovetail`, `pistolgrip` (critical), `forend` (critical), `mag` (critical),
  `m24x15_muzzle` (critical) take modules; 0.69d0 links it 53 modules.

So the plan holds on 0.26g: the lobby has the same projection (0.26g's `point_to_screen` 0x23DEC0,
which `update_player_region` 0x21DF00 already uses for the movie's drop region), and patch 161
already knows every slot's place on the gun (the modules table).

## What the prototype does

**The exe (patch 161, `attachments/markers.rs`, `vostok/bench_gun.rs`).** At `weapon::draw`
(`attachments.draw`), the weapon the lobby's own character holds (lobby menu+694h, its player
+D2B8h, the player's held item) while patch 45 has it at the bench is the bench gun. When its item's
dict id is a gun of the modules table and its model has the gun's bones, every slot of
`weapon_modules::slot_places` (the gun's own slots and each one its carriers bring, those carriers
seated as a fit seats them: the crate's one rule) is placed with the game's own `calculate_locator`
(the draw's world matrix and bones) and projected with `point_to_screen` on the lobby menu. The movie
gets `root.srp_slot_points(gun, slot, x, y, ...)` (pixels; a slot off the screen left out), only when
a point moved by a pixel, the set changed, or the gun came back to the bench. One state
(`Markers`: away, or told these points), one transition function. Drawing only: nothing of a
match's world.

**The movie (`survarium.upgrade`).** `ModuleTree` (the tables `ModificationSlots`,
`ModificationCarriers`, `ModificationWays`, written by `scripts/weapon-modules/modification_slots.py`
from the pack's record; check row `modification-slots-table`): the gun's slots where a player puts
something, what each takes, the carriers and the slots they bring, the slot of each way of an optic.
`ModificationView` (a child of the Attachments window, under its panel) shows, on the attachments
page of a gun with a tree, a `SlotMarker` at each slot the tree shows: the dot on the gun, a line,
a 56 px square with what stands there, in a row above or below the gun (the table's row), pushed
apart along the gun, new ones growing in as 0.69d0's. A click on a square opens a `ModuleList`
beside it: the gun's own part (Standard) or nothing (Empty), the carriers that go there, the
inventory's modules that go there (loose or on this gun); the one standing there is marked.

The AK-74M's markers (weapon-modules@11):

| slot | row | takes |
|---|---|---|
| `m24x15_muzzle` | below | Standard (its own brake) or a muzzle device |
| `picatinny_scope_handguard` (the tactical handguard's) | above | an optic on its handguard way |
| `picatinny_grip` (the tactical handguard's) | below | a grip |
| `forend` | below | Standard or the Midwest tactical handguard |
| `picatinny_scope_medium` (the rail adapter's) | above | an optic on its receiver way |
| `scope_dovetail` | above | nothing, the dovetail rail adapter, or the PSO-1 |

A slot a carrier brings is shown while that carrier stands: while something stands in a slot it
brings, else while the player put it on with nothing on it yet.

## What reaches a match, what is the lobby's alone

- **Reaches a match, as today (the weapon kit):** a choice of a muzzle device, a grip or an optic is
  the window's own put (`relocate_upgrade_item`, the same as a drop on the slot pages); an optic on
  the handguard rail is that put and then its way (`srp_set_upgrade_value`, #2717's 46 05) once the
  account holds it; Standard/Empty takes the module off (`drop_upgrade_item`). The carriers a kit
  module needs come with it as they always did (patch 161: a grip brings the tactical handguard, a
  rail optic the rail adapter). Camouflage, decals and the second sight are not on the markers and
  are as before.
- **The lobby's alone [srp]:** a carrier put on with nothing on it (the tactical handguard or the
  rail adapter alone). It is kept in the movie for the session, a weapon's (no server, no kit, no
  match, lost at a restart), and it only makes its slots' markers appear. **The gun is not drawn
  with it** until a module stands on it: patch 161 draws what the kit holds, and a carrier alone
  would need the crate's `Wanted` to name carriers (OPEN: the owner's pick). Taking a carrier off
  (Standard on the handguard, Empty on the dovetail) takes off what stands on it too.
- **Stats: none.** The prototype changes no value; the modules a match gets change what they always
  did.

## Not done (0.69d0 has it)

The camera's focus on a module, the hover light, the critical slots' look, the other 0.69d0 modules
of the gun (charging handle, pistol grip, magazines, lasers on the tactical rails: no models in the
modules table), 0.69d0's own box layout (a fixed two-row rule here), and any gun but the AK-74M.

## Test build and state

Built, not run in a game: patch 161's points, the markers, the lists and every choice. The local
setup's dictionaries were compiled over the setup's own (other sessions' keys kept).
