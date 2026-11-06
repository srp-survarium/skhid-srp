//! What the server's bots wear: at a match's equipment level, the kits
//! of the highest-rated players at that level (the owner's of 2026-10-26:
//! the bots "available on all equipment levels, and their equipment based
//! on presets and top players"). `docs/notes/player_bots.md` §8 has the
//! rules in short.
//!
//! - **Whose.** Every account put in a match at least [`ACTIVE_MATCHES`]
//!   times (the launcher board's count, the only activity an account
//!   keeps), no test bot, ranked by rating (ties by account id).
//! - **Which kits.** Each of its three kits that can play now
//!   ([`rules::check_play`]) and each kit of its other presets. Of each, a
//!   bot takes a weapon of its role's category, its ammo and the armour
//!   (`loadouts::bot_gear::KitParts`), and makes its own kit of them with
//!   its own ammo counts and pockets (`bot_gear::bot_kit_of`); a kit that
//!   fails the bot kit rules (`bot_gear::kit_problems`) is left out. A
//!   kit's level is that bot kit's (`Loadout::gear_level`).
//! - **The top players.** For each level and role the [`DONORS`]
//!   highest-ranked accounts with such a kit. Their kits are the
//!   candidates, each weighted by how it is used: the kit an account plays
//!   (its active kit) counts [`PLAYED_WEIGHT`], any other 1, an account
//!   counting one kit once (its highest), summed over the accounts.
//! - **A bot's pick.** The `k`-th bot of a role in a match takes the
//!   candidates in smooth weighted round robin (the heaviest first), so the
//!   pick depends only on the table and `k`: no randomness.
//! - **Too few players.** A level with no candidate of a role takes the
//!   nearest level's (the lower of two as near); no candidate at any level
//!   (a new server), the role's fixed kit (`loadouts::bot_kits`).
//! - **The second kit** (a place's ammo lasts the match): the first's
//!   armour and pockets with the weapon and ammo of the backup role's pick
//!   of the same `k`, of the first kit's level or lower; else with the
//!   backup role's fixed weapon; else none.
//!
//! Nothing of an account reaches a bot but item ids: no name, no id, no
//! modification. The table is the lobby's, rebuilt at most every few
//! minutes (`lobby-server` `play`); a rebuild reads every account's kits
//! and presets once.

use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::loadouts::bot_gear::{KitParts, bot_kit_of};
use survarium::v026g::loadouts::{self, BotRole, Loadout, SlotItem};

use super::account::{Account, KIT_SLOTS};
use super::rules;

/// The equipment levels: 0 to 15 (`items_levels_modifiers`).
pub const LEVELS: u8 = 16;

/// The accounts whose kits a level's bots of a role take: the
/// highest-ranked this many with a kit of the role at the level [choice].
pub const DONORS: usize = 10;

/// An account counts once put in this many matches [choice: fewer is a
/// player who tried the game; the board counts one].
pub const ACTIVE_MATCHES: u32 = 5;

/// What the kit an account plays counts for against its other kits and
/// presets [choice].
pub const PLAYED_WEIGHT: u32 = 2;

/// A kit a level's bots of a role may wear: the parts taken of a player's
/// kit, the bot's kit made of them and how much the top players use it.
#[derive(Clone, Debug, PartialEq)]
struct Candidate {
    parts: KitParts,
    kit: Loadout,
    weight: u32,
}

/// A level's candidates of a role, the heaviest first (ties by parts), and
/// the accounts they are of.
#[derive(Clone, Debug, Default, PartialEq)]
struct RoleKits {
    candidates: Vec<Candidate>,
    players: usize,
}

/// Where a bot's first kit is from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KitSource {
    /// The top players' kits of `level` (the match's, or the nearest with
    /// any of the role): `kits` candidates of `players` accounts.
    Players {
        level: u8,
        players: usize,
        kits: usize,
    },
    /// The role's fixed kit: no player has a kit of the role.
    Fixed,
}

