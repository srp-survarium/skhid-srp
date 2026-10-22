//! When a 0.26g match starts, ends and who wins
//! (`docs/notes/modes_maps.md` §10 and §11): the client's rule
//! objects, as far as the server has to know them. The clients run the
//! rules themselves; the server needs them to size the roster, to know the
//! phase without a snapshot, and for the results.
//!
//! All of it is PROVEN (read in the exe at the addresses the notes give)
//! unless a comment says otherwise.

use super::super::maps::{MapMode, mode};

/// `timelimit_rule_core`'s states (+0x148).
pub mod state {
    pub const INACTIVE: u8 = 0;
    pub const WAITING_FOR_PLAYERS: u8 = 1;
    pub const COUNTDOWN: u8 = 2;
    pub const IN_PROCESS: u8 = 3;
}

/// `m_winner_team`: a team, or a draw.
pub const DRAW: u8 = 2;

/// A mode rule that has its end sets "completed at now + 3000 ms".
pub const END_DELAY_MS: u32 = 3_000;

/// What `timelimit_rule_core` is built with: the mode config's times and
/// the roster size of the static info (constructor 0x317DA0).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimelimitParams {
    /// `match_time` × 1000: 900 000 in every shipped config.
    pub match_ms: u32,
    /// `wait_time` × 60 000 (minutes): 300 000.
    pub wait_ms: u32,
    /// `countdown_time` × 1000: 20 000.
    pub countdown_ms: u32,
    /// `wait_player_percent`: 0.8.
    pub fraction: f32,
    /// The static info's `u8 N`: the one value the server chooses.
    pub roster: u8,
}

impl TimelimitParams {
    /// The times of a map's mode config, for a roster of `roster` places.
    pub fn of(map: &MapMode, roster: u8) -> Self {
        Self {
            match_ms: map.match_time.saturating_mul(1_000),
            wait_ms: map.wait_time.saturating_mul(60_000),
            countdown_ms: map.countdown_time.saturating_mul(1_000),
            fraction: map.wait_player_percent,
            roster,
        }
    }

    /// The players that must have entered before the countdown starts.
    pub fn players_needed(&self) -> u32 {
        players_needed(self.roster, self.fraction)
    }
}

/// `floor(N × fraction)`, in f32 as the client computes it
/// (0x31824E..0x31826F). With 0.8: 1 → 0, 2 → 1, 3 → 2, 4 → 3, 5 → 4,
/// 6 → 4, 8 → 6, 10 → 8, 16 → 12, 20 → 16.
pub fn players_needed(roster: u8, fraction: f32) -> u32 {
    (f32::from(roster) * fraction).floor().max(0.0) as u32
}

/// Whether the rule leaves "waiting for players": someone has entered, and
/// at least [`players_needed`] have (0x318246..0x318271). `mask` is the
/// players the server activated with 0xE9 and has not sent 0xEA for; it is
/// neither the roster nor the connected clients.
pub fn start_condition(mask: u32, roster: u8, fraction: f32) -> bool {
    mask != 0 && mask.count_ones() >= players_needed(roster, fraction)
}

/// What a step of the rule did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transition {
    /// Waiting → countdown: "Match begins in … seconds".
    CountdownStarted,
    /// Countdown → in process: "Match started". The rules get their
    /// "match started" event.
    MatchStarted,
}

/// `timelimit_rule_core`, stepped by the server alongside the clients: its
/// state after each 0xE9 / 0xEA the server sent and each tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimelimitRule {
    pub params: TimelimitParams,
    /// [`state`].
    pub state: u8,
    /// `m_state_start_time_ms`: when the current state began. `None` until
    /// the match is ready (0xFFFFFFFF on the wire).
    pub start: Option<u32>,
    /// `m_players_mask`: entered and not left.
    pub mask: u32,
}

impl TimelimitRule {
    pub fn new(params: TimelimitParams) -> Self {
        Self {
            params,
            state: state::INACTIVE,
            start: None,
            mask: 0,
        }
    }

    /// `on_match_ready(t)` 0x317EE0: waiting begins.
    pub fn on_match_ready(&mut self, time: u32) {
        self.state = state::WAITING_FOR_PLAYERS;
        self.start = Some(time);
    }

