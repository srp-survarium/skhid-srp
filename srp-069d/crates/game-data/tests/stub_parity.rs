//! Compares the Rust rules with reference values computed by an earlier implementation from the same tables
//! (`expected_069d.json` in the test tables folder; not in the repository because it holds game data).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use game_data::craft::{self, Roll, SplitMix};
use game_data::levels;
use game_data::pockets::{self, Placed, PocketCapacity};
use game_data::{GameData, Wallet};
use serde_json::Value;

fn gd() -> &'static GameData {
    static G: OnceLock<GameData> = OnceLock::new();
    G.get_or_init(|| game_data::testing::tables())
}

/// The reference values (`expected_069d.json` in the test tables folder), if they are there.
fn reference() -> Option<&'static Value> {
    static E: OnceLock<Option<Value>> = OnceLock::new();
    E.get_or_init(|| game_data::testing::reference("expected_069d.json").map(|t| serde_json::from_str(&t).unwrap())).as_ref()
}

fn exp() -> &'static Value {
    reference().expect("expected_069d.json")
}

/// First line of a test that compares with the reference values: needs the tables and `expected_069d.json` next to
/// them, else it returns at once with a note.
macro_rules! require_reference {
    () => {
        game_data::require_tables!();
        if reference().is_none() {
            eprintln!("skipped {}: no expected_069d.json in the test tables folder", module_path!());
            return;
        }
    };
}

fn u(v: &Value) -> u64 {
    v.as_u64().unwrap_or_else(|| panic!("not a u64: {v}"))
}

fn did(k: &str) -> u16 {
    k.parse().unwrap()
}

#[test]
fn dictionary_rules() {
    require_reference!();
    let g = gd();
    let items = exp()["items"].as_object().unwrap();
    assert_eq!(g.items.len(), items.len());
    for (k, e) in items {
        let d = did(k);
        assert_eq!(g.items.category(d) as u64, u(&e["cat"]), "category {d}");
        assert_eq!(g.items.is_stackable(d), e["stack"].as_bool().unwrap(), "stackable {d}");
        assert_eq!(g.items.amount_field(d, 7) as u64, u(&e["amount7"]), "amount7 {d}");
        assert_eq!(g.items.amount_field(d, 100) as u64, u(&e["amount100"]), "amount100 {d}");
        assert_eq!(g.items.is_module(d), e["module"].as_bool().unwrap(), "module {d}");
        let it = g.items.get(d).unwrap();
        assert_eq!(it.level_min1() as u64, u(&e["level"]), "level {d}");
        assert_eq!(it.is_premium(), e["premium"].as_bool().unwrap(), "premium {d}");
    }
    // spot values from the dictionary: AK-74M (faction 3, level 2), Mosin (base id 107), RGD-5 stackable
    assert_eq!(g.items.get(3).unwrap().cfg_name, "ak_74m");
    assert_eq!(g.items.get(3).unwrap().faction, 3);
    assert_eq!(g.items.amount_field(107, 1), 107);
    assert!(g.items.is_stackable(5));
    assert_eq!(g.items.by_cfg_name("mosin_nagant"), Some(107));
    assert_eq!(g.items.amount_field(9999, 5), 9999);
}

/// consumables the Items page (fan wiki, May 2022) prices although the reference had no price (data/shop_069d.tsv)
const ITEMS_PAGE_PRICED: [u16; 6] = [5, 66, 67, 219, 734, 736];

