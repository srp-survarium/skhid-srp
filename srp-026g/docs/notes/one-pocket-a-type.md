# One pocket an item type

The owner's rule of 2026-10-21. Code: `session::v026g::account` (`doubled_pockets`,
`Account::merge_pockets`, `repair`), `rules::move_items` / `buy` / `check_play`
(`ActionError::PocketTaken`), the load (`GameAccounts::with_repository`); the lobby:
`LocalPaperDoll` (`pocketOf`, `pocketUnits`, `selectAgeements`, `getSlot`), `ItemPaperDollSlot`,
`InventoryList`, `AmmoShareLite.updateElement`.

- **The rule:** "people should be able to stack the same item type only once". In a kit, an item
  type (one dictionary item: a grenade, a medkit, a booster) is in at most one pocket. Its units
  in that pocket are capped by the pocket's size, as before.
- **Pockets:** profile slots 13..18 (`loadouts::POCKET_SLOTS`), the only slots for drugs and
  boosters (category 10), traps (11) and grenades (24), all stacks. The lobby shows the first
  `pocket_slots` of them; the rest are artefact places (locked, empty) or hidden.
- **Not covered:**
  - ammo slots: the ammo window spreads one ammo over the ammo bags;
  - weapons and armour: each one is one item in one slot already;
  - other kits: each kit has its own pocket for a type.
- **Why:** a booster works by being carried. At every spawn each item in the 23 slots adds its
  modifiers to the player's lists, and the match sums them (`loadouts::BOT_RADIO`). A booster
  fills a whole pocket, so N pockets gave its bonus N times:
  - `speed_booster`: +5 % movement and +10 % stamina regeneration each;
  - `respawn_booster`: -20 % respawn time each (five pockets: no respawn delay at all);
  - the others likewise.
  The server never checked that the pockets of a type held no more units than were owned, either.
- **Server refuses** a move (`46 00`, the ammo window, a purchase dragged onto a pocket) that
  leaves a type in two pockets of the kit. The check is on what the whole request leaves, so the
  ammo window (every pocket emptied, then filled again) and two pockets changing places pass.
  Nothing is changed or bought. The text is the game's `st_cannot_relocate_item` (the game has
  none of its own for it). Play (`check_play`) refuses such a kit too.
- **Kits saved before the rule,** at load (logged once) and in every `repair`:
  - the first pocket of a type keeps it, with the most units any of its pockets held;
  - the type's other pockets are emptied.
  - Nothing is lost or made: a pocket only names units of the inventory's stack, so the units of
    an emptied pocket stay in the storage.
  - The most, not the sum: all pockets of a kit have one size. Each pocket held what fit it.
- **Pocket size** (`Account::pocket_size`, `fit_pockets`; the fix of 2026-10-25):
  - The exe's sum (PROVEN, `kit_parameter` 0x2F7660 and `item_parameter_modifier` 0x2F7470): the
    player's `pocket_slot_size` (1), plus the skills' `pocket_slot_size_modifier` (no skill has
    one), plus the modifier in the default and own modifications of the items in the pockets and
    the armour slots (0..6). Weapons give nothing. Only 1170 (+1) and 1171 (+2) carry it. They
    are defaults only and never roll (drop weight 0): Chemical protection pants, some vests and
    backpacks.
  - An item that `fills_whole_pocket` takes one unit.
  - The exe trims the pockets itself only in its own move (`relocate_item`: gear taken off, a
    pocket beyond the new size is asked for the size). A purchase dropped onto the paperdoll
    trims nothing, and a match takes a pocket's units as they are (at most those owned).
  - Server: every kit change (`sync_kits`: moves, purchases, presets, the load) trims a pocket
    to the kit's size. The rest stays in the storage.
  - The bug it fixes (Chandrian, 2026-10-25): two kits with the same armour instances. One held
    medkit x2 in a one-unit pocket, left over from gear that had let two in. The lobby drew it
    as two units, so that pocket looked twice as deep.
- **Lobby, dragging from the inventory or the shop:** only the pocket that holds the type lights
  up. Another pocket does not, and a drop there snaps back. Dropped on its own pocket, the drop
  adds one unit up to the pocket's size (one for an item that fills a whole pocket). A pocket of
  another type is replaced, as before.
- **Lobby, dragging between pockets:** a drag from a pocket may go to any pocket (a move or a
  swap leaves one pocket of the type).
- **Lobby, a double click or "wear":** goes into the type's own pocket when the kit has one.
- **The "4th grenade empties the slot" bug:** a client bug.
  - The setup: grenades in two pockets (2 and 1), and the second pocket's slider moved.
  - The cause: `AmmoShareLite.updateElement` took the first pocket of the type for the moved
    one. So the request named the first pocket twice and left the second out, and the server
    emptied it (the units stayed in the storage).
  - The fix: it takes the target pocket's own item first. With one pocket a type the case no
    longer arises.
- **Match:** nothing a match computes changes. The kit sent at the start is what the match plays,
  so no `next` row in `simulation_changes.tsv`.
- **Release:** server and lobby movie (`build_lobby_026g.py`). The server alone already refuses;
  an old lobby lights up a second pocket and the drop is refused with the text.
- **Revision 28's bug, fixed in the next:** every drop on a pocket left it empty ("medkit x0",
  then gone; the owner, 2026-10-22). The request (`46 00`, from the local packet log:
  `... 64 0E 00000000 01000000`, slot 100 -> 14, count 0) asked for no units, which the server
  takes as leaving the pocket empty.
  - The cause (PROVEN): `relocate_item` (0x288D60) reads each field of a move through the GFx
    getter and then the value's raw low bits (`count`: `movzx edi, word [esp+0x90]` at 0x289234),
    whatever its type. An int arrives as itself; a Number (a double) has 0 in those bits for any
    small whole number. #277's `count: <cond> ? pocketUnits(...) : 1` reached the exe as a
    Number (an int or a uint return type alike: tried); the literal `count: 1` it replaced was an
    int (an experiment build with the literal back sent 1).
  - The fix: every move the lobby builds for the exe is made by `survarium.data.Relocation.move`,
    whose fields are typed parameters (an expression is coerced to an integer at the call): the
    paper doll's drops, the inventory's double click and drop region, the relocate question, the
    character's `relocate_to_paperdoll` and the ammo window's rows. Never build such an object
    literal by hand.
