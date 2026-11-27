//! Per-player match facts: everything the daily quest and achievement conditions reference.
//!
//! A fact the server does not have is `None` (or an empty `Option` list), NEVER zero: a counter whose fact is `None`
//! does not move. A local (bots) match fills only `mode`, `map`, `won`, `finished` and `faction` (its 53 flush carries
//! only a win / loss marker). A paid online PvP match also gets `match_type`, `in_squad` /
//! `in_clan_squad`, `deaths`, `score`, `team_place` and `score_diff` (lobby `online::pay`) and, with world host data,
//! the event row, kill streak, badges, inactive flag, kill log and vitals (lobby `match_data::fill_facts`).
//!
//! Where a field comes from in the data (names of `achievements.options` / `daily_quests.options`):
//! * `KillFact::kill_flags` = achievement `kill_type`, a bit mask: 2 melee (202), 4 bleeding (206), 8 headshot (203),
//!   16 ricochet (209). PROVEN by the achievement names.
//! * `inflictor_type` 0 bullet, 1 grenade, 2 mine (205 / 207 / 226 "kill by one grenade / bullet / mine": a figure of
//!   kills by ONE inflictor, hence `inflictor_id`). LIKELY numbering, PROVEN by names.
//! * `victim_state` 1 aiming through the sight (228), 2 reloading (208), 3 destroying an anomaly (211): from names;
//!   `KillFact::victim_state` is a mask of the states the victim was in (bit `1 << state`), from three byte arguments
//!   of the world's kill record (lobby `match_data.rs`).
//! * `weapon_category` = item category of the weapon (`item_category_id`, `need_item_of_category`).
//! * `stat_events` keys = `stats_event_type`: 1 assist (204), 24 battery acquired (617), 26 battery stolen (618),
//!   29 `capture_the_point` (619; 100 per whole capture, split over the capturers), 47 `sad_bomb_planted` = the
//!   "Glow of the Forest" delivered to a container in Search and Destroy (626).
//!
//! With the world host (protocol 3) `stat_events` is the player's whole row of the world's event matrix: the count (sum
//! of amounts) of each of the 74 events of [`EVENT_NAMES`], the same table the achievement / quest keys index
//! Kill-kind counts come from those rows (the kill handler 0x20033DCF0 adds
//! headshot / melee / bleeding / grenade / trap rows on top of `kill`); the kill list (`kills`) is needed only for
//! per-kill conditions (weapon class, victim state, killer health, ...).

use std::collections::BTreeMap;

/// The statistics event table of the exe (0x200986E10, 74 names): index = event id of the world's matrix, of `stats_event_type` and of the daily quest type 7
/// `parameter`. The `events.options` reward rows are keyed by these names (row 42 is `match_state_event_pve_open_chest`
/// there: [`event_row_name`]).
pub const EVENT_NAMES: [&str; 74] = [
    "kill",
    "assist",
    "death",
    "suicide",
    "team_kill",
    "artefact",
    "bring_victory_item",
    "artefact_get",
    "teammate_rescue",
    "revenge",
    "victory_item_carry_kill",
    "participating",
    "won",
    "melee_kill",
    "trap_kill",
    "headshot_kill",
    "put_victory_artefact_in_container",
    "steal_victory_item",
    "recover_victory_item",
    "victory_item_found",
    "put_victory_item_in_container",
    "trap_hit",
    "grenade_exploded",
    "grenade_hit",
    "bring_victory_item_singly",
    "grenade_kill",
    "steal_victory_item_singly",
    "played_first_game",
    "kill_invader",
    "capture_the_point",
    "artefact_kill",
    "ressuply_crate_use_ally",
    "artefact_activated",
    "scanner_use_ally",
    "bring_victory_artefact_singly",
    "bring_victory_artefact",
    "multi_kill",
    "bleeding_kill",
    "trap_place",
    "victory_item_carry_rescue",
    "victory_artefact_carry_kill",
    "victory_artefact_carry_rescue",
    "pve_open_chest",
    "pve_respawn_with_voucher",
    "pve_respawn_with_premium",
    "gunrace_rank_progress",
    "gunrace_rank_loss",
    "sad_bomb_planted",
    "sad_bomb_defuse",
    "sad_round_won",
    "ressuply_crate_place",
    "ressuply_crate_use_self",
    "totem_place",
    "totem_use_self",
    "totem_use_ally",
    "scanner_placed",
    "scanner_thrown",
    "scanner_use_self",
    "smoke_use",
    "capacitor_use",
    "medkit_use",
    "antitoxin_use",
    "anesthetic_use",
    "medkit_ally_use",
    "medkit_destroyed",
    "device_destroyed",
    "trap_destroyed",
    "trap_disarmed",
    "flash_use",
    "artefact_collection",
    "zone_defence",
    "killstreak_end",
    "batteries_protected",
    "research_progress",
];

