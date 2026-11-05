# Repeated-kill payment limit

PR #691, revised 2026-10-28 after the owner asked to preserve normal play.

- In PvP, kills of one enemy pay at most **10 + 2 × that enemy's total enemy kills**.
  A passive feeder still supplies only ten paying kills. An active opponent with six kills
  allows 22, protecting the historical small-match 18-kill case that lost 33% under the flat cap.
- Bot matches skip this per-victim cap; settlement caps their reward tier at 7 instead. PvP is not
  exempt just because its roster is small.
- Headshot, melee, silent and grenade payment counters are reduced in the same proportion.
  Score, displayed kills/deaths, all quest counters and rating retain the original statistics.
- The server sees running totals, not explicit killer-victim events. A trusted snapshot with
  exactly one player's kills increased and matching enemy deaths (no teammate or unknown-team
  deaths) pairs them. Ambiguous kills still pay. This cannot detect every farm.
- Every allowance uses the final **original** victim kill count, captured before reducing anyone's
  pay. Otherwise roster order would change the outcome. A victim's late combat counts too.
- `kill_cap.rs` applies the limit; `reward_caps::Payment` owns its payment-only copy and applies
  it before the Research allowance. Unpaid feeding kills do not buy more Research objective pay.
- Active opponents can also collude, and a legitimate player can repeatedly kill a passive
  opponent. This deliberately conservative heuristic does not prove intent.
