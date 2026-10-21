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

```sh
# Is the tree the baseline?
python3 tools/actionscript/flash_tree.py check srp-026g/flash/<movie>
# Build the movie of the tree as it is: every file that is not the baseline's is compiled whole
# (needs private/actionscript/tools.json; ../docs/notes/lobby-source.md)
python3 tools/actionscript/flash_tree.py build srp-026g/flash/<movie> <original.swf> <new movie.swf>
# Compile every script of the tree, link and assemble: the complete movie must be byte-identical to the
# original (needs private/actionscript/tools.json: JDK, FFDec 25.1.3, Flex 4.16.1; hours for the lobby)
python3 tools/actionscript/flash_tree.py full srp-026g/flash/<movie> <original.swf> <new work dir>
# (Re)make a tree from its pinned original (FFDec export plus the reconstruction diffs)
python3 tools/actionscript/flash_tree.py export srp-026g/flash/<movie> <original.swf> --release <release>
```
