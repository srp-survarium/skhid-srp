# Artefacts (the owner, 2026-10-22)

Client patch 54 (README section 54, client_patch.md section 3.51) and the edit list
`scripts/content-edit/artefacts-1.json`. The stock code and numbers:
`tools/re-kit/v026g` and the decoded retail configs (`gameplay/items/artefacts/*.options`).

## The idea

- Three combat artefacts (Onyx, Spring, Lifebone) that are best used together, one of each.
- Stacks of one kind stay possible and add up: a stack is a stronger burst, not a longer one.
- The original developers mostly buffed them after 0.47 and nerfed Onyx; here they are buffed
  from 0.26g's numbers, none is nerfed.

## Every artefact

- Passive from pickup to death, as before; each copy's passive adds up with the others'.
- A press of an artefact's key fires every artefact of that kind in the slots that is ready
  (its passive, its cooldown 0): one key, all of a kind, their actives added up.
- A copy on its cooldown or already active does nothing on that press; it is not fired later.
- The key of a copy that is not ready still fires the ready ones of its kind.
- Different kinds have their own keys and cooldowns: Onyx, Spring and Lifebone fire on their own.
- Every game of the match computes this the same way (the press is an input).

## Spring

- Passive: movement speed +5 % (was +2 %), aim time -5 %.
- Active (10 s, cooldown 40 s): movement speed +10 % and aim time -10 % on top of the passive
  (+15 %, -15 % in all), stamina as before (no drain, regeneration x6), and bullets do not slow
  you (their stopping power; their stamina damage as before).
- The speed applies while aiming too (aiming's own speed modifier, 20, is 0 in retail).
- `max_carried_weight_mod` (0 in retail) is no longer read: its node is the passive's aim time.

## Lifebone

- Passive: each of its seven pools (body, head, both hands, both legs, pain) regenerates
  base + 0.1/s, up to 100 %, as before.
- Regeneration starts after the pool's own 8 s without a hit (the artefact no longer adds 2 s).
- Active (3 s, cooldown 15 s): hand and leg injuries removed as before, and pain, body and head full
  at once.

## Onyx

- As it is (passive -5 % damage and piercing, active -90 % for 3 s, cooldown 60 s); its key fires
  every ready Onyx too.

## Release

- Changes what a match computes: a `next` row in `crates/session/src/v026g/simulation_changes.tsv`,
  the server asks for the release's revision.
- The exe reads `passive/aim_time_mod` and `active/aim_time_mod`: it needs the edit list's
  resources.db patch (the release's exe requires its patches, patch 49).

## Aim time at its limit (the owner's decision, 2026-10-23)

- The aim-time modifiers add up: the getter 0x2D5AE0 (index 49) gives the weapon's `aim_time` x
  max(0, 1 + the sum), floored at 0 by the game itself; scope-in 0x2E1B50 and scope-out 0x2E1BF0
  use it. Sources on real data: Spring -0.05 passive / -0.10 active each (artefact slots 13..18),
  the skill tree up to -0.10, gloves (modifications 1080..1084) and weapons (2070..2074) up to
  -0.10 each.
- The sum reaches -100 % only with five or six Springs all active and the other sources; the
  passives alone stop at -0.30 (-0.60 with all others). At 0 the scope is instant for those
  seconds; nothing faults (the timer 0x32FE40 snaps to the end; SSE exceptions masked) and every
  game computes the same.
- The owner: "Leave it" (asked whether to floor it at 25 % or lower Spring's values): no floor.

