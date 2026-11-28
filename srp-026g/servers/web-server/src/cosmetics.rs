//! The wiki's camouflages and decals, `/wiki/camouflages` and `/wiki/decals` on the 0.26g site:
//! every item of `items_dict` category 25 or 26 with its lobby icon, its name and how a player
//! gets it on Skhid, grouped by that, and what it goes on. Two pages, not one: they share no item,
//! their "where it goes" differs, and together they would be some 1,500 entries.
//!
//! Made from the server's own game data and rules, so it is the release's:
//! - **sold**: the price list the lobby is sent ([`shop_offers::price_list`]: the factions' own
//!   camouflages, the badges, the premium items one by one), each record's seller, rank and silver
//!   (every price is silver since 2026-11-08, a premium item's too);
//! - **dropped**: an item with a drop weight of a category a drop table rolls
//!   ([`DropTable::candidates`], the roll's own choice): after a match, a first match, a drop card;
//!   a camouflage or decal the player has is never given ([`drops::COSMETIC_CATEGORIES`]);
//! - **given**: the number decals, each to the account of its number
//!   ([`number_decals::Number::of_account`], `docs/notes/number-decals.md`);
//! - nothing of these: not listed (the owner, 2026-11-28: "no need to show not obtainable at
//!   all"; the flags and arms not for sale are skipped on purpose).
//!
//! The icon is the lobby's: the cell `ui_desc.icon` ([row, column], from 1) of the camouflage or
//! decal atlas, as `build_icons.py` cuts them ([`IconSet::Camouflage`], [`IconSet::Decal`]).
//! Where it goes is each item's `camo_available` / `decal_available` and srp's slots
//! ([`modifications::cosmetic_slots`]).

use std::collections::BTreeMap;
use std::fmt::Write;
use std::sync::OnceLock;

use binary_config_parser::tree::ConfigValue;
use session::v026g::number_decals::Number;
use survarium::v026g::dictionary::{Dictionary, Item};
use survarium::v026g::drops::{DropTable, Drops};
use survarium::v026g::item_params::{self, ItemParams};
use survarium::v026g::modifications;
use survarium::v026g::{game_data, prices, shop_offers};

use crate::icons::{self, IconSet};
use crate::modifiers::{self, Group};
use crate::skills::thousands;
use crate::{Lang, equipment, escape};

/// The pages' addresses.
pub(crate) const CAMOUFLAGES: &str = "/wiki/camouflages";
pub(crate) const DECALS: &str = "/wiki/decals";

/// The two pages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Page {
    Camouflages,
    Decals,
}

impl Page {
    pub(crate) const ALL: [Self; 2] = [Self::Camouflages, Self::Decals];

    pub(crate) const fn path(self) -> &'static str {
        match self {
            Self::Camouflages => CAMOUFLAGES,
            Self::Decals => DECALS,
        }
    }

    /// The page at `path`, if it is one.
    pub(crate) fn at(path: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|page| page.path() == path)
    }

    /// Its `items_dict` category.
    const fn category(self) -> u8 {
        match self {
            Self::Camouflages => 25,
            Self::Decals => 26,
        }
    }

    pub(crate) const fn icons(self) -> IconSet {
        match self {
            Self::Camouflages => IconSet::Camouflage,
            Self::Decals => IconSet::Decal,
        }
    }

    /// What [`modifications::cosmetic_slots`] reads of an upgrade's `type`: its ending.
    const fn upgrade_kind(self) -> &'static str {
        match self {
            Self::Camouflages => "_camo",
            Self::Decals => "_decl",
        }
    }

    pub(crate) fn title(self, lang: Lang) -> String {
        match self {
            Self::Camouflages => lang.t("Camouflages"),
            Self::Decals => lang.t("Decals"),
        }
    }
}

/// A cell of the lobby's atlas: `ui_desc.icon` [row, column], from 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Cell {
    row: u8,
    column: u8,
}

