//! Achievements (push `B9`, request `55 04`): the 79 definitions of `gameplay/quests/achievements.options`, progress
//! stored per account (`Profile::ach`), updated from facts the server really has. The client only loads the
//! descriptors (loader 0x20034CA80, preconditions per type in the switch at 0x20034D041) and shows the values B9 /
//! `83 55 02` carry; the counting is the server's. Labels: PROVEN (exe / AS / data / 0.69d text), LIKELY, CHOICE.
//! Each entry carries the progress `value` and `current_state` = the number of states that value reached
//! (`Achievement::reached`); the client's window takes "achieved", the level and the next threshold from the latter.
//!
//! What is counted and where:
//!
//! | kind (`type`) | achievements | counted in | rule |
//! |---|---|---|---|
//! | 0, 1, 3-7, 9-11, 13, 15, 17, 18 | kills, killstreak, stats events, badges, artefacts ... | [`on_match`] / [`figure`] from `MatchFacts` (`stats::record_match`) | each needs its fact; a local match has none of them (the 97 numbers are synthetic), so they move only when a match source fills the facts |
//! | 2 | matches won by mode, win streak, squad / match-type / score-difference wins (418, 419, 608-616, 627, 628) | same | finished match: won, mode (`mode_mask` bit n = game mode n, see [`mode_mask`]); for an online PvP match `online::pay` also fills `match_type`, `in_squad` / `in_clan_squad` and `score_diff` (the winner's team value minus the loser's) |
//! | 19 | 420 upgrade_item | `workshop::craft_pick` (`45 08` adds a modifier) | progress = best quality an upgrade took an item to (rank after it: 1 green, 2 blue, 3 orange) |
//! | 20 | 404 reroll_item | `workshop::reroll_pick` (`45 06` replaces a modifier by a different offered one) | progress = best quality of an item whose modifier was changed (a kept modifier does not count) |
//! | 21 | 413 level | [`refresh_state`]: login, match, skills and after every lobby event | `Levels::level_of(xp)` |
//! | 22 / 23 | 408-412 faction ranks | same | `levels::faction_level(points)` |
//! | 24 | 403 customize_equipment | `decals::relocate` (an accepted `45 01`) | 1 |
//! | 25 / 26 | 401, 405 skill branches | [`refresh_state`] | `Profile::skills`: 401 = the fewest points of the masked branches (reached at 5, the text's minimum), 405 = branches with at least the data's `points_*` (20) |
//! | 8 | 415-417 play_with_N_mods | [`on_match`] / [`figure`] from `MatchFacts::item_mods` (`online::pay`) | a finished online match that counts (not left, not inactive, no clan battle or custom training): the most random modifiers on ANY item of the loadout he played it with ([`match_mods`]: the options profiles he spawned with, the world host's `Used` row, protocol 6; without that row the options' start profile, LIKELY), as the profile was at the match start; condition 1 / 2 / 3. Wearing the items counts nothing; a local match has no figure |
//! | 27 | 620 complete_gold_quests | `quests::record` | +1 per completed daily quest |
//! | 28 | 406 search_all_stashes | `hunt::search_request` | 1 when a find leaves no item of the hunt page unfound |
//! | 29 | 629 / 630 win_pve | `pve::record_run_notify` / `record_run` (once per run) | +1 for every player of a won mission whose caches found >= `num_stashes_found` (3 / 10) |
//! | 30 | 623-625 hero_of_the_day | `hero::tick` (each new title of the rewarded ranking) | progress = best place reached in the nomination: third 1, second 2, first 3 |
//!
//! Evidence of the lobby-event rules:
//! * 420: state texts `st_achievement_desc_upgrade_item_1..3` "Upgrade piece of equipment to green / blue / orange
//!   quality" (PROVEN); an item's frame colour is `Colors.RARITY_<rank>` = 0x92C025 green, 0x429BF6 blue, 0xFF6825
//!   orange (PROVEN, AS `ItemData.getOverFrameColor`, `Colors`), rank = `min(3, base_rarity_level + modifiers)` (PROVEN
//!   0x24966D). The item upgrade is "Upgrade equipment" (`st_item_craft_message_label`), the module merge "Improve
//!   module" (`st_upgrade_module_title`): only `45 08` counts (LIKELY). The file says `apply_type` 1, but a plain count of
//!   upgrades would show "orange" after three upgrades of green items; the progress is the best quality so the reached
//!   state is the one its text names (LIKELY).
//! * 404: texts `st_achievement_desc_reroll_item_1..3` "Change modifier on a green / blue / orange piece of equipment"
//!   (PROVEN); quality as the client colours the item: a module by its rarity (`ModuleRarityEnums` 1 green, 2 blue,
//!   3 orange), anything else by its rank ([`quality`]); the same `apply_type` note as 420 (LIKELY).
//! * 403: text "Change appearance of an item"; the camouflage / decal editor is the "Appearance" button
//!   (`st_btn_upgrade_label`, closed by "Finish changing item's appearance") (LIKELY). Weapon skins (`45 11`) are not
//!   counted: OPEN (the text could cover them, nothing ties them to it).
//! * 406: text "Receive all rewards in the Treasure Hunt" (PROVEN); the find that opens the last item of the page.
//! * 620: text "Complete %d daily quests" (PROVEN); the "gold quests" of the id are the old quest system that 0.69d does
//!   not load (`docs/notes/quests.md` 1), so every completed daily quest counts (LIKELY).
//! * 629 / 630: texts "Win %d matches in team missions", "Find all stashes in a team mission" (PROVEN); the client
//!   loads `num_stashes_found` for type 29 only (0x20034D291), a mission has `chests_total` 10 = the 10 of 630 (data);
//!   ">=" and "won" = `success` (LIKELY); every player of the won run counts (CHOICE: the success and the caches are
//!   the squad's).
//! * 623-625: `nomination` (loaded for type 30, 0x20034D3D1) 1 / 2 / 3 = "Best Player" / "Killstreak" / "Winstreak" by
//!   the achievement texts; state texts `st_achievement_desc_hero_of_the_day_*_1..3`: 1 = third place, 2 = second, 3 =
//!   first (PROVEN). Friends / clan lists are not titles ("not rewarded") and count nothing.
//!
//! Counting: `Add` events grow a counter, `Max` events keep the best figure; progress never goes back. A repeated event
//! cannot count twice: PvE runs are counted only when their match id is stored the first time, hero titles once per
//! calculated slot, the others are a best figure or happen once per real change. Every lobby event also runs
//! [`refresh_state`] (a quest or a hunt find may have paid experience or reputation).
//!
//! Per-kill facts from the world host's kill log (`match_data.rs`; every argument read in the exe): 207 two kills by one
//! bullet (shot id + pellet), 208 reloading victim, 209 ricochet, 210 killer health, 211 victim destroying an anomaly,
//! 212 kill after own death, 224 hipfire sniper kill (the kill feed's hipfire rule), 225 a kill of an enemy damaged by
//! the whole squad, 227 the last round, 228 melee kill of an aiming enemy sniper (victim weapon category from the item
//! data).
//!
//! 205 / 226 two kills by one grenade / mine (the grenade's / mine's shot id in the damage type); 225's assist half (the
//! core's assist rule, computed by the host, protocol 8); 402 regenerated health in one life and 414 an artefact at
//! <= 1 % health (the host's per-step health sample, protocol 8).
//!
//! Evidence of the event-row rules (type 3, 4, 9):
//! * 619 "Capture N units in Research mode" = the sum of event 29 `capture_the_point` amounts; a whole capture is 100
//!   shared by the capturing players (zone step 0x2003523C7, capture 0x2003AA29F, statistics slot 20 0x2001FC2C0), so
//!   the conditions 500..20000 are the texts' 5..200 units (PROVEN; `reward_factor` 100 in events.options agrees).
//! * 626 "Bring N Glows of the Forest" = event 47 `sad_bomb_planted`, +1 for the Search and Destroy planter
//!   (0x2003AF8B6, statistics slot 15 0x2001FBFB0; PROVEN). No Artifact Hunt delivery adds event 47.
//! * 407 / 414 (type 9) read event 5 `artefact`; event 7 `artefact_get` is added in the same call (slot 8 0x2001FBAB0),
//!   so it is a subset of event 5 (LIKELY).
//! * 603 "Get N gold medals" = badge instances of `badge_state` 2 counted 0-based (the `97` R block order `3t + s`):
//!   the third, gold state of a three-state badge (descriptor +0x210, 0x20034D2DE). One-state gold medals do not
//!   count. LIKELY.
//! * 613 "Win ... in the Protective device mode with a score of 10-0" = `score_diff` >= 10: a team value (+0x274 /
//!   +0x275, 0x2003A610D / 0x2003A616C) never exceeds the battery list size (match end 0x2003A61DC). LIKELY.
//!
//! OPEN (not counted, nothing invented): 213 grenade kept in hand (no marker found that tells it from a thrown one:
//! 0x2003EB680, hit handle +0x220), 229 "different ways" (no source says what a way is). 621 (14) counts from
//! `squad_all_medals`.
//!
//! With the world host's event row (`MatchFacts::stat_events`, protocol 3) the kill achievements whose only kill
//! condition is the kind of kill (201, 202, 203, 206, 601, 602) count the world's rows ([`figure`]); the per-kill
//! conditions (weapon class 215-222, victim state, killer health, ...) need the kill list. 210 counts within one life
//! (its text: "during one life").
//!
//! Rewards: reaching a state gives its reward (`reward_type` 0 = item `reward_id`; the file has no other type). Every
//! reward is a one-of cosmetic (decal / camo): an item the account already has is not given twice (choice).