    /// `on_player_entered_match` 0x3180C0: the bit, then a logic tick.
    pub fn on_player_entered(&mut self, player: u8, time: u32) -> Option<Transition> {
        self.mask |= 1u32.checked_shl(player.into()).unwrap_or(0);
        self.tick(time)
    }

    /// `on_player_left_match` 0x3180E0. The countdown and the match go on
    /// whoever leaves: the rule never returns to waiting.
    pub fn on_player_left(&mut self, player: u8, time: u32) -> Option<Transition> {
        self.mask &= !1u32.checked_shl(player.into()).unwrap_or(0);
        self.tick(time)
    }

    /// `logic_tick(t)` 0x318100: at most one step a call.
    pub fn tick(&mut self, time: u32) -> Option<Transition> {
        match self.state {
            state::WAITING_FOR_PLAYERS => {
                if !start_condition(self.mask, self.params.roster, self.params.fraction) {
                    return None;
                }
                self.state = state::COUNTDOWN;
                self.start = Some(time);
                Some(Transition::CountdownStarted)
            }
            state::COUNTDOWN => {
                let started = self.start?.saturating_add(self.params.countdown_ms);
                if started > time {
                    return None;
                }
                self.state = state::IN_PROCESS;
                // Not `time`: the match begins when the countdown ran out.
                self.start = Some(started);
                Some(Transition::MatchStarted)
            }
            _ => None,
        }
    }

    /// `rule_completed(t)` 0x318360: the match's time is up, or it waited
    /// for players the whole waiting time without starting. Never during
    /// the countdown.
    pub fn completed(&self, time: u32) -> bool {
        let Some(start) = self.start else {
            return false;
        };
        let elapsed = time.wrapping_sub(start);
        match self.state {
            state::WAITING_FOR_PLAYERS => elapsed >= self.params.wait_ms,
            state::IN_PROCESS => elapsed >= self.params.match_ms,
            _ => false,
        }
    }

    /// When play began, once it has.
    pub fn match_started_at(&self) -> Option<u32> {
        (self.state == state::IN_PROCESS)
            .then_some(self.start)
            .flatten()
    }

    /// Whether a kit choice for the tick at `time` may still change a live
    /// player's kit in a game (patch 31, `client_patch.md` section 3.29):
    /// for a tick before the one that begins play. In a client's tick the
    /// respawn rule runs before the places, which take the tick's kit
    /// choices (tick list 0), and the time rule after both (list 1): a
    /// choice for a tick is played at the next one, and the last tick that
    /// still sees the countdown is the one that begins play.
    pub fn before_play(&self, time: u32) -> bool {
        match (self.state, self.start) {
            (state::WAITING_FOR_PLAYERS, _) => true,
            (state::COUNTDOWN, Some(start)) => {
                time < start.saturating_add(self.params.countdown_ms)
            }
            (state::COUNTDOWN, None) => true,
            (state::IN_PROCESS, Some(start)) => time < start,
            _ => false,
        }
    }

    /// When the time limit ends the match: known from the countdown on.
    pub fn time_limit_at(&self) -> Option<u32> {
        let start = self.start?;
        match self.state {
            state::COUNTDOWN => Some(start + self.params.countdown_ms + self.params.match_ms),
            state::IN_PROCESS => Some(start + self.params.match_ms),
            _ => None,
        }
    }
}

/// The leader by points: more wins, equal is a draw. Modes 2 and 3 with the
/// batteries in each team's container (0x313AA5..0x313ADC); also mode 4's
/// tickets while they differ, and mode 5's shown points (`rr_leader`).
pub fn winner_by_points(points: [u16; 2]) -> u8 {
    match points[0].cmp(&points[1]) {
        std::cmp::Ordering::Greater => 0,
        std::cmp::Ordering::Less => 1,
        std::cmp::Ordering::Equal => DRAW,
    }
}

/// Research's leader (exe patch 53, `rr_leader`): by the shown points, equal
/// a draw; once a team reached the goal (the rule's end is set), by the
/// millionths, so 100 shown for both (99.5 rounds up) is not a draw when one
/// team reached 100.
pub fn research_winner(scores: [u16; 2], points: [u32; 2], at_goal: bool) -> u8 {
    if at_goal && points[0] != points[1] {
        return u8::from(points[1] > points[0]);
    }
    winner_by_points(scores)
}

