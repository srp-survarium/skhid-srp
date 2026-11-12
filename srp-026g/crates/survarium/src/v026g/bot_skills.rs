//! The skills of the server's own bots ("Player vs Bots"), by tier: one
//! table, [`BOT_SKILL_TIERS`], keyed by the gear level of a bot's kits
//! (the matchmaking's measure). `docs/notes/player_bots_skills.md` has the
//! reasoning and the numbers.
//!
//! A bot's skills reach the match the way a player's do: summed into the
//! 68 player modifiers of each of its kits ([`SkillsTree::modifiers`],
//! `static_info.md` §3.1), which every client simulates from.
//!
//! **What carrying does** (the client, PROVEN in the exe unless marked;
//! `default.player`'s `stamina_params`):
//! - The capacity is `max_carried_weight` (30 kg) plus modifier 5
//!   `max_carried_weight`, which adds (table 0x927E08): skill 209 gives 2 kg
//!   a level, 10 kg at its five; some modifications add or take kilograms
//!   (1120.. +1..+4, the defect 4080 -3), counted through the item's nodes.
//! - From `min_carried_weight` (20 kg) to the capacity the weight costs
//!   `t = clamp((w - 20) / (capacity - 20), 0, 1)` (0x329B20): movement
//!   speed `-0.2 × t` (`max_carried_weight_movement_speed_modifier`, scaled
//!   by `1 + overweight_penalty`: skill 202 takes 5 % of it a level, a
//!   quarter at five; skill 214 removes it) and stamina spent `× (1 + 1.5 ×
//!   t)` (`max_carried_weight_spending_speed_modifier`).
//! - At or over the capacity the player is overburdened: no sprint
//!   (`player_stamina::is_overburdened` 0x329C60, read where a sprint is
//!   decided, 0x2DBF89), unless modifier 31 `can_sprint` (skill 213) is set.
//!
//! [`movement_level`] is the speed factor that follows (with skill 208's
//! movement speed), which the match server's bots take as a body's speed
//! level before a world has shown it.

use super::dictionary::Dictionary;
use super::item_params;
use super::loadouts::{Loadout, default_modifier};
use super::match_msgs::static_info::MatchProfile;
use super::modifications::{Modifications, Modifier};
use super::skills::{MODIFIERS, SkillError, SkillsTree, modifier_index};

/// The player modifier `max_carried_weight` (index 5 of the 68).
pub const MAX_CARRIED_WEIGHT: usize = 5;

/// A build as it is wanted: the skills in order of want and the points to spend on them. It names no rank count, row or
/// threshold: those are the live tree's, and [`resolve`] spends the points against it when the server starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SkillIntent {
    /// The skills, the most wanted first. Each takes as many points as its ranks and the points left allow; one whose row is
    /// still locked is opened first with the skills after it in this list that sit in the rows above it, no more than its
    /// threshold needs.
    pub want: &'static [u16],
    /// The points to spend.
    pub points: u8,
}

/// The skills of the bots whose kits' gear level is at most
/// [`BotSkillTier::up_to_gear_level`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BotSkillTier {
    /// The highest gear level ([`gear_level`]) of the kits this tier is for.
    pub up_to_gear_level: u8,
    /// What the tier wants ([`resolve`] makes it a set the live tree allows).
    pub intent: SkillIntent,
}

/// The bots' skills by tier, lowest first; kits above every tier's
/// [`BotSkillTier::up_to_gear_level`] get the last.
///
/// Each tier is the build of the best players of the 0.26g server at that
/// gear level (the ten best by the hidden rating with 20 matches or more,
/// 2026-10-23; `player_bots_skills.md` §3): their skills by the points
/// they put in, at about their level. The carry path (a row-1 skill of the
/// survival tab, then 209: +10 kg) is in every tier: the owner's ask, and
/// what all ten of the 11-15 players have. In the lowest tier 201 comes
/// after 209: it is there to open 209's row (204 alone no longer does, since
/// the tree's change of 2026-11-09), and takes what is left. Left out, with
/// the reason in the note: 103 (the bots time a bolt's chambering by the
/// clip), 213 (two of ten). 102 (no speed in the client's aiming speed,
/// LIKELY) and 108 (the bots time a reload by the clip) are left out of
/// these rows and given only by the owner's kit ([`AMMO_POUCH`]), where
/// they are the way to skill 109.
#[rustfmt::skip]
pub const BOT_SKILL_TIERS: [BotSkillTier; 3] = [
    // Gear 1 to 5 (the starting roles): aim time, stamina regeneration,
    // the carry.
    BotSkillTier { up_to_gear_level: 5, intent: SkillIntent { want: &[101, 204, 209, 201], points: 15 } },
    // Gear 6 to 10: and stamina spent.
    BotSkillTier { up_to_gear_level: 10, intent: SkillIntent { want: &[101, 201, 204, 209], points: 20 } },
    // Gear 11 to 15: and movement speed and the overweight penalty.
    BotSkillTier { up_to_gear_level: 15, intent: SkillIntent { want: &[101, 201, 204, 209, 202, 208], points: 30 } },
];

/// What the owner's kit ([`new_kit`](super::loadouts::new_kit)) adds to
/// every tier's skills, 11 points, for **skill 109**, the extra ammo bag
/// (`ammo_bags` +1): a pouch of ammo more, one more magazine of rounds.
/// 109 is in row 4 of the aim branch and needs its threshold in the rows
/// above, which the tiers' 101 starts; the rest are the least that carry no
/// aim, damage or protection skill (the other rows are 105 breath hold, 106
/// and 107 dispersion):
/// - 102 moving while aiming: no speed (LIKELY, §5 of
///   `player_bots_skills.md`);
/// - 104 weapon swap speed: the bots carry one weapon;
/// - 108 reload speed: the client reloads faster; the bots time a
///   reload by the clip (`Gun::reload_ms`), so they wait a little longer
///   than needed unless the bots' upgrades are on (`SRP_BOT_UPGRADES`), where
///   the gun learns the factor (`guns::Factors`);
/// - 103 bolt cocking speed, last: only to open 109's row where the others
///   no longer reach its threshold (the tree of 2026-11-09 gives 102 fewer
///   ranks); the bots time a bolt by the clip, so they wait a little longer.
pub const AMMO_POUCH: SkillIntent = SkillIntent { want: &[102, 104, 108, 109, 103], points: 11 };

/// [A341] The medical block every tier adds (`SRP_BOT_MEDICAL_SKILLS`, on unless off), 15 points in the medical tab
/// (`scripts/content-edit/medical-skills-1.json`, the tab the players' data ships), whose thresholds count apart from the
/// aim and survival tabs, so the tiers' other rows and the tier a player meets do not change:
/// - 302 Fast Bandaging: a healing medicine's activation delay shorter (the client's `Skills::use_time` at
///   `medical.use_time` 0x33D486, XMM0 the activation delay, `sites_026g.tsv`; the rate a rank is the client patch's);
/// - 312 Medical Training (its row needs the threshold above): `drug_amount_modifier` (modifier 57), the drug's amounts up;
/// - 311 Metabolism: natural regeneration starts sooner (`Skills::regeneration_delay_ms`);
/// - 301 Potent Painkiller, last: only to open the second row where 302 alone no longer reaches its threshold (302 takes
///   fewer ranks since 2026-11-09); the client applies it, the bots' heal model does not read it.
///
/// The bots' heal model reads the first two from the kit's modifiers (`match-server` `bots/plays/heal.rs`
/// `DrugFactors`). Left out: 321 Adrenalin (a speed the bots' motion model does not know), 322 Additional Medicine and 341
/// Medic (the bots' pockets ignore skills; no bot carries a Larkspur).
pub const MEDICAL: SkillIntent = SkillIntent { want: &[302, 312, 311, 301], points: 15 };