use accounts::{AchProgress, Profile};
use game_data::achievements::{mode_mask, Achievement};
use game_data::levels::faction_level;
use game_data::facts::KillFact;
use game_data::{GameData, MatchFacts};
use protocol::s2c::achievements::{category_reply, progress_reply, AchEntry, AchievementsB9, MAX_REACHED};
use protocol::s2c::S2c;
use protocol::Encode;

use crate::build;
use crate::dispatch::{update, Ctx};
use crate::peer::Peer;

/// One progress change: the new value and the states reached by it (`lo..hi`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Change {
    pub id: u16,
    pub kind: u8,
    pub value: u32,
    pub lo: u8,
    pub hi: u8,
}


/// Branches of the skill tree (id hundreds digit 1..=5).
fn branch_points(p: &Profile) -> [u32; 5] {
    let mut b = [0u32; 5];
    for (&id, &pts) in &p.skills {
        let br = (id / 100) as usize;
        if (1..=5).contains(&br) {
            b[br - 1] += u32::from(pts);
        }
    }
    b
}

/// 401 (type 25) "Put in all the branches of the skill tree a minimum of 5 points": the fewest points of the branches
/// `skill_branches_mask` selects (bit b-1 = branch b, the `47 01` branch byte order; bits without a branch in the 0.69d
/// tree are skipped, the data's 63 has a sixth bit), so the one state (condition 5) is reached when every branch has
/// at least 5 points. The client only loads the mask (type 25 case 0x20034D2E9 -> descriptor +0x20C); the condition is
/// the text's "minimum of 5 points". A mask without any existing branch gives 0.
fn least_branch_points(branches: &[u32; 5], mask: u64) -> u32 {
    (0..5usize).filter(|&i| mask & (1 << i) != 0).map(|i| branches[i]).min().unwrap_or(0)
}

/// Store a new value; give the rewards of the states it reaches. Returns the change (None = nothing changed).
fn store(gd: &GameData, rewards: bool, p: &mut Profile, a: &Achievement, value: u32, run: u32) -> Option<Change> {
    let old = p.ach.get(&a.id).copied().unwrap_or_default();
    // progress never goes back (a state once reached stays reached)
    let value = if a.apply == 1 { value } else { value.max(old.value) };
    if value == old.value && run == old.run {
        return None;
    }
    p.ach.insert(a.id, AchProgress { value, run });
    let (lo, hi) = (a.reached(old.value), a.reached(value));
    for s in a.states.iter().take(hi as usize).skip(lo as usize).filter(|_| rewards) {
        grant(gd, p, s.reward_type, s.reward_id, s.reward_num);
    }
    Some(Change { id: a.id, kind: a.kind, value, lo, hi: hi.max(lo) })
}

fn grant(gd: &GameData, p: &mut Profile, rtype: u32, id: u32, num: u32) {
    if num == 0 || rtype != 0 {
        return;
    }
    let Ok(dict) = u16::try_from(id) else { return };
    if gd.items.get(dict).is_none() || p.items.iter().any(|i| i.dict_id == dict) {
        return;
    }
    let value = if gd.items.is_stackable(dict) { num } else { gd.items.amount_field(dict, 100) };
    let uid = p.add_item(dict, value);
    // a module reward (none in 0.69d data) is green, never grey
    crate::chests::give_rarity(gd, p, uid, &mut crate::fresh_roll());
}

/// Update every achievement whose figure is stored state (level, ranks, skills).
pub fn refresh_state(gd: &GameData, rewards: bool, p: &mut Profile) -> Vec<Change> {
    let mut out = Vec::new();
    let level = gd.levels.level_of(p.xp);
    let ranks: Vec<u32> = (1..=4u8).map(|f| faction_level(p.points(f))).collect();
    let branches = branch_points(p);
    for a in gd.achievements.iter() {
        let value = match a.kind {
            21 => level,
            22 => {
                let Some(mask) = a.pre_u64("faction_mask") else { continue };
                (1..=4u8).filter(|f| mask & (1 << (f - 1)) != 0).map(|f| ranks[usize::from(f - 1)]).max().unwrap_or(0)
            }
            23 => ranks.iter().copied().min().unwrap_or(0),
            25 => {
                let Some(mask) = a.pre_u64("skill_branches_mask") else { continue };
                least_branch_points(&branches, mask)
            }
            26 => {
                let need = a.pre.iter().filter(|(k, _)| k.starts_with("points_")).filter_map(|(_, v)| v.as_u64()).min().unwrap_or(0) as u32;
                branches.iter().filter(|&&b| need > 0 && b >= need).count() as u32
            }
            _ => continue,
        };
        let run = p.ach.get(&a.id).map_or(0, |x| x.run);
        out.extend(store(gd, rewards, p, a, value, run));
    }
    out
}

fn pre_list(a: &Achievement, key: &str) -> Option<Vec<u16>> {
    a.pre.get(key)?.as_array().map(|v| v.iter().filter_map(|x| x.as_u64().map(|n| n as u16)).collect())
}

/// Does the kill satisfy the kill-level preconditions of `a`? None = a needed fact of the kill is absent.
fn kill_matches(a: &Achievement, k: &KillFact) -> Option<bool> {
    let mut ok = true;
    if let Some(mask) = a.pre_u64("kill_type") {
        ok &= k.has_flag(mask as u8)?;
    }
    if let Some(v) = a.pre_u64("victim_state").filter(|&v| v < 8) {
        ok &= k.victim_state? & (1 << v) != 0;
    }
    if let Some(c) = a.pre_u64("victim_item_category_id") {
        ok &= k.victim_category? == c as u16;
    }
    if let Some(list) = pre_list(a, "item_category_id") {
        ok &= list.contains(&k.weapon_category?);
    }
    Some(ok)
}

/// Match-level preconditions (`is_won_match`, `deaths`). None = a needed fact is absent.
fn match_ok(a: &Achievement, f: &MatchFacts) -> Option<bool> {
    let mut ok = true;
    if let Some(w) = a.pre_bool("is_won_match") {
        ok &= w == f.won;
    }
    if let Some(d) = a.pre_u64("deaths") {
        ok &= u64::from(f.deaths?) == d;
    }
    Some(ok)
}

/// The world's event row a kill achievement counts when its only kill condition is the kind of kill (world host
/// protocol 3): no condition = `kill` (0), `kill_type` 2 melee = `melee_kill` (13), 4 bleeding = `bleeding_kill` (37),
/// 8 headshot = `headshot_kill` (15) (the kill handler 0x20033DCF0 adds these rows per kill).
/// None = it needs the kill list (weapon class, victim state, ricochet).
fn event_row_of(a: &Achievement) -> Option<u16> {
    use game_data::facts::ev;
    if a.pre.keys().any(|k| !matches!(k.as_str(), "kill_type" | "is_won_match" | "deaths")) {
        return None;
    }
    match a.pre_u64("kill_type") {
        None => Some(ev::KILL),
        Some(2) => Some(ev::MELEE_KILL),
        Some(4) => Some(ev::BLEEDING_KILL),
        Some(8) => Some(ev::HEADSHOT_KILL),
        Some(_) => None,
    }
}