impl Cell {
    /// The cell an item's config names, if it names one.
    fn of(config: &[u8]) -> Option<Self> {
        let root = ConfigValue::parse(config).ok()?;
        let icon = root.get("ui_desc")?.get("icon")?.children();
        let [row, column] = icon[..] else {
            return None;
        };
        let at = |value: &ConfigValue| u8::try_from(value.as_i32()?).ok().filter(|&n| n >= 1);
        Some(Self {
            row: at(row)?,
            column: at(column)?,
        })
    }

    /// Its icon's key ([`icons::url`]).
    fn key(self) -> String {
        format!("{}_{}", self.row, self.column)
    }
}

/// A drop table that gives cosmetics.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Table {
    Match,
    FirstMatch,
    DropCard,
}

/// One way a player gets an item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Way {
    /// A record of the price list: who sells it, at which reputation level with them, for how
    /// much silver.
    Sold { seller: u8, rank: u8, silver: u32 },
    /// A drop table that can give it.
    Dropped(Table),
    /// Given to the account of this id.
    Given(u16),
}

/// Where an item is listed: how it is got, by its `type` where that says it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Shelf {
    /// A faction's own camouflage (`stk_camo`, [`prices::SHOP_CAMO`]).
    Faction,
    /// A badge ([`prices::BADGE`]).
    Badge,
    /// A premium camouflage or decal (`prm_*`), sold one by one, but for the flags and the
    /// coats of arms.
    Premium,
    /// A premium decal of the flags' row ([`DecalRow::Flags`]).
    Flags,
    /// A premium decal of the coats of arms' and emblems' row ([`DecalRow::Arms`]).
    Arms,
    /// A drop camouflage or decal (`drp_*`).
    Drop,
    /// Given to one account each.
    Given,
}

/// The rows of the game's decal naming that the page shelves apart: a decal's config is
/// `char_decal_<row>_<n>` and its texture `character/human/decal/<row>_<n>`, and 0.26g and 0.69d
/// keep their flags in row 4 and their coats of arms and emblems (NATO's, the UN's) in row 5;
/// Skhid's own flag and coat of arms of Kosovo too (`4_76`, `5_43`; docs/notes/kosovo-decals.md).
/// The owner, 2026-11-28: "separate flags and coats into their own category", "nato and UN goes
/// there too".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DecalRow {
    Flags,
    Arms,
}

impl DecalRow {
    /// The row of `item`'s config name, when it is one of these.
    fn of(item: &Item) -> Option<Self> {
        let file = item.cfg_name.rsplit(['/', '\\']).next()?;
        let row = file.strip_prefix("char_decal_")?.split('_').next()?;
        match row {
            "4" => Some(Self::Flags),
            "5" => Some(Self::Arms),
            _ => None,
        }
    }

    const fn shelf(self) -> Shelf {
        match self {
            Self::Flags => Shelf::Flags,
            Self::Arms => Shelf::Arms,
        }
    }
}

/// A premium item's shelf: its row's for a flag or coat of arms, else Premium.
fn premium(item: &Item) -> Shelf {
    DecalRow::of(item).map_or(Shelf::Premium, DecalRow::shelf)
}

/// A camouflage or decal.
#[derive(Debug)]
struct Cosmetic {
    item: &'static Item,
    cell: Option<Cell>,
    ways: Vec<Way>,
    shelf: Shelf,
}

/// The tables that can give `params`: those with a group of its category, and the item among
/// what a roll of it picks from (its level, unless the category goes by none).
fn tables(dictionary: &Dictionary, item: &Item, params: &ItemParams) -> Vec<Table> {
    let drops = Drops::shared();
    let gives = |table: &DropTable| {
        let rolled = table
            .categories
            .iter()
            .any(|group| group.weight > 0 && group.categories.contains(&item.category));
        // The item levels a roll can ask for: any, for a category that goes by none.
        let independent = table.level_independent.contains(&item.category);
        let columns = table.levels.iter().map(Vec::len).max().unwrap_or(0);
        let mut levels = (0..columns).filter(|&level| {
            independent
                || table
                    .levels
                    .iter()
                    .any(|row| row.get(level).is_some_and(|&weight| weight > 0))
        });
        rolled
            && levels.any(|level| {
                u8::try_from(level).is_ok_and(|level| {
                    table
                        .candidates(dictionary, item.category, level)
                        .iter()
                        .any(|candidate| candidate.dict_id == params.dict_id)
                })
            })
    };
    [
        (Table::Match, &drops.generic),
        (Table::FirstMatch, &drops.new_player),
        (Table::DropCard, &drops.drop_card),
    ]
    .into_iter()
    .filter(|(_, table)| gives(table))
    .map(|(name, _)| name)
    .collect()
}