/// Whether the bots take [`MEDICAL`]: `SRP_BOT_MEDICAL_SKILLS` on unless `0`/`off`/`false`, and the players' skills tree of
/// the game data in use has its skills (0.26g's own archive, the tests' data, has no medical tab: it came with a content
/// patch, as the live data has it). Read once a process.
pub fn medical_skills() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| {
        let asked = super::loadouts::on_unless_off(
            "SRP_BOT_MEDICAL_SKILLS",
            std::env::var("SRP_BOT_MEDICAL_SKILLS").ok().as_deref(),
        );
        let tree = SkillsTree::shared();
        let in_data = MEDICAL.want.iter().all(|&id| tree.skill(id).is_some());
        log::info!(
            "the bots' medical skills: SRP_BOT_MEDICAL_SKILLS {}, the game data's tree {} them",
            if asked { "on" } else { "off" },
            if in_data { "has" } else { "lacks" }
        );
        asked && in_data
    })
}

/// Why a wanted skill got fewer points than it has ranks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Short {
    /// The tree has no such skill.
    Unknown,
    /// The intent's points ran out.
    Points,
    /// Its row stayed locked: it needs `needs` points in the rows above, and the intent's skills there opened `unlocked`.
    Locked { needs: u32, unlocked: u32 },
    /// A skill that excludes it has points.
    Exclusive(u16),
}

/// A wanted skill that got less than its ranks, and why.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shortfall {
    pub skill: u16,
    pub got: u8,
    pub ranks: u8,
    pub why: Short,
}

/// An intent spent against a tree: the set, its level (the points spent) and what fell short.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedBuild {
    /// `(skill id, points)`, in the order the points went.
    pub skills: Vec<(u16, u8)>,
    /// The account level: the points spent.
    pub level: u8,
    /// The points the intents had and did not spend.
    pub unspent: u8,
    /// The wanted skills that got less than their ranks.
    pub short: Vec<Shortfall>,
    /// What the set gives ([`SkillsTree::modifiers`]).
    pub modifiers: [f32; MODIFIERS],
}

/// A resolved set the tree refuses (never, unless [`resolve`] is wrong): the bots do not start on it.
#[derive(Clone, Debug, PartialEq)]
pub struct BuildRefused {
    pub up_to_gear_level: u8,
    pub pouch: bool,
    pub skills: Vec<(u16, u8)>,
    pub error: SkillError,
}

impl std::fmt::Display for BuildRefused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "the bots' build for gear up to {} ({}) is {:?}, which the skills tree refuses: {}",
            self.up_to_gear_level,
            if self.pouch { "the owner's kit" } else { "no pouch" },
            self.skills,
            self.error
        )
    }
}

impl std::error::Error for BuildRefused {}

fn points_of(set: &[(u16, u8)], id: u16) -> u8 {
    set.iter().filter(|&&(other, _)| other == id).map(|&(_, points)| points).sum()
}

fn add_point(set: &mut Vec<(u16, u8)>, id: u16, points: u8) {
    match set.iter_mut().find(|(other, _)| *other == id) {
        Some((_, held)) => *held += points,
        None => set.push((id, points)),
    }
}

/// A held skill that excludes `skill` (either way round).
fn excluded_by(tree: &SkillsTree, set: &[(u16, u8)], skill: &super::skills::Skill) -> Option<u16> {
    set.iter().filter(|&&(_, points)| points > 0).map(|&(id, _)| id).find(|&id| {
        id != skill.id && (skill.antiskills.contains(&id) || tree.skill(id).is_some_and(|other| other.antiskills.contains(&skill.id)))
    })
}

/// The points the rows above `skill` unlock in `set` (the window's walk), when that is less than its threshold.
fn locked(tree: &SkillsTree, set: &[(u16, u8)], skill: &super::skills::Skill) -> Option<u32> {
    let unlocked = tree.unlocked_branch_points(skill.branch(), set);
    (skill.branch_points > unlocked).then_some(unlocked)
}

/// `set` with `skill`'s row opened by the wanted skills `want` that sit in the rows above it in its branch, a point at a time
/// to the first that can take one (in want order), spending at most `most`; `None` when they can not open it.
fn open_row(
    tree: &SkillsTree,
    set: &[(u16, u8)],
    skill: &super::skills::Skill,
    want: &[u16],
    most: u8,
) -> Option<(Vec<(u16, u8)>, u8)> {
    let mut trial = set.to_vec();
    let mut spent = 0;
    while locked(tree, &trial, skill).is_some() {
        if spent == most {
            return None;
        }
        let opener = want.iter().filter_map(|&id| tree.skill(id)).find(|other| {
            other.id != skill.id
                && other.branch() == skill.branch()
                && other.row < skill.row
                && points_of(&trial, other.id) < other.levels_amount
                && excluded_by(tree, &trial, other).is_none()
                && locked(tree, &trial, other).is_none()
        })?;
        add_point(&mut trial, opener.id, 1);
        spent += 1;
    }
    Some((trial, spent))
}

/// The intents `blocks`, each with its own points, spent in turn against `tree` (see [`SkillIntent`]); the set is checked by
/// [`SkillsTree::validate`] at its level.
pub fn resolve(tree: &SkillsTree, blocks: &[SkillIntent]) -> Result<ResolvedBuild, SkillError> {
    let mut set: Vec<(u16, u8)> = Vec::new();
    let mut why: Vec<(u16, Short)> = Vec::new();
    let mut unspent = 0u8;
    for block in blocks {
        let mut left = block.points;
        for &id in block.want {
            let Some(skill) = tree.skill(id) else {
                why.push((id, Short::Unknown));
                continue;
            };
            let held = points_of(&set, id);
            if held >= skill.levels_amount || left == 0 {
                continue;
            }
            if let Some(other) = excluded_by(tree, &set, skill) {
                why.push((id, Short::Exclusive(other)));
                continue;
            }
            if let Some(unlocked) = locked(tree, &set, skill) {
                // Keep a point for the skill itself.
                match open_row(tree, &set, skill, block.want, left - 1) {
                    Some((opened, spent)) => {
                        set = opened;
                        left -= spent;
                    }
                    None => {
                        why.push((id, Short::Locked { needs: skill.branch_points, unlocked }));
                        continue;
                    }
                }
            }
            let give = (skill.levels_amount - points_of(&set, id)).min(left);
            add_point(&mut set, id, give);
            left -= give;
        }
        unspent += left;
    }
    let level: u32 = set.iter().map(|&(_, points)| u32::from(points)).sum();
    let level = u8::try_from(level).map_err(|_| SkillError::OverLevel { spent: level, level: u8::MAX })?;
    tree.validate(level, &set)?;
    let mut short = Vec::new();
    for block in blocks {
        for &id in block.want {
            if short.iter().any(|s: &Shortfall| s.skill == id) {
                continue;
            }
            let ranks = tree.skill(id).map_or(0, |skill| skill.levels_amount);
            let got = points_of(&set, id);
            if tree.skill(id).is_some() && got >= ranks {
                continue;
            }
            let reason = why.iter().rev().find(|&&(other, _)| other == id).map_or(Short::Points, |&(_, reason)| reason);
            short.push(Shortfall { skill: id, got, ranks, why: reason });
        }
    }
    let modifiers = tree.modifiers(&set);
    Ok(ResolvedBuild { skills: set, level, unspent, short, modifiers })
}

