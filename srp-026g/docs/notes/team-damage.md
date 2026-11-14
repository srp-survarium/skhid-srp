# Team damage turned back on its dealer (0.26g)

A player's ask, through the owner (2026-10-23): "Any chance to add old auto-damage for team
damage/killing?" The owner: it should kick in only past a threshold; small accidental team hits
are not turned back.

## The rule (as 0.28a to 0.31a2 had it)

- **Counted per player, per match:** all the health his hits took off other players, and how
  much of that was teammates'. 1.0 is a full health bar. The counts never go down (no decay) and
  start again only with the next match. A place keeps its counts when its player leaves and comes
  back; a late joiner gets a new place, so his start from zero.
- **The threshold:** a player's team damage is **more than 3.0** (three full health bars) **and
  more than 67 %** of all the damage he dealt. Both must hold: a player who also fights the
  enemy stays under the share.
- **Past it**, each hit of his on a teammate:
  - does nothing to the teammate;
  - hits him instead, as the same hit (same body part, his own armour) with **twice** its
    damage.
- **Below it**, team damage is dealt as before and counted.
- **A kill by a turned hit** is his own death: a suicide (a death, no kill, no team kill; in Team
  Deathmatch a ticket, as any suicide). The teammate he aimed at is not hurt, so there is no team
  kill.
- **What counts:** a hit with a dealer and a source (a bullet, a melee hit, a grenade, a trap).
  Not counted and never turned: falls, anomalies, his own grenade or trap on himself (his own
  place), hits of a dealer no longer in the match.
- **Explosives:** each player a grenade hurts is one hit. Past the threshold, a grenade that
  catches two teammates hits its thrower twice (2 x its damage each) and them not at all.
- **Late joiners, rollbacks, replays:** the counts are part of the saved world, so every game of
  the match has the same; a replay plays them as the match did.
- **Which matches:** a public match of two teams only, by the anti-AFK's own test (the owner,
  2026-11-14: "FFA shouldn't have this, bots shouldn't, private matches shouldn't. Same rules (same
  code) as for 3min afk"): not a private room's (a PIN), not one against bots, not a
  free-for-all. One function decides it (`registry::MatchKind::of`, from the formed match's own
  fields), once, when the match is made; a late joiner's static info has the rule the match was
  made with. The free-for-all became exempt from the anti-AFK by the same change (`anti-afk.md`).
- **Server setting** `SRP_TEAM_DAMAGE` (srp's environment): unset or `on` for 0.28a's numbers
  (3.0, 0.67, 2.0), `off` for none, or `max_amount,max_ratio,return_ratio` (e.g. `2,0.5,1`;
  0 to 100, 0 to 1, 0 to 16). **On by default** in those matches: 0.28a to 0.31a2 had it in every
  match, with no switch.
- **Not ported:** 0.28's two messages ("You attacked an ally" at each team hit, "You were killed
  for injuring your allies" when turned): 0.26g has no such texts (a content and HUD change of
  their own); 0.28a2's drop-card refusal after team kills (`st_reject_drop_for_team_kill`); 0.46's
  later variant (no `max_amount`).

## How (exe patch 63; srp)

