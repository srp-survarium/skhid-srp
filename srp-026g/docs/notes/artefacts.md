# Artefacts (the owner, 2026-10-22)

Client patch 54 (README section 54, client_patch.md section 3.51) and the edit list
`scripts/content-edit/artefacts-1.json`. The stock code and numbers:
`tools/re-kit/v026g` and the decoded retail configs (`gameplay/items/artefacts/*.options`).

## The idea

- Three combat artefacts (Onyx, Spring, Lifebone) that are best used together, one of each.
- Stacks of one kind stay possible and add up: a stack is a stronger burst, not a longer one.
  Onyx is the exception (2026-10-25): a stack is a longer burst, not a stronger one.
- The original developers mostly buffed them after 0.47 and nerfed Onyx; here Spring and
  Lifebone are buffed from 0.26g's numbers, Onyx nerfed (2026-10-25).

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
- Active (3 s, cooldown 45 s): hand and leg injuries removed as before, and pain, body and head full
  at once.
- Cooldown 45 s, was 15 s (the owner, 2026-10-23): since its active is a full heal, 15 s was too
  often (Spring 40 s, Onyx 60 s). `data/cooldown_sec` 15 -> 45 in `artefacts-1.json`.
- Nothing hard-codes it: artefact_base's constructor (0x312FD0) reads `cooldown_sec` x 1000 into
  +274h, the active's end (0x3135EC) starts the cooldown from it (times the holder's modifier 38),
  and the HUD slot's fill (0x313880) is 100 - time left x 100 / it. Patch 54 only reads the time
  left (+28Ch).

## Onyx (the owner, 2026-10-25: "nerf onyx to 3 seconds 50% damage reduction and every single new
onyx gives +1 second to activation time"; then 60 %, not 50 %)

- Passive as before: -5 % damage and piercing, each carried Onyx's adding up (x0.95 each).
- Active: -60 % damage and piercing (`active/damage_mul` and `armor_piercing_mul` 0.1 -> 0.4) for
  3 s, cooldown 60 s as before.
- Its key fires every ready Onyx; n fired together are each active 3 s + (n - 1) s (1 Onyx 3 s, 2
  Onyx 4 s, 3 Onyx 5 s) and end and cool down together. A cooling Onyx adds no time.
- Several active Onyx reduce a hit once (-60 %): only the first active Onyx of the slots takes its
  share; the others leave the hit as it is (before: x0.1 per active Onyx, all 3 s).
- An Onyx in its passive (on its cooldown) beside an active one still gives its -5 %.
- "Activation time" read as the active's duration: 0.26g has no delay before an Onyx works (the
  press sets it active on the same tick). Owner to confirm.
- The +1 s is patch 54's (`ONYX_EXTRA_MS`), not a config key; the 3 s is `active/duration`.

## Texts

- The only texts: each artefact's pickup hint (the passive) and activation hint (the active), fixed
  sentences in the six localization tables; no lobby card. `artefact-texts-1.json` makes them say
  the numbers above (Onyx's stock 10 % and 95 % were wrong too; its active 60 % since 2026-10-25).

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