#[test]
fn shop_table_matches_the_reference_values() {
    require_reference!();
    let g = gd();
    let prices = exp()["prices"].as_object().unwrap();
    // the reference table plus the voucher / booster rows (categories 23 and 27) the dictionary prices
    let extra = g.items.iter().filter(|i| matches!(i.category, 23 | 27 | 37 | 38) && !prices.contains_key(&i.dict_id.to_string())).count();
    // league shop items (league_069d.json) are listed even when the reference had no row for them
    let league_extra = g.league.shop.iter().filter(|s| !prices.contains_key(&s.dict_id.to_string()) && !matches!(g.items.category(s.dict_id), 23 | 27)).count();
    assert_eq!(g.shop.len(), prices.len() + extra + league_extra);
    // the Items-page prices (fan wiki, May 2022) fill 6 consumables the reference had no price for (data/shop_069d.tsv, docs/notes/store.md)
    assert_eq!(g.shop.real_prices as u64, u(&exp()["real_prices"]) + ITEMS_PAGE_PRICED.len() as u64);
    for (k, e) in prices {
        let p = g.shop.price(did(k)).unwrap_or_else(|| panic!("no price for {k}"));
        let a = e.as_array().unwrap();
        let got = [p.a as u64, p.b as u64, p.c as u64, p.kind as u64, p.for_sale as u64];
        let want: Vec<u64> = a.iter().map(u).collect();
        // deliberate changes against the reference (docs/notes/workshop.md section 2): modules (no invented gold price, cost 0
        // not sold), vouchers / boosters (dictionary cost), consumables without a real price (not sold)
        let cat = g.items.category(did(k));
        if ITEMS_PAGE_PRICED.contains(&did(k)) || g.shop.is_choice(did(k)) {
            continue;
        }
        // the league shop (league_069d.json): kind 7, priced in league tokens, a deliberate change against the reference
        if let Some(t) = g.league.shop_price(did(k)) {
            assert_eq!((p.a, p.kind, p.for_sale), (t, 7, true), "league item {k}");
            continue;
        }
        if g.items.is_module(did(k)) || matches!(cat, 23 | 27 | 37 | 38) || (g.items.is_consumable(did(k)) && !p.for_sale && want[4] == 1) {
            continue;
        }
        // ammo is free: listed, price 0, sells for 0
        if g.items.is_ammo(did(k)) {
            assert_eq!(got, [0, 0, 0, 0, 1], "ammo row of dict {k}");
            continue;
        }
        assert_eq!(got.to_vec(), want, "8A row of dict {k}");
    }
    // a few readable anchors from the wiki table
    let ak = g.shop.price(3).unwrap();
    assert_eq!((ak.a, ak.b, ak.kind, ak.for_sale), (60000, 15000, 0, true));
    let ammo = g.shop.price(7).unwrap(); // 5.45x39 7N6M: clip of 30, free (the wiki pack price is not used)
    assert_eq!(g.shop.clip(7), 30);
    assert_eq!((ammo.a, ammo.for_sale), (0, true));
}

#[test]
fn clip_and_ammo_cost() {
    require_reference!();
    let g = gd();
    for (k, v) in exp()["clip"].as_object().unwrap() {
        assert_eq!(g.shop.clip(did(k)) as u64, u(v), "clip {k}");
    }
    // ammo is free: every cost 0; the slider still adds whole clips
    for r in exp()["ammo_cost"].as_array().unwrap() {
        let (d, rounds) = (u(&r[0]) as u16, u(&r[1]) as u32);
        assert_eq!(g.shop.ammo_cost(d, rounds), 0, "ammo_cost({d}, {rounds})");
    }
    for r in exp()["ammo_top_up"].as_array().unwrap() {
        let (d, short, added) = (u(&r[0]) as u16, u(&r[1]) as u32, u(&r[3]) as u32);
        assert_eq!(g.shop.ammo_top_up(d, short), (0, added), "top up dict {d} short {short}");
    }
}

#[test]
fn buy_sell_pocket_money() {
    require_reference!();
    let g = gd();
    for r in exp()["buy_total"].as_array().unwrap() {
        let (d, c) = (u(&r[0]) as u16, u(&r[1]) as u32);
        if ITEMS_PAGE_PRICED.contains(&d) || g.shop.is_choice(d) || g.league.shop_price(d).is_some() {
            continue;
        }
        let want = if r[2].is_null() { None } else { Some(u(&r[2])) };
        if g.items.is_ammo(d) {
            assert_eq!(g.shop.buy_total(&g.items, d, c), None, "ammo is not sold: {d}");
            continue;
        }
        if g.shop.price(d).is_some_and(|p| !p.for_sale) {
            assert_eq!(g.shop.buy_total(&g.items, d, c), None, "an item without a real price is not sold: {d}");
            continue;
        }
        assert_eq!(g.shop.buy_total(&g.items, d, c), want, "buy_total({d}, {c})");
    }
    for r in &exp()["buy_unknown"].as_array().unwrap().clone() {
        assert!(r.is_null());
    }
    assert_eq!(g.shop.buy_total(&g.items, 9999, 1), None);
    for r in exp()["sell"].as_array().unwrap() {
        let (d, rq, ow) = (u(&r[0]) as u16, u(&r[1]) as u32, u(&r[2]) as u32);
        if ITEMS_PAGE_PRICED.contains(&d) || g.shop.is_choice(d) || g.league.shop_price(d).is_some() {
            continue;
        }
        let gain = if g.items.is_ammo(d) { 0 } else { u(&r[4]) };
        assert_eq!(g.shop.sell_gain(&g.items, d, rq, ow), (u(&r[3]) as u32, gain), "sell({d}, {rq}, {ow})");
    }
    for r in exp()["pocket_cost"].as_array().unwrap() {
        let (d, short) = (u(&r[0]) as u16, u(&r[1]) as u32);
        if matches!(g.items.category(d), 23 | 27) || ITEMS_PAGE_PRICED.contains(&d) || g.shop.is_choice(d) {
            continue; // Items-page prices; boosters and vouchers carry their dictionary price now
        }
        let w = if u(&r[3]) == 1 { Wallet::Gold } else { Wallet::Silver };
        // an unlisted item costs nothing (CHOICE)
        let want = if g.shop.listed(d) { u(&r[2]) } else { 0 };
        assert_eq!(g.shop.pocket_cost(d, short), (w, want), "pocket cost dict {d}");
    }
}

