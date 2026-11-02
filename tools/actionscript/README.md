# Flash movies from ActionScript source

The game's Flash movies are committed ActionScript source trees,
`srp-026g/flash/<movie>/src` (`srp-026g/flash/README.md`). A movie is built
like any ActionScript project: one compiler run over its whole tree.

```sh
python3 tools/actionscript/flash_tree.py build srp-026g/flash/<movie> <original.swf> <new movie.swf>
python3 tools/actionscript/flash_tree.py original srp-026g/flash/<movie> <archive> <original.swf>
```

- `build` compiles every file of `src/` in one `compc` run of Apache Flex
  4.16.1 (strict, optimized, no debug information, the original's SWF
  version) against the Flash Player API only: every class the movie uses is
  in the tree. The compiled code (a DoABC tag per script, lazily initialized,
  as the original's) goes into the original movie in place of all of the
  original's code, where that code began. Everything else (graphics, fonts,
  sounds, exports, SymbolClass bindings) is the original's. About 5 seconds
  for the lobby. compc lays the code out by where the source lies, so every
  tree is compiled from one place (`/tmp/srp-flash-build/<movie>`, one build
  at a time): the same tree gives the same bytes from any checkout.
- A bitmap the original lacks is `bitmaps/<class>.png` beside `src/` (8-bit RGBA or RGB):
  the build adds it as a DefineBitsLossless2 of a new character id before the code, bound
  to `<class>` by a SymbolClass after it; `<class>` is a `BitmapData` of the tree, as the
  original's own bitmaps are (the hud's `hudItems`), made with `new <class>()`.
  A picture (a render, a capture) is `bitmaps/<class>.jpg` instead, a baseline JPEG: the
  build adds the file whole as a DefineBitsJPEG2, as compact as the game's own map renders
  (JPEG, DefineBits); a progressive JPEG is refused (Flash does not read it).
- **Tables.** Data the movie's code reads, which other tools read too (the
  lobby's shop filters, which the wiki's icons take), is a table
  `tables/<package path>/<Class>.tsv` beside `src/`: the build writes the
  class `<Class>` into the copy of `src/` it compiles (a table naming a class
  `src/` has is refused). Tab-separated, `#` lines comments; the header is
  `list`, then `<field>:<type>` a column (`int`, `String`, or a qualified
  class whose static constant the cell names, compiled as `Class.NAME`). Each
  list is `public static const <list>:Array` of `{field: value}` rows, in the
  file's order. Other tools read it with `flash_tree.read_table`.
- The build checks what the movie needs to load: every ABC block parses, and
  every class a SymbolClass tag binds is defined, once.
- `original` cuts the original movie (the one whose graphics the build keeps)
  from the archive `movie.json` names.
- Whether a movie works is checked in the game: the build proves that it
  compiles and defines its classes, not what Scaleform does with them.

## Tools

`private/actionscript/tools.json` (ignored by git) names a JDK and the Flex SDK:

```json
{
  "java": "/path/to/jdk/bin/java",
  "flex": "/path/to/apache-flex-sdk-4.16.1-bin"
}
```