/// What one finished match adds to an event achievement: the match's figure (count of qualifying events, best streak,
/// ...). None = a fact the achievement needs is absent, so the achievement must not move. Kinds not listed here are
/// either lobby events ([`on_event`]: upgrade / reroll / customise, daily quests, treasure hunt, PvE, hero of the day)
/// or OPEN ("different ways" 229, grenade kept in hand 213).
pub fn figure(a: &Achievement, f: &MatchFacts) -> Option<u32> {
    let count = |p: &dyn Fn(&KillFact) -> Option<bool>| f.count_kills(|k| p(k));
    let gated = |n: Option<u32>| -> Option<u32> { if match_ok(a, f)? { n } else { Some(0) } };
    match a.kind {
        0 if f.stat_events.is_some() && event_row_of(a).is_some() => gated(f.stat(event_row_of(a)?)),
        0 => gated(count(&|k| kill_matches(a, k))),
        1 => f.max_killstreak,
        2 => {
            let mut ok = true;
            if let Some(w) = a.pre_bool("is_won_match") {
                ok &= w == f.won;
            }
            if let Some(m) = a.pre_u64("mode_mask") {
                ok &= m & u64::from(mode_mask(f.mode)) != 0;
            }
            if let Some(m) = a.pre_u64("match_type_mask") {
                ok &= m & (1 << (f.match_type?.saturating_sub(1))) != 0;
            }
            if a.pre_bool("is_in_squad") == Some(true) {
                ok &= f.in_squad?;
            }
            if a.pre_bool("is_in_clan_squad") == Some(true) {
                ok &= f.in_clan_squad?;
            }
            if let Some(d) = a.pre_u64("score_diff") {
                ok &= f.score_diff? >= d as u32;
            }
            Some(u32::from(ok))
        }
        3 => f.stat(a.pre_u64("stats_event_type")? as u16),
        4 => {
            let b = f.badges.as_ref()?;
            let n = b
                .iter()
                .filter(|(t, st)| a.pre_u64("badge_type").is_none_or(|x| x == u64::from(*t)) && a.pre_u64("badge_state").is_none_or(|x| x == u64::from(*st)))
                .count();
            Some(n as u32)
        }
        5 => count(&|k| k.after_own_death),
        6 => {
            // "Kill N opponents having less than 15% health during one life": the best life of the match
            let max = a.pre_u64("killer_max_hp")? as u32;
            let kills = f.kills.as_ref()?;
            let mut per: std::collections::BTreeMap<u16, u32> = std::collections::BTreeMap::new();
            for k in kills.iter().filter(|k| k.killer_hp.is_some_and(|h| h <= max)) {
                *per.entry(k.life.unwrap_or(0)).or_default() += 1;
            }
            Some(per.values().copied().max().unwrap_or(0))
        }
        7 => f.killed_all_enemies.map(u32::from),
        9 if a.pre.is_empty() => f.artefacts,
        // 414 "Destroy the anomaly having 1% of health": an artefact event (type 9, as 407) at health <= holder_max_hp
        9 => {
            let max = a.pre_u64("holder_max_hp")?;
            f.artefact_hp_min.map(|h| u32::from(h != 0xFF && u64::from(h) <= max))
        }
        // 402 "Restore more than %d% health in one life": the best life's regenerated percent
        16 => f.regen_best,
        10 => {
            let t = a.pre_u64("inflictor_type")? as u8;
            let kills = f.kills.as_ref()?;
            let mut per: std::collections::BTreeMap<u32, u32> = std::collections::BTreeMap::new();
            for k in kills {
                if let (Some(kt), Some(id)) = (k.inflictor_type, k.inflictor_id) {
                    if kt == t {
                        *per.entry(id).or_default() += 1;
                    }
                }
            }
            Some(per.values().copied().max().unwrap_or(0))
        }
        11 => count(&|k| k.grenade_in_hand),
        12 => f.squad_damage_kill.map(u32::from),
        13 => {
            // 622: all four "best of the match" badges (types 23..=26) in one match
            let b = f.badges.as_ref()?;
            Some(u32::from((23..=26u8).all(|t| b.iter().any(|(x, _)| *x == t))))
        }
        14 => f.squad_all_medals.map(u32::from),
        // 415-417 "Play a match with items having at least N random modifiers": the loadout he played it with
        8 => f.item_mods,
        17 => count(&|k| k.last_bullet),
        18 => count(&|k| Some(k.no_scope? && k.weapon_category? == 13)),
        _ => None,
    }
}

/// A lobby event that moves achievements without match facts. The rules (and their evidence) are in the module docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// kind 19 (420): an item upgrade (`45 08`) added a modifier; `quality` = the item's rank after it (1 green, 2 blue,
    /// 3 orange)
    Upgraded { quality: u32 },
    /// kind 20 (404): a modifier of an item of `quality` (1 green, 2 blue, 3 orange) was changed (`45 06`)
    Rerolled { quality: u32 },
    /// kind 24 (403): a camouflage / decal was put on an item (`45 01`)
    Customised,
    /// kind 27 (620): `n` daily quests were completed
    DailyQuests(u32),
    /// kind 28 (406): the last not-found item of the treasure hunt page was found (`46 07`)
    HuntCompleted,
    /// kind 29 (629, 630): a won PvE mission with `stashes` caches found
    PveWin { stashes: u32 },
    /// kind 30 (623..625): a hero of the day title of `nomination` (1 Best Player, 2 Killstreak, 3 Winstreak) at
    /// `place` (1..3)
    HeroTitle { nomination: u8, place: u8 },
}

/// What an event does to one achievement's progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    /// a counter grows by n
    Add(u32),
    /// the progress becomes at least n
    Max(u32),
}

fn event_op(a: &Achievement, e: Event) -> Option<Op> {
    match (a.kind, e) {
        // state i = "... green / blue / orange" (i = 1, 2, 3): the progress is the best quality reached
        (19, Event::Upgraded { quality }) | (20, Event::Rerolled { quality }) if (1..=3).contains(&quality) => Some(Op::Max(quality)),
        (24, Event::Customised) | (28, Event::HuntCompleted) => Some(Op::Max(1)),
        (27, Event::DailyQuests(n)) if n > 0 => Some(Op::Add(n)),
        (29, Event::PveWin { stashes }) => (u64::from(stashes) >= a.pre_u64("num_stashes_found")?).then_some(Op::Add(1)),
        (30, Event::HeroTitle { nomination, place }) => {
            (a.pre_u64("nomination")? == u64::from(nomination) && (1..=3).contains(&place)).then(|| Op::Max(4 - u32::from(place)))
        }
        _ => None,
    }
}

/// Count a lobby event, then bring the state figures up to date (a quest or a hunt find may have paid experience or
/// reputation).
pub fn on_event(gd: &GameData, rewards: bool, p: &mut Profile, e: Event) -> Vec<Change> {
    let mut out = Vec::new();
    for a in gd.achievements.iter() {
        let Some(op) = event_op(a, e) else { continue };
        let old = p.ach.get(&a.id).copied().unwrap_or_default();
        let value = match op {
            Op::Add(n) => old.value.saturating_add(n),
            Op::Max(n) => old.value.max(n),
        };
        out.extend(store(gd, rewards, p, a, value, old.run));
    }
    out.extend(refresh_state(gd, rewards, p));
    out
}

/// [`on_event`] inside a profile update that the caller runs: returns the changes and the item count before them (for
/// [`push_changes`] / [`changes_messages`]).
pub fn event_in(gd: &GameData, rewards: bool, p: &mut Profile, e: Event) -> (Vec<Change>, usize) {
    let before = p.items.len();
    (on_event(gd, rewards, p, e), before)
}

/// [`on_event`] for an account in its own profile update (no peer at hand). None = the profile could not be updated.
pub fn event_for(store: &accounts::Store, gd: &GameData, rewards: bool, account: accounts::AccountId, e: Event) -> Option<(Vec<Change>, usize)> {
    match store.update_profile(account, |p| Ok::<_, ()>(event_in(gd, rewards, p, e))) {
        Ok(v) => Some(v),
        Err(err) => {
            log::error!("achievement event {e:?} of account {account}: {err:?}");
            None
        }
    }
}

/// The quality of an item as the client colours it (AS `ItemData.getOverFrameColor`: a module by its rarity, any other
/// item by its rank; 1 green, 2 blue, 3 orange). None = no such item.
pub fn quality(gd: &GameData, p: &Profile, uid: u32) -> Option<u32> {
    let row = p.item(uid)?;
    let mods = p.mods.get(&uid).map(|m| m.as_slice()).unwrap_or(&[]);
    Some(if gd.items.is_module(row.dict_id) {
        u32::from(game_data::merge::rarity(mods))
    } else {
        gd.craft.rank(p.real_dict(row), mods.len())
    })
}

/// 415-417 (type 8): the most random modifiers on one item of `slots` (any item counts). The count is the item's stored
/// modifier list (`Profile::mods`): the workshop upgrades (`45 08`) and the treasure hunt's hidden modifiers. Slots hold
/// no module (modules sit in `Profile::attach`), so a module's rarity pair (group 312) and its type's override pairs are
/// never among them.
pub fn most_mods<'a>(p: &Profile, slots: impl IntoIterator<Item = &'a u32>) -> u32 {
    slots.into_iter().filter_map(|u| p.mods.get(u)).map(|m| m.len() as u32).max().unwrap_or(0)
}

/// The 415-417 figure of every options profile at a match start: entry p = [`most_mods`] over the slot table options
/// profile p carries ([`crate::profiles::options_order`]; 0 for a profile sent without one). Taken from the profile the
/// options were built from.
pub fn start_mods(p: &Profile, now: i64) -> Vec<u32> {
    crate::profiles::options_order(p, now).into_iter().map(|(i, sent)| if sent { most_mods(p, p.profile_slots(i).values()) } else { 0 }).collect()
}

/// Where the 415-417 figure of an online match came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModsSource {
    /// the world host's `Used` row (protocol 6): the options profiles he spawned with (PROVEN: the host marks a profile
    /// when the player's character of that profile is alive)
    Spawned,
    /// no `Used` row: options profile 0, the profile the options start him with (LIKELY: he may have switched before his
    /// first spawn)
    StartProfile,
}

/// The 415-417 figure of a finished online match: the most of `start` ([`start_mods`]) over the profiles of `spawned`
/// (the `Used` row's mask, bit p = options profile p), else over options profile 0.
pub fn match_mods(start: &[u32], spawned: Option<u16>) -> (u32, ModsSource) {
    match spawned {
        Some(mask) => {
            let n = start.iter().enumerate().filter(|(i, _)| *i < 16 && mask & (1 << i) != 0).map(|(_, n)| *n).max().unwrap_or(0);
            (n, ModsSource::Spawned)
        }
        None => (start.first().copied().unwrap_or(0), ModsSource::StartProfile),
    }
}

