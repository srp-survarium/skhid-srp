# Kit presets

The owner, 2026-10-23: "we need to build presets (5), it is a drop down menu in Character, the
idea is to people have presets for different equipment levels and allow them to join low level
lobbies (they will see a popup that they can join with a lower level preset)"; "preset is a
snapshot of 3 kit contents"; items "just like normal kits are"; on join "Apply then join", "and
then they join with a new one". Then, the same day: "no need for set or apply, just choose preset
applies it, and adding equipment to it saves things". Code: `session::v026g::presets` (the rules),
`rules::fill_kit`, `persist/presets.rs`, `lobby-server` `presets.rs` and `rooms.rs`; the lobby:
`LocalPlayerProfile` (the dropdown), `RoomsWindow` (the popup). Wire: `lobby.md` §2.16.3 and
§2.16.4.

- **Five presets an account, one active,** kept with it in srp.db (`g26_preset_slots`,
  `g26_preset_active`).
- **A preset** is the contents of the three kits (Profile 1, 2, 3: every slot, weapons, ammo,
  armour, pockets): the owned item in each slot, and for a stack its units. Not the kits' names,
  nor which kit is played [choice].
- **The active preset is the kits.** It is kept nowhere else, so every change of a kit (a move,
  the ammo window, a purchase dropped onto a slot, a sale or craft that takes an item, a match's
  ammo, a repair of the kits at load) is the active preset's, with nothing to save and no path
  that could miss it [choice: no copy kept in step]. The other four are snapshots, as they were
  left.
- **Items, as kits:** a slot names an item the account owns. Kits share one inventory: an item may
  be in each of the three kits (one slot of a kit at most). Presets follow it: the same item may be
  in several presets. A preset reserves, copies or moves nothing.
- **Choosing a preset** in the dropdown (the only control): the kits stay with the preset left as
  its snapshot, then the chosen one is put on the three kits and becomes active. One server
  action, all or nothing: each kit emptied and filled through the lobby's kit rules
  (`rules::fill_kit`, the moves' own `relocate`): the item fits the slot, one item one slot of a
  kit, a covering item takes off what it covers, one pocket an item type.
  - An item no longer owned (sold, crafted away, used up): its slot stays empty; the player is
    told how many slots stayed empty.
  - A stack: the units kept, at most what is owned now. A stack used up and bought again is a new
    item: the stack of that type owned now is taken [choice].
  - A type in two pockets (data from before the rule): the first pocket keeps it, the other stays
    empty (counted as empty).
  - Never buys (the ammo window's moves buy what is missing; choosing does not), never makes an
    item or money, never takes anything the account does not own.
  - Refused (nothing changes) while the player is in a match or on its way to one.
  - **An empty preset chosen starts as the kits worn:** the kits stay as they are, and the preset
    left keeps the same contents as its snapshot [choice: the least surprising; nothing comes off
    the character, and the new preset then changes on its own].
  - A preset left with nothing in its kits is empty.
- **Existing accounts:** preset 1 active (their kits as they are), presets 2 to 5 empty.
- **A preset's level:** the rooms' measure (`matchmaking::gear_level`, the refusal "gear over the
  max level"): the highest kit level of the three kits that can play (all three, not only the
  active one). The active preset's is the kits' own; another's is that of the kits it would give
  now (sold items gone). A preset none of whose kits can play (no weapon, too little ammo) has no
  level and is never offered.
- **Character panel:** a dropdown left of its title with the five presets ("Preset N: level L",
  "Preset N: empty", "Preset N: can't play"), the active one shown. Picking another chooses it.
  A choice that left slots empty, or was refused, says so in the game's message box ("N slots
  left empty: those items are gone.") [choice: no line of its own].
- **Joining a room** (the owner's, 2026-10-23: "If the person joins a high level room with low
  level preset, they get a warning which will switch them to the biggest one: enter anyway, switch
  and enter | for low level ones it is exit or switch and enter"; "we choose the preset which has
  the highest allowed level"):
  - The preset offered is the saved one with the highest gear level the room allows (at most the
    room's max). Ties are broken by the lower preset number.
  - A room's gear range is its max level alone (the room's "level" is its members' match level,
    no cap). A room of any max level (0) offers nothing [choice].
  - **Room allows more than the kits:** gear at most the max, and a preset higher than the gear
    fits: "Enter anyway" (the kits as they are) or "Switch and enter". None higher: joins, no popup.
  - **Gear over the max:** a preset fits: "Exit" (no join) or "Switch and enter". None fits: the
    refusal as before (code 12).
  - "Switch and enter": that preset chosen (active, its kits put on), then the join.
  - Where: Join in the list (a friend's "Join room" goes through it), and Play (Quick join), for
    the room Play would take now. "Switch and enter" from Play chooses, then Play picks again by
    the new gear [choice]. Not the queue of a full room, "Switch room" (it already takes the
    gear), nor making a room.
  - The server decides the popup (the movie asks before joining); it shows over the Rooms window
    (Play opens it), the view under it inactive until a button. "Switch and enter" is one server
    action: choose, then the join; a choice refused joins nothing (code 14).
- **Gone with the owner's change of the same day:** the Save and Apply buttons, saving into a
  preset by hand, an "apply" that left no preset active, and their requests on the wire.
- **Texts:** short, the lobby's five languages; the movie's own (`LocalPlayerProfile`,
  `RoomsText`), no content edit.
- **Wire:** srp's `3B` ranking starts and `91` rows, as the rooms; no exe change.
- **Release:** server and lobby movie together.