[Apache Flex 4.16.1](https://flex.apache.org/download-binaries.html)
(`apache-flex-sdk-4.16.1-bin.tar.gz`, sha256
`17fda7ac8d3e476cad3127f345455ef316acfb87c6f4322e5897bd8d9b09388e`) needs the
Flash Player API at `<flex>/frameworks/libs/player/32.0/playerglobal.swc` (the
one JPEXS FFDec ships as `flashlib/playerglobal32_0.swc`), and `target-player`
32.0 in its `frameworks/flex-config.xml`. The player API is a compiler
dependency only: the movie keeps the original's SWF version, and a player API
newer than Scaleform's is not supported in the game.

## Tests

```sh
python3 -m unittest discover -s tools/actionscript
# With a real compc build of a small tree:
SRP_AS_TOOLS=/absolute/path/to/private/actionscript/tools.json python3 -m unittest discover -s tools/actionscript
```

### Research UI

The Research UI regression harness is `tests/ResearchUiHarness.as`. Put the newly
built movies beside its output as `hud.swf` and `player_icons.swf`, then compile:

```sh
java -jar "$FLEX/lib/mxmlc.jar" +flexlib="$FLEX/frameworks" \
  -library-path= -external-library-path="$FLEX/frameworks/libs/player/32.0/playerglobal.swc" \
  -static-link-runtime-shared-libraries=true -debug=false -optimize=true -omit-trace-statements=false \
  -output="$OUT/ResearchUiHarness.swf" tools/actionscript/tests/ResearchUiHarness.as
```

Run the SWF with Ruffle's filesystem access enabled (use a private virtual display for
automation). It traces `RESEARCH UI PASS` or `RESEARCH UI FAIL`. It checks the absent
caption, 24/34 px sizing, steady capture icons, translucent fill, hide-interface key, contested and enemy-capture pulses
at 30/144 FPS, independent HUD visibility, repeated aim packets and fade reversal.
It also checks countdown spacing and restoration of other modes' timer styles.
Enemy warnings cover both team orientations, friendly-owned and neutral units,
neutralisation with the defender's progress, contested priority, completion, legacy
packets, repeated updates, and offscreen/removed animation cleanup.
The `ResearchAlert.as` implementation and warning bitmap are identical in both movie
trees. The editable warning artwork is `art/ResearchWarning.svg`; render it to transparent
54 x 53 RGBA at its natural size (the approved preview uses browser SVG/canvas rendering),
then copy those same PNG bytes to both `bitmaps/ResearchWarning.png` files.
The spacing regression includes hidden capture timers; centering is checked before
resize with different initial positions and scaled, translated parents.
Death/respawn transitions check immediate full-width text, late native label
validation, animation frames, and removal of the Research field on mode exit.
This exercises the controls; the game's Scaleform integration still needs a playtest.

### Minimap

`tests/MinimapHarness.as` checks the hud's minimap icons (`survarium.controls.Minimap`).
Put the newly built hud movie beside its output as `hud.swf`, compile it as above
(`-output="$OUT/MinimapHarness.swf" tools/actionscript/tests/MinimapHarness.as`) and run it
the same way. It traces `MINIMAP PASS` or `MINIMAP FAIL`. A team-0 game that joined late
gets its ally's data before its own team (update_local_player waits for its first spawn):
the ally's icon is the enemy's until the team comes, then the ally's. It also checks an
enemy, an ally taking and dropping the battery, an enemy carrier, a player that leaves,
and a team-1 game.

### Tab map names

`tests/TabMapNamesHarness.as` checks the names on the Tab map (`srp-026g/docs/notes/tab-map-names.md`)
with the hud movie's own Tab list (`SrvPlayerList`, its map) and corner minimap, fed as `HUD.as`
feeds them (the hud's root does not run outside the game). Put the newly built hud movie beside
its output as `hud.swf`, compile and run it as above. It traces `NAMES ... ok` lines and
`NAMES RESULT pass` or `NAMES RESULT fail: ...`: no names without Alt; with Alt each other
player's icon its place's name centred over it, the own icon and a place with no row none, the
corner minimap none; the names following their icons over 100 updates with the same text fields;
Alt let go and held again; a player lost from sight, one spotted, an ally taking the battery, the
rows made again; and Blackout (the list kept hidden): none on screen.

### Weapon upgrade window

`srp-026g/scripts/weapon-kit/check.sh <lobby_menu.swf> [work dir]` checks the lobby's weapon upgrade window with the weapon kit (two decal slots, the kit's records, a drop shown at once, every language's texts fitting) and prints `WEAPONKIT RESULT pass` or `WEAPONKIT RESULT fail: ...`.

## History

Until 2026-10-21 the trees were proven to recompile revision 24 byte for byte
(per-class compiler settings, reconstruction diffs, an exact layout linker),
and a change was compiled class by class into revision 24's code. The owner,
2026-10-21: "we don't need anything for decomp matching now". That machinery
is in git history, before the one-run build.