- **One owner of the two formats:** `crates/srp-validate/src/team_damage.rs`. The rule
  (`Rule`) is srp's entry 1 in the static info's block after the squads (`SRPX`), before the
  mode's own entry (Blackout's 2, a free-for-all's 3); the match server writes it, the game reads
  it, the server's check of the options takes it. The counts (`Counts`) are a record after a
  world's effects: `SRTD`, the places up to the last that dealt any (1 to 20), two f32 a place
  (all, teammates'); no record when nobody dealt any. The game writes and reads it, the server's
  world check (`world::effects`) reads it as the game does: the places at most the match's, each
  count a number of no sign, the teammates' at most all, the last place one that dealt.
- **The patch** (Rust, `rust/patches/src/team_damage.rs`, `vostok/team_damage.rs`): six lohooks
  (`sites_026g.tsv`): `player.hit_start` (hit_impl past its tests of a dead and a dying victim,
  0x2D9BB6; its exit `skipped` the function's own way out, 0x2D9F1A), `player.pain_lost`
  (0x2D9EA9), `world_save.effects_written` (0x2D2716), `world_load.effects_read` (0x2D2C75) and
  the two options reads (Blackout's sites, last in their chains). The turned hit goes through
  `engine_hit_impl`. The rule and the counts are one state (`Match`, off or on with them), its
  one transition function `step`. `client-patches/README.md` patch 63, `client_patch.md`
  "Team damage (patch 63)"; its check `check_team_damage_026g.py`.
- A simulation change: `crates/session/src/v026g/simulation_changes.tsv` `next`.

## Where it comes from (sources)

PROVEN unless tagged. Builds: `/mnt/hdd/Projects/weapon-survey-20261016/work/builds/<v>` (exes
x86-BCJ decoded), `/mnt/toshiba/surv-resources/<v>` (0.1.1a to 0.26e), the 0.10b store db.

| build | team damage |
|---|---|
| 0.10b, 0.1.1a-e, 0.20e/f, 0.21d, 0.23h, 0.26e, 0.26g, 0.27d0 | none: no key in the gameplay configs (all but items, dumped), no text (0.20e on; 0.10b and 0.1.1 have no English table to dump); none in the exes read (0.21d with its PDB, 0.26g, 0.27d0). `team_kill` is only a score event, a ban reason and (0.27d) a badge limit |
| 0.28a2, 0.28d0, 0.31a2 | `gameplay/players/default.player` `player.teamdamage_params` {`max_amount` 3, `max_ratio` 0.67, `return_ratio` 2}; texts `st_team_damage_inflicted` "You attacked an ally", `st_team_damage_received` "You were killed for injuring your allies" |
| 0.46c2 | the texts and `teamdamage_params` with `max_ratio` and `return_ratio` only (exe strings; its config not read) |

0.28a2's code (VAs of the decoded exe, image base 0x10000):

- `teamdamage_params` is read by the player parameters' loader 0x5A66F0 (at 0x5A67B4) into
  +1D0h (`max_amount`), +1D4h (`max_ratio`), +1D8h (`return_ratio`); the player uses them at
  +3E0h, +3E4h, +3E8h (LIKELY the same struct at +210h: the three offsets differ by the same
  210h, and each is used as its name says).
- Each roster place has a client record (`game_world_core_client`, 3Ch bytes, world+1B808h) with
  `m_inflicted_damage` (+34h) and `m_inflicted_teamdamage` (+38h), saved with the world
  (`game_world_core_client::serialize`, 0x5FABE6). Only the record's construction at the match's
  start sets them (0x5FAA60 with the world 0x5A3410).
- After a hit took health off a player (the player's hit function 0x5AA3E0, at 0x5AA620), the
  dealer (the hit's +8, another place than the victim's, the hit's source +4 set) gets
  0x5AA940: the health lost added to +34h, and to +38h for a teammate, whose game then shows
  "You attacked an ally" (hud slot 44h, 0x49DCF0).
- Before a hit from a teammate is dealt (0x5AA3E0 from 0x5AA43B, the victim alive), 0x5AA850:
  `team / all > max_ratio` and `team > max_amount`; then the hit (78h bytes, copied) has its
  damage values times `return_ratio`, goes to the dealer's own hit function, his game shows "You
  were killed for injuring your allies" (hud slot 48h, 0x49DD50), and the victim's returns 0
  (no damage).
- 0.26g: the health that kills is the pool "pain" (0.28's "health"); `base_player::hit_impl`
  0x2D9B90 computes its loss at 0x2D9EA9 for the player's on_hit; the hit's dealer is +44h, its
  source +20h, its damage +34h (+38h the armour piercing, left as it is). Nothing in 0.26g counts
  team damage: the statistics' received damage (0x2F58E0) skips teammates and is cleared at each
  death (0x2F5330..0x2F5445).
