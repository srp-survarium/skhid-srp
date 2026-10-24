# No tutorial

The owner's of 2026-10-19. Code: `session::v026g::tutorial`; the client's side: `premium.md` §5.

- **Why:** the tutorial took a new player through Play into a match. With rooms nobody is
  sent into a match they did not choose.
- **What the client is told:** every `9D` says every step is done (`a` = 0x3F, `b` = 1): no
  lobby tutorial, no tutorial in the first match, no notes' rewards. `show` on at sign-in,
  off for the ranking's recounts.
- **Steps sent anyway** (`4B 01`, `4B 02`, `46 0A`): not answered, nothing paid.
- **A new account starts with** (its first 0.26g sign-in, `GameAccounts::sign_up`, once,
  before its client hears of it: no window, no notice; `tutorial::signup_grant`):
  - 15 000 silver and 150 gold (the owner's of 2026-10-24; before, 10 000 and 500);
  - a month of premium: 30 days (the shop's month, `PREMIUM_OFFERS` id 3) from the account's
    making at its first sign-in, added as all premium is (`premium::add_premium`); the lobby
    is told the hours left (720);
  - the steps' 250 experience (the game's `unique_awards.options`);
  - exactly 1000 rounds of every ammo: each stack of the dictionary under
    `gameplay/items/weapons/ammo/`, the ported weapons' too (34 on the released data);
  - the Scavengers at reputation level 5, their highest (3000 points), as earned: all they
    sell is buyable; they are its quest faction (never neutral);
  - its three kits (the owner's of 2026-10-24) wearing, each the plain item of its English
    name, new, full condition, one item worn by all three (the kits share the inventory):
    - Novice's knit cap (1821), Amber respirator (43), Survivor's jacket (325, not the
      premium 345), Oxygen tank (9, back slot), Chemical protection pants (46), gloves (42)
      and boots (38);
    - one weapon, in weapon slot 1 (weapon slot 2 empty; no SOK-94): kit 1 OTs-02 Kiparis
      (88, not the premium 332) with 150 rounds of 9x18 (70); kit 2 TOZ-122 (12; the game has
      no TOZ-11) with 30 rounds of 7.62x51 (51); kit 3 MP-153 Baikal (110) with 45 rounds of
      12x70 smallshot (22);
    - rounds: the ammo slot full, as the ammo window fills it: the player's 3 ammo bags (no
      bag modification on the gear) × one clip a bag: 150, 30 and 45;
    - pockets: 2 medkits (67) and 2 antitoxins (82), from stacks of 50 each owned (a player's
      one pocket, +1 pocket from the jacket's 1030, +1 unit each from the pants' 1170);
    - put on by the lobby's rule (`rules::fill_kit`);
  - the TT-33, the Mosin and the PPSh-41 stay owned, in the inventory;
  - modifications of their own (the reroll's storage): the respirator 1183 (shown
    "Isolation: +20"), the gloves 1133 (shown "Collection time: -20%"; beside their own
    default 1130, -5%);
  - autobuy on (`premium.md` §3).
- **The kits' levels are 3, 3 and 4** (`match_making_level`: the armour, the oxygen tank and
  the drugs 2 and 3, the Kiparis 2, the TOZ-122 3, the MP-153 4): each can play the bots
  (max gear 5).
- **srp refuses to start** on game data without a starter item or modification
  (`known::check_code`). The test data lacks the knit cap; the whole kit on
  released data: `SRP_TEST_GAME_DATA=<server-game-data.db> cargo test -p session --test
  starter_kit -- --ignored`.
- **New player** (the first match's drop table): no more experience than an account starts
  with.
- **Accounts made before:** keep what they have and get no tutorial; steps they had not done
  are not paid.
- **No switch:** `SRP_TUTORIAL` is gone.
- **Release:** server only; no client patch.
