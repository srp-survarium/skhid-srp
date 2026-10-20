# Spotting (patch 38)

The rules the code and the tests follow. Code: `client-patches/spot_026g.asm` and
`SPOT_COOLDOWN` in `client-patches/patch_026g.py`, README patch 38, `client_patch.md`
section 3.35; the check: `client-patches/check_spot_026g.py`.

- **Decision (the owner, 2026-10-19):** no spot cooldown at all, for every spot (players
  and batteries); a hit on a battery's body spots it, as its use box does. Reported:
  "battery marking sometimes works, sometimes doesn't".
- **A spot:** one per press of the spot key (input bit 28 is set on the key going down
  only; holding it or the key's repeat sends no new spot). A 100 m ray from the eye;
  the first spottable hit before something walkable and hittable is spotted.
- **No cooldown:** every press casts the ray, hit or miss. The stock game waited 3 s after
  any press, a miss too (`m_time_left_to_try_spot_in_ms`, player+34Ch, set to 3000 at
  0x2D87C1); now it is 0 at every tick, in every game and in every snapshot.
- **What spots a battery:** its use box (`!victory_item_collision!`) and its rigid body.
  Before, only the use box; a ray that met the body first spotted nothing and could stop at
  the floor behind it.
- **Players:** an enemy is marked for 10 s; a press on them again starts the 10 s again.
  A teammate is never spotted; the ray goes on past them.
- **Battery markers** (unchanged by this patch):
  - none at spawn;
  - wiped on pickup;
  - the dropper's team after a drop;
  - cleared in a container.
- **A battery spotted again:** the marker stays (the team's bit is set already); the match
  statistics get the spot event again, as a press after the stock 3 s did; nothing else
  listens to it. Only the battery's first spotter gets the statistics' spot award, once
  (type 13h, 0x334B32); a spot again awards nothing, so pressing fast earns nothing.
- **Who computes it:** every game, in the lockstep simulation (base_player::apply_new_input);
  srp only relays the input. A game without the patch spots otherwise: a server asks for
  the release's revision.
- **Release:** an exe patch (the default build) and a `next` row in
  `crates/session/src/v026g/simulation_changes.tsv`: recordings of earlier revisions are no
  longer watchable after the release, as for every simulation change.
