//! Ranked matches (leagues): the data tables, the rating rule and the season schedule. Notes: `docs/notes/league.md`.
//!
//! PROVEN = read from the exe or the game data (address / file), CHOICE = server rule with no client or data evidence.
//!
//! Data (`data/league_069d.json`, `crates/data-extract`): `leagues_parameters` of
//! `gameplay/event_rules/leagues.options` (resources.22.db): 10 leagues, league 1 = the lowest. Each row has
//! `points_for_levelup` (the points that fill the league's stars; the client shows `points_for_levelup >> 2` stars
//! and one star = 4 points, `set_league_progress` multiplies the points by 0.25, 0x20027EAD8 / 0x2009E5E30),
//! `possible_level_down`, `min_equip_level` and, per place of the player in his team (0 = best, 8 entries),
//! `winner_team_points` / `looser_team_points` = the points a win / a loss adds (PROVEN names, 0x20027D5C0..0x20027D9A0
//! put them into the `root.set_league_levels` objects as `winner_team_points` / `looser_team_points`).
//!
//! Not in any file (so a CHOICE or an input): the season schedule (`Season`, absent by default = ranked off), the
//! token reward per league, the carry-over of surplus points, where a demoted player lands.

use serde::Deserialize;

pub const FILE: &str = "league_069d.json";

/// Item dictionary id of the league currency ("Tokens", category 29; `st_league_currency_name`).
pub const TOKEN_ITEM: u16 = 1356;

/// Teams have at most 8 players in the tables (`winner_team_points` has 8 entries).
pub const PLACES: usize = 8;

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct LeagueParams {
    pub possible_level_down: bool,
    pub min_equip_level: u8,
    pub points_for_levelup: u32,
    pub looser_team_points: Vec<i32>,
    pub winner_team_points: Vec<i32>,
}

/// One league shop item: bought with league tokens.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ShopItem {
    pub dict_id: u16,
    pub cfg_name: String,
    pub item_level: u32,
    pub tokens: u32,
    /// where the price comes from (`PROVEN web ...` or `CHOICE ...`)
    pub source: String,
}

#[derive(Debug, Deserialize)]
struct Raw {
    leagues: Vec<LeagueParams>,
    rating_match_bonus_percent: u32,
    shop: Vec<ShopItem>,
}

#[derive(Debug)]
pub struct LeagueData {
    leagues: Vec<LeagueParams>,
    pub rating_match_bonus_percent: u32,
    pub shop: Vec<ShopItem>,
}

/// An account's place in the ladder of the running season.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Position {
    /// 1 = lowest league
    pub league: u8,
    /// points inside the league (`0 ..= points_for_levelup`)
    pub points: u32,
}

/// What one ranked match did to a [`Position`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Outcome {
    pub before: Position,
    pub after: Position,
    /// points the tables gave (before carrying over / demotion)
    pub delta: i32,
}

impl Outcome {
    pub fn promoted(&self) -> bool {
        self.after.league > self.before.league
    }
    pub fn demoted(&self) -> bool {
        self.after.league < self.before.league
    }
}

impl LeagueData {
    pub fn parse(json: &str) -> Result<LeagueData, serde_json::Error> {
        let raw: Raw = serde_json::from_str(json)?;
        Ok(LeagueData { leagues: raw.leagues, rating_match_bonus_percent: raw.rating_match_bonus_percent, shop: raw.shop })
    }

    /// Number of leagues (the K of the client's level table).
    pub fn count(&self) -> u8 {
        self.leagues.len() as u8
    }

    /// Parameters of league `l` (1-based).
    pub fn league(&self, l: u8) -> Option<&LeagueParams> {
        self.leagues.get((l as usize).checked_sub(1)?)
    }

    /// Lowest equipment tier any league asks for = the client's `x` byte of 9F (a player below it cannot start a ranked
    /// match: `st_low_item_level_for_rating_match`, 0x2001E8350 compares the equipment tier with `L+0x7A5290`, the
    /// byte 9F stores at 0x20018926E). LIKELY that the server value is the lowest `min_equip_level` (3 in the data,
    /// and the text says "Minimal tier is 3").
    pub fn min_equip_level(&self) -> u8 {
        self.leagues.iter().map(|l| l.min_equip_level).min().unwrap_or(0)
    }

    pub fn shop_price(&self, dict: u16) -> Option<u32> {
        self.shop.iter().find(|s| s.dict_id == dict).map(|s| s.tokens)
    }