/// Update the achievements a finished match counts. A leaver's match counts nothing.
pub fn on_match(gd: &GameData, rewards: bool, p: &mut Profile, f: &MatchFacts) -> Vec<Change> {
    let mut out = Vec::new();
    if !f.finished {
        return out;
    }
    let with_victim_categories;
    let f = match victim_categories(gd, f) {
        Some(g) => {
            with_victim_categories = g;
            &with_victim_categories
        }
        None => f,
    };
    for a in gd.achievements.iter() {
        let old = p.ach.get(&a.id).copied().unwrap_or_default();
        let ch = match a.kind {
            21..=26 => None, // state figures: refresh_state
            2 if a.apply == 2 => {
                // streak: wins in a row; the figure is the best streak (choice), `run` the current one
                let hit = figure(a, f).is_some_and(|n| n > 0);
                let run = if hit { old.run + 1 } else { 0 };
                store(gd, rewards, p, a, old.value.max(run), run)
            }
            _ => match (figure(a, f), a.apply) {
                (Some(n), 1) if n > 0 => store(gd, rewards, p, a, old.value.saturating_add(n), 0),
                (Some(n), 0) if n > old.value => store(gd, rewards, p, a, n, 0),
                _ => None,
            },
        };
        out.extend(ch);
    }
    out
}

/// The facts with each kill's `victim_category` taken from the item data of the victim's weapon (`victim_weapon_dict`;
/// 228 "enemy sniper"); None = no kill needs it.
fn victim_categories(gd: &GameData, f: &MatchFacts) -> Option<MatchFacts> {
    let kills = f.kills.as_ref()?;
    if !kills.iter().any(|k| k.victim_category.is_none() && k.victim_weapon_dict.is_some()) {
        return None;
    }
    let mut g = f.clone();
    for k in g.kills.iter_mut().flatten() {
        if k.victim_category.is_none() {
            k.victim_category = k.victim_weapon_dict.map(|d| gd.items.category(d)).filter(|&c| c != 0);
        }
    }
    Some(g)
}

/// The wire entry of one achievement. `state` is the client's `current_state`: the number of states the stored
/// progress has reached (PROVEN, exe 0x2001C98C0 + AS3 `createAchievement`, see `protocol::s2c::achievements`); the
/// client draws `achieved`, the level pips, the current description and the next threshold from it, not from `value`.
fn entry(a: &Achievement, p: &Profile, lo: u8, hi: u8) -> AchEntry {
    let value = p.ach.get(&a.id).map_or(0, |x| x.value);
    AchEntry { id: a.id, value, state: a.reached(value), lo, hi }
}

/// Every achievement with its stored progress, no popups (`lo == hi`).
pub fn all_entries(gd: &GameData, p: &Profile) -> Vec<AchEntry> {
    gd.achievements.iter().map(|a| entry(a, p, 0, 0)).collect()
}

/// The B9 of the login pushes.
pub fn login_push(gd: &GameData, p: &Profile) -> S2c {
    raw_b9(all_entries(gd, p))
}

fn raw_b9(entries: Vec<AchEntry>) -> S2c {
    let payload = AchievementsB9 { entries }.to_bytes().unwrap_or_else(|e| {
        log::error!("B9 not encodable: {e}");
        vec![0; 4]
    });
    S2c::Raw { id: 0xB9, payload }
}

/// B9 for changes: the changed entries with the states they reached (popups), at most 200 reached states per push.
fn changes_b9(gd: &GameData, p: &Profile, changes: &[Change]) -> Vec<S2c> {
    let mut out = Vec::new();
    let mut batch = Vec::new();
    let mut reached = 0usize;
    for c in changes {
        let Some(a) = gd.achievements.get(c.id) else { continue };
        let n = usize::from(c.hi.saturating_sub(c.lo));
        if reached + n > MAX_REACHED && !batch.is_empty() {
            out.push(raw_b9(std::mem::take(&mut batch)));
            reached = 0;
        }
        reached += n;
        batch.push(entry(a, p, c.lo, c.hi));
    }
    if !batch.is_empty() {
        out.push(raw_b9(batch));
    }
    out
}

/// Messages for changes: B9 per batch and, when a reward item was given, the inventory (89).
pub fn changes_messages(cx: &Ctx, account: accounts::AccountId, changes: &[Change], items_before: usize) -> Vec<S2c> {
    if changes.is_empty() {
        return vec![];
    }
    let Ok(p) = cx.lobby.store.load_profile(account) else { return vec![] };
    let gd = &cx.lobby.data;
    let mut v = changes_b9(gd, &p, changes);
    if p.items.len() != items_before {
        // LIKELY: 89 replaces the client's inventory list (setup_inventory_data), so the full list is sent
        v.push(S2c::Inventory(build::inventory_89(gd, &p)));
    }
    v
}

/// Tell the client about changes.
pub fn push_changes(cx: &Ctx, peer: &mut Peer, changes: &[Change], items_before: usize) {
    let Some(id) = peer.account else { return };
    for m in changes_messages(cx, id, changes, items_before) {
        peer.push(&m);
    }
}

/// Count a finished match for an account and return the messages for the player (called by `stats::record_match`).
pub fn record(cx: &Ctx, account: accounts::AccountId, f: &MatchFacts) -> Vec<S2c> {
    if !f.finished || f.map == 0 {
        return vec![]; // a match left early pays nothing; map 0 is the tutorial
    }
    let gd = &cx.lobby.data;
    let rw = cx.lobby.cfg.achievement_rewards;
    let res = cx.lobby.store.update_profile(account, |p| {
        let before = p.items.len();
        let mut ch = on_match(gd, rw, p, f);
        ch.extend(refresh_state(gd, rw, p));
        Ok::<_, ()>((ch, before))
    });
    let Ok((ch, before)) = res else { return vec![] };
    log::info!("account {account}: achievements after the match: {} changed", ch.len());
    // the levels reached in this match (the B9 popups): achievement id and level numbers, 1-based
    let reached: Vec<String> = ch.iter().filter(|c| c.hi > c.lo).map(|c| format!("{} level {}..={}", c.id, c.lo + 1, c.hi)).collect();
    if !reached.is_empty() {
        log::info!("account {account}: achievement levels reached in the match: {}", reached.join(", "));
    }
    changes_messages(cx, account, &ch, before)
}

/// After a skill change (47 00 / 47 01): the skill achievements.
/// Refresh the state figures after a lobby change that is not an event (skills learned, equipment changed) and show the
/// changes.
pub fn after_skills(cx: &mut Ctx, peer: &mut Peer) {
    let gd = &cx.lobby.data;
    let rw = cx.lobby.cfg.achievement_rewards;
    if let Some((ch, before)) = update(cx, peer, |p| {
        let before = p.items.len();
        (refresh_state(gd, rw, p), before)
    }) {
        push_changes(cx, peer, &ch, before);
    }
}

/// Sign-in: bring the state-driven figures up to date (silently, no popups) before the B9 of the login pushes.
pub fn sync_on_login(cx: &Ctx, id: accounts::AccountId) {
    let gd = &cx.lobby.data;
    let rw = cx.lobby.cfg.achievement_rewards;
    if let Err(e) = cx.lobby.store.update_profile(id, |p| Ok::<_, ()>(drop(refresh_state(gd, rw, p)))) {
        log::error!("achievement sync of account {id}: {e:?}");
    }
}