/// Every tier's build without and with the owner's kit's pouch (and the medical block while it is on), resolved once against
/// the players' tree ([`SkillsTree::shared`]) and logged; the bots start only on these.
#[derive(Clone, Debug, PartialEq)]
pub struct BotBuilds {
    /// By [`BOT_SKILL_TIERS`]'s order: without the pouch, with it.
    builds: Vec<[ResolvedBuild; 2]>,
}

impl BotBuilds {
    /// The intents of each tier, with the pouch or without, and the medical block when `medical`.
    pub fn intents(tier: &BotSkillTier, pouch: bool, medical: bool) -> Vec<SkillIntent> {
        let mut blocks = vec![tier.intent];
        if pouch {
            blocks.push(AMMO_POUCH);
        }
        if medical {
            blocks.push(MEDICAL);
        }
        blocks
    }

    /// Every build against `tree`.
    pub fn resolve(tree: &SkillsTree, medical: bool) -> Result<Self, BuildRefused> {
        let builds = BOT_SKILL_TIERS
            .iter()
            .map(|tier| {
                let one = |pouch: bool| {
                    let intents = Self::intents(tier, pouch, medical);
                    resolve(tree, &intents).map_err(|error| BuildRefused {
                        up_to_gear_level: tier.up_to_gear_level,
                        pouch,
                        skills: intents.iter().flat_map(|intent| intent.want.iter().map(|&id| (id, 0))).collect(),
                        error,
                    })
                };
                Ok([one(false)?, one(true)?])
            })
            .collect::<Result<_, BuildRefused>>()?;
        Ok(Self { builds })
    }

    /// The players' tree's, resolved and logged once a process.
    pub fn shared() -> Result<&'static Self, &'static BuildRefused> {
        static SHARED: std::sync::OnceLock<Result<BotBuilds, BuildRefused>> = std::sync::OnceLock::new();
        SHARED
            .get_or_init(|| {
                let builds = Self::resolve(SkillsTree::shared(), medical_skills());
                match &builds {
                    Ok(builds) => builds.log(),
                    Err(refused) => log::error!("{refused}: the bots do not start"),
                }
                builds
            })
            .as_ref()
    }

    /// The build of `tier` with the pouch or without.
    pub fn of(&self, tier: &BotSkillTier, pouch: bool) -> &ResolvedBuild {
        let index = BOT_SKILL_TIERS.iter().position(|other| other == tier).expect("a tier of BOT_SKILL_TIERS");
        &self.builds[index][usize::from(pouch)]
    }

    /// One line a build: the tier, what was wanted, the set, its level, what fell short and why.
    pub fn log(&self) {
        for (tier, builds) in BOT_SKILL_TIERS.iter().zip(&self.builds) {
            for (pouch, build) in [false, true].into_iter().zip(builds) {
                log::info!(
                    "the bots' skills, gear up to {}{}: wanted {}; resolved {:?}, level {} ({} points unspent); short {}",
                    tier.up_to_gear_level,
                    if pouch { " with the owner's kit" } else { "" },
                    Self::intents(tier, pouch, medical_skills())
                        .iter()
                        .map(|intent| format!("{:?} for {}", intent.want, intent.points))
                        .collect::<Vec<_>>()
                        .join(" + "),
                    build.skills,
                    build.level,
                    build.unspent,
                    if build.short.is_empty() {
                        "none".to_owned()
                    } else {
                        build.short.iter().map(|s| format!("{} {}/{} ({:?})", s.skill, s.got, s.ranks, s.why)).collect::<Vec<_>>().join(", ")
                    },
                );
            }
        }
    }
}

/// The bots' builds ([`BotBuilds::shared`]); only called once the bots started, which they do only on builds the tree allows.
fn builds() -> &'static BotBuilds {
    BotBuilds::shared().expect("the bots' builds are refused by the skills tree; the bots are not started on them (BotBuilds)")
}

impl BotSkillTier {
    /// Its resolved build with the owner's kit `pouch` or without ([`BotBuilds`]).
    pub fn build(&self, pouch: bool) -> &'static ResolvedBuild {
        builds().of(self, pouch)
    }

    /// Its skills with the owner's kit `pouch` or without.
    pub fn skills_with(&self, pouch: bool) -> Vec<(u16, u8)> {
        self.build(pouch).skills.clone()
    }

    /// Its account level with the owner's kit `pouch` or without: the points its build spent.
    pub fn level_with(&self, pouch: bool) -> u8 {
        self.build(pouch).level
    }

    /// The player modifiers its skills give with the owner's kit `pouch`
    /// or without.
    pub fn modifiers_with(&self, pouch: bool) -> [f32; MODIFIERS] {
        self.build(pouch).modifiers
    }

    /// The tier of kits whose gear level is `gear_level`.
    pub fn for_gear_level(gear_level: u8) -> &'static BotSkillTier {
        BOT_SKILL_TIERS
            .iter()
            .find(|tier| gear_level <= tier.up_to_gear_level)
            .unwrap_or(&BOT_SKILL_TIERS[BOT_SKILL_TIERS.len() - 1])
    }

    /// The tier of a bot wearing `kits` ([`gear_level`]).
    pub fn for_kits(kits: &[&Loadout]) -> &'static BotSkillTier {
        Self::for_gear_level(gear_level(kits))
    }

    /// The player modifiers its skills give ([`Self::modifiers_with`],
    /// the owner's kit unless it is off).
    pub fn modifiers(&self) -> [f32; MODIFIERS] {
        self.modifiers_with(super::loadouts::new_kit())
    }
}

/// A kit's gear level as the matchmaking counts a player's
/// (`session::v026g::matchmaking::kit_level`): the highest
/// `match_making_level` of what it wears times its slot's weight, rounded.
pub fn kit_gear_level(kit: &Loadout) -> u8 {
    let weights = &Dictionary::shared().match_maker.slots_weight;
    let highest = kit
        .items
        .iter()
        .filter(|item| item.count > 0)
        .filter_map(|item| {
            let level = item_params::item_params(item.dict_id)?.match_making_level;
            let weight = weights.get(usize::from(item.slot)).copied().unwrap_or(1.0);
            Some(f32::from(level) * weight)
        })
        .fold(0.0_f32, f32::max);
    (highest + 0.5).floor().clamp(0.0, 255.0) as u8
}