#[test]
fn weapons_and_modules() {
    require_reference!();
    let g = gd();
    for (k, v) in exp()["weapon_slots"].as_object().unwrap() {
        let want: Vec<&str> = v.as_array().unwrap().iter().map(|s| s.as_str().unwrap()).collect();
        let got: Vec<&str> = g.modules.weapon_slot_ids(did(k)).iter().map(|s| s.as_str()).collect();
        assert_eq!(got, want, "slots of {k}");
    }
    for (k, v) in exp()["compatible_ammo"].as_object().unwrap() {
        let want: Vec<u16> = v.as_array().unwrap().iter().map(|x| u(x) as u16).collect();
        assert_eq!(g.modules.compatible_ammo(did(k)), want.as_slice(), "ammo of {k}");
    }
    for (k, v) in exp()["seed"].as_object().unwrap() {
        let want: Vec<u16> = v.as_array().unwrap().iter().map(|x| u(x) as u16).collect();
        assert_eq!(g.modules.seed_modules(did(k)), want.as_slice(), "seed of {k}");
    }
    for (k, v) in exp()["linked"].as_object().unwrap() {
        let want: Vec<u16> = v.as_array().unwrap().iter().map(|x| u(x) as u16).collect();
        assert_eq!(g.modules.linked_modules(did(k)), want.as_slice(), "links of {k}");
        for m in &want {
            assert!(g.modules.fits(*m, did(k)));
            assert!(g.modules.possible_owners(*m).contains(&did(k)));
        }
    }
    // what the reference seeded for the default Mosin (uid 4.. = modules of dict 107)
    let seeded: Vec<u16> = exp()["stub_seed_items"].as_array().unwrap().iter().map(|t| u(&t[1]) as u16).collect();
    assert_eq!(seeded.as_slice(), g.modules.seed_modules(107));
    for t in exp()["stub_seed_items"].as_array().unwrap() {
        assert_eq!(u(&t[2]) as u32, g.items.amount_field(u(&t[1]) as u16, 1));
    }
    // module prices: spare parts a = dictionary cost, gold c = a / 20 (CHOICE, no source), sold only
    // when cost > 0
    let m = g.modules.seed_modules(107)[0];
    let p = g.shop.price(m).unwrap();
    assert_eq!(p.a, g.items.get(m).unwrap().cost_int());
    assert_eq!((p.b, p.c, p.kind, p.for_sale), (0, p.a / 20, 0, p.a > 0));
    // slot checks
    let slots = g.modules.weapon_slot_ids(3);
    assert!(!slots.is_empty());
    assert!(g.modules.slot_valid(3, &slots[0]));
    assert!(!g.modules.slot_valid(3, "no_such_slot"));
    assert!(g.modules.slot_valid(9999, "anything"));
}

