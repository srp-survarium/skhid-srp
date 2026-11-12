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
  - The most, not the sum: all pockets of a kit have one size, which the server does not know
    (gear, modifications and skills set it in the exe). Each pocket held what fit it.
- **Lobby, dragging from the inventory or the shop:** only the pocket that holds the type lights
  up. Another pocket does not, and a drop there snaps back. A drop fills the pocket (below, "A
  full pocket stays full"). A pocket of another type is replaced, as before.
- **Lobby, dragging between pockets:** a drag from a pocket may go to any pocket (a move or a
  swap leaves one pocket of the type).
- **Lobby, a double click:** goes into the type's own pocket when the kit has one, else into the
  next free pocket, and fills it. With every pocket taken by other types: a kit of two pockets or
  more picks the item up, for the player to choose the pocket it replaces; a kit of one pocket
  replaces what is in it, as before. "Wear" asks for the count, as before.
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

## An RGD-5 takes two pocket units (2026-11-05)

The owner: "each one takes two pocket slots". A pocket holds at most 5 units (`MAX_POCKET_UNITS`).

- **The rule:** a pocket holds `max(1, size / 2)` RGD-5s, `size` the units it holds of any other
  stack: 1 for size 1 to 3, 2 for size 4 and 5. Never more than 2. Only the RGD-5 (`rgd-5`, dict 5);
  smoke grenades, traps, medicines and boosters as before.
- **The data:** `parameters/pocket_units` 2 in `rgd-5.options`, srp's key (`throwables.tsv` column
  `pocket_units`, none 1). Read once into `item_params::PocketShare` (`Unit`, `Pair`, `Whole`: the
  boosters' `fills_whole_pocket`); a value other than 1 or 2, or one beside `fills_whole_pocket`,
  stops srp at start.
- **The lobby:** the exe gives the movie a fixed set of an item's parameters
  (`fill_items_dictionary_impl` 0x286410: `fills_whole_pocket` yes, other keys no), but every card
  line. `pocket-units-1` carries `pocket_units` to the item descriptors as card property 166, not
  comparable (no row on the card). `QuickSlotSize.of` reads it (`PAIR`), `PlayerInfo.pocketUnits`
  halves the pocket: drops, the slider, the ammo window, the buy dialog's pocket count.
- **The server** holds an RGD-5 pocket to the kit's own pocket size, as every other slot the
  layout limits (below, "Every kit held to its layout"). The units over it stay in the storage,
  nothing is lost. Bots' pockets hold 2 units, so a bot carries one RGD-5 (it had two), and its
  grenade play throws one a life.
- **Kits saved before the rule:** the release that shipped it emptied every RGD-5 pocket of every
  kit and preset once, in the database with srp stopped (kit revisions raised).

## Every kit held to its layout (2026-11-09)

What a kit's worn items and the account's skills open (`loadouts::KitLayout`): the pockets, the
units each holds of an item (the pocket size, one more medicine with Additional Medicine, half for
an RGD-5, one booster), and the second weapon (none with Deep Pockets or Supply Pocket, no second
double-handed one without Terminator).

- **The rule:** the server keeps every kit to it (`Account::keep_to_layout`,
  `KitLayout::kept`): at the end of each move request, on the gear the whole request leaves
  (`relocate`: moves, buys onto a pocket, presets put on), in every `sync_kits` (after a match, a
  modification, a sale; at load through `repair`), and for kits and presets when the skills
  change (`change_skills`). What the layout does not open is left out, not refused; it stays in
  the storage.
- **Why:** the server took it back only when a skill changed, and capped a pocket at the largest
  one otherwise (5 units, 2 RGD-5s). Any later request put it back: the owner's reports of
  2026-11-09, "there is a way to get more grenades", an extra medkit kept after Additional
  Medicine was taken back, Terminator's second rifle after Terminator; #bugreports "Secondary slot
  changing" (2026-10-31: a second weapon played with Deep Pockets). The requests that do it:
  - the exe's own trim when gear that shrinks the pockets comes off
    (`process_default_relocate_item` 0x289B04: `min(units, pocket size)`, `fills_whole_pocket` 1)
    knows no `pocket_units` and asks to keep 2 RGD-5s in a pocket of 2 or 3;
  - a lobby whose skills changed may still count what they opened.
- **Confidence:** the exe's sum (`apply_kit_parameter` 0x2F7660 / 0x2F7470) is B + the kit's float +
  the worn gear's and pockets' own parameter and their modifications' `lobby_info` values (row loader
  0x2C9E10; slots 0..6 and 13..18); on revision 56 `KitLayout` (type-1 modifiers over every kit item)
  equals it: own parameters are 0 in every item, `lobby_info` equals the type-1 sum in 182 of 182 rows,
  no weapon or ammunition carries one (DERIVED). It differs only below 0: the exe keeps the low byte of
  the truncation (no clamp: -1 is 255), `KitLayout` gives 0. `KitLayout` is the count the new-account
  grant (`new_account.rs` checks every preset against it) and the bots'
  kits are built with. A kit the server holds to a smaller layout than the exe's would show fewer
  units than the lobby allows, never more.
