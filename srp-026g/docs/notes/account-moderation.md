# Account moderation by the admin

The rules the code and the tests follow. Code: `session::v026g::moderation`
(the plan and the change), `GameAccounts::write_admin` and
`GameAccounts::admin_actions` (the write and the audit trail),
`servers/web-server/src/moderation.rs` (the page). Server-only: no exe or
content change, `REVISION` unchanged, no `simulation_changes` row. Needs a
server release before use.

- **Why (the owner, 2026-10-20):** "we need the ability to take away items
  in the admin panel"; for farmed money, "we need to calculate how much
  money pain and Fasojib farmed and take this silver out", and "the same
  for Mandalorienc, but he has gold instead". How much was farmed is worked
  out apart (from the logs and the results); these tabs take it out.
- **Where:** the 0.26g admin page (`/<SRP_ADMIN_PATH>`, `SRP_ADMIN_IPS`, the
  token in the form), tabs **Money** and **Take items**. The 0.21d page's
  Money tab is unchanged (at once, for testing).
- **The account:** the user name it signs in with (any case), the character
  name if only one account has it, or `#<account id>` (bare digits too, when
  no name matches). The page shows user name, character name, account id
  (decimal and hex, as the logs write it) and whether its game is open.
- **Money:** a signed amount of silver (`generic_money`) and/or gold
  (`premium_money`). A balance never goes below 0 (nor past `u32::MAX`): a
  take of more than the account has takes what it has. The preview shows
  each balance before → after, the change really made, and the amount asked
  when that differs. A change that changes nothing (nothing left to take)
  is not offered.
- **Take items:** the account's items listed: name, dictionary id, item id,
  units of a stack or condition %, the kits wearing it (`kit 1*, slot 7`; `*`
  is the kit it plays), modifications and defects (id and name key), the
  cosmetics on it. Tick items; a stack takes the units typed, or all of it
  when empty (units past what it has are what it has). At most 32 items at
  once; all or nothing.
  - An item taken whole (a stack down to 0 too) leaves the inventory, and
    the kits follow as when it is sold (`Account::sync_kits`): it is taken
    off every kit wearing it, a stack's kit slot takes at most what is left,
    an upgrade whose cosmetic was taken is taken off the item it was on, and
    each kit changed gets a new revision. A cosmetic on a taken item stays in
    the inventory.
- **Two steps, no accidental change:** a request without `confirm=1` only
  shows the plan, with its digest (`money_plan` / `items_plan`: SHA-256 of
  the account id and the account's state the plan was made from: balances,
  or each item's dictionary id, units/condition and units taken). The Apply
  button sends the same fields, the digest and `confirm=1`; the change is
  planned again under the account's lock and made only when the digest is
  the same and a reason is given. So a reload of the confirm, a change by
  the game between preview and confirm (gold spent, ammo used, an item
  sold), or fields changed after the preview change nothing: the page says
  so and shows the new preview. The preview's button comes first in the
  form, so Enter never applies. The page is POST only, as every admin
  action: the token never goes in a URL.
- **Live account, never the database by hand:** srp holds the accounts in
  memory and saves them; an edit of `srp.db` while it runs is written over.
  The change is made on the live `GameAccounts` account (the one the lobby
  and the matches change, under its lock), then:
  1. written at once on the admin request's thread: this account and the
     audit rows waiting, nothing else (not the saver's whole pass). The
     saver holds the database for its pass, so the write waits for it 2 s
     at most (`RESET_WRITE_WAIT`); past that the saver, woken, writes it
     ("the database was busy"). If the database refuses, the page says
     "Error: the change stands in memory only" and the saver retries;
  2. then the account's open game is sent its account again
     (`Inbox::account_changed`, as after a purchase: inventory, account and
     kits, money, level and skills, reputations, quests, services), and its
     squad's view of it is compared again. An offline account sees it at
     its next sign-in. A match it plays now pays it as usual at the end; an
     item taken meanwhile is skipped by the match's wear.
  No other account is read for writing or changed.
- **Reason:** required (trimmed, at most 200 characters, control characters
  made spaces). Admin-only: the log, the audit trail and the admin page. It
  is never sent to the player.
- **Audit trail:** `g26_admin_actions` in `srp.db` (0.26g schema step 9):
  time, admin's address, account id, character name, `money` / `items`, the
  change, the reason, before and after. Rows are only added. The page shows
  the account's latest 8 with every preview. A store kept in memory
  (`SRP_DB=off`) keeps the log only.
- **Log:** `Admin from <address>: money of <name> ("<login>", account 0x…):
  silver -480000 (asked -500000), gold +0; before …, after …; reason "…"`
  and `… items of <name> (…) taken: …`, at info; a write left to the saver
  is a warning, a refused one an error.
