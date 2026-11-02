# v0.11 maps: spawn protection 6 s

The owner's rule and what it changes. Data: `projects/<map>/<project>/client_project`
`server_objects/respawn_points[i]/invulnerability_time`; the change ships as a resources.db patch
(`scripts/content-edit/maps.tsv`, the `spawn_protection_s` column).

- **Decision (the owner, 2026-10-21):** after "v0.11 maps have very fast protection drop off
  compared to other maps": "just make it 6 s and that's it".
- **Maps:** Vostok Radar Station (v0.11) `v011e_level_03` (map_id 13, 43 points) and Chemical Plant
  (v0.11) `v011e_level_04` (map_id 12, 44 points), both projects of each: `day_2` (Protective
  device) and `day_4` (Team Deathmatch). 174 values.
- **Values:** `invulnerability_time` 2 -> 6 (an int, kept), as every respawn point of 0.26g's own
  maps and of the 0.21d ports (level_09's Team Deathmatch has 3). One byte a point.
- **Not changed (the owner's choice):** no base `effect_zone` is added and no
  `ally_check_radius` / `enemy_check_radius` (both stay 0, the loader's default).
- **Why it was 2 s:** 0.11e authored `invulnerability_time = 2000` (ms; its loader kept it as is);
  the port converted it to 2 native seconds. Faithful to 0.11e, not a unit bug.
- **How protection works** (the client's code, 0.26g, PROVEN):
  - the respawn point's loader (0x2F4580) reads the key as an int and keeps it in ms
    (`imul edx,edx,3E8h`, 0x2F462A, respawn point +0x20); a missing key is 0 (constructor 0x2F4400);
  - the spawn (0x316E90) sets the damage model's `m_invulnerability_time_in_ms` (+0x32C) to the
    spawn time plus it and `m_invulnerability_attenuation` (+0x330) to 0: a hard end, no fade;
  - `damage_model::get_invulnerability` (0x2E5550) is 1 until that time, then
    1 - (now - end) / attenuation, then 0; a hit's damage is times (1 - it) (0x2E50A8).
- **What stays different from 0.26g's maps:** in the battery modes 0.26g's maps also have a base
  `effect_zone` a team (0x31AC10 / 0x31ACA0: while an own-team player is inside, the end is now +
  `invulnerability_duration`, the fade `invulnerability_attenuation`; mostly 2000 / 2000 ms, so
  protected in base and 2 s + a 2 s fade after leaving it). The v0.11 maps have none: their
  protection now ends 6 s after the spawn wherever the player is, as on the 0.21d port of
  level_06 (also without a base zone).
- **Who computes it:** every game, in the lockstep simulation (the damage model's invulnerability
  is in the world): the change adds its words to the `next` row of
  `crates/session/src/v026g/simulation_changes.tsv`. Every player must have the patch.
- **Release:** the edit list's row in `release/content.tsv`; `release/build-content` takes the
  four projects as the base leaves them (`v011e_level_03` from patch 23, `v011e_level_04` from
  patch 22), packs them and builds the server game data with them.
