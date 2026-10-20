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

The trees are no longer FFDec's text as it came out. Every change below still compiles to the
original movie byte for byte (`full`), and `movie.json` pins the cleaned text; `export` makes the
raw FFDec tree again, without these changes.

- **Names.** FFDec calls a local `_loc2_` and a parameter `param1` where the bytecode has no name.
  They have names now: the original one where another movie's copy of the same function has it in
  its debug info (the hud, player-icons and replay movies carry debug info), or where the movie's
  own debug info or parameter table records it and FFDec ignored it; otherwise one read from how
  the variable is used. Local and parameter names do not reach the bytecode of a method without
  debug info, which is why the rebuild stays identical.
- **Placeholders that stay.** Some lobby methods keep `_loc2_`/`param1`: srp's earlier patches
  compiled FFDec text with debug info, so the shipped movie records exactly those names
  (`UpgradeWindow`, `ItemRendererInv.updateData`, `ItemInfoRenderer.set data`, ...). Some
  player-icons/replay interfaces keep their parameters too: their movie's parameter table holds
  unrelated strings. The lobby's own game classes (`survarium/**`, the top-level scripts and the
  timeline classes) are not cleaned yet: the lobby features are being moved into them first.
- **Named numbers.** Sound ids, key codes, item categories and the like are Flex compile-time
  constants, declared in the file that uses them:

  ```as3
  package survarium.buttons
  {
     import flash.external.ExternalInterface;

     config namespace SOUND;
     SOUND const CLICK:int = 0;
     SOUND const HOVER:int = 1;
     ...
              ExternalInterface.call("sound_play",SOUND::CLICK);
  ```

  The compiler inlines `SOUND::CLICK` as the literal itself, so the bytecode does not change (a
  `static const` would compile to a property lookup and change it). The build passes no `-define`
  options; each file declares the constants it uses. The groups: `SOUND` (the order of
  `gameplay/ui_sounds`), `KEY` (key codes), `CATEGORY` (`item_category`), `SUPER_CATEGORY`
  (`ItemFactory.getCategory`), `PLACE` (`ItemData.descriptor`: inventory, shop, bundle; below 100
  the profile slot), `MOUSE` (`MouseEventEx` buttons) and `GAMEPAD` (the key codes Scaleform gives
  gamepad buttons, in `InputDelegate`).

```sh
# Is the tree the baseline?
python3 tools/actionscript/flash_tree.py check srp-026g/flash/<movie>
# Compile every script of the tree, link and assemble: the complete movie must be byte-identical to the
# original (needs private/actionscript/tools.json: JDK, FFDec 25.1.3, Flex 4.16.1; hours for the lobby)
python3 tools/actionscript/flash_tree.py full srp-026g/flash/<movie> <original.swf> <new work dir>
# (Re)make a tree from its pinned original (FFDec export plus the reconstruction diffs)
python3 tools/actionscript/flash_tree.py export srp-026g/flash/<movie> <original.swf> --release <release>
```