#[test]
fn craft_params_ranks_costs() {
    require_reference!();
    let g = gd();
    let c = exp()["craft"].as_object().unwrap();
    assert!(c.len() > 900);
    for (k, e) in c {
        let d = did(k);
        let mask: BTreeSet<u32> = e["mask"].as_array().unwrap().iter().map(|x| u(x) as u32).collect();
        assert_eq!(g.craft.mask(d), mask, "mask {d}");
        assert_eq!(g.craft.params(d).is_some(), e["has_params"].as_bool().unwrap(), "has params {d}");
        assert_eq!(g.craft.reroll_price(d, false) as u64, u(&e["reroll"][0]), "reroll parts {d}");
        assert_eq!(g.craft.reroll_price(d, true) as u64, u(&e["reroll"][1]), "reroll gold {d}");
        for n in 0..5usize {
            assert_eq!(g.craft.rank(d, n) as u64, u(&e["ranks"][n]), "rank {d} n={n}");
            assert_eq!(g.craft.craft_price(d, n, false) as u64, u(&e["parts"][n]), "parts {d} n={n}");
            assert_eq!(g.craft.craft_price(d, n, true) as u64, u(&e["gold"][n]), "gold {d} n={n}");
            let q = g.craft.craft_quote(d, n, false);
            assert_eq!(q.allowed, q.rank < 3 && q.price > 0);
        }
        assert_eq!(g.craft.max_upgrades(d), 3 - g.craft.rank(d, 0));
    }
    // documented anchors (craft.md): Mosin parts 15/22/30 gold 10/15/20, reroll 3 parts / 5 gold; AK-74M parts 30/45/60
    assert_eq!(g.craft.craft_price(107, 0, false), 15);
    assert_eq!(g.craft.craft_price(107, 2, false), 30);
    assert_eq!(g.craft.craft_price(107, 0, true), 10);
    assert_eq!(g.craft.craft_price(107, 3, false), 0);
    assert_eq!(g.craft.reroll_price(107, false), 3);
    assert_eq!(g.craft.reroll_price(107, true), 5);
    assert_eq!(g.craft.craft_price(3, 1, false), 45);
    // rank-3 items are never craftable
    let never: Vec<u16> = c.iter().filter(|(_, e)| u(&e["base"]) >= 3).map(|(k, _)| did(k)).collect();
    assert!(!never.is_empty());
    for d in never {
        assert!(!g.craft.craft_quote(d, 0, false).allowed);
        assert_eq!(g.craft.max_upgrades(d), 0);
    }
}

#[test]
fn craft_groups_and_candidates() {
    require_reference!();
    let g = gd();
    let groups = exp()["mod_groups"].as_object().unwrap();
    assert_eq!(g.craft.groups().count(), groups.len());
    for (k, e) in groups {
        let m = g.craft.group(k.parse().unwrap()).unwrap();
        assert_eq!(m.kind as u64, u(&e["type"]));
        let want: Vec<(u32, f64)> = e["grades"].as_array().unwrap().iter().map(|p| (u(&p[0]) as u32, p[1].as_f64().unwrap())).collect();
        assert_eq!(m.grades, want, "grades of group {k}");
    }
    for case in exp()["candidates"].as_array().unwrap() {
        let d = u(&case["did"]) as u16;
        let mods: Vec<(u16, u16)> = case["mods"].as_array().unwrap().iter().map(|p| (u(&p[0]) as u16, u(&p[1]) as u16)).collect();
        let keep = case["keep"].as_array().map(|p| (u(&p[0]) as u16, u(&p[1]) as u16));
        let want: BTreeMap<u32, Vec<u32>> = case["by_type"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(t, v)| (t.parse().unwrap(), v.as_array().unwrap().iter().map(|x| u(x) as u32).collect()))
            .collect();
        assert_eq!(g.craft.candidates(d, &mods, keep), want, "candidates dict {d} mods {mods:?} keep {keep:?}");
    }
}

#[test]
fn craft_choices_follow_the_pool() {
    game_data::require_tables!();
    let g = gd();
    let mut rng = SplitMix(7);
    // Mosin (107): mask 0,2,3,4,18 or similar: offers one group per type, all inside the candidates
    for d in [107u16, 3, 31, 17] {
        let by_type = g.craft.candidates(d, &[], None);
        for _ in 0..50 {
            let offer = g.craft.choices(d, &[], None, &mut rng);
            assert_eq!(offer.len(), by_type.len().min(3));
            let mut types = BTreeSet::new();
            for (grp, grade) in &offer {
                let m = g.craft.group(*grp as u32).expect("known group");
                assert!(by_type[&m.kind].contains(&(*grp as u32)), "group {grp} not allowed on {d}");
                assert!(m.grades.iter().any(|(gr, _)| *gr == *grade as u32));
                assert!(types.insert(m.kind), "one group per type");
            }
            assert!(offer.windows(2).all(|w| w[0] <= w[1]), "sorted");
        }
    }
    // an item holding a modifier never gets the same type again; reroll excludes only the other pairs
    let by_type = g.craft.candidates(107, &[], None);
    let (t0, grps) = by_type.iter().next().unwrap();
    let held = (grps[0] as u16, 0u16);
    let c2 = g.craft.candidates(107, &[held], None);
    assert!(!c2.contains_key(t0));
    assert!(g.craft.candidates(107, &[held], Some(held)).contains_key(t0));
    // reroll offers are padded to three rows
    let r = g.craft.reroll_choices(107, &[held], held, &mut rng);
    assert_eq!(r.len(), 3);
    // an item with no mask offers nothing: reroll falls back to the old pair
    assert!(g.craft.choices(9999, &[], None, &mut rng).is_empty());
    assert_eq!(g.craft.reroll_choices(9999, &[], (201, 0), &mut rng), vec![(201, 0); 3]);
}

