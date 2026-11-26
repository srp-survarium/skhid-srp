//! The world's own match data (world host protocol 3): what the exe's statistics object knew at the end of an online
//! match, turned into [`MatchFacts`] and the medal fields of the `97`.
//!
//! The world host reads, per record slot, the statistics block `B = S + 0x30 + slot*0xA24` (PROVEN offsets): the
//! 74-event count row, the kill streaks, the inactive flag and, after the world's match end (`S.vt[0x110]`), the 27
//! badge counts. It also catches every death at the statistics
//! handler's slot 3 (0x2001FB490): the kill log. The relay hands both over at the match end ([`WorldData`], one per
//! match, shared by the players' [`crate::EndedStat`]).
//!
//! What fills which fact ([`fill_facts`]):
//! * `stat_events` = the player's event row (every event of table 0x200986E10); `max_killstreak` = best streak
//!   (`B+0xA1E`); `artefacts` = event 5 (`artefact`, quest 28's parameter; achievement 407 "Pick up N artifacts");
//!   `inactive` = `B+0x9F0` (set by the world only when the options match type is 1 or 3); an inactive player gets no
//!   reward, no drops, no achievement, no daily-quest progress and no Winner's Contract day (CHOICE: online.rs `pay`,
//!   stats.rs `record_match`);
//! * `badges` = one `(type, state)` per earned instance, `state` 0-based (the `97` R block byte order `3t + s`;
//!   LIKELY the numbering of achievement 603's `badge_state`, OPEN);
//!   `squad_all_medals` = the player's squad of this match holds the four post-match medals (types 23..26);
//! * from the kill log (when it agreed with the event rows; the relay checks that): `kills` = the player's kills of
//!   enemies ([`kill_facts`]); `killed_all_enemies` = every entered enemy was a victim of the player (achievement 223's
//!   text); `squad_damage_kill` = a kill of his on an enemy that every member of his squad had damaged (225).
//!
//! The kill arguments (death handler 0x2003046B0, slot 3 0x2001FB490, core 0x20033DCF0; PROVEN): `weapon_category` =
//! the killing item's `category_id` (+0x307); headshot / bleeding = the hit zone ([`kill_flag::HEAD_ZONE`],
//! [`kill_flag::HEALTH_ZONE`]); melee = the handler's melee byte (`arg80`) on a kill that is not a grenade (24), trap
//! (11 / 55) or artefact (12) kill, as the core counts `melee_kill` (0x20033E155); a bullet kill = a firearm kill whose
//! `damage_type` is the killer's shot id ([`shot_of_killer`]); ricochet = `arg80` bit 1 (0x20039A17F); `killer_hp` = the
//! killer's health in percent, truncated (0x200344E70); [`kill_flag::KILLER_ALIVE`] = killer +0x748; victim aiming = his
//! weapon's aim flag +0x44C4; reloading = the weapon's reload state (0x2006D6A20); [`kill_flag::VICTIM_ANOMALY`] = the
//! victim was using an anomaly object (victim+0x290); [`kill_flag::HIPFIRE`], [`kill_flag::LAST_ROUND`]; `damagers` =
//! the victim's damage-received rows (`B+0x678`) at the death. Not read (no use): `item_uid`, `INFLICTOR_69`, `ARG_A0` /
//! `ARG_A8` / `ARG_B0`, `KILLER_6D8` / `VICTIM_6D8`.

use std::sync::Arc;

use accounts::AccountId;
use game_data::facts::{ev, KillFact};
use game_data::MatchFacts;
use game_data::facts::{KILL_BLEEDING, KILL_HEADSHOT, KILL_MELEE, KILL_RICOCHET};
use world_host_proto::{kill_flag, KillRecord, PlayerDetail, BADGES};

/// Weapon categories of firearms (8 assault, 13 sniper, 14 shotgun, 15 SMG, 16 machine gun, 17 handgun: 216..221).
pub const FIREARMS: [u8; 6] = [8, 13, 14, 15, 16, 17];
/// The throwing-weapon category (215; the core's `grenade_kill`, 0x20033E102).
pub const GRENADES: u8 = 24;
/// The trap categories (222; the core's `trap_kill`, 0x20033E129).
pub const TRAPS: [u8; 2] = [11, 55];
/// The artefact category (the core's `artefact_kill`, 0x20033E131).
pub const ARTEFACTS: u8 = 12;
/// Kills by one shot id lie closer than this (ms): the per-player shot counter wraps only after 2048 shots.
const SHOT_WINDOW_MS: u32 = 10_000;