    /// Apply one ranked match. `place` = the player's place in his team by score (0 = best); `won` = his team won.
    /// A player who left the match counts as a loss in the last place (CHOICE: leavers get nothing in other
    /// rewards, and a free exit must not dodge a loss).
    ///
    /// PROVEN: the points are `winner_team_points[place]` / `looser_team_points[place]` of the league the player was in
    /// before the match (LIKELY the league before, the client table is per league). CHOICE: `place` above 7 uses
    /// entry 7; reaching `points_for_levelup` promotes and carries the surplus into the next league; in the top league the
    /// points keep counting past `points_for_levelup` (OFFICIAL Q&A 2019-06-27: the stars count on above rank 1 and the top
    /// 100 is ordered by them; CHOICE); a loss below 0 demotes only when the league has `possible_level_down`
    /// and lands in the league below at `points_for_levelup - |deficit|` (just under its promotion mark), else the
    /// points stop at 0.
    pub fn apply(&self, pos: Position, won: bool, left: bool, place: usize) -> Option<Outcome> {
        let lg = self.league(pos.league)?;
        let i = if left { PLACES - 1 } else { place.min(PLACES - 1) };
        let table = if won && !left { &lg.winner_team_points } else { &lg.looser_team_points };
        let delta = *table.get(i)?;
        let mut league = pos.league;
        let mut points = pos.points as i64 + delta as i64;
        loop {
            let need = self.league(league)?.points_for_levelup as i64;
            if points >= need && league < self.count() {
                points -= need;
                league += 1;
                continue;
            }
            break;
        }
        if points < 0 {
            if self.league(league)?.possible_level_down && league > 1 {
                league -= 1;
                let need = self.league(league)?.points_for_levelup as i64;
                points = (need + points).max(0);
            } else {
                points = 0;
            }
        }
        Some(Outcome { before: pos, after: Position { league, points: points as u32 }, delta })
    }

    /// Points for the top list: all the points a player earned climbing to this position (CHOICE; the client shows
    /// the number only, `points` of `root.set_league_top_players`, 0x20027EF1D).
    pub fn total_points(&self, pos: Position) -> u32 {
        let below: u32 = (1..pos.league).filter_map(|l| self.league(l)).map(|l| l.points_for_levelup).sum();
        below + pos.points
    }

    /// Tokens a season pays for the highest league reached: the top league 1000 (OFFICIAL
    /// 2017-10-04: the best players "each received 1000 tokens"), the ten values sum to 3500 (FAN forum clue); the other
    /// nine share 2500 in proportion to league^2 (CHOICE, the individual values are NOT FOUND).
    pub fn default_tokens(league: u8) -> u32 {
        const TOKENS: [u32; 10] = [10, 35, 80, 140, 220, 315, 430, 560, 710, 1000];
        match league {
            0 => 0,
            l => TOKENS[(l as usize - 1).min(9)],
        }
    }
}

/// One ranked season. No game file has a schedule; the client has `st_season_*` texts 1..15 that no movie of the lobby
/// builds a key for (the 9F `S` only has to be nonzero) and shows "Ranked match unavailable: next season has not yet
/// started" between seasons (`st_season_not_started`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Season {
    /// season number (above 0): the 9F `S` field and the stored season of an account. Monthly seasons: [`Season::month`]
    pub number: u16,
    /// unix seconds, inclusive
    pub start: i64,
    /// unix seconds, exclusive
    pub end: i64,
}

/// Days since 1970-01-01 of a civil date (proleptic Gregorian, UTC; days_from_civil of H. Hinnant).
pub fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = i64::from(month);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Month (1..12) and year of a unix time (UTC; civil_from_days of H. Hinnant).
pub fn month_year(t: i64) -> (u8, u16) {
    let z = t.div_euclid(86400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (m as u8, y.clamp(0, 65535) as u16)
}

/// The first year of the monthly numbering: January 2000 is monthly season 1.
pub const MONTHLY_BASE_YEAR: i64 = 2000;

impl Season {
    /// The monthly season of a calendar month (UTC): from the 1st 00:00 to the next 1st (OFFICIAL: seasons are calendar
    /// months starting on the 1st). Its number is `(year - 2000) * 12 + month` (CHOICE: the client shows no season
    /// number; every month keeps a unique number, November 2026 = 323). None before 2000 or past the u16 range.
    pub fn month(year: i64, month: u32) -> Option<Season> {
        if !(1..=12).contains(&month) || year < MONTHLY_BASE_YEAR {
            return None;
        }
        let number = u16::try_from((year - MONTHLY_BASE_YEAR) * 12 + i64::from(month)).ok()?;
        let (ny, nm) = if month == 12 { (year + 1, 1) } else { (year, month + 1) };
        Some(Season { number, start: days_from_civil(year, month, 1) * 86400, end: days_from_civil(ny, nm, 1) * 86400 })
    }

    /// The monthly season `now` lies in.
    pub fn month_at(now: i64) -> Option<Season> {
        let (m, y) = month_year(now);
        Season::month(i64::from(y), u32::from(m))
    }

    /// The monthly season with this number.
    pub fn month_numbered(number: u16) -> Option<Season> {
        let n = i64::from(number.checked_sub(1)?);
        Season::month(MONTHLY_BASE_YEAR + n / 12, (n % 12) as u32 + 1)
    }
}

/// The ranked seasons. Without configured seasons (`seasons` empty) a season runs every calendar month
/// (OFFICIAL rule: monthly seasons from the 1st; [`Season::month`]); configured seasons replace it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Schedule {
    pub seasons: Vec<Season>,
    /// tokens per league 1..K for the season end; empty = [`LeagueData::default_tokens`]
    pub tokens: Vec<u32>,
}

