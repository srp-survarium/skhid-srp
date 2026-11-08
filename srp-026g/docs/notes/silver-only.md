# Silver only: no gold

The owner's rules (2026-11-08): "Completely remove gold from the game, only silver should be left";
"all prices where gold or silver was proposed should be changed to just silver, and gold (converted
to silver) price be used"; "modification chances/etc taken from gold values"; "that means everything
in silver will also get cheaper".

- **The rate:** 100 silver a gold, the game's own (`prices::GOLD_IN_SILVER`; 0.21d's web page sold
  gold at it, 0.26g's lobby was told it as the exchange rate, and the exchange window traded at it
  both ways).
- **Prices:** whatever had a gold price costs that gold times 100 in silver, whether it also had a
  silver price or not. What only ever had a silver price keeps it.
  - premium items one by one (the drop card, premium camouflages and decals, the AK-74N): their
    `base_cost` times 100 (`scripts/content-edit/items.tsv`, 199 rows); they stay premium items
    (`is_premium`: sold one by one by seller 0, never by a faction, never worn by bots);
  - premium time and the seven bundles (`shop_offers`): their gold prices times 100, offered in
    silver (money type 0); `SRP_PREMIUM_PRICES` is silver;
  - a nickname change: 25,000, what the rename voucher cost (`SRP_RENAME_COST` is silver,
    `premium::DEFAULT_RENAME_COST`), paid in silver only;
  - a skills reset: free and without a question (the owner, 2026-11-08: "let's make skill reset
    free", "user presses reset and it resets by itself immediately", "there should be no
    confirmation at all"; it was 100 silver or 1 gold, after a dialog): the skills window's Reset
    button sends `48 01` (the open tab, payment 0) at once, `CharWindow.rerollChanges`; 0x8D's
    two reset prices are 0 and nothing reads them; the stock dialog's code is gone
    (`SrvMessageRerollSkills` is a bare symbol class);
  - a modification change (reroll): `reroll_price_premium` times 100, half the old silver price;
  - an upgrade (craft): `craft_price_premium` times 100, half the old silver price, at the gold
    chances table (`craft_probabilities_premium`; silver's was 50 / 25 / 12.5 %), which srp sets to
    100 / 95 / 90 % for a difference of 0 / 1 / 2 (`craft-menu.md`, "Success chances");
  - a repair of defects: the gold repair ("quick repair"), at once, 5,000 a defect (its 50 gold;
    the silver repair was 100 and six minutes a defect): `defects.tsv` `fix_cost` 5,000,
    `fix_time` 0, `fix_gold` 0 (the exe reads it). Wear is repaired as before, by the item's
    `repair_cost_ratio`. Silver is the only payment: no repair kits.
- **No vouchers but the drop card** (the owner, 2026-11-08: "for renames remove rename ticket too",
  "and remove it from shop too", "premium ticket and repoair kit should be removed too", "remove
  them completely and for players just give back them in silver as a release script"): the repair
  kit (151), the rename voucher (152) and the one-day premium voucher (213) are sold by no shop
  (`shop.tsv` `not_sold`) and dropped by no table (`vouchers-removed-1.json`: their `drop_weight` 0,
  `generic_drop` rescaled so every other rate, the drop card's too, stays). Nothing uses them: a
  rename pays silver (`49 04`'s voucher id is not kept, `83 49 04` says 0), a repair kit (`47 03`
  kind 2) is refused, `46 04` (using an item) is no request. Their dictionary rows stay, for the
  client.
- **Income:** nothing gives gold.
  - The gold rewards are gone, not converted (the owner, 2026-11-08: "I know there are also gold
    rewards after 7 consequtive days, I would just remove that part", "there is also gold given for
    levels, I would remove that too"): the daily bonus's seventh day gives nothing (its 50 gold was
    all `daily_bonus_drop` gave; the run of days and the doubled victories stay; `99` carries silver
    x 0 as its reward item); a level that paid gold (100 at the wiki's level 1 and every fifth, 250
    at the last) pays no money, only its skill point (`83` important message 14 says 0).
  - A gold drop of the match tables is silver, 100 a gold (`drops::dropped`).
  - A new account: 30,000 silver (`new-account.tsv`; its `gold` key is gone).
- **The pay-back, once, at the release's first start** (the owner, 2026-11-08: "we should have
  release script which will give players silver back for gold they had"):
  `persist::SqliteGameRepository::pay_back_once`, run by `GameAccounts::from_env` after srp's own
  copy of srp.db (a rollback puts that back) and before the accounts load, in one transaction with
  its `g26_meta` mark `silver_only_paid_back`: a gold balance is silver at 100 a gold; each repair
  kit, rename voucher and premium-day voucher held is its shop price (5,000, 25,000, 5,000) and is
  taken out of the inventory, the kits and the presets. srp's log has a line per account and the
  totals. The dry run: `python3 scripts/silver-only/preview.py <a copy of srp.db>` prints the same
  per account and the totals. (Migration 14, #1523's conversion, which never shipped, does nothing
  now; its number stays.)
- **The gold column** (`g26_accounts.premium_money`) stays in the schema, empty and unused (the
  owner, 2026-11-08: "we can keep that column empty and unused"): only the pay-back reads and zeroes
  it; a new row writes 0. So do `g26_extras.bonus_reward_dict` and `bonus_reward_count`.
- **The client:** the lobby has no gold balance, no exchange window, no "Get Gold", no gold option
  in any window (`client-patches/README.md`, "Silver only"). `8B` carries gold 0; 0x8D's gold reset
  price and exchange rate are 0; 0x9B's gold chances and prices are 0. A stock window's "gold" byte
  (`46 07`, `46 09`, `47 03` kind 1, `48 01` payment 1) pays silver; `47 04` (the exchange) is no
  request.
- **The admin:** `release/moderate money` and the page's Money tab change silver only.
- **Not a simulation change:** prices and pay are the lobby's; no replay plays out otherwise.
