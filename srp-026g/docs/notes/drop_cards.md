# Drop cards: one per player of a match

The rules the code and the tests follow. Code: `crates/session/src/v026g/rewards.rs`
`Results::use_drop_card` (the rule, kept with the reports) and `use_drop_card` (the roll);
`servers/lobby-server/src/v026g/dispatch.rs` `use_drop_card` (`46 03`). Tests:
`rewards/tests.rs` `a_drop_card_is_taken_once_for_each_player_of_a_match_across_a_restart`,
`core_tests.rs` `a_drop_card_is_used_once_on_the_screen_of_a_match_played`.

- **Why:** 2026-10-18..20, players bought silver -> gold (100:1), gold -> drop cards (100
  gold each, so 10,000 silver), opened thousands of cards on one kept results screen and sold
  the items (about 11,100 silver a card on average): +11 % a card, about 1M silver an hour of
  clicking. A fresh screen came from a 1-player PIN-room match left at once.
- **The owner's rule (2026-10-20):** "C: one card per player per match, own matches only,
  enforced on the server. It also sends allow_drop_card: false once the card is used, so the
  button greys out."
- **The original's rule:** one card per results screen. `VictoryScreen.as` (lobby movie):
  `onTreasureMapClick` sends `use_drop_card(_match_id, voucher id)` and hides the search icon;
  `setDropItem` puts the card's item in the slot, and `updateVoucherCount` makes the slot
  clickable again only while it holds no item, never on a cached (reopened) screen (`_cache`),
  never when the screen's data has `allow_drop_card == false`.
- **Request:** `46 03 <match id> <card item id>` (`results.md` §3). The other order is still
  tried after; the proven order's reason is the one logged.
- **Accepted** only when all hold, in this order: the match's report is kept (the last 64, and
  any screen shown in the last two hours); the account is a player of it (a place of the
  report is this account); the account used no card for it yet; the item is a drop card of
  the account with a count above 0. Then one roll of `drop_card_drop` at the match's level, at
  full condition, as before.
- **Refused** otherwise with the game's own `st_cannot_use_drop_card`; nothing changes (no
  card, no item, no mark).
- **One per (account, match):** a card for another match the account played is a new card.
  A card whose `98` cannot be written still counts (the card is spent and the item given).
- **Kept:** the used `(match, account)` pairs are in the kept results' state row
  (`g26_results` `state`, field `cards`), written with the reports at each flush and read back
  at start, so a restart does not reopen the screen. A state written before 2026-10-20 has no
  `cards` and reads as none used. The pairs go when their report goes; a card for a report
  that is gone is refused anyway.
- **`allow_drop_card`:** srp sends no such field. The client makes it itself from `97`'s
  dropped item: true when its item id or dict id is not 0 (exe 0x29D156..0x29D165,
  `results.md` §1.3 row 11). So it says "this match dropped an item", and today a screen of a
  match with no drop is already greyed (alpha 0.5) while its search icon stays clickable
  (`setData` sets `searchIcon.visible` before the grey-out). `97` goes once per match per
  player, before any card can be used; the only resend (`41 00`) needs a `#mstats` chat line
  srp never sends, and a fresh `97` would empty the slot and re-enable the search icon. So
  the server refusal is what counts; no `97` is changed for it.
- **Logged** (`log::info!`, one line each, name and account id): every card used (match,
  level, item dict and id, rarity, cards left), every refusal and its reason, every sale
  (count, dict, item id, silver paid, silver after), every silver -> gold (`bought N gold for
  S silver`) and gold -> silver (`exchanged N gold for S silver`) with the balances after.
- **Release:** server only. No client patch, no lobby movie change.