/// Team Deathmatch's leader (0x3161F0): more tickets wins; equal and not
/// zero is a draw; both at zero, the team with more players alive, a draw
/// if equal.
pub fn winner_tdm(tickets: [u16; 2], alive: [u32; 2]) -> u8 {
    if tickets[0] != tickets[1] || tickets[0] != 0 {
        return winner_by_points(tickets);
    }
    match alive[0].cmp(&alive[1]) {
        std::cmp::Ordering::Greater => 0,
        std::cmp::Ordering::Less => 1,
        std::cmp::Ordering::Equal => DRAW,
    }
}

/// The players alive in each team: `alive_mask` over the roster's teams
/// (`teams[i]` is roster place i's team, 0 or 1).
pub fn alive_by_team(alive_mask: u32, teams: &[u8]) -> [u32; 2] {
    let mut alive = [0; 2];
    for (player, &team) in teams.iter().enumerate().take(32) {
        if alive_mask & (1 << player) != 0 && team < 2 {
            alive[usize::from(team)] += 1;
        }
    }
    alive
}

/// Modes 2 and 3 end (3 s later) when a team has every battery in its
/// container (0x313AE2..0x313B34). `batteries` is the mode config's
/// `victory_items_count`, 10; in mode 3 too, where they appear over time.
pub fn gather_finished(points: [u16; 2], batteries: u32) -> bool {
    batteries != 0 && points.iter().any(|&team| u32::from(team) == batteries)
}

/// Team Deathmatch ends (3 s later) when a team has no tickets and no
/// player alive (0x316470).
pub fn tdm_finished(tickets: [u16; 2], alive: [u32; 2]) -> bool {
    (0..2).any(|team| tickets[team] == 0 && alive[team] == 0)
}

/// Research's points are kept in millionths (`research_mode.md`, "Wire").
pub const RESEARCH_POINT: u32 = 1_000_000;

/// Research ends (3 s later) when a team's points, in millionths, reach the
/// mode config's `resources_count` (exe patch 53, `rr_tick`). The rule
/// writes that end into its own bytes (`snapshot::Research::end`), which
/// is what the server goes by; the shown scores are rounded and can say
/// 100 a little before.
pub fn research_finished(points: [u32; 2], goal: u32) -> bool {
    goal != 0
        && points
            .iter()
            .any(|&team| u64::from(team) >= u64::from(goal) * u64::from(RESEARCH_POINT))
}

/// Whether a mode rule that set its end at `completion` reports the match
/// over at `time`: the gather rules at `completion ≤ time` (0x314230),
/// Team Deathmatch at `completion < time` (0x316520), Research at
/// `completion ≤ time` (exe patch 53, `rr_completed`).
pub fn mode_rule_completed(game_mode: u8, completion: u32, time: u32) -> bool {
    if game_mode == mode::TEAM_DEATHMATCH {
        completion < time
    } else {
        completion <= time
    }
}

