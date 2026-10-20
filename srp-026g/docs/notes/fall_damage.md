# Fall damage

The owner's rule and what it changes. Data: `gameplay/players/default.player`
`player.fall_params`; the change ships as a resources.db patch
(`scripts/content-edit/fall-damage-1.json`, one `set`).

- **Decision (the owner, 2026-10-19):** `fatal_damage` 1.0 as in 0.27: the
  0.27d0 value, kept by every later build (0.28a2, 0.28d0, 0.31a2, 0.46c2,
  0.69d: `i32:1`). 0.20e to 0.26g have `f32:1.5`. Nothing else changes.
- **0.27's changes only (the owner, 2026-10-20):** this takes what 0.27 changed
  in falls and nothing a later version (0.30 and on) changed. A fall value that
  differs only from 0.28 on is not taken here; a later version's changes would
  be a decision of their own.
- **Values:** `fatal_damage` 1.5 -> 1 (0.27 writes `i32:1`; the patch keeps the
  field's f32 and writes 1.0, which the loader reads alike: it takes i32 or
  f32). Unchanged: `harmless_height` 5, `fatal_height` 10, `gravity` 9.8,
  `soft_landing_height` 1.8, `noiseless_height` 1.
- **Who computes it:** the client, in the lockstep simulation (handler
  0x2d8220; the loader 0x2d6600 reads `fall_params`). srp does not simulate
  falls, and its server data reads nothing of `fall_params`.
- **Formula:** landing speed v = sqrt(2·g·h). Up to `harmless_height`
  (v ≤ 9.9 m/s) nothing; from there to `fatal_height` (v = 14 m/s)
  D = fatal_damage · (v − 9.9) / (14 − 9.9); higher, D = fatal_damage.
  D is dealt as a blunt hit to each foot (foot blunt -> leg ×1, pain ×0.8).
  The `fall_damage` skill (207, −0.15 a level, 2 levels) is the player's
  modifier of it (LIKELY; not read in the handler).
- **What it means:**

  | height | v (m/s) | D stock (1.5) | D new (1.0) |
  |---|---|---|---|
  | 5 m | 9.9 | 0 | 0 |
  | 6 m | 10.8 | 0.35 | 0.23 |
  | 7 m | 11.7 | 0.66 | 0.44 |
  | 8 m | 12.5 | 0.96 | 0.64 |
  | 9 m | 13.3 | 1.24 | 0.82 |
  | 10 m and more | 14.0 | 1.5 | 1.0 |

  - damage starts just above 5 m in both (the formula: any v over 9.9 m/s);
  - lethal from full health: about 6.8 m stock, so about 7.8 m new (a fall kills
    at the same D, about 0.6, whatever `fatal_damage` is).
- **Bots:** their map data takes a drop of at most `harmless_height` (5 m),
  unchanged.
- **Release:** the resources.db patch, made by `scripts/content-edit/content_edit.py`
  from `scripts/content-edit/fall-damage-1.json` (with `default.player` from the
  live server game data) and `release/build-patch`; the server game data built
  again with that patch (srp reads `default.player` for the carry limits, and the
  server data must be the one the players' patches make); and a `next` row of
  `crates/session/src/v026g/simulation_changes.tsv` (a match plays out
  otherwise), so recordings from earlier revisions are no longer watchable
  after the release, as for every simulation change.
