//! What srp decides about the items and the lobby that the game's files do
//! not say, as JSON for the site's wiki (`srp-026g/scripts/wiki`, its
//! README): who sells each item at which reputation level and price, what
//! selling and repairing give, which modifications and defects an item can
//! get and how likely each is (by [`modifications::weight`]), the scope
//! pairs, drops, pay rates, gifts and limits. Every value comes from srp's
//! own code over the game data of the process
//! ([`survarium::v026g::game_data`]), so the wiki shows what the server
//! does.
//!
//! The wiki is built from [`rules`] over the released game data (the
//! example `wiki_rules`); it records the SHA-256 of [`rules`] over the test
//! data too, and a web-server test compares that with this code's: a change
//! here or in the rules it reads fails that test until the wiki is built
//! again.

use serde_json::{Value, json};
use survarium::v026g::{
    dictionary::Dictionary,
    drops::Drops,
    item_params, loadouts,
    lobby::rooms,
    modifications::{self, MAX_RANK, Modifications, REROLL_CHOICES, REROLL_WAIT_SECONDS},
    prices,
    scopes::ScopeFits,
    shop_offers::{self, BUNDLES, DEFAULT_PREMIUM_PRICES, PREMIUM_OFFERS},
};

use super::{account, pay, rules as lobby_rules, tutorial};

/// The rules over the process's game data, as pretty JSON.
pub fn rules() -> String {
    let dictionary = Dictionary::shared();
    let mods = Modifications::shared();
    let drops = Drops::shared();
    let premium: Vec<u16> = shop_offers::premium_price_list()
        .iter()
        .map(|price| price.dict_id)
        .collect();

    // What can drop: every item a roll of any category at any level can
    // pick (srp's `drops::roll` reads the same lists).
    let mut droppable = std::collections::BTreeSet::new();
    let categories: std::collections::BTreeSet<u8> =
        dictionary.items.iter().map(|item| item.category).collect();
    for &category in &categories {
        for level in 0..=15 {
            for params in drops.generic.candidates(dictionary, category, level) {
                droppable.insert(params.dict_id);
            }
        }
    }

    let items: Vec<Value> = dictionary
        .items
        .iter()
        .map(|item| {
            let id = item.dict_id;
            let sellers: Vec<Value> = shop_offers::price_list()
                .iter()
                .filter(|price| price.dict_id == id)
                .map(|price| {
                    json!({
                        "seller": price.faction_id,
                        "level": price.level,
                        "cost": price.cost,
                        "gold": premium.contains(&id),
                    })
                })
                .collect();
            let stack = item.is_stack;
            let unit = item_params::shop_unit(id);
            let full = if stack { unit } else { 100 };
            let candidates: Vec<Value> = mods
                .candidates(id, &[])
                .iter()
                .map(|row| json!([row.id, modifications::weight(row.drop_weight)]))
                .collect();
            let defects: Vec<Value> = mods
                .defect_candidates(id, &[], &[])
                .iter()
                .map(|row| json!([row.id, modifications::weight(row.drop_weight)]))
                .collect();
            json!({
                "dict_id": id,
                "name": item.name,
                "sellers": sellers,
                "unit": unit,
                "sell": prices::sell_price(dictionary, id, full, lobby_rules::PREMIUM_EXCHANGE_RATE),
                "repair_from_zero": prices::repair_cost(dictionary, id, 0),
                "droppable": droppable.contains(&id),
                "modifications": candidates,
                "defects": defects,
            })
        })
        .collect();

    let scopes: Vec<Value> = ScopeFits::shared()
        .all()
        .iter()
        .map(|fit| json!({"weapon": fit.weapon, "scope": fit.scope, "variant": fit.variant}))
        .collect();
    let bundles: Vec<Value> = BUNDLES
        .iter()
        .map(|bundle| {
            json!({
                "id": bundle.bundle_id,
                "gold": bundle.gold,
                "items": bundle.items,
            })
        })
        .collect();
    let premium_time: Vec<Value> = PREMIUM_OFFERS
        .iter()
        .zip(DEFAULT_PREMIUM_PRICES)
        .map(|(&(_, days), gold)| json!({"days": days, "gold": gold}))
        .collect();
    let craft = &mods.craft;
    let reroll = &mods.reroll;
    let out = json!({
        "items": items,
        "scopes": scopes,
        "bundles": bundles,
        "premium_time": premium_time,
        "modifications": {
            "max": MAX_RANK,
            "reroll_choices": REROLL_CHOICES,
            "reroll_wait_s": REROLL_WAIT_SECONDS,
            "reroll_price_silver": reroll.price_general,
            "reroll_price_gold": reroll.price_premium,
            "reroll_growth": reroll.multiplicator,
            "craft_price_silver": craft.price_general,
            "craft_price_gold": craft.price_premium,
            "craft_chances_silver": craft.chances_general,
            "craft_chances_gold": craft.chances_premium,
            "defect_slots": mods.defect_slots.iter().map(|slot| json!({
                "condition": [slot.condition.0, slot.condition.1],
                "chance": [slot.probability.0, slot.probability.1],
            })).collect::<Vec<_>>(),
        },
        "drops": {
            "chance": drops.generic.item_chance(),
            "per_battery": pay::DROP_CHANCE_PER_BATTERY,
            "premium": pay::DROP_CHANCE_PREMIUM,
            "condition": pay::DEFAULT_DROP_CONDITION,
            // How many modifications a dropped weapon or piece of gear has:
            // the share of each band of `rarity_points` (`DropTable::rarity_at`).
            "rarity": rarity_shares(&drops.generic.rarity_points),
        },
        "pay": {
            "money_rates": pay::MONEY_RATES,
            "experience_rate": pay::DEFAULT_EXPERIENCE_RATE,
            "reputation_rate": pay::DEFAULT_REPUTATION_RATE,
        },
        "exchange_rate": lobby_rules::PREMIUM_EXCHANGE_RATE,
        "skills_reset": {"silver": lobby_rules::REROLL_COST, "gold": lobby_rules::REROLL_COST_PREMIUM},
        "kits": account::KITS,
        "rooms": {
            "sizes": [rooms::SIZES.start(), rooms::SIZES.end()],
            "min_sides": [rooms::MIN_SIDES.start(), rooms::MIN_SIDES.end()],
            "waits": rooms::WAITS,
            "level_max": rooms::LEVEL_MAX,
        },
        "signup": {
            "silver": tutorial::SIGNUP_SILVER,
            "gold": tutorial::SIGNUP_GOLD,
            "ammo": tutorial::SIGNUP_AMMO,
            "faction": tutorial::SIGNUP_FACTION,
        },
        "starter": {
            "worn": loadouts::STARTER.iter().map(|&(_, name)| name).collect::<Vec<_>>(),
            "stash": loadouts::STARTER_STASH,
        },
    });
    serde_json::to_string_pretty(&out).expect("JSON")
}

/// The share of each band of a drop table's `rarity_points`: rarity `n` is
/// a point of `[points[n], points[n + 1])` on the line up to the last.
fn rarity_shares(points: &[f32]) -> Vec<f32> {
    let top = points.last().copied().unwrap_or(0.0);
    if top <= 0.0 {
        return Vec::new();
    }
    points
        .windows(2)
        .map(|band| (band[1] - band[0]) / top)
        .collect()
}