/// One slot of the match: a human with his account, or a server bot (no account).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RosterSlot {
    pub slot: u8,
    pub account: Option<AccountId>,
    pub team: u8,
    pub entered: bool,
}

/// The world's data of one finished match (world host protocol 3).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WorldData {
    pub roster: Vec<RosterSlot>,
    /// detail rows of the slots (event row, streaks, inactive flag, badges: badges only after the world's match end)
    pub details: Vec<PlayerDetail>,
    /// every death of the match, in time order; None = no complete kill log
    pub kills: Option<Vec<KillRecord>>,
    /// PvP: the AI players with their name indexes and the host's counters, for the result window's rows
    pub bots: Vec<BotRow>,
}

/// One AI player of a bot-filled PvP match as the `97` lists it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BotRow {
    pub slot: u8,
    pub team: u8,
    /// `players_1st_names` / `players_2nd_names` indexes of its record (+0x2F2 / +0x2F3); None = not known
    pub names: Option<(u8, u8)>,
    /// its `ai_players` index (the record's AI id, +0x2E0): the kit the result window's equipment view shows (`93`);
    /// None = not known
    pub ai: Option<u16>,
    /// event rows 0 / 2 and the score `+0xA1C` of its statistics block (the host's Final / Outcome)
    pub kills: u16,
    pub deaths: u16,
    pub score: u16,
}

pub type SharedWorld = Arc<WorldData>;

impl WorldData {
    pub fn slot_of(&self, account: AccountId) -> Option<&RosterSlot> {
        self.roster.iter().find(|r| r.account == Some(account))
    }

    pub fn detail(&self, slot: u8) -> Option<&PlayerDetail> {
        self.details.iter().find(|d| d.slot == slot)
    }

    fn team_of(&self, slot: u8) -> Option<u8> {
        self.roster.iter().find(|r| r.slot == slot).map(|r| r.team)
    }

    /// The account holding post-match medal `t` (badge slot t, count of its only state > 0), if any.
    pub fn medal_holder(&self, t: u8) -> Option<AccountId> {
        let d = self.details.iter().find(|d| d.badges.get(usize::from(t)).is_some_and(|c| c[0] > 0))?;
        self.roster.iter().find(|r| r.slot == d.slot)?.account
    }
}

/// `(type, state)` per earned badge instance of a detail row (state 0-based; count copies).
pub fn badge_list(d: &PlayerDetail) -> Vec<(u8, u8)> {
    let mut v = vec![];
    for (t, c) in d.badges.iter().enumerate().take(BADGES) {
        for (s, &n) in c.iter().enumerate() {
            v.extend(std::iter::repeat_n((t as u8, s as u8), usize::from(n)));
        }
    }
    v
}

/// The 97 R block's badge kinds of a player: `(type, [count of state 1, 2, 3])` for every type with a count
/// (a direct copy of the slot bytes).
pub fn badge_kinds(d: &PlayerDetail) -> Vec<(u8, [u8; 3])> {
    d.badges.iter().enumerate().filter(|(_, c)| c.iter().any(|&n| n > 0)).map(|(t, c)| (t as u8, *c)).collect()
}

/// The kill facts of `me` from the kill log: his kills of enemies, each with the weapon category and the life. No victim
/// filter (CHOICE: bot kills count for rewards, achievements and quests): a server bot in the
/// roster (no account) is an enemy or a teammate by its team like a human; a victim slot outside the roster counts as an
/// enemy.
fn kill_facts(w: &WorldData, me: &RosterSlot, log: &[KillRecord]) -> Vec<KillFact> {
    let mut out = vec![];
    for k in log.iter().filter(|k| k.killer == me.slot && k.victim != me.slot) {
        if w.team_of(k.victim).is_some_and(|t| t == me.team) {
            continue; // a team kill is not a kill (event row 4)
        }
        let life = log.iter().filter(|d| d.victim == me.slot && d.t < k.t).count().min(usize::from(u16::MAX)) as u16;
        out.push(kill_fact(k, life, log));
    }
    out
}

/// Is `k` a bullet kill: a firearm kill (not melee, not a bleeding death) whose `damage_type` is a shot id of the
/// killer (`(killer << 11 | counter) << 8 | pellet`; a bleeding death has no inflictor and -1 here).
pub fn bullet_kill(k: &KillRecord) -> bool {
    let cat = (k.weapon_dict != 0).then_some(k.weapon_category);
    cat.is_some_and(|c| FIREARMS.contains(&c)) && k.arg80 & 1 == 0 && k.flags & kill_flag::HEALTH_ZONE == 0 && shot_of_killer(k)
}