/// Whether a death costs the victim's team a ticket in Team Deathmatch
/// (0x31614E..0x31618C): every death does, but a kill by a teammate. A
/// suicide and a death with no killer (`None`, 0xFF on the client) cost
/// one. With no tickets left the victim is out instead.
pub fn tdm_death_costs_ticket(killer: Option<(u8, u8)>, victim: (u8, u8)) -> bool {
    match killer {
        // `(player, team)`.
        Some((player, team)) => player == victim.0 || team != victim.1,
        None => true,
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::maps;
    use super::*;

    fn params(roster: u8) -> TimelimitParams {
        // Map 61: level_01 day, mode 2.
        TimelimitParams::of(maps::map_mode(61).unwrap(), roster)
    }

    #[test]
    fn the_times_come_from_the_mode_config() {
        assert_eq!(
            params(4),
            TimelimitParams {
                match_ms: 900_000,
                wait_ms: 300_000,
                countdown_ms: 20_000,
                fraction: 0.8,
                roster: 4,
            }
        );
    }

    #[test]
    fn four_fifths_of_the_roster_rounded_down_must_have_entered() {
        // modes_maps.md §10.
        let table = [
            (1, 0),
            (2, 1),
            (3, 2),
            (4, 3),
            (5, 4),
            (6, 4),
            (8, 6),
            (10, 8),
            (16, 12),
            (20, 16),
        ];
        for (roster, needed) in table {
            assert_eq!(players_needed(roster, 0.8), needed, "{roster}");
        }
    }

    #[test]
    fn the_start_needs_someone_and_enough_of_them() {
        // A roster of 1 needs 0 players by the fraction, but one by the
        // "mask not empty" test.
        assert!(!start_condition(0, 1, 0.8));
        assert!(start_condition(0b1, 1, 0.8));
        // A roster of 2 starts on the first player in.
        assert!(start_condition(0b10, 2, 0.8));
        // 5 need 4.
        assert!(!start_condition(0b00111, 5, 0.8));
        assert!(start_condition(0b10111, 5, 0.8));
        // 20 need 16.
        assert!(!start_condition(0x7FFF, 20, 0.8));
        assert!(start_condition(0xFFFF, 20, 0.8));
        // A lone tester in a padded roster of 3 waits.
        assert!(!start_condition(0b001, 3, 0.8));
    }

    #[test]
    fn before_play_is_the_wait_and_the_countdown_to_the_tick_before_play() {
        let mut rule = TimelimitRule::new(params(2));
        assert!(!rule.before_play(0), "not ready: no match yet");
        rule.on_match_ready(50);
        assert!(rule.before_play(50) && rule.before_play(1_000_000));
        assert_eq!(
            rule.on_player_entered(0, 1_000),
            Some(Transition::CountdownStarted)
        );
        assert!(rule.before_play(1_000) && rule.before_play(20_950));
        assert!(
            !rule.before_play(21_000),
            "a choice for the tick play begins in is played in play"
        );
        assert_eq!(rule.tick(21_000), Some(Transition::MatchStarted));
        assert!(rule.before_play(20_950) && !rule.before_play(21_000) && !rule.before_play(30_000));
    }

    #[test]
    fn a_match_waits_counts_down_twenty_seconds_and_plays_fifteen_minutes() {
        let mut rule = TimelimitRule::new(params(3));
        assert_eq!(rule.state, state::INACTIVE);
        assert_eq!(rule.tick(0), None);
        rule.on_match_ready(50);
        assert_eq!(rule.state, state::WAITING_FOR_PLAYERS);
        assert_eq!(rule.start, Some(50));
        assert_eq!(rule.on_player_entered(0, 400), None);
        assert_eq!(rule.tick(450), None);
        assert_eq!(rule.time_limit_at(), None);
        // The second of three: the countdown starts at that tick.
        assert_eq!(
            rule.on_player_entered(2, 1_000),
            Some(Transition::CountdownStarted)
        );
        assert_eq!((rule.state, rule.start), (state::COUNTDOWN, Some(1_000)));
        assert_eq!(rule.mask, 0b101);
        assert_eq!(rule.time_limit_at(), Some(921_000));
        // Leaving does not stop it.
        assert_eq!(rule.on_player_left(0, 5_000), None);
        assert_eq!(rule.on_player_left(2, 5_050), None);
        assert_eq!(rule.state, state::COUNTDOWN);
        assert_eq!(rule.tick(20_950), None);
        assert!(!rule.completed(20_950));
        // 20 s after it began; a late tick still starts the match at 21 000.
        assert_eq!(rule.tick(21_100), Some(Transition::MatchStarted));
        assert_eq!((rule.state, rule.start), (state::IN_PROCESS, Some(21_000)));
        assert_eq!(rule.match_started_at(), Some(21_000));
        assert_eq!(rule.time_limit_at(), Some(921_000));
        assert_eq!(rule.tick(30_000), None);
        assert!(!rule.completed(920_950));
        assert!(rule.completed(921_000));
    }

    #[test]
    fn one_step_a_tick() {
        // A roster of 1 with no countdown left still takes two ticks.
        let mut rule = TimelimitRule::new(TimelimitParams {
            countdown_ms: 0,
            ..params(1)
        });
        rule.on_match_ready(50);
        assert_eq!(
            rule.on_player_entered(0, 100),
            Some(Transition::CountdownStarted)
        );
        assert_eq!(rule.tick(100), Some(Transition::MatchStarted));
    }

    #[test]
    fn five_minutes_of_waiting_end_a_match_that_never_started() {
        let mut rule = TimelimitRule::new(params(3));
        assert!(!rule.completed(1_000_000), "not ready yet");
        rule.on_match_ready(50);
        rule.on_player_entered(1, 100);
        assert!(!rule.completed(300_000));
        assert!(rule.completed(300_050));
    }

    #[test]
    fn batteries_decide_modes_two_and_three() {
        assert_eq!(winner_by_points([3, 1]), 0);
        assert_eq!(winner_by_points([1, 3]), 1);
        assert_eq!(winner_by_points([2, 2]), DRAW);
        assert_eq!(winner_by_points([0, 0]), DRAW);
        assert!(!gather_finished([9, 9], 10));
        assert!(gather_finished([10, 0], 10));
        assert!(gather_finished([3, 10], 10));
        assert!(!gather_finished([0, 0], 0));
        // The gather rules are over at their completion time, Team
        // Deathmatch one tick after.
        assert!(mode_rule_completed(mode::BATTERIES, 5_000, 5_000));
        assert!(mode_rule_completed(mode::TIMED_BATTERIES, 5_000, 5_000));
        assert!(!mode_rule_completed(mode::TEAM_DEATHMATCH, 5_000, 5_000));
        assert!(mode_rule_completed(mode::TEAM_DEATHMATCH, 5_000, 5_050));
        assert!(!mode_rule_completed(mode::BATTERIES, 5_000, 4_950));
    }

    #[test]
    fn research_at_the_goal_is_led_by_the_millionths() {
        // Both show 100; team 0 reached it, team 1 has 99.5.
        assert_eq!(research_winner([100, 100], [100_000_000, 99_500_000], true), 0);
        assert_eq!(research_winner([100, 100], [99_600_000, 100_000_000], true), 1);
        // Before the goal, the shown points: equal is a draw.
        assert_eq!(research_winner([37, 37], [37_400_000, 37_200_000], false), DRAW);
        assert_eq!(research_winner([40, 37], [40_000_000, 37_000_000], false), 0);
    }

    #[test]
    fn research_ends_at_the_goal_in_millionths() {
        assert!(!research_finished([99_999_999, 0], 100));
        assert!(research_finished([100_000_000, 0], 100));
        assert!(research_finished([3, 100_000_000], 100));
        assert!(!research_finished([0, 0], 0));
        // Its end is the rule's end time, as the gather rules'.
        assert!(!mode_rule_completed(mode::RESEARCH, 5_000, 4_950));
        assert!(mode_rule_completed(mode::RESEARCH, 5_000, 5_000));
        // The leader: more shown points, equal a draw.
        assert_eq!(winner_by_points([100, 64]), 0);
        assert_eq!(winner_by_points([37, 37]), DRAW);
    }

    #[test]
    fn tickets_then_the_living_decide_team_deathmatch() {
        assert_eq!(winner_tdm([40, 12], [0, 5]), 0);
        assert_eq!(winner_tdm([0, 12], [3, 0]), 1);
        // Equal and not zero: a draw, whoever lives.
        assert_eq!(winner_tdm([100, 100], [5, 1]), DRAW);
        // Both out of tickets: the living.
        assert_eq!(winner_tdm([0, 0], [2, 1]), 0);
        assert_eq!(winner_tdm([0, 0], [0, 1]), 1);
        assert_eq!(winner_tdm([0, 0], [1, 1]), DRAW);
        // The end: no tickets and nobody alive.
        assert!(!tdm_finished([0, 5], [1, 3]));
        assert!(tdm_finished([0, 5], [0, 3]));
        assert!(!tdm_finished([1, 5], [0, 3]));
        assert!(tdm_finished([7, 0], [2, 0]));
        // Players 0, 2 in team 0 and 1, 3 in team 1; 0, 1 and 3 alive.
        assert_eq!(alive_by_team(0b1011, &[0, 1, 0, 1]), [1, 2]);
        assert_eq!(alive_by_team(0xFFFF_FFFF, &[0, 1]), [1, 1]);
    }

    #[test]
    fn a_teammates_kill_costs_no_ticket() {
        // `(player, team)`.
        assert!(tdm_death_costs_ticket(Some((1, 1)), (0, 0)), "an enemy");
        assert!(!tdm_death_costs_ticket(Some((2, 0)), (0, 0)), "a teammate");
        assert!(tdm_death_costs_ticket(Some((0, 0)), (0, 0)), "a suicide");
        assert!(tdm_death_costs_ticket(None, (0, 0)), "no killer");
    }
}
