# Regeneration booster

The owner (2026-11-05): the Regeneration booster (`regeneration_booster`, item 341, "Rations")
lets health regenerate to full and starts it 2 s sooner, while it is in a pocket.

## The rules

- Stock regeneration (`gameplay/hit_params/human_hit_params.options`, per pool): body and head
  0.03/s up to 50 %, arms and legs 0.03/s up to 20 %, pain 0.1/s up to 100 %; each starts 8 s
  (`regeneration_timeout`) after the pool's last hit. Other pools do not regenerate (rate 0).
- Before: built-in modification 1094, regeneration speed +15 % (kit modifier 2). Kept.
- Added: delay -2 s and limit +100 %, so every regenerating pool goes to full (at most 100 %).
- Metabolism (skill 311, -0.5 s a rank) stays where it was (the pool's delay, at the player's
  construction); the booster's seconds come after it: 8 s, with Metabolism 5 ranks 5.5 s, with
  the booster 6 s and 3.5 s. Never below 0.
- Two would add up (a second booster, a future item of the kind): the limit stays at most 100 %.
- The card shows them as built-in bonuses, as the other boosters' (the owner): "Regeneration
  delay: -2s" and "Regeneration limit: +100%" under "Regeneration: +15%" (UI properties 171 and
  172, labels in all 10 languages). Alt on cards covers weapons and medicines only, not
  regeneration: nothing to show there.

## How

- Data: built-in modifications 1290 (`parameters/engineer_success_chance_modifier` -2) and 1300
  (`parameters/engineer_use_time_modifier` 1), type 21 (regeneration's), drop weight 0, made by
  `regeneration-booster-1.json` with their card lines and the two keys at 0 in the booster's
  config (the item cook skips a modification whose key the config lacks); their values are rows
  of `modifications.tsv`, the booster's list the columns `regen_delay` and `regen_limit` of
  `boosters.tsv`. A booster in a pocket makes a kit modifier of each parameter that is not 0
  (`survarium::booster`, 0x33F690 / 0x33F990), at every spawn, for the kit it is in.
- Modifiers 10 (engineer_success_chance) and 9 (engineer_use_time): the exe has no call of the
  getter with either index and no config sets them (re-kit `abi_inputs.tsv`, 0x2D5AE0). 10 is
  additive in the getter (list 0x927E08): seconds; 9 is multiplicative: read as
  max(0, 1 + S) - 1, a fraction.
- Exe patch 94 (`medical_skills`, site `medical.regenerate`, 0x32A53F): `body_part::regenerate`
  where both paths of its regeneration-modifier loop join. Each tick, for a pool with a rate
  above 0, the local delay (EAX) loses the kit's seconds and the local limit (ESP+18h) gains its
  fraction. The pool's fields are not written: nothing compounds, and the HUD doll's break point
  (+A8h) keeps the stock limit. The current kit is read each tick, so a kit change takes effect
  at the next spawn with no state of ours.
- A match computes it: the `next` row of `simulation_changes.tsv`.

## Checks

- `check_medical_skills_026g.py` (Unicorn, kits 0 and 2): the join site with and without the
  modifiers for body, head, a leg, pain and a pool without regeneration; the whole native
  `body_part::regenerate` from 10 % body health, to 50 % without and 100 % with, nothing before
  the delay.
- `medical_skills` crate tests (`regeneration_starts_sooner_and_goes_to_full_health`).
- Not played.