#[test]
fn craft_rolls_and_exchange() {
    game_data::require_tables!();
    let mut rng = SplitMix(1);
    assert!((0..100).all(|_| game_data::CraftData::roll_success(100.0, &mut rng)));
    assert!((0..100).all(|_| !game_data::CraftData::roll_success(0.0, &mut rng)));
    assert_eq!(rng.below(1), 0);
    assert_eq!(craft::CRAFT_CHANCE, (50.0, 100.0)); // fan wiki Workshop, same rarity (silver 50 %, gold 100 %)
    assert_eq!(craft::exchange(10, 25), (10, 2500));
    assert_eq!(craft::exchange(100, 25), (25, 6250));
    let g = gd();
    assert_eq!(g.exchange_rates.len(), 15);
    assert!(g.exchange_rates.iter().all(|&r| r == craft::EXCHANGE_RATE));
}

#[test]
fn pockets() {
    require_reference!();
    let g = gd();
    // pocket_contains of every item that has one
    let pc = exp()["pocket_contains"].as_object().unwrap();
    assert_eq!(pc.len(), 22);
    for (k, v) in pc {
        let (grp, cells) = g.craft.pocket_group(did(k)).unwrap_or_else(|| panic!("no group for {k}"));
        let o = v.as_object().unwrap();
        assert_eq!(o.len(), 1);
        assert_eq!(grp.name, o.keys().next().unwrap());
        assert_eq!(cells as u64, u(o.values().next().unwrap()));
    }
    // the 19 pocket consumables of the client's table, all with cells 1
    for d in [5u16, 65, 66, 67, 68, 82, 219, 733, 734, 736, 759, 760, 1089, 1090, 1091, 1101, 1106, 1133, 1196] {
        assert_eq!(g.craft.pocket_group(d).map(|x| x.1), Some(1), "dict {d}");
    }
    // limits and categories (pockets.md 1)
    let by = |n| pockets::group_by_name(n).unwrap();
    assert_eq!((by("drug").max_used_cells, by("grenade").max_used_cells), (8, 2));
    assert_eq!((by("generic").max_used_cells, by("resupply").max_used_cells), (4, 1));
    assert_eq!(pockets::group_of_category(11).unwrap().name, "grenade");
    assert_eq!(pockets::group_of_category(55).unwrap().name, "generic");
    assert!(pockets::group_of_category(23).is_none());
    // slot restrictions of the dictionary
    let sc = exp()["slot_categories"].as_array().unwrap();
    assert_eq!(g.slot_categories.len(), sc.len());
    for (i, row) in sc.iter().enumerate() {
        let want: Vec<u16> = row.as_array().unwrap().iter().map(|x| u(x) as u16).collect();
        assert_eq!(g.slot_categories[i], want);
    }
    assert!(g.slot_allows(2, 31)); // scavenger torso in the torso slot
    assert!(!g.slot_allows(2, 107));
    assert!(g.slot_allows(13, 66)); // bandages in a pocket
    assert!(g.slot_allows(7, 107));
    // capacity from armour defaults
    for r in exp()["pocket_capacity"].as_array().unwrap() {
        let cap = g.craft.pocket_capacity(&[u(&r[0]) as u16], &[]);
        assert_eq!((cap.slots as u64, cap.cells as u64), (u(&r[1]), u(&r[2])), "armour {}", r[0]);
    }
    // scav torso 1 (dict 31): one pocket, two cells (pockets.md)
    let cap = g.craft.pocket_capacity(&[31], &[]);
    assert_eq!(cap, PocketCapacity { slots: 1, cells: 2 });
    // crafted +1 slot / +2 cells (groups 173/174) add
    let cap2 = g.craft.pocket_capacity(&[31], &[(173, 1), (174, 2)]);
    assert_eq!(cap2, PocketCapacity { slots: 2, cells: 4 });
    // placing: one slot, two cells
    assert!(g.craft.pocket_fits(cap, &[], Placed { dict_id: 66, count: 2 }));
    assert!(!g.craft.pocket_fits(cap, &[], Placed { dict_id: 66, count: 3 }), "over the cell depth");
    assert!(!g.craft.pocket_fits(cap, &[Placed { dict_id: 66, count: 1 }], Placed { dict_id: 65, count: 1 }), "no free slot");
    assert!(g.craft.pocket_fits(cap2, &[Placed { dict_id: 66, count: 1 }], Placed { dict_id: 5, count: 1 }));
    assert!(!g.craft.pocket_fits(cap2, &[Placed { dict_id: 5, count: 1 }], Placed { dict_id: 733, count: 2 }), "grenade group holds 2 cells");
    assert!(!g.craft.pocket_fits(cap2, &[], Placed { dict_id: 107, count: 1 }), "a rifle has no pocket group");
}