/// The highest gear level of `kits` ([`kit_gear_level`]).
pub fn gear_level(kits: &[&Loadout]) -> u8 {
    kits.iter()
        .map(|kit| kit_gear_level(kit))
        .max()
        .unwrap_or(0)
}

/// What the type 1 (added) modifiers of `modifiers` add to `parameter`.
fn added(modifiers: &[Modifier], parameter: &str) -> f32 {
    modifiers
        .iter()
        .filter(|modifier| {
            modifier.kind == 1
                && modifier
                    .path
                    .strip_prefix("parameters/")
                    .is_some_and(|path| path == parameter)
        })
        .map(|modifier| modifier.value)
        .sum()
}

/// What an item adds to `parameter` (`max_carried_weight_modifier`,
/// `movement_speed_modifier`): its default modifications, which the client
/// adds from the item's config, and `modifications`, the `(id, defect)` it
/// carries on the wire.
pub fn item_added(dict_id: u16, modifications: &[(u32, bool)], parameter: &str) -> f32 {
    let table = Modifications::shared();
    let carried: f32 = modifications
        .iter()
        .map(|&(id, defect)| {
            if defect {
                table
                    .defect(id)
                    .map_or(0.0, |row| added(&row.modifiers, parameter))
            } else {
                table
                    .modification(id)
                    .map_or(0.0, |row| added(&row.modifiers, parameter))
            }
        })
        .sum();
    default_modifier(dict_id, parameter) + carried
}

/// What `item_weight` multiplies an item's weight by: `1 +` the sum of the
/// type 2 (multiplying) `parameters/weight` changes of its `modifications`
/// (the weight upgrades, -5 % to -15 %; the client's rule, as
/// [`Modifier::kind`]). The path was `weight` once, which no modification
/// has, so no weight upgrade ever lightened a bot.
pub fn item_weight(dict_id: u16, count: u32, modifications: &[(u32, bool)]) -> f32 {
    let table = Modifications::shared();
    let change: f32 = modifications
        .iter()
        .filter(|&&(_, defect)| !defect)
        .filter_map(|&(id, _)| table.modification(id))
        .flat_map(|row| &row.modifiers)
        .filter(|modifier| modifier.kind == 2 && modifier.path == "parameters/weight")
        .map(|modifier| modifier.value)
        .sum();
    item_params::carried_weight(dict_id, count) * (1.0 + change).max(0.0)
}

/// A slot item's modifications as the wire gives them, `(id, defect)`.
pub fn wire_mods(mods: &[u32]) -> Vec<(u32, bool)> {
    mods.iter().map(|&id| (id, false)).collect()
}

/// What a player with `modifiers` (a kit's skills) carries in `kit` before
/// being overburdened: `max_carried_weight` plus the skills' and the worn
/// items' default modifications' `max_carried_weight` and the carry
/// upgrades they wear (see the module).
pub fn carry_capacity(kit: &Loadout, modifiers: &[f32; MODIFIERS]) -> f32 {
    let items: f32 = kit
        .items
        .iter()
        .map(|item| {
            item_added(
                item.dict_id,
                &wire_mods(item.mods),
                "max_carried_weight_modifier",
            )
        })
        .sum();
    item_params::player().max_carried_weight + modifiers[MAX_CARRIED_WEIGHT] + items
}

/// The weight's share of its cost, 0 to 1: `t` of the module.
pub fn weight_cost(weight: f32, capacity: f32) -> f32 {
    let min = item_params::player().min_carried_weight;
    if capacity <= min {
        return if weight > min { 1.0 } else { 0.0 };
    }
    ((weight - min) / (capacity - min)).clamp(0.0, 1.0)
}

/// The body's speed factor (`kin::Kinematics::level_of` of the match
/// server's bots): `1 + movement_speed` of the skills and the items'
/// modifications (`items_movement`), plus the weight's node (see the
/// module). A weapon's own `movement_speed_modifier` (±0.05) is the
/// caller's to add to `items_movement`
/// ([`profile_movement_level_in`] does, with `weapon` set).
pub fn movement_level(
    weight: f32,
    capacity: f32,
    modifiers: &[f32; MODIFIERS],
    items_movement: f32,
) -> f32 {
    let index = |name| modifier_index(name).expect("a modifier of the 68");
    let carried = if modifiers[index("max_carried_weight_ultimate")] > 0.0 {
        0.0
    } else {
        item_params::player().max_carried_weight_movement_speed_modifier
            * (1.0 + modifiers[index("overweight_penalty")]).max(0.0)
            * weight_cost(weight, capacity)
    };
    (1.0 + modifiers[index("movement_speed")] + items_movement + carried).max(0.0)
}

/// [`movement_level`] of a match profile's `kit` (0 on): its items' weight
/// and modifications, its modifiers; the held weapon's own speed modifier
/// when the bots' upgrades are on ([`super::loadouts::upgrades_on`]). `None`
/// without that kit.
pub fn profile_movement_level(profile: &MatchProfile, kit: usize) -> Option<f32> {
    profile_movement_level_in(profile, kit, super::loadouts::upgrades_on())
}

/// The slot of weapon 1 in a profile's kit.
const WEAPON_SLOT: usize = 7;

/// [`profile_movement_level`], with the weapon in slot 7 adding its own
/// `parameters/movement_speed_modifier` (LIKELY: the client adds it to the
/// player modifier 4 while the weapon is held) if `weapon`.
pub fn profile_movement_level_in(profile: &MatchProfile, kit: usize, weapon: bool) -> Option<f32> {
    let kit = profile.kits.get(kit)?;
    let (mut weight, mut capacity_added, mut movement) = (0.0, 0.0, 0.0);
    for (slot, &(index, count)) in kit
        .slots
        .iter()
        .enumerate()
        .filter_map(|(slot, entry)| entry.as_ref().map(|entry| (slot, entry)))
    {
        let Some(item) = profile.items.get(usize::from(index)) else {
            continue;
        };
        if weapon && slot == WEAPON_SLOT {
            movement += super::weapons::weapon(item.dict_id).map_or(0.0, |w| w.movement_speed_modifier);
        }
        weight += item_weight(item.dict_id, u32::from(count), &item.modifications);
        capacity_added += item_added(
            item.dict_id,
            &item.modifications,
            "max_carried_weight_modifier",
        );
        movement += item_added(item.dict_id, &item.modifications, "movement_speed_modifier");
    }
    let capacity = item_params::player().max_carried_weight
        + kit.modifiers[MAX_CARRIED_WEIGHT]
        + capacity_added;
    Some(movement_level(weight, capacity, &kit.modifiers, movement))
}

