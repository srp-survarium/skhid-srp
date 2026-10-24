# Bandages (the owner, 2026-10-24)

The Pack of bandages (`bandages`, dict id 66, `gameplay/items/drugs/bandages.options`), the only
bandage in 0.26g. The edit list is `scripts/content-edit/bandages-1.json`.

## The rule

- Hands and legs: fully healed.
- Pain, body and head: 35 % of each pool healed.
- Hand and leg injuries removed, as before.
- Use time 1 s, then active for 1 s, as before. Weight and price stay the same.
- The lobby says so: a new description and no "Heal amount" line on the card.

## How a drug heals (PROVEN, `medkit::tick` 0x33D790)

- `data/influences`: one entry per pool (`body_part`), `amount` = the pool's health added per second.
- Healing starts after `activation_delay_sec` and lasts `activity_time_sec`. Each tick adds
  amount x elapsed s to the pool (`increase_health` 0x32A420, clamped to the pool's max).
- Total = amount x activity time. Pool max: body 1.0, head 0.4, each hand and leg 0.5, pain 1.0.
- The amount goes through modifier 57 `drug_amount` and the activity time through 34
  `drug_activity_time` (k = `parameters/activity_time_skill_influence`). No skill or item in 0.26g
  sets either one, so both stay as configured.
- Pain is a pool like the others: an influence on `pain` takes pain away (as Lifebone's active does).
- The heal stays when the drug ends. `set_active` only resets the per-influence total at +284h.

## Values (activity 1 s)

| pool | before | after |
|---|---|---|
| each hand, each leg | 0.3/s: +0.3 of 0.5 (60 %) | 1/s: full (as the medkit's 1/s) |
| body | none | 0.35/s: +0.35 of 1.0 (35 %) |
| head | none | 0.14/s: +0.14 of 0.4 (35 %) |
| pain | none | 0.35/s: +0.35 of 1.0 (35 %) |

- "35 %" means 35 % of the pool's max, not 35 % of what is missing.
- The medkit's pain influence is 0, so the medkit takes no pain away. Bandages now do. The
  medkit stays like this (the owner, 2026-10-25).
- The match simulation changes (`simulation_changes.tsv` `next` row).

## The lobby (the owner, 2026-10-25)

- Description (`st_base_bandages_description`, all six tables): "Fully heals arms and legs.
  Restores 35% of body, head and pain."
- The card's "Heal amount" line (`ui_desc/props_list` prop_id 12, "30") is removed, so it can't
  contradict the description. Use time, active time and weight stay on the card.
- Each text uses only characters its table already has, and writes "35%" with no space, as the
  tables do.