/// Event ids of [`EVENT_NAMES`] the lobby reads by name.
pub mod ev {
    pub const KILL: u16 = 0;
    pub const ASSIST: u16 = 1;
    pub const DEATH: u16 = 2;
    pub const SUICIDE: u16 = 3;
    pub const TEAM_KILL: u16 = 4;
    pub const ARTEFACT: u16 = 5;
    /// added by the match end (`S.vt[0x110]`) only: the lobby sets 11 / 12 / 27 itself
    pub const PARTICIPATING: u16 = 11;
    pub const WON: u16 = 12;
    pub const MELEE_KILL: u16 = 13;
    pub const TRAP_KILL: u16 = 14;
    pub const HEADSHOT_KILL: u16 = 15;
    pub const GRENADE_KILL: u16 = 25;
    pub const PLAYED_FIRST_GAME: u16 = 27;
    pub const BLEEDING_KILL: u16 = 37;
    pub const PVE_OPEN_CHEST: u16 = 42;
}

/// The `events.options` row name of event `e` (None beyond the table).
pub fn event_row_name(e: u16) -> Option<&'static str> {
    match e {
        ev::PVE_OPEN_CHEST => Some("match_state_event_pve_open_chest"),
        _ => EVENT_NAMES.get(usize::from(e)).copied(),
    }
}

/// One kill of the player.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KillFact {
    pub weapon_category: Option<u16>,
    pub kill_flags: Option<u8>,
    pub inflictor_type: Option<u8>,
    /// instance of the grenade / mine / bullet that did it (kills with the same id share an inflictor)
    pub inflictor_id: Option<u32>,
    /// mask of the victim's states at the kill: bit `1 << s` for state `s` (1 aiming, 2 reloading, 3 destroying an
    /// anomaly)
    pub victim_state: Option<u8>,
    pub victim_category: Option<u16>,
    /// dict id of the victim's active item (its category becomes `victim_category` where the item data is at hand)
    pub victim_weapon_dict: Option<u16>,
    /// the killer's health when he killed (achievement 210 `killer_max_hp`)
    pub killer_hp: Option<u32>,
    pub after_own_death: Option<bool>,
    pub grenade_in_hand: Option<bool>,
    pub last_bullet: Option<bool>,
    pub no_scope: Option<bool>,
    /// the killer's life the kill happened in (his deaths before it; achievement 210 counts within one life)
    pub life: Option<u16>,
}

pub const KILL_MELEE: u8 = 2;
pub const KILL_BLEEDING: u8 = 4;
pub const KILL_HEADSHOT: u8 = 8;
pub const KILL_RICOCHET: u8 = 16;

impl KillFact {
    pub fn has_flag(&self, mask: u8) -> Option<bool> {
        self.kill_flags.map(|f| f & mask != 0)
    }
}

