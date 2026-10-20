# The Flash movies from source

The owner, 2026-10-20: "instead of doing patches, we want to fully transition to this"; "we first
use that to compile all .swf, then we apply all we have already", "this can be stored in a repo".

Every Flash movie of the game is a committed ActionScript source tree, `srp-026g/flash/<movie>/src`
([../../flash/README.md](../../flash/README.md)). Each tree is revision 24's complete source, which
rebuilds byte for byte. Every change of ours is an ordinary edit of a tree; nothing is patched in
bytecode.

## Where things are

- `srp-026g/flash/<movie>/src`: the source (LF). `movie.json` pins the original movie and the
  sha256 of every baseline file, which tells the build what changed.
- `srp-026g/flash/<movie>/srp.json`: srp's own classes of that movie. Each is owned whole: any of its
  methods, fields and declarations may change or grow. A new one names `abc_with`, a class of the
  release whose ABC takes it (FFDec compiles it there).
  - lobby: `SrpRooms`, `SrpExchange`, `SrpBuyGold`, `SrpCraftMenu`, `SrpScopeContextMenu`,
    `SrpPlayerGear`;
  - login menu: `SrpLoginHint`;
  - main menu: `SrpFovSlider`.
- `private/actionscript/` (ignored by git): `tools.json` (JDK, FFDec 25.1.3's full distribution, Flex
  4.16.1; `tools/actionscript/README.md`) and the pinned FFDec export of each original movie, which
  the build makes the first time (the baseline source of a changed class).

## Making a change

- **srp's class:** edit `flash/<movie>/src/<Class>.as`.
- **A game class:** edit its file. Keep FFDec's layout of a method (the header line, then `{` and
  `}` on lines of their own at the header's indent). The build reads the diff by method:
  - a method whose text changed may change;
  - a method, `var` or `const` that is new, or whose declaration changed, may be added or
    redeclared;
  - a removed method is refused;
  - everything else of the class must compile to revision 24's bytes.
- **A new class:** a new file in the tree, named in `srp.json` with its `abc_with`.

## Building and checking

```sh
python3 srp-026g/client-patches/build_lobby_026g.py <lobby_menu.swf of resources.db.patch_24> <new lobby_menu.swf> [--work DIR]
python3 tools/actionscript/flash_tree.py build srp-026g/flash/<movie> <the original> <new movie> [--work DIR]
```

- The first command is the lobby's, with its pinned `RESULT_SHA256`. The second works for any movie:
  `flash_tree.py original srp-026g/flash/<movie> <archive> <out.swf>` cuts its original from the
  archive its pin names.
- A build takes about 5 minutes. Only files that are not the baseline's are compiled.
- Each changed class first recompiles unchanged and must give the original movie exactly. Then:
  - new classes compile (FFDec), then srp's classes, then the game classes;
  - every method, trait, class header and script the change does not touch is checked byte for
    byte against the original, as are all non-script tags.
- `--work DIR` keeps the reports for review:
  - `build/report.json`;
  - `build/changes.tsv`: every method changed or added;
  - `build/review-calls.tsv`: each edited game method whose sequence of calls, casts or `new`
    changed, with its before/after assembly;
  - `build/tree-changes.json`: the classes and the permissions derived from the diff.
- The lobby movie must be `RESULT_SHA256` of `build_lobby_026g.py`. After a change the build refuses
  the new movie: check it in the game, then set `RESULT_SHA256` to it, with a line on what it holds.
  `--any-result` writes it for a test build.
- To see what a change does to the code, compare the movie with the last one:
  `python3 tools/actionscript/compare_decompiled.py --tools private/actionscript/tools.json old.swf new.swf OUT`.
  It exports both with FFDec and compares every script and method (`OUT/report.txt`).
- Full proof of a baseline tree, about 4 hours for the lobby:
  `python3 tools/actionscript/flash_tree.py full srp-026g/flash/<movie> <the original> <work dir>`.
- Tests: `python3 -m unittest discover` in `srp-026g/client-patches` and in `tools/actionscript`.
  `SRP_AS_TOOLS=private/actionscript/tools.json` adds the real-compiler tests.

## The port of 2026-10-20

Everything main's bytecode driver applied is now an edit of the lobby tree:

- the rooms (`SrpRooms` with #219, #211, #212, #209, #229, #233, #234; the results hook in
  `LobbyMenu.show_victory_screen`);
- Get Gold (#213);
- the exchange (#214);
- Buy on owned items (#215);
- the craft window (#216);
- Optics (#217);
- the player gear (#232).

The tree builds to `bdd66ed5…`. Against the driver's last movie (main's `e626a01a…`):

- Both change the same 15 ABC blocks; the other 861 blocks are revision 24's in both.
- Decompiled with FFDec, 878 of 882 scripts are the same text. The rest differ only in compiler
  encodings:
  - `SrpExchange`: `String(x)` compiles to `convert_s`, as in revision 24 and as the source says. The
    driver's FFDec compile gave `coerce_s`. The values are numbers, so the text is the same.
  - `SrpRooms`: the same 135 methods; its fields are declared in another order. The source build keeps
    revision 24's fields first and appends the new ones.
  - `InventoryList.showContextMenu`:
    - the craft lock reads `_inventoryLocked` after looking up `SrpCraftMenu` (the driver: before;
      neither has side effects);
    - Buy's test is `!rank || itemAvailable` as written (the driver: `!(rank & !itemAvailable)`,
      built without a jump);
    - the modifiers loop is laid out by Flex: it decompiles as `while`, with no `continue` in it.
  - `LobbyMenu.try_buy_item`, `fill_victory_screen_player` and `showContextMenu`: an edited method is
    the compiler's own output, without the original's debug names of locals (no effect in the game).

The login and main menu builders (`login_hint_026g.py`, `fov_slider_026g.py`) went the same way.
Their released movies (patches 12 and 22) are the `login_menu` and `main_menu` trees, whose
`SrpLoginHint.as` and `SrpFovSlider.as` are the repository's sources byte for byte.