/// The account each number decal goes to, by the decal's name.
fn given() -> &'static BTreeMap<String, u16> {
    static GIVEN: OnceLock<BTreeMap<String, u16>> = OnceLock::new();
    GIVEN.get_or_init(|| {
        (1..=Number::LAST)
            .filter_map(|id| Number::of_account(id.into()))
            .map(|number| (number.decal_name(), number.get()))
            .collect()
    })
}

/// Every item of `page`'s category, by `dict_id`.
fn all(page: Page) -> &'static [Cosmetic] {
    static ALL: [OnceLock<Vec<Cosmetic>>; 2] = [OnceLock::new(), OnceLock::new()];
    ALL[page as usize].get_or_init(|| {
        let dictionary = Dictionary::shared();
        let prices = shop_offers::price_list();
        dictionary
            .items
            .iter()
            .filter(|item| item.category == page.category())
            .filter_map(|item| {
                let params = item_params::item_params(item.dict_id);
                let mut ways: Vec<Way> = prices
                    .iter()
                    .filter(|price| price.dict_id == item.dict_id)
                    .map(|price| Way::Sold {
                        seller: price.faction_id,
                        rank: price.level,
                        silver: price.cost,
                    })
                    .collect();
                if let Some(params) = params {
                    ways.extend(
                        tables(dictionary, item, params)
                            .into_iter()
                            .map(Way::Dropped),
                    );
                }
                ways.extend(given().get(&item.name).map(|&id| Way::Given(id)));
                let kind = params.map_or("", |params| params.kind);
                if ways.is_empty() {
                    return None;
                }
                let shelf = if ways.iter().any(|way| matches!(way, Way::Given(_))) {
                    Shelf::Given
                } else if kind == prices::SHOP_CAMO {
                    Shelf::Faction
                } else if kind == prices::BADGE {
                    Shelf::Badge
                } else if ways.iter().any(|way| matches!(way, Way::Sold { .. })) {
                    // Sold, though it may drop too (Dust, camo_khaki_fabric_03, 100 silver): on
                    // a sold shelf, its price on its card (the owner, 2026-11-28).
                    premium(item)
                } else if kind.starts_with("drp_") {
                    Shelf::Drop
                } else if kind.starts_with("prm_") || item.is_premium {
                    premium(item)
                } else if ways.iter().any(|way| matches!(way, Way::Dropped(_))) {
                    Shelf::Drop
                } else {
                    premium(item)
                };
                Some(Cosmetic {
                    item,
                    cell: game_data::shared().file(&item.cfg_name).and_then(Cell::of),
                    ways,
                    shelf,
                })
            })
            .collect()
    })
}

/// An item's name in `lang`: the game's (English where a language lacks it), else its
/// dictionary name.
fn name(lang: Lang, item: &Item) -> String {
    item_params::item_params(item.dict_id)
        .and_then(|params| params.name_key)
        .map_or_else(|| item.name.clone(), |key| modifiers::text(lang, key))
}

/// The address of `name`'s icon on `page` (an item of its category), for the wiki's index.
pub(crate) fn icon_of(page: Page, name: &str) -> Option<String> {
    let cosmetic = all(page)
        .iter()
        .find(|cosmetic| cosmetic.item.name == name)?;
    icons::url(page.icons(), &cosmetic.cell?.key())
}

fn picture(page: Page, cell: Option<Cell>) -> String {
    cell.and_then(|cell| icons::url(page.icons(), &cell.key()))
        .map_or_else(
            || "<span class=cosmetic-none></span>".to_owned(),
            |url| {
                format!(
                    "<img src=\"{}\" alt=\"\" width=72 height=72 loading=lazy decoding=async>",
                    escape(&url)
                )
            },
        )
}