/// Is `damage_type` one of the killer's shot ids (`(killer << 11 | counter) << 8 | type`)? The per-player counter
/// (0x2003BDAE0) numbers every bullet (0x2006D64FA), grenade (0x20039DA20) and trap explosion (0x2003D539F): one id =
/// one bullet / grenade / mine.
pub fn shot_of_killer(k: &KillRecord) -> bool {
    k.damage_type != u32::MAX && k.killer != 0xFF && k.damage_type >> 19 == u32::from(k.killer)
}

/// The kind of inflictor (0 bullet, 1 grenade, 2 mine) of a kill whose damage type is a shot id of the killer.
fn inflictor_kind(k: &KillRecord) -> Option<u8> {
    let cat = (k.weapon_dict != 0).then_some(k.weapon_category)?;
    if bullet_kill(k) {
        Some(0)
    } else if cat == GRENADES && shot_of_killer(k) && k.flags & kill_flag::HEALTH_ZONE == 0 {
        Some(1)
    } else if TRAPS.contains(&cat) && shot_of_killer(k) && k.flags & kill_flag::HEALTH_ZONE == 0 {
        Some(2)
    } else {
        None
    }
}

/// The facts of one kill (module docs: what each argument means).
fn kill_fact(k: &KillRecord, life: u16, log: &[KillRecord]) -> KillFact {
    let cat = (k.weapon_dict != 0 && k.weapon_category != 0xFF).then_some(k.weapon_category);
    let thrown = cat == Some(GRENADES);
    let trap = cat.is_some_and(|c| TRAPS.contains(&c));
    let melee = k.arg80 & 1 != 0 && !thrown && !trap && cat != Some(ARTEFACTS);
    let bullet = bullet_kill(k);
    let has = |b: u16| k.flags & b != 0;
    let mut flags = 0u8;
    for (on, bit) in [(has(kill_flag::HEAD_ZONE), KILL_HEADSHOT), (melee, KILL_MELEE), (has(kill_flag::HEALTH_ZONE), KILL_BLEEDING), (bullet && k.arg80 & 2 != 0, KILL_RICOCHET)] {
        if on {
            flags |= bit;
        }
    }
    let mut states = 0u8;
    for (on, s) in [(has(kill_flag::VICTIM_C0), 1), (has(kill_flag::VICTIM_C8), 2), (has(kill_flag::VICTIM_D0) && has(kill_flag::VICTIM_ANOMALY), 3)] {
        if on {
            states |= 1 << s;
        }
    }
    // one inflictor: a bullet = its shot id and pellet (`damage_type`); the earliest kill of that bullet within the
    // shot window names the group, so kills of one bullet share `inflictor_id`
    let (inflictor_type, inflictor_id) = match inflictor_kind(k) {
        Some(kind) => {
            let first = log.iter().find(|o| o.killer == k.killer && o.damage_type == k.damage_type && inflictor_kind(o) == Some(kind) && k.t.abs_diff(o.t) < SHOT_WINDOW_MS).map_or(k.t, |o| o.t);
            (Some(kind), Some(first))
        }
        None if thrown => (Some(1), None),
        None if trap => (Some(2), None),
        None => (None, None),
    };
    let firearm = cat.is_some_and(|c| FIREARMS.contains(&c)) && !melee;
    KillFact {
        weapon_category: cat.map(u16::from),
        kill_flags: Some(flags),
        inflictor_type,
        inflictor_id,
        victim_state: Some(states),
        victim_category: None,
        victim_weapon_dict: (k.victim_weapon_dict != 0).then_some(k.victim_weapon_dict),
        killer_hp: (k.killer != 0xFF).then_some(u32::from(k.killer_hp)),
        after_own_death: Some(!has(kill_flag::KILLER_ALIVE)),
        grenade_in_hand: None,
        last_bullet: firearm.then_some(has(kill_flag::LAST_ROUND)),
        no_scope: firearm.then_some(has(kill_flag::HIPFIRE)),
        life: Some(life),
    }
}