/// [A331] The speed factor F of a match profile's `kit` (0 on) for each weapon its slots could have in hands, and with none: the
/// client's `max(0, 1 + S4)` (`apply_modifier` 0x2D5AE0, enum 4; [`movement_level`]) with the kit's items, skills and
/// modifications, the carried weight, and the held weapon's own
/// `parameters/movement_speed_modifier` (a node added in `weapon_core::initialize` 0x2E2C9C, removed in `deactivate` 0x2E2E9F; the
/// weight is the kit's items alone, as the client's own `w` on live worlds).
/// fx-review's A332 check: with the kit in use and the weapon in hands it gives the humans' measured level to about 0.01 (16 live
/// matches). The first is the weapon in slot 7 (the primary), if any; then the kit's other weapons; the last, none. `None`
/// without that kit.
pub fn profile_movement_candidates(profile: &MatchProfile, kit: usize) -> Option<Vec<f32>> {
    profile_kit_speeds(profile, kit).map(|speeds| speeds.candidates())
}

/// [A334] A kit's speed factor F with each of its weapons in hands, by the weapon's kit slot, and with none
/// ([`profile_kit_speeds`]).
#[derive(Clone, Debug, PartialEq)]
pub struct KitSpeeds {
    /// Each weapon's kit slot and F with it in hands, by ascending slot.
    pub held: Vec<(u8, f32)>,
    /// F with empty hands.
    pub none: f32,
}

impl KitSpeeds {
    /// F with the weapon of kit slot `slot` in hands; with none, or a slot that holds no weapon, empty hands.
    pub fn in_hands(&self, slot: Option<u8>) -> f32 {
        slot.and_then(|slot| self.held.iter().find(|&&(s, _)| s == slot)).map_or(self.none, |&(_, level)| level)
    }

    /// Every value, the primary (slot 7) in hands first, then the other weapons, then empty hands
    /// ([`profile_movement_candidates`]).
    pub fn candidates(&self) -> Vec<f32> {
        let mut held = self.held.clone();
        held.sort_by_key(|&(slot, _)| usize::from(slot) != WEAPON_SLOT);
        held.into_iter().map(|(_, level)| level).chain(std::iter::once(self.none)).collect()
    }
}

/// [A334] [`KitSpeeds`] of a match profile's `kit`: as [`profile_movement_candidates`], by slot. `None` without that kit.
pub fn profile_kit_speeds(profile: &MatchProfile, kit: usize) -> Option<KitSpeeds> {
    let entry = profile.kits.get(kit)?;
    // The kit's items alone: the client's own `w` read from live worlds equals it (fx-review A334c: median 0.000, 94-95 % within
    // 1.5 kg), with no loaded-round term.
    let (mut weight, mut capacity_added, mut movement) = (0.0, 0.0, 0.0);
    let mut weapons: Vec<(u8, f32)> = Vec::new();
    for (slot, &(index, count)) in entry
        .slots
        .iter()
        .enumerate()
        .filter_map(|(slot, entry)| entry.as_ref().map(|entry| (slot, entry)))
    {
        let Some(item) = profile.items.get(usize::from(index)) else {
            continue;
        };
        let modified = item_added(item.dict_id, &item.modifications, "movement_speed_modifier");
        if let Some(weapon) = super::weapons::weapon(item.dict_id) {
            // A weapon's speed term, its modifications' included, counts only while it is the hands item (fx-review A334: the node
            // of weapon+0x61C, the item's parameters with its modifications, added on draw 0x2E2C9C, removed on holster 0x2E2E9F).
            weapons.push((slot as u8, weapon.movement_speed_modifier + modified));
        } else {
            movement += modified;
        }
        weight += item_weight(item.dict_id, u32::from(count), &item.modifications);
        capacity_added += item_added(item.dict_id, &item.modifications, "max_carried_weight_modifier");
    }
    let capacity = item_params::player().max_carried_weight + entry.modifiers[MAX_CARRIED_WEIGHT] + capacity_added;
    // Skill 112 (modifier 62, `move_speed_weapon_penalty`) > 0: a weapon's term at or under 0 is not added (fx-review A334: the
    // same skip in 0x2E2C9C, 0x2E37E4 and 0x2E2E9F).
    let skips_penalty = entry.modifiers[modifier_index("move_speed_weapon_penalty").expect("a modifier of the 68")] > 0.0;
    let level = |held: f32| {
        let held = if skips_penalty && held <= 0.0 { 0.0 } else { held };
        movement_level(weight, capacity, &entry.modifiers, movement + held)
    };
    Some(KitSpeeds { held: weapons.iter().map(|&(slot, held)| (slot, level(held))).collect(), none: level(0.0) })
}