/// A seller's name and, with a rank, its rank's.
fn seller(lang: Lang, seller: u8, rank: u8) -> String {
    let faction = equipment::faction_name(lang, seller);
    let rank = Dictionary::shared()
        .factions
        .iter()
        .find(|row| row.id == seller)
        .and_then(|row| row.levels.get(usize::from(rank)))
        .filter(|_| rank > 0)
        .map(|level| modifiers::text(lang, &level.name));
    match rank {
        Some(rank) => format!("{faction} · {rank}"),
        None => faction,
    }
}

/// How a player gets a cosmetic, a line each way.
fn ways(lang: Lang, cosmetic: &Cosmetic) -> String {
    let silver = modifiers::text(lang, "st_regular_money");
    let lines: Vec<String> = cosmetic
        .ways
        .iter()
        .flat_map(|&way| match way {
            // The price, and under it who sells it (none: every trader).
            Way::Sold {
                seller: who,
                rank,
                silver: price,
            } => {
                let price = format!("<b>{}</b> {}", thousands(lang, price), escape(&silver));
                if who == 0 {
                    vec![price]
                } else {
                    vec![price, escape(&seller(lang, who, rank))]
                }
            }
            // The Drops shelf's line says it; a card does not (the owner, 2026-11-28).
            Way::Dropped(_) => vec![],
            Way::Given(id) => vec![escape(&lang.tf("Given to account {0}", &[("0", &id)]))],
        })
        .collect();
    lines
        .iter()
        .map(|line| format!("<span>{line}</span>"))
        .collect()
}

fn card(lang: Lang, page: Page, cosmetic: &Cosmetic) -> String {
    let ways = ways(lang, cosmetic);
    let ways = if ways.is_empty() {
        String::new()
    } else {
        format!("<small>{ways}</small>")
    };
    format!(
        "<li class=cosmetic id=\"i{}\">{}<b>{}</b>{ways}</li>",
        cosmetic.item.dict_id,
        picture(page, cosmetic.cell),
        escape(&name(lang, cosmetic.item)),
    )
}

/// The items given to one account each that share an icon (the number decals), as one card:
/// the first's and the last's names and how many.
fn given_card(lang: Lang, page: Page, cosmetics: &[&Cosmetic]) -> String {
    let (Some(first), Some(last)) = (cosmetics.first(), cosmetics.last()) else {
        return String::new();
    };
    if cosmetics.len() == 1 {
        return card(lang, page, first);
    }
    format!(
        "<li class=\"cosmetic cosmetic-series\" id=\"i{}\">{}<b>{} – {}</b><small><span>{}</span></small></li>",
        first.item.dict_id,
        picture(page, first.cell),
        escape(&name(lang, first.item)),
        escape(&name(lang, last.item)),
        escape(&lang.tf(
            "{count} items, each given to the account of the same id",
            &[("count", &thousands(lang, cosmetics.len() as u32))]
        )),
    )
}

fn shelf_title(lang: Lang, shelf: Shelf) -> String {
    match shelf {
        Shelf::Faction => lang.t("Sold by the factions"),
        Shelf::Badge => lang.t("Badges"),
        Shelf::Premium => lang.t("Premium"),
        Shelf::Flags => lang.t("Flags"),
        Shelf::Arms => lang.t("Coats of arms and emblems"),
        Shelf::Drop => lang.t("Drops"),
        Shelf::Given => lang.t("Given"),
    }
}

fn shelf_line(lang: Lang, shelf: Shelf) -> String {
    match shelf {
        Shelf::Faction => lang.t("Each faction sells its own for silver."),
        Shelf::Badge => lang.t(
            "Sold for silver: a faction's badge by that faction at the rank it names, the others by every trader.",
        ),
        Shelf::Premium | Shelf::Flags | Shelf::Arms => lang.t("Sold for silver by every trader."),
        Shelf::Drop => lang.t("Found in drops, never one you already have."),
        Shelf::Given => lang.t("Each is given to one account."),
    }
}

