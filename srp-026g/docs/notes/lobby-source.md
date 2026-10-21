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
  release whose ABC takes it (FFDec compiles it there). Its `removed` lists srp's classes of the
  release that are gone from the tree; the build takes them out of the movie.
  - lobby: the rooms window, `survarium/rooms` (`RoomsWindow`, its views `RoomsList`, `RoomsCreate`,
    `RoomsRoom`, and `RoomsWire`, `RoomsText`, `RoomsLook`); removed: `SrpExchange`, `SrpRooms`;
  - login menu: `SrpLoginHint`;
  - main menu: `SrpFovSlider`.
- `private/actionscript/` (ignored by git): `tools.json` (JDK, FFDec 25.1.3's full distribution, Flex
  4.16.1; `tools/actionscript/README.md`) and the pinned FFDec export of each original movie, which
  the build makes the first time (the baseline source of a changed class).

## How a lobby feature is written

The owner, 2026-10-21, on the lobby's `Srp*` classes: "Maybe some of them should literally be inside
classes" "that already exist" "instead of being external entities".

- **In the game's class it belongs to**, in that class's own style: its fields, its methods, its
  helpers. The class that owns the behaviour owns the code:
  - Get Gold and the exchange: the exchange window, `MessageMoneyExchange` (`buyGold`, `open`);
    every caller calls it directly;
  - a right-click menu: the list that builds it (`InventoryList`: Optics, the craft window's Buy and
    Sell); a dialog's buttons: the dialog (`MessageCraftItem`: Continue and Exit);
  - a request and its answer: the class that has the answer's handler (`LobbyMenu`: the player
    gear's ask, `fill_victory_screen_player`); the click that asks: the window clicked
    (`PlayerRanking`).
- **New classes only for genuinely new UI**, in a package of their own (the rooms window:
  `survarium/rooms`), named in `srp.json`. They may name each other. The game's classes call the
  new UI's API (`RoomsWindow.me`), and keep what is theirs: `TopMenu` draws its own tab, `StatusPanel`
  its own Play button, `LobbyMenu` decides Esc and the results, `BottomPanel` hides its Matches tab.
  No static hook that is handed a game object to reach into.
- **No `Srp*` class for a feature.** One of the release that is no longer needed goes into
  `srp.json`'s `removed`.

## Making a change

- **srp's class:** edit `flash/<movie>/src/<path>.as`.
- **A game class:** edit its file. Keep FFDec's layout of a method (the header line, then `{` and
  `}` on lines of their own at the header's indent). The build reads the diff by method:
  - a method whose text changed may change (the class's own name: its constructor);
  - a method, `var` or `const` that is new, or whose declaration changed, may be added or
    redeclared; one with an initial value lets the class initializer (`static`) or the
    constructor change too;
  - a removed method is refused;
  - everything else of the class must compile to revision 24's bytes.
- **Where in a game class:** new methods at the class's end, after every method it has; new fields
  after its last field of their kind. Flex numbers locals through the whole file: a method put
  before an untouched one can change that one's code (2026-10-21: four new methods in the middle of
  `LobbyMenu` gave the untouched `test_bandle_data` an extra local initialisation), and the build
  then refuses the class.
- **No `try` in an edited game method:** it may change the method's flags, which an edit may not
  (a `try` in `BottomPanel.createMatchesTab` did). Put it in a new method the edited one calls.
- **A game class using another's new member:** the build compiles the class whose new members
  another changed class uses first (Flex checks a member against the movie compiled so far); a use
  both ways is refused.
- **A new class:** a new file in the tree (any package), named in `srp.json` with its `abc_with`.
  Each new class is first added as its declaration alone, then compiled whole (FFDec compiles a
  class only against types the movie has).
- **Removing srp's class of the release:** delete its file and list it in `srp.json`'s `removed`. Its
  script, class and methods must be the last records of its ABC (as srp's appended classes are);
  every other record stays byte for byte. The tree may not name it any more (comments too).

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

## In the game's classes (2026-10-21)

The lobby's features moved into the game's classes they belong to, and the rooms window into a
package of its own; no behaviour changes:

- Optics: `InventoryList.isUpgradeable` and `upgradeLabelKey` (was `SrpScopeContextMenu`);
- the craft window's Buy and Sell: `InventoryList.showCraftMenu`, `findCraftWindow`,
  `buyFromCraftMenu`; its Continue and Exit: `MessageCraftItem.offerContinue`, `onContinue`, `holds`
  (was `SrpCraftMenu`);
- Get Gold: `MessageMoneyExchange.buyGold` (was `SrpBuyGold`); the exchange: `MessageMoneyExchange`
  itself (was `SrpExchange`, removed from the movie);
- the player gear: `LobbyMenu.showPlayerInfoMenu`, the ask in flight, `takePlayerInfo`;
  `PlayerRanking.onPlayerRightClick` (was `SrpPlayerGear`);
- the rooms: `survarium/rooms` (was `SrpRooms`, removed from the movie); their parts in `TopMenu`,
  `StatusPanel`, `LobbyMenu`, `BottomPanel` and `PlayerRanking`.

The tree builds to `a3b76872…`. Against `bdd66ed5…` (this tree before):

- Every method, trait and class no change touches is revision 24's byte for byte (the build's
  check).
- Decompiled with FFDec, 860 of 889 scripts are the same text; the others are the edited game
  classes, the rooms package and the `Srp*` classes gone.
- Each method of `SrpRooms` is the same source text in the package but for the names it now reaches
  through another class (`win.`, `look.`, `RoomsWire.` and the like): 103 of 135. The rest are
  written anew around the same statements: the window's constructor and its API for the game's
  classes, the hooks now in the game's classes (`topLabel`, `topIcon`, `topColour`, `skinTint`, the
  button half of `applyPlay`, `overPanels`), `render`'s three views, and `banner`'s
  `TEXT[why] != null` as `RoomsText.known(why)`.
- The other features' methods decompile as their `Srp*` methods did but for the move itself (the
  game object a static method was handed is now `this`) and Flex's encodings (`Boolean(x)` around a
  test, locals declared at the top, a parameter a closure keeps copied to a local).
- Two differences of form, none of behaviour:
  - Continue after a craft is a method listener, not a closure (`addEventListener` keeps one): a
    craft window shows one result;
  - the null checks of a value that cannot be null there (`SrpScopeContextMenu.isUpgradeable`'s,
    `SrpExchange.accept`'s window, `SrpCraftMenu.locked`'s slot) are gone.

The login and main menu builders (`login_hint_026g.py`, `fov_slider_026g.py`) went the same way.
Their released movies (patches 12 and 22) are the `login_menu` and `main_menu` trees, whose
`SrpLoginHint.as` and `SrpFovSlider.as` are the repository's sources byte for byte.
