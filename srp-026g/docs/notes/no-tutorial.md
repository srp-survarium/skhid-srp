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
    AK-74N (the owner's of 2026-11-04), the Glock 17 (2283; the owner's of 2026-11-08, "it can
    be kept in inventory"), 50 stash maps (the owner's of 2026-11-08; 10 from 2026-11-04), 50
    medkits, 50 antitoxins and 20 bandages (the owner's of 2026-11-02), 10 Tornado energy
    drinks (`speed_booster` 340; the owner's of 2026-11-08). The kits' pockets take theirs
    from these stacks: the inventory shows 20 bandages, each kit's pocket some of them;
  - two of its five kit presets (`presets.md`), each the item of its English name, new, full
    condition, one item worn by every kit and preset that wears it (they share the
    inventory), put on by the lobby's rule (`rules::fill_kit`):
    - **preset 1**, the kits, active (the owner's of 2026-10-24; "we keep as is", 2026-11-08):
      the Novice's knit cap (1821), Amber respirator (43), Survivor's jacket (325), Novice's
      backpack (`stk2_novice_026_back`, the owner's of 2026-11-02, before the Oxygen tank 9),
      Chemical protection pants (46), gloves (42) and boots (38); one weapon, in weapon slot
      1 (no SOK-94): kit 1 AK-74N (306) with 90 rounds of 5.45x39 7N6M (7), kit 2 Remington
      700 (14; before 2026-11-04 the Kiparis and the TOZ-122) with 30 of 7.62x51 (51), kit 3
      MP-153 Baikal (110) with 45 of 12x70 smallshot (22): the player's 3 bags, no bag
      modification; three pockets of three (a medkit's, bandages', antitoxins');
    - **preset 2**, kept with the account, put on when the player chooses it (the owner's of
      2026-11-08: "keeper of the sun + keeper set to all starting players + AK-12 in their
      second loadout", "e2k1 is ak-12, e2k2 is saiga", "no need for e2k3", a backpack with
      an artefact slot, light): kits 1 and 2 the "Keeper" helmet (202), "Keeper" mask (200),
      the Keeper Of The Sun Vest (977) and Pants (978), Keeper's gloves (201) and boots (198),
      all level 14, the "Wanderer" container (199, level 14, 4 kg: an artefact slot, a pocket
      and a pocket's unit), and three pockets of two: 2 medkits, a Tornado energy drink (a
      booster fills a pocket alone; "instead of bandages") and 2 antitoxins; kit 1 the AK-12
      (363, not the premium copy) with 150 of 6.5x39 (369), kit 2 the Saiga-12 (2227) with
      100 of 12x70 buckshot (2205); kit 3 nothing; no plain Keeper vest and pants ("can be
      removed from inventory");
    - rounds: an ammo slot full, as the ammo window fills it, from the kit's bags (the
      player's 3, +1 from each Keeper of the Sun piece's default 1050), a clip a bag; a kit of
      two weapons would share them, one bag for weapon 2's ammo, the rest for weapon 1's;
    - pockets: a player's one pocket of one unit, +1 pocket from the vest's 1030, +1 from the
      container's built-in 1030 (armor.tsv, every player's container) and +1 unit from its
      1170: three of two; `pocket_units` is a pocket's size, an item gets what such a pocket
      holds of it;
    - carried (the -15 % counted): preset 2's kits 27.5 and 30.7 kg, within the player's
      30 kg `max_carried_weight` and the container's +5 kg;
  - modifications of their own (the reroll's storage; the owner's of 2026-11-08, "-15%
    weight and +8 armor"), after those an item comes with (items.tsv `start`), at most 3 an
    item (`MAX_RANK`): the Keeper helmet, gloves and boots armour +8 (1004, the strongest
    armour row) and weight -15 % (1064, the strongest weight row), the gloves aim time -10 %
    too (1084; the owner's, "-10% aiming speed"); the mask, which rolls no
    armour (weight, isolation and reliability only): weight -15 % alone; the container
    weight -15 % and carried weight +5 kg (1124, the strongest; "weight -15% and +5kg");
    the AK-12 and the Saiga-12 weight -15 % (2064, the weapons' strongest weight row),
    recoil -10 % (2084) and aim time -10 % (2074; "+10% aiming speed"); the
    Keeper of the Sun pieces come with experience +20 % (1240) and reputation +50 % (1280),
    so only armour +8 is added. Preset 1's respirator 1183 and gloves 1133;
  - autobuy on (`premium.md` §3).
- **The kits' levels** (`match_making_level`, the highest item's): preset 1's 7, 5 and 4;
  preset 2's 15 (the AK-12) and 14 (the Keeper set and the container).
- **srp refuses to start**, and `srp-check-game-data` refuses the release's data
  (`game_data::init`, `survarium::v026g::new_account`, naming each), on data without the
  table or a key of it (preset 1 and every preset's `pocket_units`), with a
  key it does not know, a preset after one missing, a faction whose quests the lobby does
  not show (only 1 to 4), an item, modification or faction level the data lacks, an item
  that does not fit its slot, a pocket the gear does not open (its items' modifications,
  the grant's too) or that takes fewer units, a pocket item owned with fewer units than a
  pocket's, a modification of a type its item does not take, on a stack or on an item it
  is not given (worn or owned), or more than 3 on an item with those it comes with. The
  test data lacks the knit cap, the backpack, the Keeper of the Sun, the Saiga-12, its
  buckshot and the Glock 17, and predates the container's pocket; the whole grant on released data, preset 2 chosen as a player
  chooses it: `SRP_TEST_GAME_DATA=<server-game-data.db> cargo test -p session --test
  starter_kit -- --ignored`.
- **Accounts made before:** raised once to where a new one starts (37 999 experience, level
  15; the owner's of 2026-11-06) by the release that ships it, in the database with srp
  stopped (`UPDATE g26_accounts SET exp_total = 37999 WHERE exp_total < 37999`): the levels
  passed pay nothing, their points are theirs to spend. They get no tutorial; steps they had
  not done are not paid. Preset 2's items and the Glock 17 (the owner's of 2026-11-08,
  "everyone"): given once by the release that ships them, the same way, into storage with
  the same modifications of their own, 10 Tornado energy drinks to an account without one
  and stash maps up to 50 (`private/release/keeper-ak12-once.sql`, its SQL in the PR); an
  account's preset 2, when empty and not active, becomes the new account's, its stacks the
  account's own (a stack it lacks leaves its slot out); its kits and other presets stay as
  the player made them.
- **No switch:** `SRP_TUTORIAL` is gone.
- **Release:** the server, and the content patch that carries `new_account.options` (srp
  refuses data without it), the container's pocket and the premium bonuses (+20 % experience,
  +50 % reputation, `premium_armour.md`; both in `simulation_changes.tsv`'s `next` row); no
  exe change.
