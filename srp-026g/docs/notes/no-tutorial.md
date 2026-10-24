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
  - 60 000 silver (10 000, +50 000 since 2026-10-24) and 500 gold;
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
    - weapon slot 1: SOK-94 (2215) with 60 rounds of 7.62x39 (81);
    - weapon slot 2: kit 1 OTs-02 Kiparis (88, not the premium 332) with 100 rounds of 9x18
      (70); kit 2 TOZ-122 (12; the game has no TOZ-11) with 20 rounds of 7.62x51 (51); kit 3
      MP-153 Baikal (110) with 30 rounds of 12x70 smallshot (22);
    - rounds: each weapon's category's `optimal_count` in whole clips;
    - pockets: 2 medkits (67) and 2 antitoxins (82), from stacks of 50 each owned (a player's
      one pocket, +1 pocket from the jacket's 1030, +1 unit each from the pants' 1170);
    - put on by the lobby's rule (`rules::fill_kit`);
  - the TT-33, the Mosin and the PPSh-41 stay owned, in the inventory;
  - modifications of their own (the reroll's storage): the respirator 1183 (shown
    "Isolation: +20"), the gloves 1133 (shown "Collection time: -20%"; beside their own
    default 1130, -5%);
  - autobuy on (`premium.md` §3).
- **Each kit's level is 6** (the 7.62x39's `match_making_level`; the SOK-94 is 4, the
  armour 2 and 3, the second weapons 2 to 4): over the bots' max gear 5, so playing the bots
  takes a kit changed by the player.
- **srp refuses to start** on game data without a starter item or modification
  (`known::check_code`). The test data lacks the knit cap and the SOK-94; the whole kit on
  released data: `SRP_TEST_GAME_DATA=<server-game-data.db> cargo test -p session --test
  starter_kit -- --ignored`.
- **New player** (the first match's drop table): no more experience than an account starts
  with.
- **Accounts made before:** keep what they have and get no tutorial; steps they had not done
  are not paid.
- **No switch:** `SRP_TUTORIAL` is gone.
- **Release:** server only; no client patch.
