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
  - premium items one by one (vouchers, premium camouflages and decals, the AK-74N): their
    `base_cost` times 100 (`scripts/content-edit/items.tsv`, 199 rows); they stay premium items
    (`is_premium`: sold one by one by seller 0, never by a faction, never worn by bots);
  - premium time and the seven bundles (`shop_offers`): their gold prices times 100, offered in
    silver (money type 0); `SRP_PREMIUM_PRICES` is silver;
  - a nickname change: the rename voucher's price, 25,000 (`SRP_RENAME_COST` is silver);
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
    `fix_time` 0, `fix_gold` 0. Wear is repaired as before, by the item's `repair_cost_ratio`.
- **Income:** nothing gives gold; what gave gold gives that gold times 100 in silver: a level that
  paid gold (100 gold at the wiki's level 1 and every fifth, 250 at the last: 10,000 and 25,000), a
  gold drop (`drops::dropped`: the daily bonus's 50 gold is 5,000), a new account's grant
  (`new-account.tsv`: 30,000 silver, gold 0; a table's `gold` is given in silver).
- **Balances:** each account's gold became silver at 100 a gold, once, when the server first opens
  the database (`persist` migration 14), as did a daily-bonus reward waiting in gold. The gold
  column stays, always 0.
- **The client:** the lobby has no gold balance, no exchange window, no "Get Gold", no gold option
  in any window (`client-patches/README.md`, "Silver only"). `8B` carries gold 0; 0x8D's gold reset
  price and exchange rate are 0; 0x9B's gold chances and prices are 0. A stock window's "gold" byte
  (`46 07`, `46 09`, `47 03` kind 1, `48 01` payment 1) pays silver; `47 04` (the exchange) is no
  request.
- **The admin:** `release/moderate money` and the page's Money tab change silver only.
- **Not a simulation change:** prices and pay are the lobby's; no replay plays out otherwise.
