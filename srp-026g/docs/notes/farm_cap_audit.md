# PR #691: preserving contested play while limiting farming pay

Revised 2026-10-28. These are counterfactual payment limits, not retroactive balance changes.
The evidence identifies patterns, not intent; a high objective score alone is not proof of farming.

## Evidence and method

The initial read-only audit used 166 saved matches: the pre-revision-36 backup and the current
sample, with duplicate match IDs resolved in favour of the current report. It decoded 64,725
unique checked snapshots with no decode failures or counter resets. The reconstructed baseline
silver matched all 765 paid player results exactly. Snapshots and account identities stayed on the
server; only anonymous counter/payout summaries were retained locally.

The revision was checked against those **existing anonymous summaries**, not a fresh server
replay. They retain objective totals, kills, assists, teams and the old paired-kill excess. This
suffices to decide whether each result still crosses a cap. For missing victim attribution, the
new excess is bounded between `max(0, old excess - 2 * sum(enemy kills))` and
`max(0, old excess - 2 * min(enemy kills))`. Those bounds agree for every affected PvP result except
one player in `5d0a1a32` (zero or one unpaid kill); that player's objectives exceed the allowance
in either case. Bots are exempt. The original audit's snapshot-attribution limitations still apply.

| Cohort | Paid results | Previously reduced | Meets revised cap conditions | Full payment restored |
|---|---:|---:|---:|---:|
| Earlier human matches | 596 | 17 | 10 | 7 |
| Current-sample human matches | 107 | 77 | 70 | 7 |
| Human-match total | 703 | 94 | 80 | 14 |

This table isolates the PvP event caps. The 62 bot-match results are excluded: after the first
revision, the owner requested a separate **tier-7 PvE reward ceiling** (`results.md` §16). Their
per-victim caps are removed, but they no longer necessarily retain their original payouts.

No previously unaffected PvP result crosses a revised event cap. This does **not** classify all remaining
80 results as farming or all restored results as legitimate. It does not recompute the entire new
payout distribution. Exact revised aggregate silver, including the one uncertain attribution,
requires replaying the revised code against saved snapshots. The silver floor is excluded from
these cap comparisons; its implementation is unchanged.

## Calibration cases

| Pattern / match | Flat caps | Revised rules |
|---|---|---|
| `5a0aaa5c`: 21 captures + 20 neutralisations, zero kills/assists | 272,703 → about 45,382 silver for one player | Same 200-point allowance, about 83% reduction |
| `7964b894`: contested Research, 12/10 and 9/8 | 106,106 → 57,422; 102,475 → 58,491 | Both keep full payment |
| `64b3770`: 8/10 with 2 assists, 504 objective points | Exceeded 200 | All objectives pay; allowance 560 |
| `64b3770`: 5/10 with 4 assists, 499.2 objective points | Exceeded 200 | Allowance 480; about 3.6% total silver reduction estimated |
| `51de3e34`: small PvP, 18/8 | 3,384 → 2,275 | Full payment; the opposing player had 19 kills |
| `537680e1`: small PvP, 14/6 | 23,645 → 19,439 | Full payment; the opponent had 5 kills |
| `2cbb0374`: bot match, 128/11 | 63,389 → 48,207 | No per-victim cap; tier-7 reward ceiling applies separately |
| `74fbcbde`: 23 kills of a zero-kill opponent | 13 kills unpaid | Still 13 kills unpaid |

The 3.6% estimate scales the old objective-payment loss by the newly removed objective share;
it is not an exact by-kit payout replay and may differ with integer rounding and bonuses. The
zero-combat example uses the same counters as the original cap and so retains its old reduction.

## Rules and tradeoffs

- Research captures and neutralisations share **200 + 40 per paid kill + 20 per assist** points.
  The combat increment is four times ordinary kill/assist score (10/5). Kill-invader pay is whole.
  The allowance deliberately errs towards keeping contested play paid; it does not exempt every
  player who happens to have a few kills. A 1,230-point zero-combat farm still has a 200-point limit.
- Repeated kills of each opponent pay up to **10 + 2 per enemy kill made by that opponent**.
  An active opponent is less evidence of passive feeding. Bot matches skip this cap; their reward tier is capped at seven instead.
- One owner applies both rules to a payment-only copy, repeated kills first. Reduced kills cannot
  buy additional objective allowance. Original counters still supply score, quests and rating;
  battery-cap quest semantics are unchanged.
- Objective specialists can still lose pay above the allowance. Repeated victories over a
  legitimate passive opponent can still hit the kill limit. Reciprocal or ambiguous kill farming
  can escape it. More precise detection needs richer event evidence, not claims about intent
  inferred from these aggregate counters.

Regression tests cover the historical contested cases, zero-combat trading, active/passive
victims, ambiguous attribution, roster-order independence, bot exemption, integer rounding,
cap ordering, match-result counters, and settlement's score/quest isolation.
