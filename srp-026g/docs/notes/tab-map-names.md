# Names on the Tab map

The owner (2026-10-30, release 40): "when I open the map in Tab, when I press Alt I should see
nicknames on top of the player icons".

- **When:** while the Tab map is open (Tab held) and an Alt key is held (left or right). Letting
  Alt go hides them; letting Tab go hides the map with them.
- **Alt is the game's own names key:** the stock game already shows the names over the players in
  the world while left Alt is held (`render_name` 0x1F2DCD). Nothing else in a match is on Alt
  (the third-person and warm-up input of `process_third_person_mode` is not a match's). Press Tab
  first, then Alt: Alt held first and then Tab is Windows' window switcher.
- **Which players:** each player icon the map shows, as it shows it: the allies, and an enemy only
  while spotted (`update_minimap_players` 0x239420). A dead player has no icon and no name.
- **Your own icon:** no name.
- **The name:** as the Tab list shows it (`root.list_set_players`, the clan tag in brackets
  first). The map's icon and the list's row are one place (`player_id` = `id` = the roster
  place); a place with no row (not in the match) has no name.
- **Where:** centred just above the icon, small white text with a dark outline. Close icons'
  names may overlap; the outline keeps each readable.
- **Updates:** a name moves with its icon (it is part of it); the text changes only when the Tab
  list's rows do (a player entering or leaving). Nothing is made again each frame.
- **Blackout:** no names. Blackout's Tab never shows the Tab list or its map (`HUD.show_player_list`,
  forced: only ammo, quick slots, the timer and the chat, or the kits when dead), so there is no
  map to put names on. Its own Tab rule for names (teammates' names over their heads in the world
  while Tab is held, patch 65) is unchanged.
- **The minimap** (the corner one): never names.
- **How:** the hud movie (`HUD.show_map_names`, `Minimap.namesShown`, `MinimapObjectPlayer.showName`)
  and exe patch 82 (Rust `map_names`): at each key the game world handles (`on_keyboard_action`
  0x25CD1C, with the input world's keyboard; a held key comes every tick) it reads both Alt keys
  and tells the hud movie `root.show_map_names` when that changes. UI only: no simulation change.