/// The facts of one finished match for one player.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MatchFacts {
    // ---- known from the local match path
    pub mode: u32,
    pub map: u32,
    pub won: bool,
    /// a win / loss marker was seen (leavers: false, they get nothing)
    pub finished: bool,
    /// the account's active faction (1..=4)
    pub faction: u8,
    /// the account's first finished match: the `played_first_game` event (500 experience) counts once
    pub first_game: bool,
    /// faction reputation the match paid (faction battles count it); None = not known
    pub reputation: Option<u32>,
    // ---- not known to the server yet (a match observer fills them)
    pub duration_secs: Option<u32>,
    /// 1 random match, 2 rating match (achievement `match_type_mask`); a local training match is neither
    pub match_type: Option<u8>,
    /// the player's place in his team by score, 0 = best (the index of `winner_team_points` / `looser_team_points` of
    /// `leagues.options`); the ranked rating needs it (`league::record`), None = not observed = no rating change
    pub team_place: Option<u8>,
    pub in_squad: Option<bool>,
    /// a random / rated match played in a squad that has a clanmate of the player (achievement 418 `is_in_clan_squad`);
    /// NOT a clan battle (a clan battle is a team of one clan, no squads)
    pub in_clan_squad: Option<bool>,
    /// the match was a clan battle (custom match type 1): `stats::record_match` counts it for the clan experience and the
    /// faction battle only; achievements, daily quests, treasure hunt and the league skip it (0.69c: achievements cannot be
    /// earned in custom matches; nothing else is known to count)
    pub clan_battle: bool,
    /// a local match against bots: the client says "Rewards in this mode are not awarded" (PROVEN), so no item of any kind
    /// is paid for it (treasure-hunt cards included); the quest / achievement / league counters stay as documented (OPEN)
    pub local_bots: bool,
    /// every kill of the player; None = kills were not observed (an empty list = observed, no kills)
    pub kills: Option<Vec<KillFact>>,
    pub deaths: Option<u32>,
    pub score: Option<u32>,
    /// winning team's lead in points (`score_diff` of 613..615 / 627)
    pub score_diff: Option<u32>,
    pub max_killstreak: Option<u32>,
    pub artefacts: Option<u32>,
    /// amount per `stats_event_type`
    pub stat_events: Option<BTreeMap<u16, u32>>,
    /// badges earned: (badge_type, badge_state)
    pub badges: Option<Vec<(u8, u8)>>,
    /// the player killed every enemy (achievement 223)
    pub killed_all_enemies: Option<bool>,
    /// a kill or assist of the player on an enemy damaged by every teammate (achievement 225)
    pub squad_damage_kill: Option<bool>,
    /// the player's squad held all four post-match medals (achievement 621)
    pub squad_all_medals: Option<bool>,
    /// the world's inactive flag of the player at the match end (statistics block +0x9F0)
    pub inactive: Option<bool>,
    /// the most health (percent of max) he got back within one life (achievement 402; world host vitals, protocol 8)
    pub regen_best: Option<u32>,
    /// his lowest health percent at an `artefact` event (achievement 414 `holder_max_hp`); 0xFF = no artefact
    pub artefact_hp_min: Option<u8>,
    /// the most random modifiers on one item of the loadout he played the match with (achievements 415-417); None = not
    /// known (a local match)
    pub item_mods: Option<u32>,
}

impl MatchFacts {
    /// Facts of the local match path: only what it really knows.
    pub fn local(map: u32, mode: u32, won: bool, finished: bool, faction: u8) -> MatchFacts {
        MatchFacts { mode, map, won, finished, faction, ..MatchFacts::default() }
    }

    /// Number of kills matching `pred`; None when kills were not observed at all. A kill whose needed attribute is
    /// unknown (`pred` gives None) is not counted: the result is then a lower bound, and a count of 0 moves nothing.
    pub fn count_kills(&self, mut pred: impl FnMut(&KillFact) -> Option<bool>) -> Option<u32> {
        Some(self.kills.as_ref()?.iter().filter(|k| pred(k) == Some(true)).count() as u32)
    }

    pub fn stat(&self, kind: u16) -> Option<u32> {
        self.stat_events.as_ref().map(|m| m.get(&kind).copied().unwrap_or(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_event_table_is_the_exes() {
        // the table at 0x200986E10, spot checks across it
        assert_eq!(EVENT_NAMES.len(), 74);
        let at = |i: usize| EVENT_NAMES[i];
        assert_eq!((at(0), at(1), at(2), at(3), at(4), at(5)), ("kill", "assist", "death", "suicide", "team_kill", "artefact"));
        assert_eq!((at(12), at(13), at(15), at(24), at(26), at(29)), ("won", "melee_kill", "headshot_kill", "bring_victory_item_singly", "steal_victory_item_singly", "capture_the_point"));
        assert_eq!((at(36), at(37), at(42), at(47), at(63), at(64), at(71), at(73)), ("multi_kill", "bleeding_kill", "pve_open_chest", "sad_bomb_planted", "medkit_ally_use", "medkit_destroyed", "killstreak_end", "research_progress"));
        let mut names: Vec<&str> = EVENT_NAMES.to_vec();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 74, "no name twice");
        assert_eq!(event_row_name(42), Some("match_state_event_pve_open_chest"));
        assert_eq!(event_row_name(0), Some("kill"));
        assert_eq!(event_row_name(74), None);
    }
}
