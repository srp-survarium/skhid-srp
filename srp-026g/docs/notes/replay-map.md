# Enemies on a replay's map

The owner (2026-11-10): "add feature to replays to see dots of red players (opponents) too".

- **When:** watching a `.sur` replay ("Watch replay" in History, `-match_replay`), first or third
  person on a player.
- **What:** the minimap (the corner one) and the Tab map show every enemy that is alive and in the
  world as a red dot, spotted or not. Teammates as before.
- **Whose team:** the followed player's (the replay's local player is the one the camera is on).
  Following another player with F5/F6 or the player list turns the sides with it.
- **Free view** (F2, no player followed): no dots, as before (the game draws none without a
  followed player).
- **Live matches:** unchanged. A player sees an enemy only while it is spotted. A live spectator
  (rooms' Queue and spectate, live watching) sees the same as in the match: enemies only when spotted.
  Only a replay's own options read turns the rule on, and each live match's options read turns it off.
- **Names on map** (Alt on the Tab map, patch 82): the enemies' names show with their dots.
- **How:** exe patch 135 (Rust `replay_map`), one lohook at `update_minimap_players`' spotted test
  (0x239505). No movie change: the hud movie already draws an enemy's icon red (`player_red`).
- Nothing a match computes; no simulation change.
