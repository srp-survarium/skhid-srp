# No tutorial

The owner's of 2026-10-19. Code: `session::v026g::tutorial`; the client's side: `premium.md` §5.

- **Why:** the tutorial took a new player through Play into a match. With rooms nobody is
  sent into a match they did not choose.
- **What the client is told:** every `9D` says every step is done (`a` = 0x3F, `b` = 1): no
  lobby tutorial, no tutorial in the first match, no notes' rewards. `show` on at sign-in,
  off for the ranking's recounts.
- **Steps sent anyway** (`4B 01`, `4B 02`, `46 0A`): not answered, nothing paid.
- **A new account starts with** (its first 0.26g sign-in, `GameAccounts::sign_up`, once,
  before its client hears of it: no window, no notice): 10 000 silver and 500 gold, the
  steps' 250 experience (the game's `unique_awards.options`), and 1000 rounds each of
  7.62x25 (TT-33, PPSh-41) and 7.62x54R (Mosin) on its starting stacks, and the Scavengers as
  its quest faction (the tutorial ended in the faction's choice; never neutral). Autobuy is on
  (`premium.md` §3).
- **New player** (the first match's drop table): no more experience than an account starts
  with.
- **Accounts made before:** keep what they have and get no tutorial; steps they had not done
  are not paid.
- **No switch:** `SRP_TUTORIAL` is gone.
- **Release:** server only; no client patch.
