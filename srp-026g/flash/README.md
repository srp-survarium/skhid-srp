# The game's Flash movies as source

The owner's direction, 2026-10-20: "we first use that to compile all .swf, then we apply all we have
already", "this can be stored in a repo".

Each folder is the complete ActionScript of one movie of `resources.db`. That is FFDec's export of the
movie as revision 24 has it, with the corrections of `tools/actionscript/reconstruction/<release>/`
applied, so that every script compiles back to the original bytes:

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

That is 1,605 scripts, 5.3 MB (about 0.9 MB compressed). All of it lives in this folder, so it can move
to a repository of its own.

- `movie.json` pins the original movie (its archive entry and sha256), the reconstruction manifest
  whose per-class compiler settings the build uses, and the sha256 of every baseline source file.
- The original `.swf` is not committed. Its graphics, fonts and sounds are not source, so the build
  takes them from the pinned original: `flash_tree.py original <movie dir> <archive> <out.swf>` cuts
  it from the archive the pin names.
- The files have LF line ends (FFDec writes CRLF; `export` converts them). The full rebuild below
  compiles the LF files, so nothing compiled depends on the line ends.

## Cleaned up over the export

The trees are no longer FFDec's text as it came out; they are meant to be read and edited as
ordinary source (the owner, 2026-10-21: no decompilation matching needed now). The names below
still compiled to the original movies byte for byte; the constants classes do not, by design.
`movie.json` pins the last byte-identical text, so `check` lists what changed since, and `full` only
applies to a tree without such changes. `export` makes the raw FFDec tree again.

Every tree compiles whole with Flex's `compc` (`-source-path=src -include-sources=src
-strict=false`).

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
# Is the tree the baseline?
python3 tools/actionscript/flash_tree.py check srp-026g/flash/<movie>
# Compile every script of the tree, link and assemble: the complete movie must be byte-identical to the
# original (needs private/actionscript/tools.json: JDK, FFDec 25.1.3, Flex 4.16.1; hours for the lobby)
python3 tools/actionscript/flash_tree.py full srp-026g/flash/<movie> <original.swf> <new work dir>
# (Re)make a tree from its pinned original (FFDec export plus the reconstruction diffs)
python3 tools/actionscript/flash_tree.py export srp-026g/flash/<movie> <original.swf> --release <release>
```
