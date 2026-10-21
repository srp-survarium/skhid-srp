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
ordinary source (the owner, 2026-10-21: no decompilation matching needed now). The names below
still compiled to the original movies byte for byte; the constants classes do not, by design.

- **Names.** FFDec calls a local `_loc2_` and a parameter `param1` where the bytecode has no name.
  They have names now. Where possible they are the original ones:
  - another movie's copy of the same function has them in its debug info (the hud, player-icons and
    replay movies carry debug info);
  - the movie's own debug info or parameter table records them and FFDec ignored them.

  Otherwise the name comes from how the variable is used.
- **Placeholders that stay.** Some lobby methods keep `_loc2_`/`param1`: srp's earlier patches
  compiled FFDec text with debug info, so the shipped movie records exactly those names
  (`UpgradeWindow`, `ItemRendererInv.updateData`, `ItemInfoRenderer.set data`, ...). Some
  player-icons/replay interfaces keep their parameters too: their movie's parameter table holds
  unrelated strings. The lobby's own game classes (`survarium/**`, the top-level scripts and the
  timeline classes) are not cleaned yet: the lobby features are being moved into them first.
- **Named numbers.** The static constants used in place of literals:

  | constant | where it is defined |
  | --- | --- |
  | UI sound ids (`sound_play`) | `survarium.constants.UiSound` (the order of `gameplay/ui_sounds`) |
  | key codes | `flash.ui.Keyboard` |
  | mouse buttons | `scaleform.gfx.MouseEventEx` |
  | item categories and `ItemFactory.getCategory`'s groups | `survarium.constants.Category` (`BASE_*`) |
  | `ItemData.descriptor` | `survarium.constants.ItemPlace` (inventory, shop, bundle; below 100 the profile slot) |
  | gamepad buttons | `InputDelegate`'s `GAMEPAD_*` (the key codes Scaleform gives them) |

```sh
# Build a movie: one Flex run over the whole tree, its code in place of the original's (seconds;
# needs private/actionscript/tools.json: a JDK and Flex 4.16.1, tools/actionscript/README.md)
python3 tools/actionscript/flash_tree.py build srp-026g/flash/<movie> <original.swf> <new movie.swf>
# Cut the original from the archive movie.json names
python3 tools/actionscript/flash_tree.py original srp-026g/flash/<movie> <archive> <original.swf>
```