impl std::fmt::Display for KitSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KitSource::Players {
                level,
                players,
                kits,
            } => write!(
                f,
                "level {level} players' kits ({kits} kits of {players} players)"
            ),
            KitSource::Fixed => f.write_str("the role's fixed kit (no player's kit of the role)"),
        }
    }
}

/// What one bot wears.
#[derive(Clone, Debug, PartialEq)]
pub struct BotKit {
    pub first: Loadout,
    /// The backup weapon's kit; none when no backup weapon makes a valid
    /// kit of the first's level or lower.
    pub second: Option<Loadout>,
    pub source: KitSource,
}

impl BotKit {
    /// Its kits, first first.
    pub fn kits(&self) -> Vec<&Loadout> {
        std::iter::once(&self.first)
            .chain(self.second.as_ref())
            .collect()
    }

    /// Its equipment level: its kits' highest (the client's account rule).
    pub fn level(&self) -> u8 {
        self.kits()
            .into_iter()
            .map(Loadout::gear_level)
            .max()
            .unwrap_or(0)
    }
}

/// The bots' kits of every level and role, from the accounts gathered.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BotKits {
    /// By level, then role ([`BotRole::ALL`]'s order).
    levels: Vec<[RoleKits; 4]>,
}

/// One account's bot kits while gathering.
struct Donor {
    rating: u16,
    account_id: u32,
    /// `(level, parts, kit, weight)`, each parts once (its highest
    /// weight).
    kits: Vec<(u8, Candidate)>,
}

/// The accounts' kits being gathered (`GameAccounts::bot_kits`).
#[derive(Default)]
pub struct Gathering {
    donors: Vec<Donor>,
}

impl Gathering {
    /// Takes `account`'s kits and presets if it counts ([`ACTIVE_MATCHES`]).
    pub fn add(&mut self, account_id: u32, account: &Account, now: u64) {
        let played: u32 = account
            .map_plays
            .iter()
            .fold(0_u32, |sum, &(_, plays)| sum.saturating_add(plays));
        if played < ACTIVE_MATCHES {
            return;
        }
        let mut kits: Vec<(u8, Candidate)> = Vec::new();
        let mut take = |loadout: Loadout, weight: u32| {
            for parts in KitParts::of_kit(&loadout) {
                let Ok(kit) = bot_kit_of(&parts.armour, parts.weapon, parts.ammo, parts.role)
                else {
                    continue;
                };
                match kits.iter_mut().find(|(_, each)| each.parts == parts) {
                    Some((_, kept)) => kept.weight = kept.weight.max(weight),
                    None => kits.push((kit.gear_level(), Candidate { parts, kit, weight })),
                }
            }
        };
        for kit in &account.kits {
            if rules::check_play(account, kit.id, now).is_err() {
                continue;
            }
            let weight = if kit.id == account.active_kit {
                PLAYED_WEIGHT
            } else {
                1
            };
            take(kit_loadout(account, &kit.slots), weight);
        }
        for preset in account.presets.iter().flatten() {
            for slots in &preset.kits {
                take(preset_loadout(slots), 1);
            }
        }
        if !kits.is_empty() {
            self.donors.push(Donor {
                rating: account.rating,
                account_id,
                kits,
            });
        }
    }

    /// The table: for each level and role the candidates of the
    /// [`DONORS`] highest-ranked accounts with any.
    pub fn finish(mut self) -> BotKits {
        self.donors.sort_by(|a, b| {
            b.rating
                .cmp(&a.rating)
                .then(a.account_id.cmp(&b.account_id))
        });
        let mut levels: Vec<[RoleKits; 4]> = (0..LEVELS).map(|_| Default::default()).collect();
        for (level, roles) in levels.iter_mut().enumerate() {
            for (role, kits) in BotRole::ALL.into_iter().zip(roles.iter_mut()) {
                let of = |candidate: &&(u8, Candidate)| {
                    usize::from(candidate.0) == level && candidate.1.parts.role == role
                };
                for donor in self
                    .donors
                    .iter()
                    .filter(|donor| donor.kits.iter().any(|each| of(&each)))
                    .take(DONORS)
                {
                    kits.players += 1;
                    for (_, candidate) in donor.kits.iter().filter(of) {
                        match kits
                            .candidates
                            .iter_mut()
                            .find(|each| each.parts == candidate.parts)
                        {
                            Some(kept) => kept.weight += candidate.weight,
                            None => kits.candidates.push(candidate.clone()),
                        }
                    }
                }
                kits.candidates
                    .sort_by(|a, b| b.weight.cmp(&a.weight).then(a.parts.cmp(&b.parts)));
            }
        }
        BotKits { levels }
    }
}