#[test]
fn levels_and_factions() {
    require_reference!();
    let g = gd();
    let tbl: Vec<u32> = exp()["xp_table"].as_array().unwrap().iter().map(|x| u(x) as u32).collect();
    assert_eq!(g.levels.xp_table(), tbl.as_slice());
    // the reference counts 101 levels; the data's 100 rows end level 100 (levels.rs `max_level`)
    assert_eq!(g.levels.max_level(), 100);
    for r in exp()["level_fields"].as_array().unwrap() {
        let xp = u(&r[0]) as u32;
        if u(&r[1]) > 100 {
            assert_eq!(g.levels.level_of(xp), 100, "the cap: the reference counted a level 101 at {xp}");
            continue;
        }
        assert_eq!(g.levels.level_of(xp) as u64, u(&r[1]), "level of {xp}");
        let mut b = g.levels.level_fields(xp).to_bytes();
        // the reference has points = level; the rule is level - 1 (Levels::skill_points_for_level)
        assert_eq!(u64::from(b[1]) + 1, u64::from(b[0]), "skill points = level - 1 at xp {xp}");
        b[1] = b[0];
        let hex: String = b.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(hex, r[2].as_str().unwrap(), "level fields of {xp}");
    }
    assert_eq!(g.levels.level_of(2170), 5);
    assert_eq!(g.levels.reward_for_level(2), Some((500, 0)));
    assert_eq!(g.levels.reward_for_level(1), None);
    // match rewards follow the data's formula (progression.rs tests)
    let thr: Vec<u32> = exp()["faction_thr"].as_array().unwrap().iter().map(|x| u(x) as u32).collect();
    assert_eq!(levels::FACTION_THRESHOLDS.to_vec(), thr);
    for r in exp()["faction_level"].as_array().unwrap() {
        assert_eq!(levels::faction_level(u(&r[0]) as u32) as u64, u(&r[1]), "faction level of {}", r[0]);
    }
    assert_eq!(levels::faction_level(0), 1);
    assert_eq!(levels::faction_label(200_000), (5, 0));
    assert_eq!(levels::faction_label(350_000), (5, 1));
    assert_eq!(levels::faction_label(5_000_000), (5, 10));
}

#[test]
fn maps_and_modes() {
    require_reference!();
    let g = gd();
    // the extractor keeps only the rows whose level project ships in the client: 107 of the reference's 314 (a
    // missing project makes the exe assert "failed to load project"); a reference answer on a dropped row is not compared, only that
    // ours (if any) is another row of the same map and mode; plus the four Search and Destroy rows the extractor reads from
    // the mode files (the reference has no mode 5) and the gun race row of its own mode file (gunrace_069d.json)
    assert_eq!(g.matches.modes().len(), 107 + 4 + 1);
    assert_eq!(g.matches.modes().iter().filter(|m| m.mode == 5).count(), 4);
    assert_eq!(g.matches.modes().iter().filter(|m| m.mode == game_data::gunrace::MODE).count(), 1);
    let ours = |p: &str| g.matches.modes().iter().any(|m| m.project == p);
    for r in exp()["resolve_map"].as_array().unwrap() {
        let got = g.matches.resolve_map(u(&r[0]) as u32, u(&r[1]) as u32, u(&r[2]) as u32);
        match &r[3] {
            Value::Null => assert!(got.is_none(), "{r}"),
            row if !ours(row[4].as_str().unwrap()) => {
                if let Some(got) = got {
                    assert_eq!((got.map as u64, got.mode as u64), (u(&r[0]), u(&r[1])), "{r}");
                }
            }
            row => {
                let got = got.unwrap_or_else(|| panic!("no row for {r}"));
                let want = row.as_array().unwrap();
                assert_eq!(
                    (got.map as u64, got.mode as u64, got.variation as u64, got.weather as u64),
                    (u(&want[0]), u(&want[1]), u(&want[2]), u(&want[3])),
                    "{r}"
                );
                assert_eq!((got.project.as_str(), got.map_name.as_str()), (want[4].as_str().unwrap(), want[5].as_str().unwrap()));
                assert_eq!((got.min_players as u64, got.max_players as u64), (u(&want[6]), u(&want[7])));
            }
        }
    }
    // level_01 team deathmatch day, variation 0
    let r = g.matches.resolve_map(1, 1, 0).unwrap();
    assert_eq!((r.project.as_str(), r.map_name.as_str()), ("level_01_day_tdm", "level_01/day_tdm"));
}

