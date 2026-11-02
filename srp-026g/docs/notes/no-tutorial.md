# No tutorial

The owner's of 2026-10-19. Code: `session::v026g::tutorial`; the client's side: `premium.md` §5.

- **Why:** the tutorial took a new player through Play into a match. With rooms nobody is
  sent into a match they did not choose.
- **What the client is told:** every `9D` says every step is done (`a` = 0x3F, `b` = 1): no
  lobby tutorial, no tutorial in the first match, no notes' rewards. `show` on at sign-in,
  off for the ranking's recounts.
- **Steps sent anyway** (`4B 01`, `4B 02`, `46 0A`): not answered, nothing paid.
- **A new account starts with** (its first 0.26g sign-in, `GameAccounts::sign_up`, once,
  before its client hears of it: no window, no notice; `tutorial::signup_grant`) nothing but
  what `scripts/content-edit/new-account.tsv` says (`gameplay/srp/new_account.options`, read
  by srp from its game data; its header says each key), and the steps' experience:
  - 15 000 silver and 150 gold (the owner's of 2026-10-24; before, 10 000 and 500);
  - a month of premium: 30 days (the shop's month, `PREMIUM_OFFERS` id 3) from the account's
    making at its first sign-in, added as all premium is (`premium::add_premium`); the lobby
    is told the hours left (720);
  - the steps' 250 experience (the game's `unique_awards.options`);
  - exactly 1000 rounds of every ammo: each stack of the dictionary under
    `gameplay/items/weapons/ammo/`, the ported weapons' too (34 on the released data);
  - the Scavengers at reputation level 5, their highest (3000 points), as earned: all they
    sell is buyable; they are its quest faction (never neutral);
  - owned: the TT-33, the Mosin and the PPSh-41 (the game's free weapons), 50 medkits, 50
    antitoxins and 20 bandages (the owner's of 2026-11-02). The kits' pockets take
    theirs from these stacks: the inventory shows 20 bandages, each kit's pocket 3 of them;
  - its three kits (the owner's of 2026-10-24) wearing, each the item of its English name,
    new, full condition, one item worn by all three (the kits share the inventory):
    - Novice's knit cap (1821), Amber respirator (43), Survivor's jacket (325, the gold
      shop's; 345 is its premium-armour copy), the Novice's backpack (`stk2_novice_026_back`,
      the back slot; the owner's of 2026-11-02, before the Oxygen tank 9), Chemical
      protection pants (46), gloves (42) and boots (38);
    - one weapon, in weapon slot 1 (weapon slot 2 empty; no SOK-94): kit 1 OTs-02 Kiparis
      (88, not the premium 332) with 150 rounds of 9x18 (70); kit 2 TOZ-122 (12; the game has
      no TOZ-11) with 30 rounds of 7.62x51 (51); kit 3 MP-153 Baikal (110) with 45 rounds of
      12x70 smallshot (22);
    - rounds: the ammo slot full, as the ammo window fills it: the player's 3 ammo bags (no
      bag modification on the gear) × one clip a bag: 150, 30 and 45;
    - pockets: 3 medkits, 3 bandages and 3 antitoxins (the owner's of 2026-11-02; before, 2
      medkits and 2 antitoxins): a player's one pocket of one unit, +1 pocket from the
      jacket's 1030, +1 pocket and +1 unit from the backpack's, +1 unit from the pants' 1170:
      three pockets of three;
    - put on by the lobby's rule (`rules::fill_kit`);
  - modifications of their own (the reroll's storage): the respirator 1183 (shown
    "Isolation: +20"), the gloves 1133 (shown "Collection time: -20%"; it stacks with their
    own default 1130, -5%, for -25% in all, which the game's `fits` rule of one
    modification a type would not allow: the owner's choice);
  - autobuy on (`premium.md` §3).
- **The kits' levels are 4, 4 and 4** (`match_making_level`: the Novice's backpack 4, the MP-153 4;
  with the Oxygen tank they were 3, 3 and 4).
- **srp refuses to start**, and `srp-check-game-data` refuses the release's data
  (`game_data::init`, `survarium::v026g::new_account`, naming each), on data without the
  table or a key of it, with a key it does not know, a faction whose quests the lobby does
  not show (only 1 to 4), an item, modification or faction level the data lacks, an item
  that does not fit its slot, a pocket the gear does not open or that takes fewer units, a
  pocket item owned with fewer units than a pocket's, or a modification of a type its item
  does not take, on a stack or on an item no kit wears. The test data lacks the knit cap and
  the backpack; the whole kit on released data:
  `SRP_TEST_GAME_DATA=<server-game-data.db> cargo test -p session --test starter_kit --
  --ignored`.
- **New player** (the first match's drop table): no more experience than an account starts
  with.
- **Accounts made before:** keep what they have and get no tutorial; steps they had not done
  are not paid.
- **No switch:** `SRP_TUTORIAL` is gone.
- **Release:** the server, and the content patch that carries `new_account.options` (srp
  refuses data without it); no exe change.
