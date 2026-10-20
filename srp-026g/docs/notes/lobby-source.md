# The lobby movie from source

The owner, 2026-10-20: "instead of doing patches, we want to fully transition to this" (the
ActionScript source build of `feat/actionscript-source`).

The lobby movie (`flash_movies/lobby_menu.swf`) is built one way: revision 24's movie rebuilds
byte for byte from its ActionScript (`tools/actionscript`), and every lobby change is a source
feature over that tree. No bytecode is patched by hand.

## Where things are

- `srp-026g/lobby/src/`: srp's own classes (`SrpRooms`, `SrpExchange`, `SrpBuyGold`,
  `SrpCraftMenu`, `SrpScopeContextMenu`, `SrpPlayerGear`), whole files.
- `srp-026g/lobby/features/<feature>/feature.json`: what a feature changes; its `*.patch` files are
  diffs of the game's classes (the game's code itself is never in git).
- `srp-026g/lobby/lobby.json`: the features in build order, and the reviewed call/cast
  fingerprints (`reviewed_calls`).
- `private/actionscript/` (ignored by git): `tools.json` (JDK, FFDec 25.1.3's full distribution,
  Flex 4.16.1; `tools/actionscript/README.md`) and `rev24/`, revision 24's exported source, which
  the build makes from the movie the first time.

## Making a lobby change

- **One of srp's classes:** edit `lobby/src/<Class>.as`. Its feature owns the class: methods,
  fields and declarations may change or be added (none removed).
- **A game class** (with `R=tools/actionscript/reconstruction/026g-rev24/manifest.json`,
  `P=private/actionscript/rev24`, `L=srp-026g/lobby/lobby.json`):
  1. `python3 tools/actionscript/feature_source.py --reconstruction $R $P $L show <Class> --before <feature> -o /tmp/C.as`
     (without `--before`: after every feature);
  2. edit `/tmp/C.as`;
  3. `python3 tools/actionscript/feature_source.py --reconstruction $R $P $L diff <feature> <Class> /tmp/C.as`
     writes `features/<feature>/<Class>.patch`;
  4. in `feature.json`, name the edited methods (`methods`), and any member added or redeclared
     (`members`). Everything else of the class must stay revision 24's byte for byte.
- **A new class:** put it in `lobby/src/`, and in a feature
  `{"class": "X", "new": true, "source": "src/X.as", "abc_with": "<a class whose ABC takes it>"}`.
- **A new feature:** `features/<name>/feature.json` (`{"schema": 2, "description": …, "classes": […]}`),
  and its name in `lobby.json`'s `features`. Order matters only when two features patch one class:
  a patch applies to what the features before it left, exactly (re-record the later one if needed).
- **A changed call, cast or `new` in an edited game method** must be reviewed:
  `python3 tools/actionscript/build_feature.py --tools private/actionscript/tools.json --reconstruction $R --jobs 4 --review-calls $P $L private/actionscript/review`
  lists each with its before/after assembly (`review-calls.tsv`); read them, then copy the
  fingerprints into `lobby.json` `reviewed_calls` as `"<Class>.<method>": {"before", "after"}`.

## Building and checking

```sh
python3 srp-026g/client-patches/build_lobby_026g.py <lobby_menu.swf of resources.db.patch_24> <new lobby_menu.swf> [--work DIR]
```

- About 5 minutes. Each touched class first recompiles unchanged and must give revision 24's movie
  exactly; then the features compile (new classes, srp's classes, game classes), and every
  method, trait, class header and script no feature names is checked byte for byte against
  revision 24's, as are all non-script tags.
- `--work DIR` keeps the reports: `build/report.json`, `build/changes.tsv` (every method changed or
  added), and per class `class-*/feature-linked.swf.methods.tsv`.
- The movie must be `RESULT_SHA256` of `build_lobby_026g.py`. After a change the build refuses
  the new movie: check it in the game, then set `RESULT_SHA256` to it with a line on what it holds
  (`--any-result` writes it for a test build).
- To see what a change does to the movie's code, compare it with the last one:
  `python3 tools/actionscript/compare_decompiled.py --tools private/actionscript/tools.json old.swf new.swf OUT`
  (every script and method as FFDec decompiles them; `OUT/report.txt`).
- Tests: `python3 -m unittest discover` in `srp-026g/client-patches` and in `tools/actionscript`
  (`SRP_AS_TOOLS=private/actionscript/tools.json` adds the real-compiler tests).

## The port of 2026-10-20

Every change the bytecode driver applied became a source feature: `rooms` (`SrpRooms`, the
results hook), `buy-gold` (#213), `exchange` (#214), `buy-modified` (#215), `craft-menu` (#216),
`scope-context-menu` (#217), `player-gear` (#232), with the rooms changes of #219, #211, #212,
#209, #229, #233 and #234 in `SrpRooms.as`. The source build (`bdd66ed5…`) and the driver's last
movie (main's `e626a01a…`) change the same 15 ABC blocks; the other 861 are revision 24's in
both. Decompiled with FFDec, 878 of 882 scripts are the same text; the rest differ only in
compiler encodings:

- `SrpExchange`: `String(x)` is `convert_s`, as in revision 24 and as the source says; the
  driver's FFDec compile gave `coerce_s`. The values are numbers: the same text either way.
- `SrpRooms`: the same 135 methods; its fields are declared in another order (the source build
  keeps revision 24's fields first and appends the new ones).
- `InventoryList.showContextMenu`: the craft lock reads `_inventoryLocked` after looking up
  `SrpCraftMenu` (the driver: before; neither has side effects); Buy's test is
  `!rank || itemAvailable` as written (the driver: `!(rank & !itemAvailable)`, built without a
  jump); the modifiers loop is laid out by Flex (decompiles as `while`, with no `continue` in it).
- `LobbyMenu.try_buy_item`, `fill_victory_screen_player` and `showContextMenu`: an edited method
  is the compiler's, without the original's debug names of locals (no effect in the game).
