# A shot ends the shooter's protection

The owner (2026-11-22): "when you start shooting your invulnerability should be removed".
Exe patch 179 (`client-patches/rust/patches/src/shot_protection.rs`).

- **A shot:** a gun fired, by any player (a bot too), in every mode. A stock or knife hit, a
  battery's strike and a thrown grenade are not shots and change nothing.
- **What ends:** the life's protection, at once and whole, with no fade. It stays ended until the
  next spawn, which starts a new one as before.
- **In a base** (the battery modes' zones on 0.26g's own maps, which keep an own-team player
  protected while inside and for 2 s and a 2 s fade after): a shot ends the protection there too,
  and the zone gives none back in that life. A player who has not fired is protected by the zone
  as before, on every return to it.
- **Not firing changes nothing:** the spawn's time (6 s on most maps, 0.5 s in Free for All) and
  the zones run as they did.
- **The same in every game:** it is decided in the match simulation, so every game, a late joiner
  and a replay agree. The match simulation changes (`simulation_changes.tsv` `next`). The world
  format does not change: the patch writes a field the world already holds, with a value it
  already takes (`srp-validate`'s `invulnerability_time_ms`, `None`).

## How protection works (0.26g)

- The damage model holds `m_invulnerability_time_in_ms` (+32Ch), the time the player is
  invulnerable until, and `m_invulnerability_attenuation` (+330h), the fade after it.
  `damage_model::get_invulnerability` (0x2E5550) is 0 when the time is -1, 1 until the time, then
  1 - (now - time) / fade, then 0. A hit's damage is times (1 - it) (`v011e_spawn_protection.md`).
- Three things write the time:
  - the spawn (0x316E90): the spawn's time + its point's protection, the fade 0;
  - an effect zone (`effect_zone_core`, 0x31ACA0, its `invulnerability_duration` +48h,
    `invulnerability_attenuation` +4Ch and `team` +50h): for each own-team player inside it, the
    time + the duration and the fade, each time it runs;
  - the two-team rule (0x316470, from its on_event and on_player_left_match): when a team's word
    (+110h / +112h) and byte (+11Ch / +11Dh) are both 0, the players of its mask (+120h) are
    protected for 3 s, the fade 0.
- `base_player::initialize` makes it -1 (`damage_model::reset` 0x2E5290) and a spawn follows, so the
  game's own code never leaves a living player's time at -1. Patch 31's kit switch before the start
  makes it -1 when the spawn's protection has already run out (`kit-change-before-start.md`).

## How (exe patch 179)

- **The shot.** `weapon_stats.fired` (0x2D7843, `base_player::on_fire` 0x2D7800, ESI the player +
  110h), shared with patch 165 (order 0, its count) as order 1. `weapon_core::instant_fire`
  (0x2E23A0) calls on_fire for every shot (0x2E258D), and
  `weapon_core_fire_state_base::on_shot_event` (0x30EC7E) is instant_fire's one caller. The hook
  writes -1 into the shooter's damage model's time (the player's +2E0h, the model `hit_impl`
  reads).
- **The zone.** `effect_zone.protect` (0x31AD0F, a lohook of the 6 bytes `mov [eax+330h],ecx`, EAX
  the damage model, before `mov [eax+32Ch],edx`): when the time is -1 the hook leaves through
  `skip` (0x31AD1B, past both stores, where the other team's players go). So a zone moves on a
  protection that runs or has run out, and never starts an ended one.
- The melee strike emits its bullet without on_fire (`weapon_core_melee_state_base::on_shoot`
  0x33AB50, `emit_bullet`'s other caller), so it is not a shot here.
- A kit switched before the start after the spawn's protection ran out also has -1: in a battery
  mode's base that life gets no protection from the zone, where it did before. The next spawn
  starts one as always.
- Check: `client-patches/check_shot_protection_026g.py` (Unicorn, `tools/check shot-protection`):
  both hooks in the patched exe beside the original's bytes at the same sites. Not run in a game.
