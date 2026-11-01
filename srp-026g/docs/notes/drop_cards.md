# Drop cards: one per player of a match

The rules the code and the tests follow. Code: `crates/session/src/v026g/rewards.rs`
`Results::use_drop_card` (the rule) and `use_drop_card` (the roll); the mark is
`Account::drop_cards`, kept in `g26_drop_cards`; `servers/lobby-server/src/v026g/dispatch.rs`
`use_drop_card` (`46 03`). Tests: `rewards/tests.rs`
`a_drop_card_is_taken_once_for_each_finisher_of_a_match_across_a_restart`,
`a_crash_after_the_accounts_write_takes_no_second_card`,
`a_roll_that_gives_nothing_keeps_the_card_and_the_match`,
`the_marks_of_matches_no_longer_kept_go`; `core_tests.rs`
`a_drop_card_is_used_once_on_the_screen_of_a_match_played`.

- **Why:** 2026-10-18..20, players bought silver -> gold (100:1), gold -> drop cards (100
  gold each, so 10,000 silver), opened thousands of cards on one kept results screen and sold
  the items (about 11,100 silver a card on average): +11 % a card, about 1M silver an hour of
  clicking. A fresh screen came from a 1-player PIN-room match left at once.
- **The owner's rule (2026-10-20):** "C: one card per player per match, own matches only,
  enforced on the server. It also sends allow_drop_card: false once the card is used, so the
  button greys out."
- **What ships:** the server refusal. A second card for a match, a card for a match the
  account did not finish, and a card for a match not kept are refused with the game's
  `st_cannot_use_drop_card`.
- **The slot greys** (lobby movie, `VictoryScreen.as`; 2026-10-21): alpha 0.5, no search
  icon, after the card's item comes (`setDropItem`), after a refusal of the card asked (the
  exe's message box while it is asked), and from the start for a match of a room with a PIN
  (`RoomsWindow.inPinRoom` when `97` comes). The movie keeps those matches for its session
  (`_cardsDone`), so a reopened screen of the match stays grey. A player who left early is
  not known to the movie: the first click is refused, then the slot greys. The server tells
  the movie nothing for it: `97` has no field the exe hands it (see `allow_drop_card` below).
- **Messages over the results screen:** `show_victory_screen` keeps messages back till the
  screen closes (`Message.hideMessages`), and the exe's refusal box comes through
  `show_system_message`, so the refusal showed only after the screen closed, as if under it.
  Now a `debug` or `ok` box (the lobby's refusals and errors) is drawn at once over the
  screen (`VictoryScreen.messageOver`), and the message layer is made visible first (a
  match's loading hides it); so is it for the screen's premium and player windows.
- **The original's rule:** one card per results screen. `VictoryScreen.as` (lobby movie):
  `onTreasureMapClick` sends `use_drop_card(_match_id, voucher id)` and hides the search icon;
  `setDropItem` puts the card's item in the slot, and `updateVoucherCount` makes the slot
  clickable again only while it holds no item, never on a cached (reopened) screen (`_cache`),
  never when the screen's data has `allow_drop_card == false`.
- **Request:** `46 03 <match id> <card item id>` (`results.md` §3). The other order is still
  tried after; the proven order's reason is the one logged.
- **Accepted** only when all hold, in this order: the match's report is kept (the last 64, and
  any screen shown in the last two hours); the account finished the match (its place in the
  report is `Standing::Finished`: in the match at its end); the account used no card for it
  yet; the item is a drop card of the account with a count above 0; the roll gives an item.
  Then one roll of `drop_card_drop` at the match's level (at most 7 against bots: `pvb::drop_level`), at full condition, as before; a
  camouflage or decal the account already has is not given unless it has them all
  (`drops::COSMETIC_CATEGORIES`, the owner's of 2026-10-31; match drops too).
- **A room with a PIN's match** (the owner's of 2026-10-20: it pays nothing,
  `rooms.md`): no card for it, refused for everyone and logged as "private match".
- **A player who leaves early** (or whose game never got in) cannot use a card on that match:
  refused, and the reason (`Left`, `Absent`, `NotPlayed`) is logged. So the 1-player PIN-room
  screen left at once gives no card.
- **Refused** otherwise with the game's own `st_cannot_use_drop_card`; nothing changes (no
  card, no item, no mark). A roll that gives nothing (`drop_card_drop` always gives; only
  broken game data would not) is its own logged reason and keeps the card.
- **One per (account, match):** a card for another match the account finished is a new card.
  A card whose `98` cannot be written still counts (the card is spent and the item given).
- **Kept with the account:** the matches an account used a card on are the account's own
  (`Account::drop_cards`, table `g26_drop_cards`, schema step 10). The check and the mark are
  made under the account's lock, and the mark is written in the same transaction as the card
  taken and the item given, so a crash keeps all three or none: no second card for a match,
  ever. The results' lock is held only to read the report, never over the roll. A mark whose
  report is no longer kept goes with the account's next card (a card for a report that is gone
  is refused anyway), so the list is never longer than the reports kept.
- **`allow_drop_card`:** srp sends no such field. The client makes it itself from `97`'s
  dropped item: true when its item id or dict id is not 0 (exe 0x29D156..0x29D165,
  `results.md` §1.3 row 11). So it says "this match dropped an item", and today a screen of a
  match with no drop is already greyed (alpha 0.5) while its search icon stays clickable
  (`setData` sets `searchIcon.visible` before the grey-out). `97` goes once per match per
  player, before any card can be used; the only resend (`41 00`) needs a `#mstats` chat line
  srp never sends, and a fresh `97` would empty the slot and re-enable the search icon. So
  the server refusal is what counts, and the movie greys the slot by itself (above); no `97`
  is changed for it. A match with no drop (most) still takes a card: its slot is greyed by
  that rule but its search icon works, as before.
- **Logged** (`log::info!`, one line each, name and account id): every card used (match,
  level, item dict and id, rarity, cards left), every refusal and its reason, every sale
  (count, dict, item id, silver paid, silver after), every silver -> gold (`bought N gold for
  S silver`) and gold -> silver (`exchanged N gold for S silver`) with the balances right after,
  read under the same lock as the exchange.
- **Release:** the refusal is server only (#243), with a new schema step (`g26_drop_cards`): an older srp refuses the
  database once this one has run. The greying is a lobby movie change (a resources.db patch).
