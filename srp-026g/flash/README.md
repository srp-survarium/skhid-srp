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

```sh
# Build a movie: one Flex run over the whole tree, its code in place of the original's (seconds;
# needs private/actionscript/tools.json: a JDK and Flex 4.16.1, tools/actionscript/README.md)
python3 tools/actionscript/flash_tree.py build srp-026g/flash/<movie> <original.swf> <new movie.swf>
# Cut the original from the archive movie.json names
python3 tools/actionscript/flash_tree.py original srp-026g/flash/<movie> <archive> <original.swf>
```
