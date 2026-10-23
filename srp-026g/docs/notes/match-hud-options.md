# Match HUD options (0.26g)

Owner 2026-10-23 (board T47). What a player sees of the map in a match, by default.

## The minimap (T47)

- **Off by default.** A game with no saved choice starts with Options, Game, "Minimap position" on
  "Minimap off": no minimap in the match HUD.
- **The player turns it on** with the same list (upper left or lower left corner); the choice is
  kept in `user.cfg` (`minimap_position`: 0 upper left, 1 lower left, 2 off) as before.
- **A saved choice wins.** Every `user.cfg` the game has written has the line, so a player who has
  played before keeps what the line says (0, the upper left, unless changed): the default is for
  a new game folder, or a `user.cfg` without the line.
- **Not changed:** the "Fix minimap" checkbox (`is_ui_minimap_fixed`, a turning or a fixed
  minimap), and the map on the Tab screen (always there).
- **How:** exe patch 57 (`client-patches/README.md`); no movie or content change.
