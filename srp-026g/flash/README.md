# The game's Flash movies as source

The owner's direction, 2026-10-20: "we first use that to compile all .swf, then we apply all we have
already", "this can be stored in a repo"; 2026-10-21: "we now got the source code" "and we just
modify it".

Each folder is the complete ActionScript of one movie of `resources.db`, first exported from the
movie as revision 24 has it (FFDec, corrected until every script compiled back to revision 24's
bytes):

| movie | scripts | the original (`movie.json`) |
| --- | ---: | --- |
| `lobby_menu` | 878 | `resources.db.patch_24`, sha256 `3ca94f2a…` |
| `hud` | 287 | `b924b29d…` |
| `main_menu` | 183 | `edc60a3d…` (patch 22: the FOV slider) |
| `login_menu` | 86 | `20ab1d22…` (patch 12: the login hint) |
| `replay_player` | 82 | `d3ffb741…` |
| `debug_animations` | 53 | `67a7576c…` |
| `player_icons` | 34 | `36aa3533…` |
| `cursor` | 2 | `1e9bc936…` |

- `src/` is the source (LF line ends). A change is an edit of it: a new class is a new file, a class
  taken out is a deleted file.
- `movie.json` names the original movie (its archive entry and sha256). Its graphics, fonts and
  sounds are not source: the build takes everything but the code from it.

## Cleaned up over the export

The trees are no longer FFDec's text as it came out; they are meant to be read and edited as
ordinary source (the owner, 2026-10-21: no decompilation matching needed now).

- **Names.** FFDec calls a local `_loc2_` and a parameter `param1` where the bytecode has no name.
  Every one has a name now. Where possible it is the original one:
  - another movie's copy of the same function has it in its debug info (the hud, player-icons and
    replay movies carry debug info);
  - the movie's own debug info or parameter table records it and FFDec ignored it.

  Otherwise the name comes from how the variable is used. The movies are built without debug
  information, so a local's or a parameter's name is not in the compiled movie: renaming one
  compiles to the same code.
- **Named numbers.** A number that stands for something (an item category, a slot, a state, a
  channel, a code of the server, a sound, a cursor, a key) is a static constant. The groups are
  enum-like classes in each movie's `survarium/constants` (the lobby: `ItemCategory`,
  `SuperCategory`, `ItemPlace`, `ProfileSlot`, `ModifierType`, `Faction`, `ChatChannel`,
  `NotificationType`, `ClanState`, `RoomCode`, `PvbCode`, ...), each with where its values come
  from. Numbers of one class only (a wire offset, a text table's index) are constants of that class.
  A meaning that could not be established is named by its use and says so. Geometry, colours,
  timings, timeline frames and arithmetic stay literal.

  | constant | where it is defined |
  | --- | --- |
  | UI sound ids (`sound_play`) | `survarium.constants.UiSound` (the order of `gameplay/ui_sounds`) |
  | key codes | `flash.ui.Keyboard`; the others `survarium.constants.KeyCode` |
  | mouse buttons | `scaleform.gfx.MouseEventEx` |
  | item categories | `survarium.constants.ItemCategory` (names: `gameplay/item_categories` and the shop filters) |
  | `ItemFactory.getCategory`'s groups | `survarium.constants.SuperCategory` (the shop's tabs) |
  | `ItemData.descriptor` | `survarium.constants.ItemPlace` (inventory, shop, bundle; below 100 the profile slot) |
  | golden weapons and their plain weapons | `survarium.constants.ItemDictId.GOLDEN_WEAPONS` / `GOLDEN_PLAIN` (srp's `golden::GOLDEN_WEAPONS`; a Rust test keeps them the same) |
  | premium armour, its plain pieces and premium bonuses | `survarium.constants.ItemDictId.PREMIUM_ARMOUR` / `PREMIUM_ARMOUR_PLAIN` / `PREMIUM_ARMOUR_BONUSES` (srp's `golden::PREMIUM_ARMOUR`; a Rust test keeps them the same) |
  | the bonuses srp gives a new golden item, as a modification | `survarium.item.ItemFactory.BONUS_UI` (the modifications' `ui_desc`; a Rust test keeps them the same) |
  | gamepad buttons | `InputDelegate`'s `GAMEPAD_*` (the key codes Scaleform gives them) |

```sh
# Build a movie: one Flex run over the whole tree, its code in place of the original's (seconds;
# needs private/actionscript/tools.json: a JDK and Flex 4.16.1, tools/actionscript/README.md)
python3 tools/actionscript/flash_tree.py build srp-026g/flash/<movie> <original.swf> <new movie.swf>
# Cut the original from the archive movie.json names
python3 tools/actionscript/flash_tree.py original srp-026g/flash/<movie> <archive> <original.swf>
```