/// An account's kit as a loadout: what each slot holds, a stack with the
/// units it takes (none when the stack is used up).
fn kit_loadout(account: &Account, slots: &[Option<super::account::KitSlot>; KIT_SLOTS]) -> Loadout {
    let dictionary = Dictionary::shared();
    let items = slots
        .iter()
        .enumerate()
        .filter_map(|(slot, held)| {
            let held = held.as_ref()?;
            let item = account.item(held.item_id)?;
            let count = if dictionary.is_stack(item.dict_id) {
                held.count.min(item.condition_or_stack)
            } else {
                1
            };
            Some(SlotItem {
                slot: slot as u8,
                dict_id: item.dict_id,
                count,
            })
        })
        .collect();
    Loadout { items }
}

/// A preset's kit as a loadout: the items it names, as it names them.
fn preset_loadout(slots: &[Option<super::presets::PresetSlot>; KIT_SLOTS]) -> Loadout {
    let items = slots
        .iter()
        .enumerate()
        .filter_map(|(slot, held)| {
            let held = held.as_ref()?;
            Some(SlotItem {
                slot: slot as u8,
                dict_id: held.dict_id,
                count: held.count,
            })
        })
        .collect();
    Loadout { items }
}

/// Of `weights`, the one the `k`-th pick (0 on) of a smooth weighted round
/// robin takes: each pick adds every weight to its credit and takes the
/// highest credit (the first of two alike), less the sum. The first pick
/// is the heaviest, and any run of picks is as near the weights as it can
/// be.
fn nth_weighted(weights: &[u32], k: usize) -> usize {
    let total: i64 = weights.iter().map(|&weight| i64::from(weight)).sum();
    let mut credit = vec![0_i64; weights.len()];
    let mut pick = 0;
    for _ in 0..=k {
        for (credit, &weight) in credit.iter_mut().zip(weights) {
            *credit += i64::from(weight);
        }
        pick = (0..credit.len()).fold(0, |best, i| if credit[i] > credit[best] { i } else { best });
        credit[pick] -= total;
    }
    pick
}

impl BotKits {
    fn role_kits(&self, level: u8, role: BotRole) -> Option<&RoleKits> {
        let index = BotRole::ALL.iter().position(|&each| each == role)?;
        self.levels
            .get(usize::from(level))
            .map(|roles| &roles[index])
    }

    /// The level nearest `level` with a candidate of `role` (the lower of
    /// two as near), among `levels`.
    fn nearest(&self, level: u8, role: BotRole, levels: impl Iterator<Item = u8>) -> Option<u8> {
        levels
            .filter(|&each| {
                self.role_kits(each, role)
                    .is_some_and(|kits| !kits.candidates.is_empty())
            })
            .min_by_key(|&each| (each.abs_diff(level), each))
    }

    /// The `k`-th pick of `role` at `level` (nearest with any), and where
    /// it is from.
    fn pick(
        &self,
        level: u8,
        role: BotRole,
        k: usize,
        levels: impl Iterator<Item = u8>,
    ) -> Option<(&Candidate, KitSource)> {
        let level = self.nearest(level, role, levels)?;
        let kits = self.role_kits(level, role)?;
        let weights: Vec<u32> = kits.candidates.iter().map(|each| each.weight).collect();
        let candidate = &kits.candidates[nth_weighted(&weights, k)];
        Some((
            candidate,
            KitSource::Players {
                level,
                players: kits.players,
                kits: kits.candidates.len(),
            },
        ))
    }

