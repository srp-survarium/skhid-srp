# Fewer skill ranks

The owner (2026-11-09): many skills "are not really useful, but take a lot of points". These
skills drop to 3 ranks, and each rank is worth more. Steps are round, and each maximum is at
least the old one:

| id  | skill        | modifier                             | was           | now            |
|-----|--------------|--------------------------------------|---------------|----------------|
| 102 | Battle stance| movement_speed_when_aiming           | 5 x +1% (+5)  | 3 x +2% (+6)   |
| 106 | Accuracy     | dispersion_from_hip                  | 5 x +2% (+10) | 3 x +4% (+12)  |
| 107 | Firm grip    | dispersion_movement_influence        | 5 x +2% (+10) | 3 x +4% (+12)  |
| 110 | Equanimity   | injury_penalty                       | 5 x -10% (-50)| 3 x -20% (-60) |
| 201 | Endurance    | stamina_spending_speed_skill         | 5 x -2% (-10) | 3 x -4% (-12)  |
| 204 | Breath       | stamina_regeneration_speed           | 5 x +2% (+10) | 3 x +4% (+12)  |
| 211 | Unstoppable  | stamina_loss_on_hit                  | 5 x -5% (-25) | 3 x -10% (-30) |
| 501 | Quick Hands  | artefact_search_time                 | 4 x -6% (-24) | 3 x -8% (-24)  |

- Data only: `scripts/content-edit/skill-ranks-1.json` (the stock seven; `set_entry` replaces
  each skill's `modifiers` and `ui/skill_descriptions` lists) and `artefact-skills-1.json`
  (Quick Hands). Rows, thresholds, icons and texts stay the same; there is no exe change.
- Saved ranks past 3 are settled when the account loads (`Account::settle_skills`): the ranks
  are taken back, the points refunded to spend again, and a row the smaller sum no longer
  reaches is taken back as well.
- Every row stays reachable. Survival rows I-IV hold 30 (row V needs 20), physical 30 (20),
  artefact 23 (20; 19 without Forest Protection, see artefact-skills.md).
- New accounts (`new-account.tsv`): Battle stance 2 still fits, so the grant is unchanged.