/// Where `page`'s items go: each group of items that takes one, how many of its items do and
/// how many each holds at once.
fn where_it_goes(lang: Lang, page: Page) -> String {
    let dictionary = Dictionary::shared();
    let mut groups: BTreeMap<Group, (usize, usize)> = BTreeMap::new();
    for host in modifications::all_item_mods() {
        let Some(item) = dictionary.item(host.dict_id) else {
            continue;
        };
        let Some(group) = Group::of(item.category) else {
            continue;
        };
        let kind = item_params::item_params(host.dict_id).map_or("", |params| params.kind);
        let slots = modifications::cosmetic_slots(host, kind, page.upgrade_kind()).len();
        if slots > 0 {
            let entry = groups.entry(group).or_default();
            entry.0 += 1;
            entry.1 = entry.1.max(slots);
        }
    }
    let rows: String = groups
        .iter()
        .map(|(&group, &(items, slots))| {
            format!(
                "<tr><th>{}{}</th><td class=num>{items}</td><td class=num>{slots}</td></tr>",
                modifiers::glyph(IconSet::Group, &group.category().to_string()),
                escape(&modifiers::text(lang, group.name_key())),
            )
        })
        .collect();
    format!(
        "<section class=modifiers id=where><h2>{}</h2><div class=table-scroll>\
         <table class=\"artefact-table cosmetic-where\"><thead><tr><th>{}</th><th>{}</th><th>{}</th>\
         </tr></thead><tbody>{rows}</tbody></table></div></section>",
        escape(&lang.t("Where it goes")),
        escape(&lang.t("Item")),
        escape(&lang.t("Items that take it")),
        escape(&lang.t("At once")),
    )
}

