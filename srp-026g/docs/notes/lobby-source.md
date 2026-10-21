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
- `srp-026g/flash/<movie>/srp.json`: the movie's new classes (files that are not revision 24's), each
  with `abc_with`, the class of the release whose ABC block takes it. lobby: the rooms window,
  `survarium/rooms` (`RoomsWindow`, its views `RoomsList`, `RoomsCreate`, `RoomsRoom`, and
  `RoomsWire`, `RoomsText`, `RoomsLook`), all in `BottomPanel`'s block.
- `private/actionscript/` (ignored by git): `tools.json` (JDK, FFDec 25.1.3's full distribution, Flex
  4.16.1; `tools/actionscript/README.md`).

## Writing a lobby feature

The owner, 2026-10-21: "we now got the source code" "and we just modify it".

- Put the code in the game's class that owns the behaviour, in that class's style:
  - Get Gold and the exchange: `MessageMoneyExchange` (`buyGold`, `open`);
  - a right-click menu: the list that builds it (`InventoryList`); a dialog's buttons: the dialog
    (`MessageCraftItem`);
  - a request and its answer: the class with the answer's handler (`LobbyMenu`); the click that asks:
    the window clicked (`PlayerRanking`).
- New classes only for genuinely new UI, in a package of their own (the rooms window:
  `survarium/rooms`). The game's classes call the new UI's API (`RoomsWindow.me`) and keep what is
  theirs (`TopMenu` draws its own tab, `StatusPanel` its own Play button).
- No `Srp*` class for a feature.

## Making a change

- **Edit a class:** edit its `.as` file, anything in it.
- **Add a class:** add its file (any package) and name it in `srp.json`'s `new_classes` with its
  `abc_with` (a class of the release; the rooms use `survarium.controls.BottomPanel`).
- **Remove a class:** delete its file. Nothing may name it any more, and it must be the last script
  of its ABC block (as srp's appended classes of the release were).

## Building

```sh
python3 srp-026g/client-patches/build_lobby_026g.py <lobby_menu.swf of resources.db.patch_24> <new lobby_menu.swf> [--work DIR]
python3 tools/actionscript/flash_tree.py build srp-026g/flash/<movie> <the original> <new movie> [--work DIR]
```

- The first is the lobby's, with its pinned `RESULT_SHA256`. The second works for any movie;
  `flash_tree.py original srp-026g/flash/<movie> <archive> <out.swf>` cuts the original from its
  archive.
- The build compiles every file that is not revision 24's, whole; the others keep revision 24's
  compiled code (it is what they compile to), and the graphics are revision 24's. A class that uses
  what another edited class adds or changes is compiled after it; then every class is compiled once
  more against all of them, so the order does not matter.
- The lobby movie must be `RESULT_SHA256`. After a change the build refuses the new movie: check it
  in the game, then set `RESULT_SHA256` to it, with a line on what it holds. `--any-result` writes it
  for a test build.
- To see what a change does to the code, compare with the last movie:
  `python3 tools/actionscript/compare_decompiled.py --tools private/actionscript/tools.json old.swf new.swf OUT`
  (`OUT/report.txt`, script by script and method by method).
- Tests: `python3 -m unittest discover` in `srp-026g/client-patches` and in `tools/actionscript`.
  `SRP_AS_TOOLS=private/actionscript/tools.json` adds the real-compiler tests.
- When the baseline itself changes (a new release, the tools, the reconstruction), prove it:
  `python3 tools/actionscript/flash_tree.py full srp-026g/flash/<movie> <the original> <work dir>`
  compiles every file of the unedited tree and requires the original movie byte for byte (about 4
  hours for the lobby).

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
  check then).
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