/// Did `me` kill, or get the assist for (the core's rule 0x20033E660 as the host computed it, `assists`, protocol 8), an
/// enemy that every member of his squad (`squad_slots`, two or more) had damaged in that life? Without the assists (an
/// older host) only his kills count.
fn squad_damage_kill(w: &WorldData, me: &RosterSlot, log: &[KillRecord], squad_slots: &[u8]) -> bool {
    squad_slots.len() > 1
        && log.iter().any(|k| {
            let took_part = k.killer == me.slot || k.assists.is_some_and(|a| me.slot < 32 && a & (1 << me.slot) != 0);
            took_part
                && k.victim != me.slot
                && w.team_of(k.victim).is_none_or(|t| t != me.team)
                && squad_slots.iter().all(|&s| usize::from(s) < 32 && k.damagers & (1 << s) != 0)
        })
}

/// Fill the facts of `account` from the world's data. `squad` = the accounts of his squad in this match (himself
/// included; empty when he played alone). A player the world has no row for keeps every fact as it was.
pub fn fill_facts(f: &mut MatchFacts, w: &WorldData, account: AccountId, squad: &[AccountId]) {
    let Some(me) = w.slot_of(account).cloned() else { return };
    let Some(d) = w.detail(me.slot) else { return };
    f.stat_events = Some(d.events.iter().enumerate().filter(|&(_, &n)| n > 0).map(|(e, &n)| (e as u16, u32::from(n))).collect());
    f.max_killstreak = Some(u32::from(d.best_streak));
    f.artefacts = Some(u32::from(d.events[usize::from(ev::ARTEFACT)]));
    f.inactive = Some(d.inactive);
    if let Some(v) = d.vitals {
        f.regen_best = Some(u32::from(v.regen_best));
        f.artefact_hp_min = Some(v.artefact_hp_min);
    }
    f.badges = Some(badge_list(d));
    let medals_of = |a: &AccountId| w.slot_of(*a).and_then(|r| w.detail(r.slot)).map(|x| game_data::badges::MEDALS.map(|t| x.badges[usize::from(t)][0] > 0));
    f.squad_all_medals = Some(
        squad.len() > 1 && {
            let mut have = [false; 4];
            for m in squad.iter().filter_map(medals_of) {
                for i in 0..4 {
                    have[i] |= m[i];
                }
            }
            have.iter().all(|&x| x)
        },
    );
    if let Some(log) = &w.kills {
        let kills = kill_facts(w, &me, log);
        let enemies: Vec<u8> = w.roster.iter().filter(|r| r.team != me.team && r.entered).map(|r| r.slot).collect();
        f.killed_all_enemies = Some(!enemies.is_empty() && enemies.iter().all(|e| log.iter().any(|k| k.killer == me.slot && k.victim == *e)));
        let squad_slots: Vec<u8> = squad.iter().filter_map(|a| w.slot_of(*a)).map(|r| r.slot).collect();
        f.squad_damage_kill = Some(squad_slots.len() == squad.len() && squad_damage_kill(w, &me, log, &squad_slots));
        f.kills = Some(kills);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world() -> WorldData {
        let roster = vec![
            RosterSlot { slot: 0, account: Some(10), team: 0, entered: true },
            RosterSlot { slot: 1, account: Some(11), team: 0, entered: true },
            RosterSlot { slot: 2, account: Some(12), team: 1, entered: true },
            RosterSlot { slot: 3, account: Some(13), team: 1, entered: true },
            RosterSlot { slot: 4, account: Some(14), team: 1, entered: false },
        ];
        let mut d0 = PlayerDetail { slot: 0, best_streak: 3, inactive: false, ..Default::default() };
        d0.events[0] = 3;
        d0.events[5] = 2;
        d0.events[15] = 1;
        d0.badges[5] = [0, 1, 0];
        d0.badges[23] = [1, 0, 0];
        d0.badges[24] = [1, 0, 0];
        let mut d1 = PlayerDetail { slot: 1, inactive: true, ..Default::default() };
        d1.badges[25] = [1, 0, 0];
        d1.badges[26] = [1, 0, 0];
        let k = |t, killer, victim, cat| KillRecord { t, killer, victim, weapon_dict: if cat == 0xFF { 0 } else { 7 }, weapon_category: cat, ..Default::default() };
        let kills = vec![k(1000, 0, 2, 13), k(2000, 2, 0, 8), k(3000, 0, 3, 8), k(3500, 0, 1, 8), k(4000, 0, 2, 0xFF)];
        WorldData { roster, details: vec![d0, d1, PlayerDetail { slot: 2, ..Default::default() }], kills: Some(kills), bots: vec![] }
    }

    #[test]
    fn the_row_streak_badges_and_kills_become_facts() {
        let w = world();
        let mut f = MatchFacts::local(4, 1, true, true, 1);
        fill_facts(&mut f, &w, 10, &[10, 11]);
        assert_eq!(f.stat(0), Some(3));
        assert_eq!(f.stat(15), Some(1));
        assert_eq!(f.stat(29), Some(0), "an observed row: zero, not unknown");
        assert_eq!((f.max_killstreak, f.artefacts, f.inactive), (Some(3), Some(2), Some(false)));
        assert_eq!(f.badges, Some(vec![(5, 1), (23, 0), (24, 0)]));
        assert_eq!(f.squad_all_medals, Some(true), "23 / 24 his, 25 / 26 his squad mate's");
        let kills = f.kills.clone().unwrap();
        // the team kill of slot 1 is left out; lives: two kills before his death at 2000, two after
        assert_eq!(kills.iter().map(|k| (k.weapon_category, k.life)).collect::<Vec<_>>(), vec![(Some(13), Some(0)), (Some(8), Some(1)), (None, Some(1))]);
        assert!(kills.iter().all(|k| k.kill_flags == Some(0) && k.victim_state == Some(0) && k.after_own_death == Some(true)), "no flag bits: no kind, no victim state, killer not alive");
        assert_eq!(f.killed_all_enemies, Some(true), "slots 2 and 3; slot 4 never entered");
        // alone: no squad medals; a player without a world row keeps his facts
        let mut g = MatchFacts::local(4, 1, true, true, 1);
        fill_facts(&mut g, &w, 11, &[]);
        assert_eq!((g.inactive, g.squad_all_medals, g.killed_all_enemies), (Some(true), Some(false), Some(false)));
        let mut h = MatchFacts::local(4, 1, true, true, 1);
        fill_facts(&mut h, &w, 99, &[]);
        assert_eq!(h, MatchFacts::local(4, 1, true, true, 1));
        // no kill log: no kill facts
        let w2 = WorldData { kills: None, ..world() };
        let mut f2 = MatchFacts::local(4, 1, true, true, 1);
        fill_facts(&mut f2, &w2, 10, &[]);
        assert_eq!((f2.kills, f2.killed_all_enemies), (None, None));
    }

    /// AI players in the roster (no account) on both teams: killing an enemy bot is a kill, killing a team bot is a
    /// team kill (left out), and "every entered enemy" includes the enemy bots.
    #[test]
    fn bot_kills_count_like_human_kills_by_team() {
        let roster = vec![
            RosterSlot { slot: 0, account: Some(10), team: 0, entered: true },
            RosterSlot { slot: 1, account: None, team: 0, entered: true },
            RosterSlot { slot: 2, account: None, team: 1, entered: true },
            RosterSlot { slot: 3, account: None, team: 1, entered: true },
        ];
        let k = |t, killer, victim| KillRecord { t, killer, victim, weapon_dict: 7, weapon_category: 8, ..Default::default() };
        let w = WorldData { roster, details: vec![PlayerDetail { slot: 0, ..Default::default() }], kills: Some(vec![k(1000, 0, 2), k(2000, 0, 1), k(3000, 0, 3)]), bots: vec![] };
        let mut f = MatchFacts::local(4, 1, true, true, 1);
        fill_facts(&mut f, &w, 10, &[]);
        assert_eq!(f.kills.as_ref().map(|v| v.len()), Some(2), "the two enemy bots; the team bot is a team kill");
        assert_eq!(f.killed_all_enemies, Some(true));
        let w2 = WorldData { kills: Some(vec![k(1000, 0, 2)]), ..w };
        let mut g = MatchFacts::local(4, 1, true, true, 1);
        fill_facts(&mut g, &w2, 10, &[]);
        assert_eq!((g.kills.as_ref().map(|v| v.len()), g.killed_all_enemies), (Some(1), Some(false)), "bot 3 was never killed");
    }

    /// The argument mapping of one kill: a bullet kill is a firearm kill whose damage type is the killer's shot id; kills
    /// share a bullet only within the shot window (the counter wraps after 2048 shots); melee is not a melee kill on a
    /// grenade; the hipfire / last-round bits are firearm facts only.
    #[test]
    fn kill_arguments_become_kill_facts() {
        let k = |t, killer, cat, damage_type, flags| KillRecord { t, killer, victim: 2, weapon_dict: 5, weapon_category: cat, damage_type, flags, ..Default::default() };
        let shot = |n: u32| ((1u32 << 11) | n) << 8;
        assert!(bullet_kill(&k(0, 1, 8, shot(7) | 3, 0)), "slot 1's shot 7, pellet 3");
        assert!(!bullet_kill(&k(0, 0, 8, shot(7), 0)), "a shot id of another player");
        assert!(!bullet_kill(&k(0, 1, 8, u32::MAX, kill_flag::HEALTH_ZONE)), "a bleeding death");
        assert!(!bullet_kill(&k(0, 1, 24, shot(7), 0)), "a grenade");
        let log = vec![k(1000, 1, 8, shot(7), 0), k(1500, 1, 8, shot(7), 0), k(40_000, 1, 8, shot(7), 0), k(41_000, 1, 8, shot(8), 0)];
        let ids: Vec<Option<u32>> = log.iter().map(|r| kill_fact(r, 0, &log).inflictor_id).collect();
        assert_eq!(ids, vec![Some(1000), Some(1000), Some(40_000), Some(41_000)], "one bullet twice; the same id 39 s later is another bullet");
        let mut m = k(0, 1, 24, 0x1000, kill_flag::HIPFIRE | kill_flag::LAST_ROUND);
        m.arg80 = 1;
        let f = kill_fact(&m, 0, &[]);
        assert_eq!((f.kill_flags, f.inflictor_type, f.inflictor_id, f.no_scope, f.last_bullet), (Some(0), Some(1), None, None, None), "a thrown grenade: no melee, no identity");
        let mut s = k(0, 1, 13, shot(1), kill_flag::HIPFIRE | kill_flag::VICTIM_C0 | kill_flag::VICTIM_D0 | kill_flag::KILLER_ALIVE);
        s.killer_hp = 14;
        s.victim_weapon_dict = 107;
        let f = kill_fact(&s, 0, &[]);
        assert_eq!((f.no_scope, f.last_bullet, f.victim_state, f.killer_hp, f.after_own_death, f.victim_weapon_dict), (Some(true), Some(false), Some(1 << 1), Some(14), Some(false), Some(107)), "using an object that is no anomaly is not state 3");
    }

    /// 225's assist half (protocol 8): a teammate's kill of an enemy the whole squad damaged counts for a squad member
    /// the host's assists name; without assists (an older host) it does not.
    #[test]
    fn a_squad_assist_counts_with_the_hosts_assists() {
        let roster = vec![
            RosterSlot { slot: 0, account: Some(10), team: 0, entered: true },
            RosterSlot { slot: 1, account: Some(11), team: 0, entered: true },
            RosterSlot { slot: 2, account: None, team: 1, entered: true },
        ];
        let by_mate = KillRecord { t: 1000, killer: 1, victim: 2, weapon_dict: 5, weapon_category: 8, damage_type: ((1 << 11) | 3) << 8, damagers: 0b011, assists: Some(0b001), ..Default::default() };
        let w = WorldData { roster, details: vec![PlayerDetail { slot: 0, ..Default::default() }], kills: Some(vec![by_mate]), bots: vec![] };
        let mut f = MatchFacts::local(4, 1, true, true, 1);
        fill_facts(&mut f, &w, 10, &[10, 11]);
        assert_eq!(f.squad_damage_kill, Some(true));
        let mut w2 = w.clone();
        w2.kills.as_mut().unwrap()[0].assists = None;
        let mut g = MatchFacts::local(4, 1, true, true, 1);
        fill_facts(&mut g, &w2, 10, &[10, 11]);
        assert_eq!(g.squad_damage_kill, Some(false), "no assists from the host: only his own kills");
        let mut w3 = w.clone();
        w3.kills.as_mut().unwrap()[0].assists = Some(0);
        let mut h = MatchFacts::local(4, 1, true, true, 1);
        fill_facts(&mut h, &w3, 10, &[10, 11]);
        assert_eq!(h.squad_damage_kill, Some(false), "under 10 % of the team's damage: no assist");
    }

    #[test]
    fn medal_holders_and_the_97_kinds() {
        let w = world();
        assert_eq!((w.medal_holder(23), w.medal_holder(24), w.medal_holder(25), w.medal_holder(26)), (Some(10), Some(10), Some(11), Some(11)));
        assert_eq!(badge_kinds(&w.details[0]), vec![(5, [0, 1, 0]), (23, [1, 0, 0]), (24, [1, 0, 0])]);
        assert!(badge_kinds(&w.details[2]).is_empty());
    }
}
