//! What `srp.db` and the code name of the game's data, checked against the
//! server's game data at start: the kept state ([`Lacking`]) and the items,
//! ids and awards the lobby's rules name ([`check_code`]).
//!
//! The accounts keep items by their dict id, their modifications and
//! defects by id, the daily bonus's reward by dict id and quests by id; the
//! kept results screens keep the items they show. All of these are
//! definitions of the server's game data (`survarium::v026g::game_data`):
//! of `resources.db` or of one of its patches. One that the data lacks
//! means data made without a patch the items came from (or a bad build),
//! not an item to throw away: `srp` refuses to start and names them
//! ([`GameAccounts::with_repository`](super::GameAccounts::with_repository)),
//! and nothing of the accounts is changed. [`Account::repair`] never deletes
//! anything for an unknown definition.
//!
//! Not checked here: the dict id an upgrade record holds. A scope's record
//! holds a variant (`survarium::v026g::scopes`), an entry of the game data
//! that is no item anyone owns; a release may withdraw a pair or a variant.
//! Such a record is repaired, not refused: the load takes it off the item
//! and says so ([`super::items_rules::upgrades_taken_off`]), a kept results
//! screen loses it when restored ([`super::rewards::Results::restored`]),
//! and the wire never sends one whose dict id the data lacks
//! ([`super::wire::item_descr_at`]). The scope item a player owns is an
//! item like any other and stays checked.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use survarium::v026g::awards::Awards;
use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::game_data::GameDataError;
use survarium::v026g::lobby::ItemDescr;
use survarium::v026g::modifications::Modifications;
use survarium::v026g::quests::Quests;
use survarium::v026g::tables::{self, Named};

use super::account::Account;
use super::rewards::MatchReport;

/// How many accounts a line names before it says how many more.
const NAMED: usize = 5;

/// What the kept state names that the game data lacks: each id with where
/// it is named (`account <id>`, `report <match id>`).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Lacking {
    pub items: BTreeMap<u32, BTreeSet<String>>,
    pub modifications: BTreeMap<u32, BTreeSet<String>>,
    pub quests: BTreeMap<u32, BTreeSet<String>>,
}

fn note(map: &mut BTreeMap<u32, BTreeSet<String>>, id: u32, place: &str) {
    map.entry(id).or_default().insert(place.to_owned());
}

impl Lacking {
    pub fn is_empty(&self) -> bool {
        self.items.is_empty() && self.modifications.is_empty() && self.quests.is_empty()
    }

    fn item(&mut self, dict_id: u32, place: &str) {
        let known = u16::try_from(dict_id)
            .ok()
            .is_some_and(|dict_id| Dictionary::shared().item(dict_id).is_some());
        if !known {
            note(&mut self.items, dict_id, place);
        }
    }

    fn modification(&mut self, id: u32, place: &str) {
        let table = Modifications::shared();
        if table.modification(id).is_none() && table.defect(id).is_none() {
            note(&mut self.modifications, id, place);
        }
    }

    /// Everything of `account` that names the game's data.
    pub fn account(&mut self, account_id: u32, account: &Account) {
        let place = format!("account {account_id}");
        for item in &account.inventory {
            self.item(u32::from(item.dict_id), &place);
            for &(id, _) in &item.modifications {
                self.modification(id, &place);
            }
            let reroll = &item.reroll;
            if reroll.modification != 0 {
                self.modification(reroll.modification, &place);
            }
            if let Some((replaced, choices)) = reroll.offer {
                for id in std::iter::once(replaced).chain(choices) {
                    self.modification(id, &place);
                }
            }
        }
        let reward = account.extras.bonus.reward_dict;
        if reward != 0 {
            self.item(u32::from(reward), &place);
        }
        let quests = Quests::shared();
        let ids = account.quests.iter().map(|quest| quest.quest_id);
        for id in ids.chain(account.quest_cooldowns.iter().map(|&(id, _)| id)) {
            if quests.quest(id).is_none() {
                note(&mut self.quests, id, &place);
            }
        }
    }

    fn descr(&mut self, item: &ItemDescr, place: &str) {
        self.item(u32::from(item.dict_id), place);
        for modification in &item.modifications {
            self.modification(modification.id, place);
        }
    }

    /// Every item a kept results screen shows.
    pub fn report(&mut self, report: &MatchReport) {
        let place = format!("report {:#x}", report.match_id);
        for player in &report.players {
            let reward = &player.reward;
            if let Some(drop) = &reward.drop {
                self.item(u32::from(drop.dict_id), &place);
            }
            if let Some(given) = &reward.daily_reward {
                self.item(u32::from(given.dict_id), &place);
            }
            for &(dict_id, _) in &reward.supplies_used {
                self.item(u32::from(dict_id), &place);
            }
            for (_, rows, _) in reward.spent.lists() {
                for &(_, dict_id) in rows {
                    self.item(u32::from(dict_id), &place);
                }
            }
            if let Some(dropped) = &player.full.dropped {
                self.descr(dropped, &place);
            }
            for item in &player.body.items {
                self.descr(item, &place);
            }
        }
    }
}