#[test]
fn bots_kits_skills_names() {
    require_reference!();
    let g = gd();
    assert_eq!(g.matches.ai_players().len(), 36);
    for (key, want) in exp()["pick_bots"].as_object().unwrap() {
        let (n, lvl) = key.split_once('/').unwrap();
        let want: Vec<usize> = want.as_array().unwrap().iter().map(|x| u(x) as usize).collect();
        assert_eq!(g.matches.pick_bots(n.parse().unwrap(), lvl.parse().unwrap()), want, "pick_bots {key}");
    }
    for k in exp()["kits"].as_array().unwrap() {
        let ai = &g.matches.ai_players()[u(&k["ai"]) as usize];
        let kit = g.matches.kit(&g.items, &ai.profiles[0]);
        let items: Vec<(u32, u16, u32)> = k["items"].as_array().unwrap().iter().map(|t| (u(&t[0]) as u32, u(&t[1]) as u16, u(&t[2]) as u32)).collect();
        // the reference ignores `modules_to_equip`: its items are the slot items; the kit's module items follow them, each
        // installed on a weapon of the kit
        assert_eq!(kit.items[..items.len()], items[..], "kit items of ai {}", k["ai"]);
        let extra: Vec<u32> = kit.items[items.len()..].iter().map(|i| i.0).collect();
        let installed: Vec<u32> = kit.attach.iter().flat_map(|(_, l)| l.iter().map(|m| m.1)).collect();
        assert_eq!(extra, installed, "module items of ai {}", k["ai"]);
        assert!(kit.items[items.len()..].iter().all(|i| g.items.is_module(i.1)));
        let slots: BTreeMap<u8, u32> = k["slots"].as_object().unwrap().iter().map(|(a, b)| (a.parse().unwrap(), u(b) as u32)).collect();
        assert_eq!(kit.slots, slots);
        let counts: BTreeMap<u8, u32> = k["counts"].as_object().unwrap().iter().map(|(a, b)| (a.parse().unwrap(), u(b) as u32)).collect();
        assert_eq!(kit.counts, counts);
    }
    for (i, want) in exp()["skill_mods"].as_array().unwrap().iter().enumerate() {
        let got = g.matches.skill_modifiers(&g.matches.ai_players()[i].skills);
        let want: BTreeMap<u32, f64> = want.as_object().unwrap().iter().map(|(a, b)| (a.parse().unwrap(), b.as_f64().unwrap())).collect();
        assert_eq!(got.keys().collect::<Vec<_>>(), want.keys().collect::<Vec<_>>(), "skill modifier ids of ai {i}");
        for (id, v) in &want {
            assert!((got[id] - v).abs() < 1e-9, "ai {i} modifier {id}: {} vs {v}", got[id]);
        }
    }
    assert_eq!(g.matches.modifier_names().len(), 138);
    for r in exp()["bot_names"].as_array().unwrap() {
        let k = u(&r[0]) as usize;
        let (f, s) = game_data::MatchData::bot_name_indices(k);
        assert_eq!(g.matches.bot_name(f, s, &format!("Bot {}", k + 1)), r[1].as_str().unwrap(), "bot name {k}");
        assert_eq!(game_data::MatchData::bot_team(k) as usize, (k + 1) % 2);
    }
}

/// The AKMN's slot set grows with the dovetail mount; the collimator attaches
/// to the added slot; its block disables scope_dovetail, from which its target slot hangs.
#[test]
fn akmn_slot_set_follows_the_mounts() {
    game_data::require_tables!();
    let g = gd();
    let m = &g.modules;
    let (akmn, mount, scope) = (1201u16, 1703u16, 1772u16);
    let bare = m.slot_set(akmn, |s| m.default_in(akmn, s));
    assert!(bare.accepts("scope_dovetail") && !bare.accepts("aim") && !bare.contains("picatinny_scope_medium"));
    let with_mount = m.slot_set(akmn, |s| if s == "scope_dovetail" { Some(mount) } else { m.default_in(akmn, s) });
    let added = with_mount.get("picatinny_scope_medium").unwrap();
    assert!(added.accept && added.parent.as_deref() == Some("scope_dovetail"));
    assert!(m.can_attach_to(scope, akmn, "picatinny_scope_medium") && !m.can_attach_to(scope, akmn, "scope_dovetail"));
    assert!(m.is_candidate(mount, akmn) && m.is_candidate(scope, akmn));
    let blk = m.block(scope, akmn, "picatinny_scope_medium").unwrap();
    assert!(blk.disable.iter().any(|s| s == "scope_dovetail"));
    assert!(with_mount.hangs_from("picatinny_scope_medium", "scope_dovetail"));
    // the collimator adds `reticle` (accept_module false) and turns `aim` off
    let full = m.slot_set(akmn, |s| match s {
        "scope_dovetail" => Some(mount),
        "picatinny_scope_medium" => Some(scope),
        _ => m.default_in(akmn, s),
    });
    assert!(full.contains("reticle") && !full.accepts("reticle") && !full.accepts("aim"));
    assert!(full.hangs_from("reticle", "scope_dovetail"));
    assert_eq!((m.detach_price(mount), m.detach_price(scope)), (50, 100));
    // exceptions[<weapon cfg>] first: medium_picatinnyrail uses attach_params_2 on the RPD
    let rpd = g.items.by_cfg_name("rpd").unwrap();
    let rail = g.items.by_cfg_name("medium_picatinnyrail").unwrap();
    assert_eq!(m.attach_slots(rail, rpd).collect::<Vec<_>>(), ["grip_picatinnyrail"]);
}

