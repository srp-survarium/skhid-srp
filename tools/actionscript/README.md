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
caption, 24/34 px sizing, steady capture icons, translucent fill, hide-interface key, contested pulses
at 30/144 FPS, independent HUD visibility, repeated aim packets and fade reversal.
It also checks countdown spacing and restoration of other modes' timer styles.
This exercises the controls; the game's Scaleform integration still needs a playtest.

## History

Until 2026-10-21 the trees were proven to recompile revision 24 byte for byte
(per-class compiler settings, reconstruction diffs, an exact layout linker),
and a change was compiled class by class into revision 24's code. The owner,
2026-10-21: "we don't need anything for decomp matching now". That machinery
is in git history, before the one-run build.