/// The modifiers of a bot wearing `kits`: its tier's skills.
pub fn bot_modifiers(kits: &[&Loadout]) -> [f32; MODIFIERS] {
    BotSkillTier::for_kits(kits).modifiers()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::loadouts::{BotEquipment, BotMask, BotRole, BotTier, KitSwitches, RusherKit, bot_kits, bot_kits_in};
    use crate::v026g::skills::SkillError;

    /// The rusher kits of `tier` the game data in use can make.
    fn made_kits(tier: BotTier) -> impl Iterator<Item = RusherKit> {
        tier.rusher_kits()
            .iter()
            .copied()
            .filter(move |&kit| kit.worn_in(tier) == kit)
    }

    fn index(name: &str) -> usize {
        modifier_index(name).unwrap()
    }

    /// The weight upgrades lighten what they are on (the path was `weight`,
    /// which no modification has): gear 1061 and a weapon's 2061.
    #[test]
    fn a_weight_upgrade_lightens_the_piece() {
        let dictionary = Dictionary::shared();
        let table = Modifications::shared();
        for (name, id) in [("edge_torso_2", 1061), ("ak_74u", 2061)] {
            let dict_id = dictionary.item_named(name).unwrap().dict_id;
            let value = table.modification(id).unwrap().modifiers[0].value;
            assert!(value < 0.0, "{id}");
            let plain = item_weight(dict_id, 1, &[]);
            let upgraded = item_weight(dict_id, 1, &[(id, false)]);
            assert!(plain > 0.0, "{name}");
            assert!((upgraded - plain * (1.0 + value)).abs() < 1e-5, "{name}: {plain} {upgraded}");
        }
    }

    /// The held weapon's own speed modifier moves a bot's speed level, only
    /// when asked for.
    #[test]
    fn the_held_weapons_own_speed_modifier_moves_the_level() {
        // The old kit: these rules are those of the kit before the owner's choices, which are the defaults now.
        let _kit = KitSwitches::OFF.scope();
        use crate::v026g::loadouts::RusherKits;
        use crate::v026g::match_msgs::static_info::MatchProfile;
        let equipment = BotEquipment::of(BotTier::Low, RusherKits::default());
        let kits = bot_kits(equipment, BotRole::Flanker);
        let weapon = crate::v026g::weapons::weapon(kits[0].slot(7).unwrap().dict_id).unwrap();
        assert_ne!(weapon.movement_speed_modifier, 0.0, "{}", weapon.name);
        let profile = MatchProfile::from_loadouts(
            "Bot-1",
            0,
            &[kits[0]],
            [0.0; MODIFIERS],
            Dictionary::shared(),
            1,
        );
        let off = profile_movement_level_in(&profile, 0, false).unwrap();
        let on = profile_movement_level_in(&profile, 0, true).unwrap();
        assert!((on - off - weapon.movement_speed_modifier).abs() < 1e-5, "{off} {on}");
        // Off by the switch: the plain level.
        assert_eq!(profile_movement_level(&profile, 0), Some(off));
    }

    /// [A331] The kit's candidates: the primary in hands first, the plain level last, the weight the kit's items alone (A334c).
    #[test]
    fn the_candidates_have_the_primary_first_and_the_plain_level_last() {
        let _kit = KitSwitches::OFF.scope();
        use crate::v026g::loadouts::RusherKits;
        use crate::v026g::match_msgs::static_info::MatchProfile;
        let equipment = BotEquipment::of(BotTier::Low, RusherKits::default());
        let kits = bot_kits(equipment, BotRole::Flanker);
        let weapon = crate::v026g::weapons::weapon(kits[0].slot(7).unwrap().dict_id).unwrap();
        let profile = MatchProfile::from_loadouts("Bot-1", 0, &[kits[0]], [0.0; MODIFIERS], Dictionary::shared(), 1);
        let speeds = profile_kit_speeds(&profile, 0).unwrap();
        let candidates = speeds.candidates();
        assert_eq!(candidates, profile_movement_candidates(&profile, 0).unwrap());
        assert!(candidates.len() >= 2, "{candidates:?}");
        let plain = *candidates.last().unwrap();
        assert!((candidates[0] - plain - weapon.movement_speed_modifier).abs() < 1e-5, "the primary's own modifier: {candidates:?}");
        assert!((speeds.in_hands(Some(7)) - candidates[0]).abs() < 1e-6 && speeds.in_hands(None) == plain);
        // The plain level is the kit's level with no weapon in hands (the kit's weapons carry no modifications here).
        let without = profile_movement_level_in(&profile, 0, false).unwrap();
        assert!((plain - without).abs() < 1e-5, "{plain} {without}");
        assert_eq!(profile_movement_candidates(&profile, 1), None);
    }

    /// Every tier and role, with each rusher kit the data can make, the
    /// owner's kit and the upgrades off and on.
    fn every_bot() -> Vec<(BotEquipment, BotRole, bool, bool)> {
        let mut all = Vec::new();
        for new in [false, true] {
            for upgrades in [false, true] {
                for tier in BotTier::ALL {
                    for rusher in made_kits(tier) {
                        let equipment = BotEquipment { tier, rusher, mask: BotMask::Table };
                        for role in BotRole::ALL {
                            all.push((equipment, role, new, upgrades));
                        }
                    }
                }
            }
        }
        all
    }

    /// What every resolved build must be on any tree: a set the skills window allows at its level (and no less), one a
    /// player's saved set would keep ([`SkillsTree::settle`]), the higher tiers keeping what the lower have, the carry path
    /// (209) at all its ranks, the pouch's 109 and (while it is on) the medical block's three skills at all theirs, no wanted
    /// row left locked, and the skills left out absent.
    fn the_builds_hold(tree: &SkillsTree, medical: bool) -> BotBuilds {
        let builds = BotBuilds::resolve(tree, medical).expect("the tree allows every build");
        let ranks = |id: u16| tree.skill(id).map_or(0, |skill| skill.levels_amount);
        let mut last: Option<&BotSkillTier> = None;
        for tier in &BOT_SKILL_TIERS {
            for pouch in [false, true] {
                let build = builds.of(tier, pouch);
                let spent: u32 = build.skills.iter().map(|&(_, points)| u32::from(points)).sum();
                assert_eq!(spent, u32::from(build.level), "{tier:?} {pouch}");
                assert_eq!(tree.validate(build.level, &build.skills), Ok(()), "{tier:?} {pouch}");
                assert_eq!(
                    tree.validate(build.level - 1, &build.skills),
                    Err(SkillError::OverLevel { spent, level: build.level - 1 }),
                    "{tier:?} {pouch}"
                );
                assert_eq!(tree.settle(build.level, &build.skills), build.skills, "{tier:?} {pouch}");
                assert_eq!(build.modifiers, tree.modifiers(&build.skills));
                let wanted: u32 =
                    BotBuilds::intents(tier, pouch, medical).iter().map(|intent| u32::from(intent.points)).sum();
                assert_eq!(spent + u32::from(build.unspent), wanted, "{tier:?} {pouch}");
                // Every wanted row opened: what fell short, fell short of points.
                assert!(build.short.iter().all(|short| short.why == Short::Points), "{tier:?} {pouch}: {:?}", build.short);
                // The carry path at its most.
                assert_eq!(points_of(&build.skills, 209), ranks(209), "{tier:?} {pouch}");
                assert_eq!(points_of(&build.skills, 109), if pouch { ranks(109) } else { 0 }, "{tier:?} {pouch}");
                for id in [302, 312, 311] {
                    assert_eq!(points_of(&build.skills, id), if medical { ranks(id) } else { 0 }, "{tier:?} {pouch} {id}");
                }
                // None of the skills left out (see the table); 102, 103 and 108 only with the pouch.
                for left_out in [213, 214] {
                    assert_eq!(points_of(&build.skills, left_out), 0, "{left_out}");
                }
                if !pouch {
                    for left_out in [102, 103, 108] {
                        assert_eq!(points_of(&build.skills, left_out), 0, "{left_out}");
                    }
                }
            }
            // Higher tiers keep what the lower had.
            if let Some(lower) = last {
                assert!(tier.up_to_gear_level > lower.up_to_gear_level);
                for pouch in [false, true] {
                    let (low, high) = (builds.of(lower, pouch), builds.of(tier, pouch));
                    assert!(high.level >= low.level, "{tier:?} {pouch}");
                    for &(id, points) in &low.skills {
                        assert!(points_of(&high.skills, id) >= points, "{tier:?} {pouch}: {id}");
                    }
                }
            }
            last = Some(tier);
            // The pouch changes only what its own skills set: the extra bag, reloads, weapon swaps, speed while aiming, the
            // bolt; no aim, damage or protection.
            let (off, on) = (builds.of(tier, false).modifiers, builds.of(tier, true).modifiers);
            let pouch_sets: Vec<usize> = AMMO_POUCH
                .want
                .iter()
                .filter_map(|&id| tree.skill(id))
                .flat_map(|skill| skill.levels.iter().flatten().map(|&(modifier, _)| modifier))
                .collect();
            for i in (0..MODIFIERS).filter(|&i| (off[i] - on[i]).abs() > 1e-6) {
                assert!(pouch_sets.contains(&i), "{tier:?}: {}", super::super::skills::MODIFIER_NAMES[i]);
            }
            assert_eq!(on[index("ammo_bags")], 1.0);
            assert_eq!(off[index("ammo_bags")], 0.0);
        }
        // The tiers by the kits' gear level.
        let tier = |gear| BotSkillTier::for_gear_level(gear).up_to_gear_level;
        assert_eq!([tier(0), tier(5), tier(6), tier(10), tier(11), tier(15), tier(16)], [5, 5, 10, 10, 15, 15, 15]);
        builds
    }

    /// On the tests' tree (0.26g's own archive: five ranks a stock skill, no medical tab) the intents resolve to the sets the
    /// tiers listed by hand until the tree changed, so nothing changes there.
    #[test]
    fn every_tier_is_a_set_a_player_could_have() {
        let tree = SkillsTree::shared();
        let builds = the_builds_hold(tree, false);
        let sets = |pouch: bool| BOT_SKILL_TIERS.iter().map(|tier| builds.of(tier, pouch).skills.clone()).collect::<Vec<_>>();
        assert_eq!(
            sets(false),
            [
                vec![(101, 5), (204, 5), (209, 5)],
                vec![(101, 5), (201, 5), (204, 5), (209, 5)],
                vec![(101, 5), (201, 5), (204, 5), (209, 5), (202, 5), (208, 5)],
            ]
        );
        let pouch = [(102, 5), (104, 2), (108, 3), (109, 1)];
        for (with, without) in sets(true).iter().zip(sets(false)) {
            assert_eq!(*with, [without.as_slice(), &pouch].concat());
        }
        let levels = |pouch| BOT_SKILL_TIERS.iter().map(|tier| builds.of(tier, pouch).level).collect::<Vec<_>>();
        assert_eq!(levels(false), [15, 20, 30]);
        assert_eq!(levels(true), [26, 31, 41]);
        // What the top tier's skills give.
        let top = builds.of(&BOT_SKILL_TIERS[2], false).modifiers;
        assert_eq!(top[MAX_CARRIED_WEIGHT], 10.0);
        assert!((top[index("aim_time")] + 0.1).abs() < 1e-6);
        assert!((top[index("movement_speed")] - 0.05).abs() < 1e-6);
        assert!((top[index("overweight_penalty")] + 0.25).abs() < 1e-6);
        assert!((top[index("stamina_regeneration_speed")] - 0.1).abs() < 1e-6);
        assert!((top[index("stamina_spending_speed_skill")] + 0.1).abs() < 1e-6);
        assert_eq!(top[index("can_sprint")], 0.0);
        assert_eq!(top[index("max_carried_weight_ultimate")], 0.0);
        let with = builds.of(&BOT_SKILL_TIERS[2], true).modifiers;
        assert!((with[index("reload_speed")] - 0.12).abs() < 1e-6);
        assert!((with[index("change_weapon_speed")] - 0.15).abs() < 1e-6);
        assert!((with[index("movement_speed_when_aiming")] - 0.05).abs() < 1e-6);
    }

    /// The resolver alone, on a small tree of its own: a locked row is opened by the intent's later skills in the rows above,
    /// no more than its threshold needs, keeping a point for the skill; one it can not open is reported; the points cap all.
    #[test]
    fn an_intent_opens_a_locked_row_with_its_own_later_skills() {
        let skill = |id: u16, ranks: u8, needs: u32, row: u8| super::super::skills::Skill {
            id,
            levels_amount: ranks,
            branch_points: needs,
            antiskills: Vec::new(),
            column: 1,
            row,
            levels: vec![vec![(0, 1.0)]; usize::from(ranks)],
            unknown_modifiers: Vec::new(),
        };
        let tree = SkillsTree {
            skills: vec![skill(201, 3, 0, 1), skill(204, 3, 0, 1), skill(209, 5, 5, 2), skill(211, 3, 15, 4)],
        };
        let one = |want: &'static [u16], points| resolve(&tree, &[SkillIntent { want, points }]).unwrap();
        // 204 alone opens 3 of 209's 5: 201 gives the 2 more, 209 its five, 201 the rest.
        let build = one(&[204, 209, 201], 15);
        assert_eq!(build.skills, [(204, 3), (201, 3), (209, 5)]);
        assert_eq!((build.level, build.unspent), (11, 4));
        let build = one(&[204, 209, 201], 9);
        assert_eq!(build.skills, [(204, 3), (201, 2), (209, 4)]);
        assert_eq!(build.short, [Shortfall { skill: 209, got: 4, ranks: 5, why: Short::Points }, Shortfall {
            skill: 201,
            got: 2,
            ranks: 3,
            why: Short::Points
        }]);
        // Nothing after it to open its row: 209 is reported, its points stay unspent.
        let build = one(&[204, 209], 15);
        assert_eq!(build.skills, [(204, 3)]);
        assert_eq!(build.short, [Shortfall { skill: 209, got: 0, ranks: 5, why: Short::Locked { needs: 5, unlocked: 0 } }]);
        // A row the rows above can not reach (6 of 15 at most) stays locked; an unknown skill is reported.
        let build = one(&[211, 201, 204, 999], 20);
        assert_eq!(build.skills, [(201, 3), (204, 3)]);
        assert!(build.short.iter().any(|s| s.skill == 211 && matches!(s.why, Short::Locked { needs: 15, .. })));
        assert!(build.short.contains(&Shortfall { skill: 999, got: 0, ranks: 0, why: Short::Unknown }));
        // Not one point more than the intent has.
        let build = one(&[204, 209, 201], 3);
        assert_eq!(build.skills, [(204, 3)]);
    }

    /// No bot kit is overburdened: every kit of every tier and role
    /// weighs less than the capacity its skills give (40 kg), and its
    /// skills are those of its tier's gear level.
    #[test]
    fn no_bot_kit_is_over_its_capacity() {
        assert_eq!(item_params::player().max_carried_weight, 30.0);
        for (equipment, role, new, upgrades) in every_bot() {
            let tier = equipment.tier;
            let owned = bot_kits_in(equipment, role, new, upgrades);
            let kits: Vec<&Loadout> = owned.iter().collect();
            let gear = gear_level(&kits);
            assert!(tier.levels().contains(&gear), "{tier} {role}: {gear}");
            let skills = BotSkillTier::for_kits(&kits);
            assert_eq!(
                skills.up_to_gear_level,
                *tier.levels().end(),
                "{tier} {role}"
            );
            let modifiers = skills.modifiers_with(new);
            for kit in kits {
                let capacity = carry_capacity(kit, &modifiers);
                // 40 kg, and the kilograms of the carry upgrades.
                assert!(capacity >= 40.0, "{tier} {role}");
                assert!(capacity <= 45.0, "{tier} {role}: {capacity}");
                assert!(kit.weight() < capacity, "{tier} {role}: {}", kit.weight());
            }
        }
    }

    #[test]
    fn the_speed_level_follows_the_weight_and_the_skills() {
        let none = [0.0; MODIFIERS];
        let close = |a: f32, b: f32| (a - b).abs() < 1e-5;
        // Up to 20 kg nothing; at the capacity -20 %; between, in line.
        assert!(close(movement_level(20.0, 30.0, &none, 0.0), 1.0));
        assert!(close(movement_level(25.0, 30.0, &none, 0.0), 0.9));
        assert!(close(movement_level(30.0, 30.0, &none, 0.0), 0.8));
        assert!(close(movement_level(45.0, 30.0, &none, 0.0), 0.8));
        assert!(close(weight_cost(25.0, 40.0), 0.25));
        // The top tier: 40 kg, a quarter less of the cost, +5 %.
        let top = BOT_SKILL_TIERS[2].modifiers();
        assert!(close(movement_level(30.0, 40.0, &top, 0.0), 1.05 - 0.075));
        // Heavyweight: no speed lost to the weight.
        let mut heavy = none;
        heavy[index("max_carried_weight_ultimate")] = 1.0;
        assert!(close(movement_level(30.0, 30.0, &heavy, 0.0), 1.0));
        // A modification's movement speed adds.
        assert!(close(movement_level(0.0, 30.0, &none, -0.1), 0.9));
        // The defect 4080 takes 3 kg of capacity; the modification 1120
        // adds 1 kg.
        assert_eq!(
            item_added(0, &[(4080, true)], "max_carried_weight_modifier"),
            -3.0
        );
        assert_eq!(
            item_added(0, &[(1120, false)], "max_carried_weight_modifier"),
            1.0
        );
    }

    /// The rusher's SMG kit slows a bot no more than the SPAS-12 kit it
    /// replaces did: lighter in both tiers (the SMG, its ammo and the
    /// helmet weigh less than the shotgun and its shells, or the helmet is
    /// all it adds at 13-15), so no more of the weight's cost.
    #[test]
    fn the_rushers_smg_kit_is_no_slower_than_the_old_one() {
        use crate::v026g::match_msgs::static_info::MatchProfile;
        let level = |equipment: BotEquipment| {
            let kits = bot_kits(equipment, BotRole::Rusher);
            let profile = MatchProfile::from_loadouts(
                "bot",
                0,
                &kits,
                bot_modifiers(&kits),
                Dictionary::shared(),
                1,
            );
            profile_movement_level(&profile, 0).unwrap()
        };
        for tier in [BotTier::Mid, BotTier::High] {
            let smg = RusherKit::Smg.worn_in(tier);
            if smg == RusherKit::Control {
                continue;
            }
            let (new, old) = (
                level(BotEquipment { tier, rusher: smg, mask: BotMask::Table }),
                level(BotEquipment {
                    tier,
                    rusher: RusherKit::Control,
                    mask: BotMask::Table,
                }),
            );
            assert!(new >= old - 1e-6, "{tier}: {new} against {old}");
        }
    }

    /// The players' data (`SRP_GAME_DATA`), read once for the ignored tests of one run (a second `init` after a table was read
    /// is refused).
    fn live_data() {
        static READ: std::sync::OnceLock<()> = std::sync::OnceLock::new();
        READ.get_or_init(|| crate::v026g::game_data::init().map(|_| ()).expect("SRP_GAME_DATA reads and passes srp's checks"));
    }

    /// On the live game data (`SRP_GAME_DATA`, the server's file) the same
    /// holds for every rusher kit of both higher tiers, the SMGs of the
    /// 0.29a3 weapons among them. Run alone:
    /// `SRP_GAME_DATA=<server-game-data.db> cargo test -p survarium --lib
    /// -- --ignored on_the_live_data_no_rusher_kit`
    #[test]
    #[ignore = "needs SRP_GAME_DATA, the server's game data"]
    fn on_the_live_data_no_rusher_kit_is_over_its_capacity() {
        live_data();
        no_bot_kit_is_over_its_capacity();
        the_rushers_smg_kit_is_no_slower_than_the_old_one();
        a_bots_profile_gives_its_speed_level();
        for tier in [BotTier::Mid, BotTier::High] {
            assert_eq!(made_kits(tier).count(), RusherKit::ALL.len(), "{tier}");
        }
    }

    #[test]
    fn a_bots_profile_gives_its_speed_level() {
        // The old kit: these rules are those of the kit before the owner's choices, which are the defaults now.
        let _kit = KitSwitches::OFF.scope();
        use crate::v026g::match_msgs::static_info::MatchProfile;
        for (equipment, role, new, upgrades) in every_bot() {
            let tier = equipment.tier;
            let owned = bot_kits_in(equipment, role, new, upgrades);
            let kits: Vec<&Loadout> = owned.iter().collect();
            let modifiers = BotSkillTier::for_kits(&kits).modifiers_with(new);
            let profile =
                MatchProfile::from_loadouts("bot", 0, &kits, modifiers, Dictionary::shared(), 1);
            for (k, kit) in kits.iter().enumerate() {
                let capacity = carry_capacity(kit, &modifiers);
                // The items' own movement speed (their default
                // modifications and the upgrades they wear) and weight.
                let items: f32 = kit
                    .items
                    .iter()
                    .map(|item| {
                        item_added(
                            item.dict_id,
                            &wire_mods(item.mods),
                            "movement_speed_modifier",
                        )
                    })
                    .sum();
                let weight: f32 = kit
                    .items
                    .iter()
                    .map(|item| item_weight(item.dict_id, item.count, &wire_mods(item.mods)))
                    .sum();
                let expected = movement_level(weight, capacity, &modifiers, items);
                let level = profile_movement_level(&profile, k).unwrap();
                assert!(
                    (level - expected).abs() < 1e-5,
                    "{tier} {role}: {level} {expected}"
                );
                // The tiers' kits: under 31 kg of 40, so a few percent
                // slower at most (the owner's kit is 1.5 to 2.5 kg
                // heavier); the skills' and the armour's speed above.
                assert!((0.93..=1.15).contains(&level), "{tier} {role}: {level}");
            }
            assert_eq!(profile_movement_level(&profile, kits.len()), None);
        }
    }

    /// [A372] On the players' data (`SRP_GAME_DATA`, the live `server-game-data.db`; 0.26g's own archive has no medical tab):
    /// the builds the server resolves at its start hold ([`the_builds_hold`], the medical block on), and the medical block
    /// changes only the client's packed ranks (modifier 43 `armored_trap`, `client-patches/rust/patches/src/medical_skills.rs`
    /// `Skills::parse`) and `drug_amount`. Prints each build (the owner's table). `tools/checks.tsv` `bot-skills-live` runs it
    /// on the server game data of the tree.
    #[test]
    #[ignore = "needs SRP_GAME_DATA, the players' game data with the medical tab (tools/check bot-skills-live)"]
    fn on_the_live_data_every_build_is_a_set_the_tree_allows() {
        live_data();
        let tree = SkillsTree::shared();
        assert!(medical_skills(), "the data's tree has the medical block's skills and the switch is on");
        let builds = the_builds_hold(tree, true);
        assert_eq!(BotBuilds::shared(), Ok(&builds));
        let plain = BotBuilds::resolve(tree, false).expect("the builds without the medical block");
        let packed = index("armored_trap");
        for tier in &BOT_SKILL_TIERS {
            for pouch in [false, true] {
                let (with, without) = (builds.of(tier, pouch), plain.of(tier, pouch));
                let changed: Vec<usize> =
                    (0..MODIFIERS).filter(|&i| (with.modifiers[i] - without.modifiers[i]).abs() > 1e-6).collect();
                assert_eq!(changed, vec![packed, index("drug_amount")], "{tier:?} {pouch}");
                let medical: Vec<(u16, u8)> =
                    with.skills.iter().copied().filter(|&(id, _)| MEDICAL.want.contains(&id)).collect();
                assert_eq!(with.modifiers[packed] - without.modifiers[packed], tree.modifiers(&medical)[packed]);
                eprintln!(
                    "A372 build gear<={} pouch={pouch}: level {} unspent {} skills {:?} short {:?}",
                    tier.up_to_gear_level, with.level, with.unspent, with.skills, with.short
                );
            }
        }
    }
}