impl Schedule {
    /// `number:start:end` entries separated by commas (unix seconds); e.g. `1:1700000000:1707000000`.
    pub fn parse(text: &str) -> Result<Schedule, String> {
        let mut seasons = Vec::new();
        for part in text.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            let f: Vec<&str> = part.split(':').collect();
            if f.len() != 3 {
                return Err(format!("season {part:?}: need number:start:end"));
            }
            let number: u16 = f[0].parse().map_err(|_| format!("season number {:?}", f[0]))?;
            let start: i64 = f[1].parse().map_err(|_| format!("season start {:?}", f[1]))?;
            let end: i64 = f[2].parse().map_err(|_| format!("season end {:?}", f[2]))?;
            if number == 0 || end <= start {
                return Err(format!("season {part:?}: number must be above 0 and end after start"));
            }
            seasons.push(Season { number, start, end });
        }
        seasons.sort_by_key(|s| s.start);
        if seasons.windows(2).any(|w| w[0].end > w[1].start) {
            return Err("seasons overlap".into());
        }
        Ok(Schedule { seasons, tokens: Vec::new() })
    }

    /// True when no season is configured: the monthly seasons run.
    pub fn is_monthly(&self) -> bool {
        self.seasons.is_empty()
    }

    /// The season running at `now`.
    pub fn current(&self, now: i64) -> Option<Season> {
        if self.is_monthly() {
            return Season::month_at(now);
        }
        self.seasons.iter().copied().find(|s| s.start <= now && now < s.end)
    }

    pub fn season(&self, number: u16) -> Option<Season> {
        if self.is_monthly() {
            return Season::month_numbered(number);
        }
        self.seasons.iter().copied().find(|s| s.number == number)
    }

    /// Start of the next season after `now`.
    pub fn next_start(&self, now: i64) -> Option<i64> {
        if self.is_monthly() {
            return Season::month_at(now).map(|s| s.end);
        }
        self.seasons.iter().map(|s| s.start).filter(|s| *s > now).min()
    }

    /// Tokens the end of a season pays for the highest league reached.
    pub fn tokens_for(&self, league: u8) -> u32 {
        match (league as usize).checked_sub(1).and_then(|i| self.tokens.get(i)) {
            Some(t) => *t,
            None => LeagueData::default_tokens(league),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn without_configured_seasons_every_calendar_month_is_a_season() {
        let s = Schedule::default();
        assert!(s.is_monthly());
        // 2026-11-24 12:00 UTC lies in November 2026: 1 Nov 00:00 .. 1 Dec 00:00, number (2026 - 2000) * 12 + 11
        let nov = s.current(1_795_521_600).unwrap();
        assert_eq!(nov, Season { number: 323, start: 1_793_491_200, end: 1_796_083_200 });
        assert_eq!(s.season(323), Some(nov));
        assert_eq!(s.next_start(1_795_521_600), Some(1_796_083_200));
        // the boundaries: the last second of November is November, the first of December is the next season
        assert_eq!(s.current(nov.end - 1).unwrap().number, 323);
        let dec = s.current(nov.end).unwrap();
        assert_eq!((dec.number, dec.start, month_year(dec.start)), (324, nov.end, (12, 2026)));
        // December -> January of the next year; February of a leap year has 29 days
        let jan = s.current(dec.end).unwrap();
        assert_eq!((jan.number, month_year(jan.start)), (325, (1, 2027)));
        let feb = Season::month(2028, 2).unwrap();
        assert_eq!((feb.end - feb.start) / 86400, 29);
        assert_eq!(Season::month(2027, 2).map(|s| (s.end - s.start) / 86400), Some(28));
        // numbering round trip and edges
        assert_eq!(Season::month_numbered(1), Season::month(2000, 1));
        assert_eq!(Season::month_numbered(0), None);
        for n in [1u16, 12, 13, 323, 324, 1200] {
            assert_eq!(Season::month_numbered(n).unwrap().number, n);
        }
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2000, 3, 1), 11_017);
    }

    #[test]
    fn configured_seasons_replace_the_monthly_ones() {
        let s = Schedule::parse("1:1000:2000").unwrap();
        assert!(!s.is_monthly());
        assert_eq!(s.current(1_795_521_600), None, "between configured seasons ranked is off");
        assert_eq!(s.current(1500).map(|x| x.number), Some(1));
        assert_eq!(s.season(323), None);
    }

    fn data() -> LeagueData {
        LeagueData::parse(&crate::testing::read("league_069d.json")).unwrap()
    }

    #[test]
    fn tables_are_the_data_files() {
        crate::require_tables!();
        let d = data();
        assert_eq!(d.count(), 10);
        assert_eq!(d.league(1).unwrap().points_for_levelup, 12);
        assert_eq!(d.league(10).unwrap().points_for_levelup, 60);
        assert_eq!(d.league(3).unwrap().looser_team_points, vec![0, 0, -1, -1, -2, -2, -2, -2]);
        assert_eq!(d.league(1).unwrap().winner_team_points, vec![4, 4, 4, 4, 3, 2, 1, 0]);
        assert!(!d.league(6).unwrap().possible_level_down && d.league(7).unwrap().possible_level_down);
        assert_eq!(d.min_equip_level(), 3);
        assert_eq!(d.rating_match_bonus_percent, 0);
        assert_eq!(d.shop_price(2087), Some(2250), "Saiga-12K Legend: 2250 tokens (official post)");
        assert!(d.shop_price(1356).is_none(), "the currency itself is not for sale");
    }

    #[test]
    fn win_adds_the_place_points_and_promotes_with_carry() {
        crate::require_tables!();
        let d = data();
        let o = d.apply(Position { league: 1, points: 10 }, true, false, 0).unwrap();
        assert_eq!(o.delta, 4);
        assert_eq!(o.after, Position { league: 2, points: 2 });
        assert!(o.promoted());
        // a seventh place wins 1 point only
        let o = d.apply(Position { league: 1, points: 0 }, true, false, 6).unwrap();
        assert_eq!(o.after.points, 1);
        // place 7 wins nothing
        assert_eq!(d.apply(Position { league: 1, points: 5 }, true, false, 7).unwrap().after.points, 5);
    }

    #[test]
    fn loss_subtracts_and_floors_or_demotes_by_the_league_flag() {
        crate::require_tables!();
        let d = data();
        // league 3 losing in place 6: -2
        assert_eq!(d.apply(Position { league: 3, points: 5 }, false, false, 6).unwrap().after, Position { league: 3, points: 3 });
        // league 5 cannot level down: stops at 0
        assert_eq!(d.apply(Position { league: 5, points: 1 }, false, false, 7).unwrap().after, Position { league: 5, points: 0 });
        // league 7 can: -4 at place 7 from 1 point -> league 6 at 20 - 3
        let o = d.apply(Position { league: 7, points: 1 }, false, false, 7).unwrap();
        assert!(o.demoted());
        assert_eq!(o.after, Position { league: 6, points: 17 });
        // league 1 never goes below
        assert_eq!(d.apply(Position { league: 1, points: 0 }, false, false, 7).unwrap().after, Position { league: 1, points: 0 });
    }

    #[test]
    fn leaver_is_a_loss_in_last_place_and_top_league_counts_on() {
        crate::require_tables!();
        let d = data();
        let o = d.apply(Position { league: 6, points: 10 }, true, true, 0).unwrap();
        assert_eq!(o.delta, -4);
        assert_eq!(o.after.points, 6);
        let top = Position { league: 10, points: 59 };
        let after = d.apply(top, true, false, 0).unwrap();
        assert_eq!(after.after, Position { league: 10, points: 59 + after.delta as u32 }, "no cap in the top league");
        assert!(after.after.points > 60);
    }

    #[test]
    fn total_points_climb() {
        crate::require_tables!();
        let d = data();
        assert_eq!(d.total_points(Position { league: 1, points: 5 }), 5);
        assert_eq!(d.total_points(Position { league: 3, points: 5 }), 12 + 12 + 5);
    }

    #[test]
    fn schedule_parse_current_and_tokens() {
        let s = Schedule::parse("1:100:200, 2:300:400").unwrap();
        assert!(s.current(99).is_none() && s.current(100).unwrap().number == 1 && s.current(200).is_none());
        assert_eq!(s.current(350).unwrap().number, 2);
        assert_eq!(s.next_start(250), Some(300));
        assert_eq!((s.tokens_for(10), s.tokens_for(1)), (1000, 10));
        assert_eq!((1..=10).map(LeagueData::default_tokens).sum::<u32>(), 3500);
        let mut t = s.clone();
        t.tokens = vec![7, 8];
        assert_eq!((t.tokens_for(1), t.tokens_for(2), t.tokens_for(3)), (7, 8, 80));
        assert!(Schedule::parse("1:5:5").is_err() && Schedule::parse("1:1:9,2:5:12").is_err() && Schedule::parse("x").is_err());
        assert!(Schedule::parse("").unwrap().is_monthly(), "an empty setting keeps the monthly seasons");
    }
}