    /// What the `k`-th bot (0 on) of `role` in a match of equipment level
    /// `level` wears: see the module.
    pub fn bot_kit(&self, level: u8, role: BotRole, k: usize) -> BotKit {
        let Some((candidate, source)) = self.pick(level, role, k, 0..LEVELS) else {
            let [first, second] = loadouts::bot_kits(role);
            return BotKit {
                first: first.clone(),
                second: Some(second.clone()),
                source: KitSource::Fixed,
            };
        };
        let first_level = candidate.kit.gear_level();
        let backup = role.backup();
        let from_players = self
            .pick(first_level, backup, k, 0..=first_level)
            .map(|(each, _)| (each.parts.weapon, each.parts.ammo));
        let fixed = fixed_weapon(backup);
        let second = [from_players, fixed]
            .into_iter()
            .flatten()
            .filter_map(|(weapon, ammo)| {
                bot_kit_of(&candidate.parts.armour, weapon, ammo, backup).ok()
            })
            .find(|kit| kit.gear_level() <= first_level);
        BotKit {
            first: candidate.kit.clone(),
            second,
            source,
        }
    }

    /// How many accounts' kits each level has of each role: the log's.
    pub fn summary(&self) -> String {
        self.levels
            .iter()
            .enumerate()
            .filter(|(_, roles)| roles.iter().any(|kits| kits.players > 0))
            .map(|(level, roles)| {
                let counts: Vec<String> = BotRole::ALL
                    .iter()
                    .zip(roles)
                    .map(|(role, kits)| format!("{role} {}", kits.players))
                    .collect();
                format!("level {level}: {}", counts.join(", "))
            })
            .collect::<Vec<_>>()
            .join("; ")
    }
}

/// The weapon and ammo of `role`'s fixed kit.
fn fixed_weapon(role: BotRole) -> Option<(u16, u16)> {
    let kit = loadouts::bot_kit(role);
    Some((
        kit.slot(7)?.dict_id,
        kit.slot(loadouts::BOT_AMMO_SLOTS[0])?.dict_id,
    ))
}

/// An item's dictionary name, for the log.
fn item_name(dict_id: u16) -> String {
    Dictionary::shared()
        .item(dict_id)
        .map_or_else(|| format!("#{dict_id}"), |entry| entry.name.clone())
}

/// A kit's items as names for the log, in slot order: `name`, `name xN`
/// for a stack; its ammo once, as the rounds of every ammo slot.
pub fn describe(kit: &Loadout) -> String {
    let dictionary = Dictionary::shared();
    let mut items: Vec<&SlotItem> = kit.items.iter().collect();
    items.sort_by_key(|item| item.slot);
    let mut said: Vec<String> = Vec::new();
    for item in items {
        if loadouts::BOT_AMMO_SLOTS.contains(&item.slot) {
            if item.slot == loadouts::BOT_AMMO_SLOTS[0] {
                said.push(describe_ammo(kit, item.dict_id));
            }
            continue;
        }
        let name = item_name(item.dict_id);
        said.push(if dictionary.is_stack(item.dict_id) {
            format!("{name} x{}", item.count)
        } else {
            name
        });
    }
    said.join(", ")
}

/// The rounds of `ammo` in a kit's ammo slots: `name xN`.
fn describe_ammo(kit: &Loadout, ammo: u16) -> String {
    let rounds: u32 = loadouts::BOT_AMMO_SLOTS
        .iter()
        .filter_map(|&slot| kit.slot(slot))
        .filter(|item| item.dict_id == ammo)
        .map(|item| item.count)
        .sum();
    format!("{} x{rounds}", item_name(ammo))
}

/// A kit's weapon 1 and its ammo for the log.
pub fn describe_weapon(kit: &Loadout) -> String {
    match (kit.slot(7), kit.slot(loadouts::BOT_AMMO_SLOTS[0])) {
        (Some(weapon), Some(ammo)) => format!(
            "{} with {}",
            item_name(weapon.dict_id),
            describe_ammo(kit, ammo.dict_id)
        ),
        (Some(weapon), None) => item_name(weapon.dict_id),
        _ => "no weapon".to_owned(),
    }
}

#[cfg(test)]
mod tests;
