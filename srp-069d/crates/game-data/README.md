# game-data (0.69d)

The client's game tables and the lobby rules that read them, as Rust (money, shop, craft, modules, pockets, levels,
skills, maps and AI players). The rules and how sure each one is are in `docs/notes/` (store, workshop, items,
progression).

```rust
let gd = game_data::GameData::from_dir(dir)?;       // the tables `srp setup` built from the game install
let ak = gd.shop.price(3).unwrap();                  // 8A row of AK-74M
let cost = gd.shop.buy_total(&gd.items, 7, 90);      // ammo: whole clip_size units
let rank = gd.craft.rank(107, 1);                    // client rank of a Mosin with one modifier
```

| Module | Content |
|---|---|
| `items` | dictionary row per dict id (category, stackable, base id, level, faction, cost), category sets, `amount_field` (record +0xC), `is_module` |
| `shop` | 8A table `Price{a,b,c,kind,for_sale}`: dictionary default, wiki prices (banker's rounding as Python), gold items, hidden unsold, consumables, module prices; `ammo_cost`, `buy_total`, `sell_gain`, `ammo_top_up`, `pocket_cost`, `clip` |
| `craft` | per item mask / costs / base rank, modifier groups; `rank`, `craft_price`, `reroll_price`, `candidates`, `choices` (random through the `Roll` trait), exchange (46 03, 250 silver per gold) |
| `modules` | weapon slot ids (B3), compatible ammo, module links, seed (first 9 links), possible owners, slot check |
| `pockets` | groups and cell limits, `pocket_group` of an item, capacity from armour defaults (groups 173/174), `pocket_fits` |
| `levels` | XP table, `level_of`, `level_fields` (14 bytes of 87/8C), faction level from points, level-up rewards |
| `skills` | the skill tree (69 skills: levels, branch points), `apply` (learn rules), reset points and price, modifiers via `matches.skill_modifiers` |
| `matches` | maps/modes rows and `resolve_map`, 36 AI players, `pick_bots`, `kit`, `skill_modifiers`, bot names/teams |

## Data (the data folder)

The tables are not in the repository. `srp setup` (or `srp extract --out <dir>`) builds them from the operator's game
install with the `data-extract` crate: 32 files (`DATA_FILES`), about 2 MB. `shop_069d.tsv` joins our own prices (fan
wiki price tables of May 2022 and CHOICE prices: `crates/data-extract/ours/shop_prices_069d.tsv`) with the dictionary
columns. The maps/modes rows keep only those whose level project ships in the client (`projects/<level>/client_project`
in the volumes) plus the four Search and Destroy layouts the client's custom-match list offers: a missing project makes
the exe assert "failed to load project" in the client and in the world host.

## Tests

`cargo test -p game-data -- --test-threads=2`. Tests that need the tables start with `require_tables!()` and read
them from `SRP_TEST_DATA` or the workspace's `data/` (build it once: `srp extract --game <game folder> --out data`);
without them they pass at once with a note (`SRP_REQUIRE_TEST_DATA=1` makes them fail instead). `tests/stub_parity.rs`
and `tests/skills.rs` also compare with reference values of an earlier implementation (`expected_069d.json`,
`skills_expected_069d.json` in the `SRP_TEST_DATA` folder); they hold game data, so they are not in the repository, and
those tests skip without them.

## Status

- Money rules, 8A table, ammo, sell, workshop slots, module seed, level fields, maps and bot kits equal the reference
  values (tested for every dict id).
- Craft: item-aware pool and rank costs (docs/notes/workshop.md). Three offered rows: server rule unknown (LIKELY).
- Pocket capacity (groups 173/174) and `pocket_fits` are LIKELY (client popup rules), limits per group PROVEN.
- Match rewards follow the data's formula (docs/notes/progression-audit.md section 4); per-player match stats are not here.
