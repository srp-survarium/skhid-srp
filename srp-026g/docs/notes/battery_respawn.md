# Battery modes: respawn 10 s

The owner's rule and what it changes. Data: `gameplay/game_modes/<config>.options`
`mode/parameters/respawn_time`; the change ships as a resources.db patch
(`scripts/content-edit/modes.tsv`, the `respawn_s` column).

- **Decision (the owner, 2026-10-21):** "in batteries mode respawns should take 10 seconds".
- **Modes:** "Protective device" (mode 2, rule 88) and "Protective device: chargers" (mode 3,
  rule 89), every map. Team Deathmatch (mode 4) keeps 8 s.
- **Values:** `respawn_time` 8 -> 10 (an int, kept) in the 58 configs of modes 2 and 3: 0.26g's
  levels 1 to 8 in every weather (level_08's night too, never sent), London (`level_09`), the 0.21d
  and v0.11e maps (their mode-2 configs; they have no mode 3), Rudnya (v0.69) (`v069d_level_01`,
  both modes). One byte a file. Not changed: the test maps (`tt5` has 10, `artefact_testlevel` 1),
  `loopback_client`, mode 4.
- **Where the time comes from** (the client's code, 0.26g, PROVEN): the map id the server sends
  names the config (`maps_dict`, modes_maps.md §3); the respawn rule's constructor
  (`player_respawn_rule_base`, 0x316700) reads `respawn_time` as an int and keeps it in ms
  (`imul ecx,ecx,3E8h`, rule+0x114). Nothing on the wire carries it (static_info.md: 0.21d's
  respawn byte is gone); srp reads it into `maps::MapMode::respawn_time` and uses it nowhere.
- **Modifiers:** at a death (0x316AA0) the delay is `respawn_time × max(0, 1 + modifier 60)`
  (the getter 0x2D5AE0 sums the kit's and the player's lists), then rounded up to the 50 ms grid.
  10 s is the base: with the radio (`respawn_booster`, -20 %; one pocket a type,
  one-pocket-a-type.md) 8 s.
- **HUD:** the respawn countdown (`root.set_respawn_time`, 0x236C30) shows the respawn rule's own
  time to the local player's respawn, so it shows 10 s by itself. No text names the seconds.
- **Who computes it:** every game, in the lockstep simulation (`m_player_respawn_times` is in the
  world, snapshot_rules.md §2.3): a match plays out otherwise, so the change has the `next` row of
  `crates/session/src/v026g/simulation_changes.tsv`. Every player must have the patch (one
  supported revision).
- **Bots:** no respawn time in their code (the clients' respawn rule spawns them); nothing to change.
- **Release:** the edit list's row in `release/content.tsv`; `release/build-content` packs the 58
  configs and builds the server game data with them.