- **Match:** the crate refills a pocket to the kit's count (`resupply_from_profile` 0x2FDC10, the
  stack's `vtable+0x40` 0x2FFCE0: up to the kit slot's count, from the stock), never to the pocket's
  size, so it keeps the rule. The kit is sent at the start: no `next` row.

## A full pocket stays full (2026-11-10)

The owner: a double click on a grenade or a medkit "should fill in free next slot with it", "fill
in completely", "always to the max available pocket level", "even if they add a skill or
equipment". Asked: with fewer units owned than the pocket holds, it carries what is owned (nothing
is bought); the slider may still lower a pocket.

- **Putting in fills** (the lobby): a drop from the inventory on a pocket and a double click ask for
  all the pocket holds of the item (`LocalPaperDoll.pocketUnits`: `PlayerInfo.pocketUnits`, at most
  the stack owned). Never more than owned: `move_items` buys what a move asks for beyond the stack
  (the ammo window's way), and a put must not spend silver.
- **A full pocket follows its size** (the server, `KitLayout::followed`): a pocket holding all its
  kit's layout let it hold of its item holds all the new layout does, as far as the units owned go.
  - gear put on in a move request (`Account::keep_to_layout_after`): only pockets the request left
    as they were. A pocket the request itself set (the slider, the ammo window, the exe's trim) gets
    what it asked for, held to the layout;
  - a skill taken (`Account::change_skills`): every kit, and every preset (a preset's units are held
    to the stack when it is put on);
  - units bought or won (`Account::stack_grew`): a pocket that carried all the units owned carries
    the new ones too, up to its size.
- **A lowered pocket stays:** fewer units than the pocket holds, with more owned, is the player's
  choice, and nothing raises it. Nothing is stored for it: "full" is read off the kit at each
  change. So two cases count as full where the player lowered the pocket:
  - lowered to exactly what a smaller layout holds, when the layout grows again (2 medkits in a
    pocket of 4, the vest off: a pocket of 2, full; the vest on: 4);
  - lowered to 2, the stack then down to those 2 (used, sold), and more bought: the pocket
    carried all that was owned, so it takes the new units.
- **Not followed** (the kit keeps its counts until the player changes it):
  - a modification crafted onto gear the kit already wears that makes its pockets or ammo bags
    larger (no move request, and both layouts are read after the change);
  - ammunition and the presets' pockets after a purchase (`stack_grew` is the kits' pockets').
- **Ammo** (the owner: "all items scale", "including ammo!"; asked: the player's split is kept):
  the ammunition slots (8, 9, 11, 12, 19..22) share the kit's ammo bags (KitLayout's ammo_bags: the
  player's 3, the worn items' ammo_bags_modifier and the skills' modifier 16), a clip taking its
  item's required_bags. When gear or a skill adds bags and the kit's ammunition took all the bags
  it had, each slot's bags grow in proportion, in whole clips (2 bags and 1 of 3: 3 and 2 of 5);
  the bags the rounding leaves go to the slots that lost most by it (KitLayout::spread_ammo).
  - ammunition that left a bag free is the player's choice and stays;
  - a move request that sets an ammunition slot itself (the ammo window, a weapon put on: the
    exe's own fill) gets what it asked for;
  - at most the rounds owned; nothing is bought. One stack in two ammunition slots: the two are
    not raised past the stack together (`Account::shared_stack_over`; a match hands out each
    slot's count);
  - the server takes no ammunition off when bags are taken away: it did not before either.
  - Confidence: the bag count is the same sum (index 16, 0x288E4B; B = `player/ammo_bags_count`,
    0x22112B; the row's `lobby_info/ammo_bags` +1Ch): DERIVED as the pockets'. A count above the exe's would fill more rounds than the ammo window
    shows room for.
- **A drop on the character** (the inventory's item dragged out of its window, `DragRegion`): a
  pocket item goes as a double click sends it, into its own pocket, else the next free one,
  filled. With no pocket free nothing is replaced. Ammunition is not placed so.
- **A weapon** double clicked or dropped on the character goes into a weapon slot that is free
  and may take it before one it would replace (`LocalPaperDoll.getSlot`: the slots
  `selectAgeements` lights up, an empty one first). So Terminator's second double-handed weapon
  and a second slot a skill closes are the paper doll's own rules, not this one's.
- **Match:** the kit sent at the start is what the match plays: no `next` row.
- **Release:** server and lobby movie. Played by the owner on 2026-11-10 (the lobby side).

## The pocket's units shown (2026-11-10)

The owner: "people are confused why they have 3 slots but only 1 grenade".

- **A pocket's cells are its units** (`ItemRendererQSlot`, `AmmoSlider.cellUnits`): an RGD-5 in a
  pocket of 3 shows 3 cells, not 1. Of the two cells a carried RGD-5 takes the first is drawn as
  any filled cell, the second in the speed warning's pale red (`WeightEffects.WARNING_COLOR`); a
  cell left over that holds no whole RGD-5 (the third of 3) is pale red too. The slider still
  counts grenades.
- **Additional Medicine's unit** is the pocket's last cell for a medicine, pale green
  (`Colors.GREEN`), dimmer while empty.
- **The card line "Pocket slots"** (`ItemCompare.pocketDepth`, the text of property 166,
  `st_srp_inv_pocket_units`): 1, the RGD-5's 2, a booster's whole pocket
  (`st_srp_inv_pocket_units_whole`, an infinity sign). The card's first row; compared as the
  other rows, less is better.
- **The weight is a card's last row** (`ItemCompare.weightLast`), for every item (held Alt, a
  medicine's skill lines come under it). A card has nine rows: with more to show, the weight
  and the skill lines stay; the stats the item has none of go first, then the stat rows before the
  weight (`fitted`). A stat only the
  compared item has is 0 held Alt too: the skills change what an item has. The exe hands
  the lobby the properties' names, not their keys, so the weight is the property named as the
  kit's weight (`equipment_weight`; the two texts are the same in all ten languages).
- **A grenade's, a trap's or a booster's card has no row for a stat only the other item has** (a
  smoke grenade beside an RGD-5 shows no damage of 0): they differ too much.
- **Weapons, armour and medicines show every stat either item has**, in one order on both cards
  (`ItemCompare.cardRows`: the properties of the item that has more of them, then the other's
  own), a stat the card's item has none of as 0 (a Medikit beside a Science Medkit: disinfection
  0 %). The owner: medkits "are almost all the same". Before, a medicine's card left such rows
  out, and a weapon's listed its own first, so the rows did not face each other.
- **A booster and a medicine are not compared** (`LocalPlayerProfile.getRelativeData`): they
  share category 10 and nothing else.
- **Both cards' stats start at one height** for every kind of item
  (`ItemInfoRenderer.alignComparisonHeader`: the taller header's height; it was the weapons' only).
- **A second weapon slot a skill closes** shows the skills' lock in its middle
  (`LocalPaperDoll.showWeaponLock`: `SkillLockSpriteRed` without its circle), and a tip over the
  slot names the skill (`st_srp_weapon_slot_closed`: Supply Pocket when it is taken, else Deep
  Pockets, whose stock name key the lobby is not handed: `st_srp_deep_pockets_name` is its copy).

## The card that forgot what it was compared with (2026-11-10)

The owner: the same shotgun's card "sometimes" opened with every round listed and no colours,
"some state is leaking".

- **The cause:** `ItemCompare.setSecondaryWeapon` refused an item equal to the card's own, asked
  of the item the card held before the hover. After a hover over the equipped weapon itself, the
  next item's card was handed that weapon to compare with, refused it, and stood alone: no
  colours, no dash for a row only the other has, a shotgun's rounds all listed.
- **The fix:** the item to compare with is kept as given; whether it is the card's own is asked
  when the card is drawn (`getSecondaryData`).

## Three more of the same day (2026-11-10)

- **"Backpack: n, Storage: m"** under a kit stack's name on its card is gone (the owner). It read
  "Storage: -3" at times: `LocalPaperDoll.updateQuickSlots` set a pocket's units owned to 0
  when the inventory had no such item at that moment (its list being filled again). The count
  now stays as it was; the slider reads the same count.
- **The Speed and Weight tips had lost their title and their sections' dividers:** they are the
  card's defects separator (`WeightTipRenderer`: `criticalSeparator`), which the card hides
  since items never wear (2026-11-08). The tip shows the ones it takes.

## The drag that stayed on the cursor (2026-11-10)

The owner, testing the drop on the character: a medkit "got stuck at the cursor", with every
window closed too, and no button answered.

- **The cause** (from the packet log and the source: no request left the game for that drop):
  `DragRegion` called `localProfile._relocateFunction(...)`. `_relocateFunction` is a getter of
  `LocalPlayerProfile`; Scaleform throws "is not a function" when it is called in place on a
  variable of the class (through the interface `ILocalPlayerProfile` it works: the paper doll's
  slots). `DragManager.handleEndDragEvent` ran the drop handler before it stopped the drag, so the
  throw left the icon dragged for good, the slot it came from without its end.
- **The fix, both ends:**
  - `ILocalPlayerProfile.relocate(moves)` is the way to ask for moves; every caller uses it. The
    getter stays for the one place that hands the function on (the count question's `onOK`).
  - `DragManager` takes the icon off the cursor first, then tells the slots; the slot the drag
    came from hears of the end whatever the drop handler did.