/// `55 04` (the window was opened): `83 55 02` progress list, then `83 55 03` category scores.
pub fn window(cx: &mut Ctx, peer: &mut Peer) {
    let Some(id) = peer.account else { return };
    let Ok(p) = cx.lobby.store.load_profile(id) else { return };
    let gd = &cx.lobby.data;
    let scores = gd.achievements.category_scores(|a| p.ach.get(&a).map_or(0, |x| x.value));
    match (progress_reply(&all_entries(gd, &p)), category_reply(&scores)) {
        (Ok(a), Ok(b)) => {
            peer.reply_raw(0x55, &a);
            peer.reply_raw(0x55, &b);
        }
        (a, b) => log::error!("{}: 83 55 not encodable: {:?} {:?}", peer.addr, a.err(), b.err()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_data::facts::{KillFact, KILL_BLEEDING, KILL_HEADSHOT, KILL_MELEE, KILL_RICOCHET};

    fn gd() -> GameData {
        game_data::testing::tables()
    }

    fn id_of(ch: &[Change], id: u16) -> Option<Change> {
        ch.iter().copied().find(|c| c.id == id)
    }

    #[test]
    fn a_won_tdm_counts_the_tdm_and_winstreak_achievements_only() {
        game_data::require_tables!();
        let gd = gd();
        let mut p = Profile::starter();
        let ch = on_match(&gd, true, &mut p, &MatchFacts::local(4, 1, true, true, 1));
        assert_eq!(id_of(&ch, 611).unwrap().value, 1); // win tdm
        assert_eq!(id_of(&ch, 616).unwrap().value, 1); // winstreak
        assert!(id_of(&ch, 610).is_none() && id_of(&ch, 612).is_none() && id_of(&ch, 628).is_none());
        assert!(id_of(&ch, 613).is_none(), "score_diff wins are not counted");
        assert!(id_of(&ch, 608).is_none(), "match_type_mask matches are not counted");
        // a loss resets the streak, keeps the best
        let ch = on_match(&gd, true, &mut p, &MatchFacts::local(4, 1, false, true, 1));
        assert!(id_of(&ch, 611).is_none());
        assert_eq!(p.ach[&616], AchProgress { value: 1, run: 0 });
        on_match(&gd, true, &mut p, &MatchFacts::local(4, 2, true, true, 1));
        on_match(&gd, true, &mut p, &MatchFacts::local(4, 2, true, true, 1));
        let ch = on_match(&gd, true, &mut p, &MatchFacts::local(4, 2, true, true, 1));
        assert_eq!(id_of(&ch, 616).unwrap(), Change { id: 616, kind: 2, value: 3, lo: 0, hi: 1 });
        assert_eq!(p.ach[&612].value, 3);
    }

    #[test]
    fn five_wins_reach_the_first_state_and_a_reward_is_given_once() {
        game_data::require_tables!();
        let gd = gd();
        let mut p = Profile::starter();
        let mut last = vec![];
        for _ in 0..5 {
            last = on_match(&gd, true, &mut p, &MatchFacts::local(4, 1, true, true, 1));
        }
        let c = id_of(&last, 611).unwrap();
        assert_eq!((c.value, c.lo, c.hi), (5, 0, 1));
        assert_eq!(gd.achievements.get(611).unwrap().reached(49), 1);
    }

    #[test]
    fn state_figures_follow_level_rank_and_skills() {
        game_data::require_tables!();
        let gd = gd();
        let mut p = Profile::starter(); // level 5, every faction 200 000 points
        p.xp = 2170;
        let ch = refresh_state(&gd, true, &mut p);
        assert_eq!(p.ach[&413].value, 5); // get_level, first state at 10
        assert_eq!(gd.achievements.get(413).unwrap().reached(5), 0);
        assert!(id_of(&ch, 413).unwrap().hi == 0);
        assert_eq!(p.ach[&410].value, faction_level(200_000)); // all ranks: lowest of the four
        assert_eq!(p.ach[&409].value, faction_level(200_000));
        p.skills.insert(112, 1);
        p.skills.insert(213, 1);
        refresh_state(&gd, true, &mut p);
        assert_eq!(p.ach.get(&401).map_or(0, |x| x.value), 0, "401: three branches still have no point");
        assert_eq!(p.ach.get(&405).map_or(0, |x| x.value), 0);
    }

    /// 401 "Put in all the branches of the skill tree a minimum of 5 points": the figure is the fewest points of the
    /// five branches; one point in each is not enough, 5 in each reaches the one state (condition 5).
    #[test]
    fn talented_needs_five_points_in_every_branch() {
        game_data::require_tables!();
        let gd = gd();
        let mut p = Profile::starter();
        for b in 1..=5u16 {
            p.skills.insert(b * 100 + 12, 1);
        }
        refresh_state(&gd, true, &mut p);
        assert_eq!(value(&p, 401), 1, "one point in every branch: the fewest is 1");
        assert_eq!(gd.achievements.get(401).unwrap().reached(value(&p, 401)), 0, "not reached with 1 point each");
        for b in 1..=4u16 {
            p.skills.insert(b * 100 + 12, 5);
        }
        refresh_state(&gd, true, &mut p);
        assert_eq!(value(&p, 401), 1, "branch 5 still has 1 point");
        p.skills.insert(512, 2);
        p.skills.insert(513, 3);
        let ch = refresh_state(&gd, true, &mut p);
        assert_eq!(id_of(&ch, 401).unwrap(), Change { id: 401, kind: 25, value: 5, lo: 0, hi: 1 }, "5 points in every branch");
        assert!(p.items.iter().any(|i| i.dict_id == 265), "the reward item of 401");
        assert_eq!(least_branch_points(&[9, 7, 5, 8, 6], 63), 5, "the sixth mask bit has no branch");
        assert_eq!(least_branch_points(&[9, 0, 5, 8, 6], 0b1), 9, "only the masked branches");
        assert_eq!(least_branch_points(&[9, 7, 5, 8, 6], 1 << 5), 0, "no existing branch masked");
    }

    /// 627 / 628: `mode_mask` 32 is bit 5 = Search and Destroy (their texts; 627 "5 - 0" = the 5 rounds of a sad win);
    /// an Artifact Hunt win (mode 9) does not count, a Search and Destroy loss neither.
    #[test]
    fn search_and_destroy_wins_count_627_and_628() {
        game_data::require_tables!();
        let gd = gd();
        let mut p = Profile::starter();
        let mut f = MatchFacts::local(2, 5, true, true, 1);
        f.score_diff = Some(4);
        let ch = on_match(&gd, true, &mut p, &f);
        assert_eq!(id_of(&ch, 628).unwrap().value, 1, "a sad win");
        assert!(id_of(&ch, 627).is_none(), "5 - 1: a lead of 4");
        assert!(id_of(&ch, 611).is_none() && id_of(&ch, 610).is_none());
        f.score_diff = Some(5);
        let ch = on_match(&gd, true, &mut p, &f);
        assert_eq!((id_of(&ch, 627).unwrap().value, value(&p, 628)), (1, 2), "5 - 0");
        let mut lost = f.clone();
        lost.won = false;
        assert!(id_of(&on_match(&gd, true, &mut p, &lost), 628).is_none());
        let mut ah = f.clone();
        ah.mode = 9;
        let ch = on_match(&gd, true, &mut p, &ah);
        assert!(id_of(&ch, 628).is_none() && id_of(&ch, 627).is_none(), "artifact hunt is not in the mask");
        assert_eq!(value(&p, 628), 2);
    }

    #[test]
    fn a_reached_state_gives_its_reward_item_once() {
        game_data::require_tables!();
        let gd = gd();
        let mut p = Profile::starter();
        p.xp = gd.levels.xp_table()[8]; // level 10: first state of get_level (413), reward item 2142
        let ch = refresh_state(&gd, true, &mut p);
        let c = id_of(&ch, 413).unwrap();
        assert_eq!((c.value, c.lo, c.hi), (10, 0, 1));
        assert_eq!(p.items.iter().filter(|i| i.dict_id == 2142).count(), 1);
        // the same state is not rewarded again, and a second figure change does not duplicate the item
        assert!(id_of(&refresh_state(&gd, true, &mut p), 413).is_none());
        p.xp = gd.levels.xp_table()[9];
        refresh_state(&gd, true, &mut p);
        assert_eq!(p.items.iter().filter(|i| i.dict_id == 2142).count(), 1);
    }

    fn kill(cat: u16, flags: u8) -> KillFact {
        KillFact { weapon_category: Some(cat), kill_flags: Some(flags), ..KillFact::default() }
    }

    fn value(p: &Profile, id: u16) -> u32 {
        p.ach.get(&id).map_or(0, |x| x.value)
    }

    #[test]
    fn the_local_facts_move_no_kill_or_score_achievement() {
        game_data::require_tables!();
        let gd = gd();
        let mut p = Profile::starter();
        let ch = on_match(&gd, true, &mut p, &MatchFacts::local(4, 1, true, true, 1));
        // only the win based ones (611 tdm, 616 streak); 601 / 602 need kills
        let ids: Vec<u16> = ch.iter().map(|c| c.id).collect();
        assert_eq!(ids, vec![611, 616]);
    }

    #[test]
    fn full_facts_drive_the_kill_and_score_achievements() {
        game_data::require_tables!();
        let gd = gd();
        let mut p = Profile::starter();
        let mut f = MatchFacts::local(4, 2, true, true, 1);
        let mut kills = vec![
            kill(13, KILL_HEADSHOT | KILL_MELEE),
            kill(8, KILL_MELEE),
            kill(8, KILL_BLEEDING),
            kill(15, KILL_RICOCHET),
            kill(24, 0),
            kill(17, 0),
        ];
        // victim states are a mask (bit 1 aiming, 2 reloading, 3 destroying an anomaly)
        kills[0].victim_state = Some(1 << 1);
        kills[0].victim_category = Some(13);
        kills[0].no_scope = Some(true);
        kills[1].victim_state = Some(1 << 2);
        kills[2].victim_state = Some(1 << 3);
        kills[3].last_bullet = Some(true);
        kills[4].inflictor_type = Some(1);
        kills[4].inflictor_id = Some(9);
        kills[5].inflictor_type = Some(1);
        kills[5].inflictor_id = Some(9);
        kills[5].killer_hp = Some(10);
        kills[5].after_own_death = Some(true);
        kills[5].grenade_in_hand = Some(true);
        f.kills = Some(kills);
        f.deaths = Some(0);
        f.max_killstreak = Some(6);
        f.artefacts = Some(4);
        f.score_diff = Some(60);
        f.killed_all_enemies = Some(true);
        // the world's row: the kill kinds of the six kills (6 kills, 2 melee, 1 headshot, 1 bleeding) and the objectives
        f.stat_events = Some([(0, 6), (13, 2), (15, 1), (37, 1), (1, 3), (24, 2), (29, 700)].into());
        // the four post-match medals (one state each, 0-based state 0), three gold (third) states, one silver
        f.badges = Some(vec![(23, 0), (24, 0), (25, 0), (26, 0), (5, 2), (7, 2), (8, 2), (9, 2), (7, 1)]);
        f.match_type = Some(1);
        f.in_squad = Some(true);
        f.in_clan_squad = Some(false);
        on_match(&gd, true, &mut p, &f);
        let v = |id| value(&p, id);
        assert_eq!(v(201), 6, "kills (counter)");
        assert_eq!((v(202), v(203), v(206), v(209)), (2, 1, 1, 1), "melee, headshot, bleeding, ricochet");
        assert_eq!((v(216), v(217), v(219), v(215)), (1, 2, 1, 1), "sniper, assault, pistol, thrown weapon");
        assert_eq!((v(208), v(211), v(228)), (1, 1, 1), "reload, anomaly, sniper in the sight");
        assert_eq!(v(204), 3, "assists");
        assert_eq!(v(214), 6, "killstreak");
        assert_eq!(v(205), 2, "two kills by one grenade (the figure; the world feed leaves grenade ids open)");
        assert_eq!(v(207), 0, "no kill by a bullet inflictor");
        assert_eq!((v(210), v(212), v(213), v(227), v(224)), (1, 1, 1, 1, 1));
        assert_eq!(v(223), 1);
        assert_eq!(v(229), 0, "229: what a 'way' is has no source (OPEN): never counted");
        assert_eq!((v(601), v(602)), (6, 6), "kills of a won match (601: without a death)");
        assert_eq!((v(617), v(618), v(619)), (2, 0, 700));
        assert_eq!(v(407), 4);
        assert_eq!((v(603), v(604), v(605), v(606), v(607), v(622)), (4, 1, 1, 1, 1, 1));
        assert_eq!((v(611), v(612), v(614), v(615), v(608), v(609)), (0, 1, 0, 1, 1, 0));
        assert_eq!((v(419), v(418)), (1, 0), "squad matches: the squad fact, clan squad false");
        assert_eq!(v(615), 1, "ctp dominating: lead 60 >= 50");
        // a lost match with deaths: 601 needs a win and no death, 602 a win
        let mut g = f.clone();
        g.won = false;
        g.deaths = Some(2);
        let before = (v(601), v(602));
        on_match(&gd, true, &mut p, &g);
        assert_eq!((value(&p, 601), value(&p, 602)), before, "the best match stays");
        assert_eq!(value(&p, 201), 12, "counters add up");
    }

    #[test]
    fn the_worlds_rows_count_without_a_kill_list_and_210_counts_per_life() {
        game_data::require_tables!();
        let gd = gd();
        let mut p = Profile::starter();
        let mut f = MatchFacts::local(4, 1, true, true, 1);
        f.deaths = Some(0);
        f.stat_events = Some([(0, 9), (13, 1), (15, 4), (37, 2)].into());
        on_match(&gd, true, &mut p, &f);
        let v = |p: &Profile, id| value(p, id);
        assert_eq!((v(&p, 201), v(&p, 202), v(&p, 203), v(&p, 206), v(&p, 601), v(&p, 602)), (9, 1, 4, 2, 9, 9));
        assert_eq!((v(&p, 216), v(&p, 209), v(&p, 210)), (0, 0, 0), "per-kill achievements need the kill list");
        // 210: three low-health kills, two of them in one life: the figure is 2
        let mut p = Profile::starter();
        let low = |life| KillFact { killer_hp: Some(10), life: Some(life), ..KillFact::default() };
        f.kills = Some(vec![low(0), low(1), low(1), KillFact { killer_hp: Some(80), life: Some(1), ..KillFact::default() }]);
        on_match(&gd, true, &mut p, &f);
        assert_eq!(v(&p, 210), 2);
        // 225 / 621 from their match facts
        assert_eq!((v(&p, 225), v(&p, 621)), (0, 0));
        f.squad_damage_kill = Some(true);
        f.squad_all_medals = Some(true);
        on_match(&gd, true, &mut p, &f);
        assert_eq!((v(&p, 225), v(&p, 621)), (1, 1));
    }

    /// The world host's kill log through `match_data::fill_facts` into the per-kill achievements: 205, 207, 208, 209, 210, 211, 212, 224, 225, 226, 227 and 228 count from the slot 3 arguments, 402 / 414
    /// from the host's vitals (protocol 8); 213 and 229 have no source and never move.
    #[test]
    fn the_world_kill_log_drives_the_per_kill_achievements() {
        game_data::require_tables!();
        use crate::match_data::{fill_facts, RosterSlot, WorldData};
        use world_host_proto::{kill_flag as kf, KillRecord, PlayerDetail};
        let gd = gd();
        let roster = vec![
            RosterSlot { slot: 0, account: Some(10), team: 0, entered: true },
            RosterSlot { slot: 1, account: Some(11), team: 0, entered: true },
            RosterSlot { slot: 2, account: None, team: 1, entered: true },
            RosterSlot { slot: 3, account: None, team: 1, entered: true },
            RosterSlot { slot: 4, account: None, team: 1, entered: true },
        ];
        let rec = |t, victim, dict, cat, flags, damage_type, hp| KillRecord {
            t,
            killer: 0,
            victim,
            weapon_dict: dict,
            weapon_category: cat,
            killer_hp: hp,
            damage_type,
            flags: flags | kf::INFLICTOR,
            damagers: 1,
            ..Default::default()
        };
        let alive = kf::KILLER_ALIVE;
        let mut sniper = rec(1000, 2, 107, 13, alive | kf::HIPFIRE | kf::HEAD_ZONE, 5 << 8, 100);
        sniper.damagers = 0b11; // damaged by both squad members
        let mut ricochet_reload = rec(2000, 3, 1201, 8, alive | kf::VICTIM_C8, 6 << 8, 10);
        ricochet_reload.arg80 = 2; // the death handler's ricochet byte
        let same_bullet = rec(2000, 4, 1201, 8, alive | kf::LAST_ROUND | kf::VICTIM_D0 | kf::VICTIM_ANOMALY, 6 << 8, 12);
        let mut melee = rec(3000, 2, 1201, 8, alive | kf::VICTIM_C0, u32::MAX, 100);
        melee.arg80 = 1;
        melee.victim_weapon_dict = 107; // an aiming mosin_nagant (category 13): an enemy sniper
        let died = KillRecord { t: 4000, killer: 2, victim: 0, weapon_dict: 952, weapon_category: 8, flags: alive, ..Default::default() };
        let after_death = rec(5000, 3, 9, 24, 0, 0x2000, 100); // a grenade of a dead killer (his shot 0x20)
        let same_grenade = rec(5000, 4, 9, 24, 0, 0x2000, 100); // the same grenade's second victim
        let mine = |t, v| rec(t, v, 30, 11, alive, 0x3001, 100); // one mine (shot 0x30, type 1), two victims
        let other_grenade = rec(9000, 2, 9, 24, alive, 0x2100, 100);
        // a bleeding death with a hipfire / last-round free record and no shot id: not a bullet kill
        let mut bleeding = rec(6000, 4, 1201, 8, alive | kf::HEALTH_ZONE | kf::INFLICTOR_69, u32::MAX, 100);
        bleeding.arg80 = 2;
        let log = vec![sniper, ricochet_reload, same_bullet, melee, died, after_death, same_grenade, bleeding, mine(7000, 2), mine(7000, 3), other_grenade];
        let vitals = Some(world_host_proto::Vitals { regen_best: 130, artefact_hp_min: 1 });
        let w = WorldData { roster, details: vec![PlayerDetail { slot: 0, vitals, ..Default::default() }], kills: Some(log), bots: vec![] };
        let mut f = MatchFacts::local(4, 1, true, true, 1);
        fill_facts(&mut f, &w, 10, &[10, 11]);
        assert_eq!(f.squad_damage_kill, Some(true));
        let mut p = Profile::starter();
        on_match(&gd, true, &mut p, &f);
        let v = |id| value(&p, id);
        assert_eq!(v(224), 1, "a hipfire sniper kill");
        assert_eq!(v(225), 1, "an enemy damaged by the whole squad");
        assert_eq!((v(207), v(209), v(208)), (2, 1, 1), "two kills by one bullet; a ricochet (not the bleeding one); a reloading victim");
        assert_eq!((v(227), v(211)), (1, 1), "the last round; a victim destroying an anomaly");
        assert_eq!(v(210), 2, "two kills under 15 % health in the first life");
        assert_eq!(v(228), 1, "melee kill of an aiming enemy sniper");
        assert_eq!(v(212), 2, "two kills after his death");
        assert_eq!((v(205), v(226)), (2, 2), "two kills by one grenade (shot 0x20; 0x21 is another), two by one mine");
        assert_eq!((v(402), v(414)), (130, 1), "130 % regenerated in one life; an artefact at 1 % health");
        assert_eq!((v(213), v(229)), (0, 0), "no source: never counted");
        assert!(p.items.iter().any(|i| i.dict_id == 171) && p.items.iter().any(|i| i.dict_id == 249), "208 / 211 rewards");
        // alone (no squad) 225 does not count; a squad mate who never damaged the victim neither
        let mut g = MatchFacts::local(4, 1, true, true, 1);
        fill_facts(&mut g, &w, 10, &[]);
        assert_eq!(g.squad_damage_kill, Some(false));
        let mut h = MatchFacts::local(4, 1, true, true, 1);
        let mut w2 = w.clone();
        w2.kills.as_mut().unwrap()[0].damagers = 0b01;
        fill_facts(&mut h, &w2, 10, &[10, 11]);
        assert_eq!(h.squad_damage_kill, Some(false));
    }

    #[test]
    fn an_absent_fact_stops_a_counter_and_a_leaver_counts_nothing() {
        game_data::require_tables!();
        let gd = gd();
        let mut p = Profile::starter();
        let mut f = MatchFacts::local(4, 1, true, true, 1);
        f.kills = Some(vec![KillFact::default()]); // kills seen, but nothing known about them
        f.deaths = None;
        on_match(&gd, true, &mut p, &f);
        assert_eq!(value(&p, 201), 1, "an unqualified kill still counts as a kill");
        assert_eq!(value(&p, 202), 0, "melee kills need the kill type");
        assert_eq!(value(&p, 217), 0);
        assert_eq!(value(&p, 601), 0, "deaths unknown");
        let mut leaver = f.clone();
        leaver.finished = false;
        assert!(on_match(&gd, true, &mut p, &leaver).is_empty());
    }

    #[test]
    fn category_scores_sum_the_reached_states() {
        game_data::require_tables!();
        let gd = gd();
        let mut p = Profile::starter();
        refresh_state(&gd, true, &mut p);
        let s = gd.achievements.category_scores(|a| p.ach.get(&a).map_or(0, |x| x.value));
        assert_eq!(s.len(), 3);
        let general = s.iter().find(|(c, _)| *c == 4).unwrap().1;
        let want: u32 = gd
            .achievements
            .iter()
            .filter(|a| a.category == 4)
            .map(|a| a.states.iter().take(a.reached(p.ach.get(&a.id).map_or(0, |x| x.value)) as usize).map(|s| s.score).sum::<u32>())
            .sum();
        assert_eq!(u32::from(general), want);
        assert!(want > 0, "the starter profile has ranks, so some state is reached");
    }

    #[test]
    fn upgrades_and_rerolls_follow_the_best_quality_reached() {
        game_data::require_tables!();
        let gd = gd();
        let mut p = Profile::starter();
        refresh_state(&gd, true, &mut p);
        assert!(on_event(&gd, true, &mut p, Event::Upgraded { quality: 0 }).is_empty(), "rank 0 is no state");
        let ch = on_event(&gd, true, &mut p, Event::Upgraded { quality: 1 });
        assert_eq!(ch, vec![Change { id: 420, kind: 19, value: 1, lo: 0, hi: 1 }], "green: the first state");
        // an orange item reached at once (an item whose base rarity starts higher) reaches every state up to it
        assert_eq!(id_of(&on_event(&gd, true, &mut p, Event::Upgraded { quality: 3 }), 420).unwrap(), Change { id: 420, kind: 19, value: 3, lo: 1, hi: 3 });
        assert!(on_event(&gd, true, &mut p, Event::Upgraded { quality: 2 }).is_empty(), "a lower quality later changes nothing");
        assert_eq!(value(&p, 404), 0, "an upgrade is not a reroll");
        let ch = on_event(&gd, true, &mut p, Event::Rerolled { quality: 2 });
        assert_eq!(ch, vec![Change { id: 404, kind: 20, value: 2, lo: 0, hi: 2 }]);
        assert!(on_event(&gd, true, &mut p, Event::Rerolled { quality: 2 }).is_empty(), "the same quality again changes nothing");
        assert!(on_event(&gd, true, &mut p, Event::Rerolled { quality: 4 }).is_empty(), "a gold module is not orange");
    }

    #[test]
    fn customise_hunt_and_daily_quests() {
        game_data::require_tables!();
        let gd = gd();
        let mut p = Profile::starter();
        refresh_state(&gd, true, &mut p);
        assert_eq!(on_event(&gd, true, &mut p, Event::Customised), vec![Change { id: 403, kind: 24, value: 1, lo: 0, hi: 1 }]);
        assert!(on_event(&gd, true, &mut p, Event::Customised).is_empty(), "once reached stays reached");
        assert_eq!(on_event(&gd, true, &mut p, Event::HuntCompleted), vec![Change { id: 406, kind: 28, value: 1, lo: 0, hi: 1 }]);
        // 620: one per completed daily quest, first state at 5, the last state's reward is item 183
        assert_eq!(on_event(&gd, true, &mut p, Event::DailyQuests(3)), vec![Change { id: 620, kind: 27, value: 3, lo: 0, hi: 0 }]);
        assert_eq!(on_event(&gd, true, &mut p, Event::DailyQuests(2)), vec![Change { id: 620, kind: 27, value: 5, lo: 0, hi: 1 }]);
        assert!(on_event(&gd, true, &mut p, Event::DailyQuests(0)).is_empty());
        on_event(&gd, true, &mut p, Event::DailyQuests(495));
        assert_eq!(value(&p, 620), 500);
        assert_eq!(p.items.iter().filter(|i| i.dict_id == 183).count(), 1, "the reward of the last state");
    }

    #[test]
    fn pve_wins_need_the_caches_of_the_precondition() {
        game_data::require_tables!();
        let gd = gd();
        let mut p = Profile::starter();
        refresh_state(&gd, true, &mut p);
        assert!(on_event(&gd, true, &mut p, Event::PveWin { stashes: 2 }).is_empty(), "629 needs 3 caches");
        assert_eq!(on_event(&gd, true, &mut p, Event::PveWin { stashes: 3 }), vec![Change { id: 629, kind: 29, value: 1, lo: 0, hi: 0 }]);
        let ch = on_event(&gd, true, &mut p, Event::PveWin { stashes: 10 });
        assert_eq!(ch, vec![Change { id: 629, kind: 29, value: 2, lo: 0, hi: 0 }, Change { id: 630, kind: 29, value: 1, lo: 0, hi: 1 }]);
        for _ in 0..8 {
            on_event(&gd, true, &mut p, Event::PveWin { stashes: 5 });
        }
        assert_eq!((value(&p, 629), value(&p, 630)), (10, 1));
        assert_eq!(gd.achievements.get(629).unwrap().reached(10), 1);
    }

    #[test]
    fn hero_titles_move_the_achievement_of_their_nomination_by_place() {
        game_data::require_tables!();
        let gd = gd();
        let mut p = Profile::starter();
        refresh_state(&gd, true, &mut p);
        // third place in Killstreak = state 1 of 624
        assert_eq!(on_event(&gd, true, &mut p, Event::HeroTitle { nomination: 2, place: 3 }), vec![Change { id: 624, kind: 30, value: 1, lo: 0, hi: 1 }]);
        assert_eq!(on_event(&gd, true, &mut p, Event::HeroTitle { nomination: 2, place: 1 }), vec![Change { id: 624, kind: 30, value: 3, lo: 1, hi: 3 }]);
        assert!(on_event(&gd, true, &mut p, Event::HeroTitle { nomination: 2, place: 2 }).is_empty(), "a lower place later keeps the best");
        assert_eq!((value(&p, 623), value(&p, 625)), (0, 0));
        assert_eq!(on_event(&gd, true, &mut p, Event::HeroTitle { nomination: 1, place: 2 }), vec![Change { id: 623, kind: 30, value: 2, lo: 0, hi: 2 }]);
        assert_eq!(on_event(&gd, true, &mut p, Event::HeroTitle { nomination: 3, place: 1 })[0].id, 625);
        assert!(on_event(&gd, true, &mut p, Event::HeroTitle { nomination: 3, place: 4 }).is_empty());
    }

    #[test]
    fn lobby_event_kinds_are_not_moved_by_match_facts() {
        game_data::require_tables!();
        let gd = gd();
        let mut p = Profile::starter();
        on_match(&gd, true, &mut p, &MatchFacts::local(4, 1, true, true, 1));
        for id in [403, 404, 406, 415, 416, 417, 420, 620, 623, 624, 625, 629, 630] {
            assert_eq!(value(&p, id), 0, "{id}");
        }
    }

    /// A starter whose first worn item has `n` random modifiers and a spare (not worn) item has three.
    fn wearing_mods(n: u16) -> Profile {
        let mut p = Profile::starter();
        let worn = *p.slots.values().next().expect("the starter wears something");
        p.mods.insert(worn, (0..n).map(|i| (10 + i, 1)).collect());
        let spare = p.add_item(107, 107);
        p.mods.insert(spare, vec![(10, 1), (20, 1), (30, 1)]);
        p
    }

    fn reached_415_to_417(gd: &GameData, p: &Profile) -> (u8, u8, u8) {
        let a = |id: u16| gd.achievements.get(id).unwrap().reached(value(p, id));
        (a(415), a(416), a(417))
    }

    #[test]
    fn wearing_items_with_random_modifiers_counts_nothing_without_a_match() {
        game_data::require_tables!();
        let gd = gd();
        let mut p = wearing_mods(3);
        // sign-in, after an equip / skill change and after every lobby event
        assert!(refresh_state(&gd, true, &mut p).iter().all(|c| !(415..=417).contains(&c.id)));
        on_event(&gd, true, &mut p, Event::Customised);
        assert_eq!((value(&p, 415), value(&p, 416), value(&p, 417)), (0, 0, 0));
        // a local match (no loadout figure) counts nothing either
        on_match(&gd, true, &mut p, &MatchFacts::local(4, 1, true, true, 1));
        assert_eq!(reached_415_to_417(&gd, &p), (0, 0, 0));
    }

    #[test]
    fn a_finished_online_match_with_n_random_modifiers_reaches_415_to_417() {
        game_data::require_tables!();
        let gd = gd();
        let now = crate::premium::unix_now();
        for (n, want) in [(0u16, (0, 0, 0)), (1, (1, 0, 0)), (2, (1, 1, 0)), (3, (1, 1, 1))] {
            let mut p = wearing_mods(n);
            // the spare item is not in the loadout: only the worn one counts
            let (mods, src) = match_mods(&start_mods(&p, now), None);
            assert_eq!((mods, src), (u32::from(n), ModsSource::StartProfile));
            let mut f = MatchFacts::local(4, 1, false, true, 1);
            f.item_mods = Some(mods);
            on_match(&gd, true, &mut p, &f);
            assert_eq!(reached_415_to_417(&gd, &p), want, "{n} modifier(s)");
        }
    }

    #[test]
    fn a_left_match_counts_no_item_modifiers() {
        game_data::require_tables!();
        let gd = gd();
        let mut p = wearing_mods(3);
        let mut f = MatchFacts::local(4, 1, true, false, 1);
        f.item_mods = Some(3);
        assert!(on_match(&gd, true, &mut p, &f).is_empty());
        assert_eq!(reached_415_to_417(&gd, &p), (0, 0, 0));
    }

    #[test]
    fn the_used_rows_spawned_profiles_pick_the_figure() {
        assert_eq!(match_mods(&[2, 3, 0], Some(0b001)), (2, ModsSource::Spawned));
        assert_eq!(match_mods(&[2, 3, 0], Some(0b011)), (3, ModsSource::Spawned));
        assert_eq!(match_mods(&[2, 3, 0], Some(0b100)), (0, ModsSource::Spawned));
        assert_eq!(match_mods(&[2, 3, 0], Some(0)), (0, ModsSource::Spawned), "never spawned");
        assert_eq!(match_mods(&[2, 3, 0], Some(0xFFF8)), (0, ModsSource::Spawned), "bits past the options' profiles");
        assert_eq!(match_mods(&[2, 3, 0], None), (2, ModsSource::StartProfile));
        assert_eq!(match_mods(&[], None), (0, ModsSource::StartProfile));
    }

    #[test]
    fn start_mods_follow_the_options_profiles() {
        let now = crate::premium::unix_now();
        let mut p = wearing_mods(1);
        // profile 1 holds a three-modifier item but is not owned: sent without slots, figure 0
        let other = p.add_item(107, 107);
        p.mods.insert(other, vec![(10, 1), (20, 1), (30, 1)]);
        p.equip.profiles[1].slots.insert(7, other);
        assert_eq!(start_mods(&p, now), vec![1, 0, 0]);
        p.equip.profiles[1].owned = true;
        assert_eq!(start_mods(&p, now), vec![1, 3, 0]);
        // a disabled tab 0: the current profile moves to the front of the options (here it is profile 0 itself)
        p.equip.profiles[0].enabled = false;
        assert_eq!(crate::profiles::options_order(&p, now), vec![(0, true), (1, true), (2, false)]);
    }

    #[test]
    fn quality_is_the_rank_of_an_item_and_the_rarity_of_a_module() {
        game_data::require_tables!();
        let gd = gd();
        let mut p = Profile::starter();
        let mosin = p.items.iter().find(|i| i.dict_id == 107).unwrap().uid;
        assert_eq!(quality(&gd, &p, mosin), Some(0));
        p.mods.insert(mosin, vec![(201, 0), (202, 1)]);
        assert_eq!(quality(&gd, &p, mosin), Some(2), "two modifiers on a base-0 item: blue");
        let module = gd.items.iter().find(|i| gd.items.is_module(i.dict_id)).unwrap().dict_id;
        let m = p.add_item(module, 1);
        p.mods.insert(m, vec![(game_data::merge::RARITY_GROUP, 3), (100, 0)]);
        assert_eq!(quality(&gd, &p, m), Some(3), "a module's quality is its rarity (3 orange)");
        assert_eq!(quality(&gd, &p, 999_999), None);
    }

    #[test]
    fn login_b9_carries_every_achievement_without_popups() {
        game_data::require_tables!();
        let gd = gd();
        let p = Profile::starter();
        let S2c::Raw { id, payload } = login_push(&gd, &p) else { panic!() };
        assert_eq!(id, 0xB9);
        let m = AchievementsB9::from_bytes(&payload).unwrap();
        assert_eq!(m.entries.len(), gd.achievements.len());
        assert!(m.entries.iter().all(|e| e.lo == e.hi));
        let e201 = m.entries.iter().find(|e| e.id == 201).unwrap();
        assert_eq!((e201.value, e201.state), (0, 0), "no kills: no state reached");
        let e413 = m.entries.iter().find(|e| e.id == 413).unwrap();
        assert_eq!(e413.state, 0, "level 1 is below the first level state (10); the type (21) is not sent");
    }

    fn with(p: &mut Profile, id: u16, value: u32) {
        p.ach.insert(id, AchProgress { value, run: 0 });
    }

    fn state_of(entries: &[AchEntry], id: u16) -> u8 {
        entries.iter().find(|e| e.id == id).unwrap().state
    }

    #[test]
    fn the_entry_byte_is_the_number_of_states_reached() {
        game_data::require_tables!();
        let gd = gd();
        let mut p = Profile::starter();
        // 201 kill (type 0), states at 10 / 100 / 1000 / 2500 / 5000
        with(&mut p, 201, 150);
        // 401 learn_all_skill_branches (type 25, one state at 5), 413 get_level (type 21, ten states 10..100)
        with(&mut p, 401, 0);
        with(&mut p, 413, 25);
        let all = all_entries(&gd, &p);
        assert_eq!(state_of(&all, 201), 2, "a type-0 kill counter at 150 has reached two states (10, 100)");
        assert_eq!(state_of(&all, 401), 0, "a large-type achievement without progress is not achieved");
        assert_eq!(state_of(&all, 413), 2, "level 25: states 10 and 20");
        with(&mut p, 201, 5000);
        assert_eq!(state_of(&all_entries(&gd, &p), 201), 5, "all five states");
        with(&mut p, 201, 999_999);
        assert_eq!(state_of(&all_entries(&gd, &p), 201), 5, "never more than the state count");
        // 83 55 02 carries the same byte: {u16 id, u8 state, u32 value} after `02 u32 n`
        with(&mut p, 201, 150);
        let r = progress_reply(&all_entries(&gd, &p)).unwrap();
        let at = 5 + 7 * gd.achievements.iter().position(|a| a.id == 201).unwrap();
        assert_eq!(r[at..at + 7], [201, 0, 2, 150, 0, 0, 0]);
    }

    #[test]
    fn every_achievement_sends_the_count_of_thresholds_its_value_reached() {
        game_data::require_tables!();
        let gd = gd();
        for a in gd.achievements.iter() {
            let conds: Vec<u32> = a.states.iter().map(|s| s.condition).collect();
            for &c in &conds {
                for v in [c.saturating_sub(1), c] {
                    let mut p = Profile::starter();
                    with(&mut p, a.id, v);
                    let want = conds.iter().filter(|&&t| v >= t).count() as u8;
                    assert_eq!(state_of(&all_entries(&gd, &p), a.id), want, "achievement {} at {v}", a.id);
                    assert!(usize::from(want) <= a.states.len());
                }
            }
        }
    }

    #[test]
    fn a_pushed_change_carries_its_reached_states_as_the_entry_byte() {
        game_data::require_tables!();
        let gd = gd();
        let mut p = Profile::starter();
        with(&mut p, 203, 49);
        let a = gd.achievements.get(203).unwrap();
        let ch = store(&gd, false, &mut p, a, 50, 0).unwrap();
        assert_eq!((ch.lo, ch.hi), (1, 2), "203 headshots: 50 reaches the second state");
        let S2c::Raw { payload, .. } = &changes_b9(&gd, &p, &[ch])[0] else { panic!() };
        let e = AchievementsB9::from_bytes(payload).unwrap().entries[0];
        assert_eq!((e.id, e.value, e.state, e.lo, e.hi), (203, 50, 2, 1, 2));
    }

    use protocol::Decode;
}