/// The page's body in `lang`.
pub(crate) fn body(lang: Lang, page: Page) -> String {
    let code = lang.code();
    let cosmetics = all(page);
    let mut shelves: BTreeMap<Shelf, Vec<&Cosmetic>> = BTreeMap::new();
    for cosmetic in cosmetics {
        shelves.entry(cosmetic.shelf).or_default().push(cosmetic);
    }
    // The long shelves by name in the page's language. The factions' camouflages and the badges
    // stay in the dictionary's order (by faction, the rank badges before the factions').
    for shelf in [Shelf::Premium, Shelf::Flags, Shelf::Arms, Shelf::Drop] {
        if let Some(items) = shelves.get_mut(&shelf) {
            items.sort_by_cached_key(|cosmetic| name(lang, cosmetic.item).to_lowercase());
        }
    }

    let mut contents = String::new();
    let mut sections = String::new();
    for (&shelf, items) in &shelves {
        let anchor = format!("{shelf:?}").to_lowercase();
        let title = shelf_title(lang, shelf);
        let _ = write!(
            contents,
            "<li><a href=\"#{anchor}\">{}</a> <small>{}</small></li>",
            escape(&title),
            thousands(lang, items.len() as u32),
        );
        let cards: String = if shelf == Shelf::Given {
            let mut by_icon: BTreeMap<Option<Cell>, Vec<&Cosmetic>> = BTreeMap::new();
            for &cosmetic in items {
                by_icon.entry(cosmetic.cell).or_default().push(cosmetic);
            }
            by_icon
                .values()
                .map(|series| given_card(lang, page, series))
                .collect()
        } else {
            items
                .iter()
                .map(|cosmetic| card(lang, page, cosmetic))
                .collect()
        };
        let _ = write!(
            sections,
            "<section class=\"modifiers cosmetics\" id={anchor}><h2>{} <small>{}</small></h2>\
             <p class=wiki-note>{}</p><ul class=cosmetic-grid>{cards}</ul></section>",
            escape(&title),
            thousands(lang, items.len() as u32),
            escape(&shelf_line(lang, shelf)),
        );
    }

    let rules = [
        lang.t("Bought once, it is yours: put it on any number of your items at the same time."),
        lang.t("Sold back, it pays a quarter of its price and comes off every item."),
    ];
    let rules: String = rules
        .iter()
        .map(|line| format!("<li>{}</li>", escape(line)))
        .collect();
    let title = page.title(lang);
    format!(
        "<p class=breadcrumb><a href=\"/wiki?lang={code}\">{}</a> / {}</p>\
         <div class=wiki-heading><div><h1>{}</h1></div><span class=catalogue-count>{}</span></div>\
         <ul class=cosmetic-contents>{contents}</ul>{}\
         <section class=modifiers><h2>{}</h2><ul class=mode-rules>{rules}</ul></section>{sections}",
        escape(&lang.t("Wiki")),
        escape(&title),
        escape(&title),
        thousands(lang, cosmetics.len() as u32),
        where_it_goes(lang, page),
        escape(&lang.t("Rules")),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_cosmetic_has_its_icon_and_a_name_in_every_language() {
        for page in Page::ALL {
            let all = all(page);
            assert!(!all.is_empty(), "{page:?}");
            for cosmetic in all {
                let cell = cosmetic
                    .cell
                    .unwrap_or_else(|| panic!("{}: no ui_desc.icon", cosmetic.item.name));
                assert!(
                    icons::url(page.icons(), &cell.key()).is_some(),
                    "{}: no icon of cell {cell:?}",
                    cosmetic.item.name
                );
                for lang in Lang::ALL {
                    let name = name(lang, cosmetic.item);
                    assert!(
                        !name.is_empty() && !name.starts_with("st_"),
                        "{lang:?} {}: {name}",
                        cosmetic.item.name
                    );
                }
            }
        }
    }

    #[test]
    fn flags_and_coats_of_arms_have_shelves_of_their_own() {
        let decals = all(Page::Decals);
        let on = |shelf| decals.iter().filter(move |cosmetic| cosmetic.shelf == shelf);
        assert!(on(Shelf::Flags).count() > 0 && on(Shelf::Arms).count() > 0);
        for cosmetic in on(Shelf::Flags) {
            let name = name(Lang::En, cosmetic.item);
            assert!(name.starts_with("Flag"), "{}: {name}", cosmetic.item.name);
        }
        for cosmetic in on(Shelf::Arms) {
            let name = name(Lang::En, cosmetic.item);
            assert!(!name.starts_with("Flag"), "{}: {name}", cosmetic.item.name);
        }
        // None of them left on Premium, and no camouflage on these shelves.
        for cosmetic in on(Shelf::Premium) {
            assert_eq!(DecalRow::of(cosmetic.item), None, "{}", cosmetic.item.name);
        }
        assert!(all(Page::Camouflages)
            .iter()
            .all(|cosmetic| !matches!(cosmetic.shelf, Shelf::Flags | Shelf::Arms)));
    }

    #[test]
    fn the_ways_are_the_servers_own() {
        let dictionary = Dictionary::shared();
        for page in Page::ALL {
            for cosmetic in all(page) {
                let item = cosmetic.item;
                let params = item_params::item_params(item.dict_id).unwrap();
                let sold: Vec<Way> = shop_offers::price_list()
                    .iter()
                    .filter(|price| price.dict_id == item.dict_id)
                    .map(|price| Way::Sold {
                        seller: price.faction_id,
                        rank: price.level,
                        silver: price.cost,
                    })
                    .collect();
                for way in &sold {
                    assert!(cosmetic.ways.contains(way), "{}: {way:?}", item.name);
                }
                // A drop needs a drop weight, and a badge or shop camouflage has a seller.
                let dropped = cosmetic
                    .ways
                    .iter()
                    .any(|way| matches!(way, Way::Dropped(_)));
                assert_eq!(dropped, params.drop_weight > 0, "{}", item.name);
                if prices::is_badge(item, params) || prices::is_shop_camo(item, params) {
                    assert!(!sold.is_empty(), "{}", item.name);
                }
                if prices::is_not_sold(item) {
                    assert!(sold.is_empty(), "{}", item.name);
                }
                assert!(!cosmetic.ways.is_empty(), "{}", item.name);
            }
        }
        // The test data's: the stock drop camouflages and decals drop after a match and from a
        // drop card, never after a first one.
        let drop_decal = all(Page::Decals)
            .iter()
            .find(|cosmetic| cosmetic.item.name == "decals_3_4")
            .unwrap();
        assert_eq!(
            drop_decal.ways,
            [Way::Dropped(Table::Match), Way::Dropped(Table::DropCard)]
        );
        assert_eq!(drop_decal.shelf, Shelf::Drop);
        assert!(dictionary.item_named("decals_3_4").is_some());
    }

    #[test]
    fn badges_and_faction_camouflages_name_their_seller_and_rank() {
        // Scavengers veteran (decals_2_2): the Scavengers at rank 5 (shop.tsv).
        let veteran = all(Page::Decals)
            .iter()
            .find(|cosmetic| cosmetic.item.name == "decals_2_2")
            .unwrap();
        assert_eq!(veteran.shelf, Shelf::Badge);
        let [Way::Sold { seller, rank, .. }] = veteran.ways[..] else {
            panic!("{:?}", veteran.ways);
        };
        assert_eq!((seller, rank), (prices::faction::SCAVENGERS, 5));
        let line = ways(Lang::En, veteran);
        assert!(line.contains(&seller_name_en(seller, rank)), "{line}");

        let shop = all(Page::Camouflages)
            .iter()
            .filter(|cosmetic| cosmetic.shelf == Shelf::Faction)
            .collect::<Vec<_>>();
        assert!(!shop.is_empty());
        for cosmetic in shop {
            assert!(
                cosmetic
                    .ways
                    .iter()
                    .any(|way| matches!(way, Way::Sold { seller, .. } if *seller != 0)),
                "{}",
                cosmetic.item.name
            );
        }
    }

    fn seller_name_en(who: u8, rank: u8) -> String {
        escape(&seller(Lang::En, who, rank))
    }

    #[test]
    fn camouflages_and_decals_go_where_the_slots_are() {
        let camouflages = where_it_goes(Lang::En, Page::Camouflages);
        let decals = where_it_goes(Lang::En, Page::Decals);
        for (page, table) in [("camouflages", &camouflages), ("decals", &decals)] {
            // The test data's: stock armour (the weapons take them by the edit lists weapon-camo-1
            // and weapon-decals-1, which only the release's data has).
            for group in ["Body armor", "Pants"] {
                assert!(table.contains(group), "{page}: {group}: {table}");
            }
        }
        // Two decals on a vest or trousers, one camouflage (modifications::cosmetic_slots).
        assert!(decals.contains("</td><td class=num>2</td></tr>"));
        assert!(!camouflages.contains("</td><td class=num>2</td></tr>"));
    }

    #[test]
    fn every_language_has_both_pages() {
        for page in Page::ALL {
            for lang in Lang::ALL {
                let body = body(lang, page);
                assert!(body.contains("class=cosmetic-grid"), "{lang:?}");
                assert!(body.contains(&format!("/wiki?lang={}", lang.code())));
                assert!(!body.contains('⟦'));
                let cards = body.matches("<li class=\"cosmetic").count()
                    + body.matches("<li class=cosmetic ").count();
                let icons = body.matches("<img src=\"/wiki/icons/").count();
                assert!(cards >= 1, "{page:?} {lang:?}");
                assert_eq!(icons, cards, "{page:?} {lang:?}: an icon a card");
                for (at, _) in body.match_indices("/wiki/icons/") {
                    let path = body[at..].split_once("?v=").unwrap().0;
                    assert!(icons::file(path).is_some(), "{path}");
                }
            }
        }
    }

    #[test]
    fn a_number_decal_is_given_to_its_account() {
        assert_eq!(given().get("decals_num_0042"), Some(&42));
        assert_eq!(given().get("decals_num_0004"), None, "the test account's");
        assert_eq!(given().get("decals_num_0590"), None, "System's");
        assert_eq!(given().len(), usize::from(Number::LAST) - 2);
    }

    #[test]
    fn an_icon_is_its_configs_cell() {
        // The drop decal decals_3_4: [3, 4] of the decal atlas.
        let decal = all(Page::Decals)
            .iter()
            .find(|cosmetic| cosmetic.item.name == "decals_3_4")
            .unwrap();
        assert_eq!(decal.cell, Some(Cell { row: 3, column: 4 }));
        assert_eq!(decal.cell.unwrap().key(), "3_4");
    }
}
