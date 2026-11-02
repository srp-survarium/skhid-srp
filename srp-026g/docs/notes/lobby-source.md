# The Flash movies from source

The owner, 2026-10-20: "instead of doing patches, we want to fully transition to this"; "we first
use that to compile all .swf, then we apply all we have already", "this can be stored in a repo".

Every Flash movie of the game is a committed ActionScript source tree, `srp-026g/flash/<movie>/src`
([../../flash/README.md](../../flash/README.md)), first exported from revision 24. Every change of
ours is an edit of a tree, and a movie is built from its whole tree; nothing is patched in bytecode.

## Where things are

- `srp-026g/flash/<movie>/src`: the source (LF). `movie.json` names the original movie, whose
  graphics, fonts and symbol bindings the build keeps.
- `private/actionscript/` (ignored by git): `tools.json` (a JDK and Flex 4.16.1;
  `tools/actionscript/README.md`).

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

Edit the tree: change any `.as` file, add a file for a new class (any package), delete the file of a
class that goes. Then build.

## Building

```sh
python3 srp-026g/client-patches/build_lobby_026g.py <lobby_menu.swf of resources.db.patch_24> <new lobby_menu.swf>
python3 tools/actionscript/flash_tree.py build srp-026g/flash/<movie> <the original> <new movie>
```

- One Flex run compiles the whole tree (about 5 seconds for the lobby); its code goes into the
  original movie in place of the original's code. The graphics are the original's.
- The build checks that every ABC block parses and every class a symbol is bound to is defined.
  Whether the movie works is checked in the game.
- `build_lobby_026g.py` says whether the movie is `RESULT_SHA256`, the one last checked in the game
  (the same tree gives the same bytes). Set `RESULT_SHA256` when the owner has checked a new one.
- `flash_tree.py original srp-026g/flash/<movie> <archive> <out.swf>` cuts an original from its
  archive.
- Tests: `python3 -m unittest discover` in `srp-026g/client-patches` and in `tools/actionscript`;
  `SRP_AS_TOOLS=private/actionscript/tools.json` adds a real compiler run.

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

## One Flex run (2026-10-21)

The owner: "we don't need anything for decomp matching now". The movies are no longer built over
revision 24's compiled code: the whole tree is compiled in one Flex run, and none of the classes
keeps revision 24's bytes. The byte-exact proof, the per-class compiler settings and the
reconstruction diffs are in git history. The lobby of 6064476c built this way (882 classes, 883 ABC
blocks) was checked in the game by the owner: "good, it works". compc lays the code out by where
the source lies, so the build compiles every tree from one place (`/tmp/srp-flash-build/<movie>`);
the tree of 6064476c gives `8b099212…` (`RESULT_SHA256`), the movie checked was `c3898574…`, the
same tree compiled from another place.

The HUD tree took three source fixes Flex asks for (those classes had been compiled with FFDec's
compiler): `Minimap`'s `§default§` written as `["default"]` and a field initializer without
`this.`, and `CombatLogRenderer.logItem` typed by its own package's `CombatLogItem` (the global one
extends it).

## The weapon window (#244, 2026-10-21)

The weapon's upgrade window of #244 (camouflage and decals on weapons; `weapon-camo.md`), written
in the classes that own it:

- its two pages (Attachments: the scope; Appearance: the decals and the camouflage), their slots
  in armour's places and the inventory's filter of each: `UpgradeWindow` (`showPage`,
  `placeWeaponSlots`, `restoreSlots`, `weaponSlotCategory`, `isUpgradeableWeapon`); the title
  bar's places are its constants;
- the page arrows: `survarium/upgrade/PageArrow` (the Confirm button's art at arrow size), new UI;
- a move of a decal between slots sends no drop for the slot it left: `UpgradeWindow.holds`;
- the window's opening and its close (`start_item_upgrade` / `exit_item_upgrade` only for an item
  the game shows): `LobbyMenu.openModifierWindow`, `closeModifierWindow`;
- the Attachments button and menu entry for a weapon with any page: `InventoryList`, `ItemCard`,
  `ItemControlPanel` (`isUpgradeable`); the upgrades tab's All lists scopes:
  `ItemFactory.filterGroup`.

Its texts are data: `scripts/content-edit/weapon-attachments-1.json` (a manifest row).

## The item search (2026-10-22)

Ctrl+F over an item list ([item-search.md](item-search.md)), written in the classes that own it:

- the box, its keys and the matching: `InventoryList` (`makeSearch`, `onSearchKey`, `closeSearch`,
  `applySearch`, `searchItems`); the search narrows what `ApplyInventoryFilter` lists, after the
  list's own filters and its order (`LIST_ORDER`); the grid's height while the box shows: `onFilterShowed`;
- Esc in the box: `LobbyMenu.close_active_window`; the exe's window keys while Ctrl is held over the
  list: `LobbyMenu.windowKeysOff`; the Ctrl keys: `onKeyDownStage`, `onKeyUpStage`,
  `MainScene.isCtrl`, `KeyCode`;
- the empty box's word is the dictionary's `st_srp_inventory_search`
  (`client-patches/localization`); the item-kind words of every language are the code's own
  (`InventoryList.SEARCH_KINDS`: search words, any language matches).

## Scaleform pitfalls

What the game's Scaleform did differently from Flash, found building the rooms window
(2026-10-17); each cost a game round to find.

- **Presses on a control placed straight in a content Sprite go to that Sprite** (the event's
  target is the container); one nested a Sprite deeper works. Not masks, z-order or redraws.
  What works: a MOUSE_DOWN listener on the container that, when `e.target` is the container,
  finds the control under the point with `hitTestPoint` and calls its handler.
- **A fill of alpha 0 is not hit-tested:** use 0.01. A gap inside a Sprite takes no click: put
  a 0.01 hit Shape over the whole row.
- `drawCircle` after `lineStyle` draws a stray line from the pen: draw two filled circles.
- `BitmapData.draw` from a BitmapData that was itself drawn into shows red; repeated
  alpha-blended draws wash out to white. Draw once, from the library bitmap.
- `trace()` reaches no log: debug with an on-screen TextField. Input sent by a helper inside the
  Wine prefix (SendInput) does not reach the game (DirectInput).
- The game's Calibri has no ★ (U+2605) or ■: use words.
- A CLIK button keeps its label at its natural height: don't squeeze its `setSize` height (a
  skinned server button: 32).
- A weakly held closure as a listener may be collected: keep listeners strong.
