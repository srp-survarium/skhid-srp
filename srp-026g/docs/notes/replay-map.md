# Enemies in a replay

The owner (2026-11-10): "add feature to replays to see dots of red players (opponents) too"; "not
just dots, also names if I press alt"; "yes in red for enemies".

- **When:** watching a `.sur` replay ("Watch replay" in History, `-match_replay`), first or third
  person on a player.
- **Dots:** the minimap (the corner one) and the Tab map show every enemy that is alive and in the
  world as a red dot, spotted or not. Teammates as before.
- **Over their heads:** every enemy alive, in the world and on the screen gets the marker
  teammates have, red. Held left Alt shows its name over it in red, as teammates' names show in
  white (a squad member's green). The name comes after the marker's usual short delay.
- **Tab map names** (Alt on the Tab map, patch 82): an enemy's name is red there too, in a live
  match as well (the spotted enemies it shows); allies' stay white.
- **Whose team:** the followed player's (the replay's local player is the one the camera is on).
  Following another player with F5/F6 or the player list turns the sides with it.
- **Free view** (F2, no player followed): no dots, as before. Over the heads, by the code (not
  seen yet): with no local player the game takes every player for an enemy (local team 2), so
  every marker shows, red, both teams (before, the movie hid the first followed player's enemies).
- **Live matches:** unchanged. A player sees an enemy's dot only while it is spotted, and never its
  marker or name over its head. A live spectator (rooms' Queue and spectate, live watching) sees
  the same as in the match.
- **Never switchable in a match** (owner 2026-11-10: "make sure this code cannot be run from normal
  matches easily"): no flag, setting, key, console command or state of the patch's turns it on.
  Each time, the hook asks the game's network client what it is (its class): only a `.sur`
  replay's is the replay client. Making a live game's client claim that ends its match.
- **How:** exe patch 135 (Rust `replay_map`, `vostok::world::Playback`): lohooks at the two spotted tests,
  `update_minimap_players` (0x239505) and `render_name` (0x1F2B50); and before `initialize_match`
  sends the icons movie its enemy list (0x235709), the movie told `root.srp_show_enemies`. The
  player icons movie (`HUDIconLayer.enemiesShown`) then shows enemies' markers. The hud movie's
  Tab map names an enemy red (`PlayerRed.nameColor`). The red over the heads is the game's own
  (`0xFF0000`, render_name 0x1F300F).
- Nothing a match computes; no simulation change.
