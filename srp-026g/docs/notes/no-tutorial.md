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
  by srp from its game data; its header says each key):
  - level 15 (the owner's of 2026-11-06; before, the steps' 250 experience): one point
    short of level 16 (37 999), so its first match brings it to 16; the levels below it pay
    nothing; its 15 points spent:
    Quick Response 5 (101), Battle Stance 2 (102), Potent Painkiller 1 (301), Fast
    Bandaging 2 (302), Metabolism 5 (311);
  - 15 000 silver and 150 gold (the owner's of 2026-10-24; before, 10 000 and 500);
  - a month of premium: 30 days (the shop's month, `PREMIUM_OFFERS` id 3) from the account's
    making at its first sign-in, added as all premium is (`premium::add_premium`); the lobby
    is told the hours left (720);
  - exactly 1000 rounds of every ammo: each stack of the dictionary under
    `gameplay/items/weapons/ammo/`, the ported weapons' too (34 on the released data);
  - the Scavengers at reputation level 5, their highest (3000 points), as earned: all they
    sell is buyable; they are its quest faction (never neutral);
  - owned: the TT-33, the Mosin and the PPSh-41 (the game's free weapons), the AKM, AKMSU and
    AK-74N and 10 stash maps (the owner's of 2026-11-04), the Remington 700 (14; kit 2's
    weapon from 2026-11-04 to 2026-11-08), the Keeper Of The Sun Vest (977) and Pants (978)
    (the owner's of 2026-11-08), 50 medkits, 50 antitoxins and 20 bandages (the owner's of
    2026-11-02). The kits' pockets take theirs from these stacks: the inventory shows 20
    bandages, each kit's pocket 2 of them;
  - its three kits (the owner's of 2026-10-24) wearing, each the item of its English name,
    new, full condition, one item worn by all three (the kits share the inventory):
    - the Keeper set (the owner's of 2026-11-08; before, the Novice's knit cap 1821, Amber
      respirator 43, Survivor's jacket 325, Chemical protection pants 46, gloves 42 and boots
      38): "Keeper" helmet (202), "Keeper" mask (200), Bulletproof vest "Keeper" (196),
      "Keeper" pants (197), Keeper's gloves (201) and boots (198), all level 14; the Novice's
      backpack (`stk2_novice_026_back`, the back slot; the owner's of 2026-11-02, before the
      Oxygen tank 9; the set has no backpack);
    - one weapon, in weapon slot 1 (weapon slot 2 empty; no SOK-94): kit 1 AK-74N (306)
      with 150 rounds of 5.45x39 7N6M (7); kit 2 AK-12 (363, not the premium copy) with 150
      rounds of 6.5x39 (369) (the owner's of 2026-11-08; before, the Remington 700, and
      before 2026-11-04 the Kiparis and the TOZ-122); kit 3 MP-153 Baikal (110) with 75
      rounds of 12x70 smallshot (22);
    - rounds: the ammo slot full, as the ammo window fills it: the player's 3 ammo bags +1
      from the vest's and +1 from the pants' default 1050 × one clip a bag: 150, 150 and 75;
    - pockets: 2 medkits, 2 bandages and 2 antitoxins (3 of each from 2026-11-02 to
      2026-11-08, with the Chemical protection pants' +1 unit, 1170, which the Keeper pants
      lack): a player's one pocket of one unit, +1 pocket from the vest's 1030, +1 pocket and
      +1 unit from the backpack's: three pockets of two;
    - put on by the lobby's rule (`rules::fill_kit`);
  - modifications of their own (the reroll's storage; the owner's of 2026-11-08, "-15%
    weight and +8 armor"), after those an item comes with (items.tsv `start`), at most 3 an
    item (`MAX_RANK`): every Keeper piece armour +8 (1004, the strongest armour row) and
    weight -15 % (1064, the strongest weight row), but the mask, which rolls no armour
    (weight, isolation and reliability only): weight -15 % alone; the Keeper of the Sun
    pieces come with experience +5 % (1240) and reputation +5 % (1280), so only armour +8
    is added. Before 2026-11-08: the respirator 1183 and the gloves 1133;
  - autobuy on (`premium.md` §3).
- **The kits' levels are 14, 15 and 14** (`match_making_level`, the highest item's: the
  Keeper set 14, the AK-12 15; before 2026-11-08: 7, 5 and 4).
- **srp refuses to start**, and `srp-check-game-data` refuses the release's data
  (`game_data::init`, `survarium::v026g::new_account`, naming each), on data without the
  table or a key of it, with a key it does not know, a faction whose quests the lobby does
  not show (only 1 to 4), an item, modification or faction level the data lacks, an item
  that does not fit its slot, a pocket the gear does not open or that takes fewer units, a
  pocket item owned with fewer units than a pocket's, a modification of a type its item
  does not take, on a stack or on an item it is not given (worn or owned), or more than 3 on
  an item with those it comes with. The test data lacks the Keeper of the Sun and the
  backpack; the whole kit on released data:
  `SRP_TEST_GAME_DATA=<server-game-data.db> cargo test -p session --test starter_kit --
  --ignored`.
- **Accounts made before:** raised once to where a new one starts (37 999 experience, level
  15; the owner's of 2026-11-06) by the release that ships it, in the database with srp
  stopped (`UPDATE g26_accounts SET exp_total = 37999 WHERE exp_total < 37999`): the levels
  passed pay nothing, their points are theirs to spend. They get no tutorial; steps they had
  not done are not paid. The Keeper set, the Keeper of the Sun and the AK-12 (the owner's of
  2026-11-08, "everyone"): given once by the release that ships them, the same way, into
  storage with the same modifications of their own (`private/release/keeper-ak12-once.sql`,
  its SQL in the PR); no kit changes, but a second kit with no weapon and no first ammo
  takes the AK-12 and up to 90 rounds of the 6.5x39 the account owns.
- **No switch:** `SRP_TUTORIAL` is gone.
- **Release:** the server, and the content patch that carries `new_account.options` (srp
  refuses data without it); no exe change.