/// `ids` of `what`, each with up to [`NAMED`] places.
fn list(
    f: &mut fmt::Formatter<'_>,
    what: &str,
    ids: &BTreeMap<u32, BTreeSet<String>>,
) -> fmt::Result {
    if ids.is_empty() {
        return Ok(());
    }
    write!(f, "; {what}")?;
    for (index, (id, places)) in ids.iter().enumerate() {
        let named: Vec<&str> = places.iter().take(NAMED).map(String::as_str).collect();
        let more = places.len().saturating_sub(NAMED);
        write!(
            f,
            "{} {id} ({}{})",
            if index == 0 { "" } else { "," },
            named.join(", "),
            if more > 0 {
                format!(" and {more} more")
            } else {
                String::new()
            }
        )?;
    }
    Ok(())
}

impl fmt::Display for Lacking {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the kept accounts name what the server's game data ({}) lacks",
            survarium::v026g::game_data::shared().origin
        )?;
        list(f, "item dict ids", &self.items)?;
        list(f, "modification ids", &self.modifications)?;
        list(f, "quest ids", &self.quests)?;
        write!(
            f,
            ". The data was made without a patch these came from (or is not the release's): \
             build it with every patch released (srp-game-data build) and start srp again. \
             Nothing was changed"
        )
    }
}

/// Every item this crate's code names (the game's files do not): the drop
/// card, the vouchers, buckshot, the grenade of the grenade quests, the
/// starter kit of a new account. See [`check_code`].
pub fn named_items() -> Vec<Named> {
    let mut named: Vec<Named> = vec![
        ("rewards::DROP_CARD", super::rewards::DROP_CARD),
        ("premium::PREMIUM_VOUCHER", super::premium::PREMIUM_VOUCHER),
        ("premium::RENAME_VOUCHER", super::premium::RENAME_VOUCHER),
        ("autobuy::BUCKSHOT", super::autobuy::BUCKSHOT),
        ("quests::GRENADE", super::quests::GRENADE),
    ];
    named.extend(super::tutorial::starter_names().map(|name| ("tutorial::starter kit", name)));
    named
}

/// What the code of the lobby's rules names of the game's data, all in
/// the data of this process ([`survarium::v026g::game_data::shared`]):
/// the items of [`named_items`], the dict ids of silver and gold the
/// rewards and the daily bonus give, and the tutorial's awards a new
/// account is given. Else an error that
/// lists every one it lacks: srp does not start (`survarium`'s own names
/// are checked by `game_data::init`, `tables::named_items`). Each is
/// otherwise looked up when used, and one the data lacks would be
/// skipped there without a word.
pub fn check_code() -> Result<(), GameDataError> {
    let dictionary = Dictionary::shared();
    let mut missing = tables::lacking(dictionary, &named_items());
    let ids = [
        ("rewards::GENERIC_MONEY", super::rewards::GENERIC_MONEY),
        ("rewards::PREMIUM_MONEY", super::rewards::PREMIUM_MONEY),
        ("bonus::GENERIC_MONEY", super::bonus::GENERIC_MONEY),
        ("bonus::PREMIUM_MONEY", super::bonus::PREMIUM_MONEY),
        ("bonus::FALLBACK", super::bonus::FALLBACK.0),
    ];
    for (place, dict_id) in ids {
        if dictionary.item(dict_id).is_none() {
            missing.push(format!("{place} dict id {dict_id}"));
        }
    }
    let awards = Awards::shared();
    for name in super::tutorial::AWARDS {
        if !awards.awards.iter().any(|(award, _)| award == name) {
            missing.push(format!("tutorial award {name}"));
        }
    }
    let table = Modifications::shared();
    for (_, modification) in super::tutorial::STARTER_MODIFICATIONS {
        if table.modification(modification).is_none() {
            missing.push(format!("tutorial::STARTER_MODIFICATIONS {modification}"));
        }
    }
    if missing.is_empty() {
        Ok(())
    } else {
        Err(tables::lacking_error(
            &survarium::v026g::game_data::shared().origin,
            &missing,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// All but the starter kit's knit cap, of a patch the test data
    /// predates: srp refuses such data. The released data has it
    /// (`tests/starter_kit.rs`).
    #[test]
    fn what_the_lobby_code_names_is_in_the_data() {
        let error = check_code().unwrap_err().to_string();
        assert!(
            error.contains("lacks what srp's code names: tutorial::starter kit novice_026_helmet."),
            "{error}"
        );
    }

    /// Data without the drop card or a voucher: named, with where.
    #[test]
    fn an_item_the_lobby_code_names_and_the_data_lacks_is_named() {
        let mut dictionary = Dictionary::shared().clone();
        dictionary.items.retain(|item| {
            item.name != super::super::rewards::DROP_CARD
                && item.name != super::super::premium::RENAME_VOUCHER
        });
        assert_eq!(
            tables::lacking(&dictionary, &named_items()),
            [
                "rewards::DROP_CARD drop_card",
                "premium::RENAME_VOUCHER rename_account_voucher",
                "tutorial::starter kit novice_026_helmet"
            ]
        );
    }
}