/// Pocket consumables without a 0.69d or wiki price carry our own silver price (CHOICE, `choice` rows
/// of data/shop_069d.tsv, docs/notes/store.md section 5): listed (8A byte e = 1, so the pocket popup builds a tile),
/// sold for silver, sell base a / 4, charged after the match at that price. The event specials (Nut 1355, Snowball
/// 1884) stay unlisted.
#[test]
fn pocket_consumables_without_a_source_price_carry_the_choice_price() {
    game_data::require_tables!();
    let g = gd();
    let want: [(u16, u32); 9] = [(733, 40), (1089, 50), (1090, 100), (1091, 50), (1101, 75), (1106, 100), (1133, 250), (1196, 40), (2072, 40)];
    assert_eq!(g.shop.choices().collect::<Vec<_>>(), want.iter().map(|w| w.0).collect::<Vec<_>>(), "exactly the choice rows");
    for (d, silver) in want {
        let p = g.shop.price(d).unwrap();
        assert_eq!((p.a, p.b, p.c, p.kind, p.for_sale), (silver, silver / 4, 0, 0, true), "dict {d}");
        assert!(g.items.is_quick_item(d), "dict {d} is a pocket item");
        assert_eq!(g.shop.pocket_cost(d, 3), (Wallet::Silver, 3 * silver as u64), "dict {d} pocket cost");
        assert_eq!(g.shop.buy_total(&g.items, d, 2), Some(2 * silver as u64), "dict {d} buy");
    }
    for d in [1355u16, 1884] {
        assert!(!g.shop.price(d).unwrap().for_sale && !g.shop.is_choice(d), "event special {d} stays unlisted");
    }
    // every one of the 19 specifics of the client's table is listed now
    for d in [5u16, 65, 66, 67, 68, 82, 219, 733, 734, 736, 759, 760, 1089, 1090, 1091, 1101, 1106, 1133, 1196] {
        assert!(g.shop.price(d).is_some_and(|p| p.for_sale && p.a > 0 && p.kind == 0), "specific {d} listed with a silver price");
    }
}

/// An unlisted pocket item (no 0.69d, wiki or choice price: the event specials Nut 1355 and Snowball 1884) costs nothing in
/// the pocket charge: its dictionary default price is a placeholder the client never shows. Listed items keep their price.
#[test]
fn unlisted_pocket_items_cost_nothing() {
    game_data::require_tables!();
    let g = gd();
    for d in [1355u16, 1884] {
        assert!(g.items.is_quick_item(d) && !g.shop.listed(d), "dict {d} is an unlisted pocket item");
        assert!(g.shop.price(d).is_some_and(|p| p.a > 0), "dict {d} still has its placeholder record");
        assert_eq!(g.shop.pocket_cost(d, 4), (Wallet::Silver, 0), "dict {d}");
    }
    for (d, silver) in [(5u16, 30u64), (760, 25), (1133, 250), (2072, 40)] {
        assert!(g.shop.listed(d));
        assert_eq!(g.shop.pocket_cost(d, 2), (Wallet::Silver, 2 * silver), "dict {d}");
    }
    // every pocket item: charged at its 8A price when listed, else free
    for it in g.items.iter().filter(|i| g.items.is_quick_item(i.dict_id)) {
        let p = g.shop.price(it.dict_id).unwrap();
        let want = if p.for_sale { 3 * p.a as u64 } else { 0 };
        assert_eq!(g.shop.pocket_cost(it.dict_id, 3).1, want, "dict {}", it.dict_id);
    }
}
