//! Lobby settings. Every feature is always on; the fields are the real settings (srp fills them from its config file,
//! `config/srp.example.toml`) and a few test-only knobs (marked TEST ONLY: srp never changes them; tests that count
//! messages turn those features off).

use std::time::Duration;

#[derive(Debug, Clone)]
pub struct LobbyConfig {
    /// A local match is finished by the first 53 flush this long after the match options went out (30 s by default).
    pub match_min_time: Duration,
    /// Most peers at once.
    pub max_peers: usize,
    /// TEST ONLY. Reaching an achievement state gives its reward item (default true; tests of the reference vectors turn it
    /// off so the starter profile keeps its 12 items).
    pub achievement_rewards: bool,
    /// TEST ONLY. Daily quests (AB push, quest messages, 4A 03 / 4A 04). Default true; tests that count messages turn it
    /// off.
    pub daily_quests: bool,
    /// TEST ONLY. The treasure hunt (item search roulette, AA push, 46 07, 5A xx; hunt.rs). Default true; tests that
    /// count the login messages turn it off.
    pub treasure_hunt: bool,
    /// Faction battle schedule (an input no game file has; None = no battle ever runs). See faction_battle.rs.
    pub faction_battles: Option<game_data::faction_battle::Schedule>,
    /// Ranked seasons (an input no game file has; no configured season = a season every calendar month). See league.rs.
    pub league: game_data::LeagueSchedule,
    /// Daily login calendar: the reward of each day of the cycle (no game file has the original; default =
    /// [`default_login_calendar`], None = no calendar).
    pub login_calendar: Option<Vec<protocol::s2c::quests::BonusDay>>,
    /// TEST ONLY. Pins every PvP queue match to this (map, mode, weather) row; None (srp) = the queue picks among the
    /// rows the world host serves.
    pub online_queue: Option<(u32, u32, u32)>,
    /// After a match ended on the match server, wait this long for the players' win / loss reports (53 events 5 / 6)
    /// before paying with the reports that arrived (a CHOICE).
    pub online_report_grace: Duration,
    /// Players a queue group needs to start an online match (CHOICE: 2; the mode row's own `min_players` is
    /// higher). Both teams need at least one player either way.
    pub online_min_players: u32,
    /// PvP bots (CHOICE): a queue search that waited this long without a full group starts a match
    /// with the humans found (at least one) and AI players up to [`crate::online::bot_team_cap`] per team. Only when
    /// the match server says it can drive bots ([`crate::OnlineMatches::bots_available`]: a world host with bots).
    /// Default 30 s; None = never.
    pub online_bot_wait: Option<Duration>,
    /// Players per team of a bot-filled PvP match (queue bot fill) and the most AI players a custom room offers per
    /// team (CHOICE: 6). Never more than [`crate::online::BOT_TEAM_MAX`], half the row's match size or the patrol
    /// graphs per team ([`crate::online::bot_team_cap`]). srp: `matches.bots_per_team`, 1..=8.
    pub bots_per_team: usize,
    /// TEST ONLY. Hero of the day (A6 at sign-in, `48 2B` / `48 2C`, the 24 h status and its bonus; hero.rs). Default
    /// true; tests that count messages turn it off.
    pub hero_of_the_day: bool,
    /// Store tabs of the 95 push (actions.rs). No game file has them; default = [`crate::actions::default_tabs`] (one
    /// tab for the default offers, CHOICE); empty = the store window shows its own built-in tabs.
    pub store_tabs: Vec<crate::actions::StoreTab>,
    /// Gold offers of the store tabs (`5C` -> `BC`). Default = [`crate::actions::default_offers`]: the 7-day premium card
    /// and the contract card (CHOICE); empty = no card.
    pub store_offers: Vec<crate::actions::StoreOffer>,
    /// Premium-time offers of the premium window (95 elements, bought with `46 02`; premium.rs). Default =
    /// [`crate::premium::default_offers`] (3 days / week / month / 3 months at the 0.6x web-shop gold prices); empty = the
    /// window lists nothing.
    pub premium_offers: Vec<crate::premium::PremiumOffer>,
    /// The game servers of the 9E list (servers.rs). Empty = the client's server list stays empty.
    pub game_servers: Vec<crate::servers::GameServer>,
    /// Most complaints / compliments one player may file per day (`48 06`). The client has texts for both limits
    /// (`st_claims_count_per_day_owerflow`, `st_compliment_count_per_day_owerflow`) but no file has the numbers:
    /// None = no daily maximum (OPEN). A complaint against the same player twice a day is always refused.
    pub claims_per_day: Option<u32>,
    pub compliments_per_day: Option<u32>,
    /// The event schedule (three event currencies, the exchange window, match drops; events.rs), ascending and not
    /// overlapping. No file has dates: empty = no event ever runs, no `94` record 23, no drops, `48 28` is refused.
    /// srp: `[[events]]` of the config file.
    pub events: Vec<game_data::events::Schedule>,
    /// Silver an online match costs by the player's equipment level (index = the level, 0 = nothing worn .. 5), charged
    /// after the match as the one "Match costs" entry (charge.rs). Default = [`EQUIPMENT_USE_COSTS_055`], the OFFICIAL
    /// patch 0.55 repair prices (CHOICE); the 0.69d data's own `equipment_use_costs` is 0 at every
    /// level. Empty = use the data's table.
    pub equipment_use_costs: Vec<u32>,
    /// Factor of the events' silver of an online match by the player's reward level (index = the level, 0..5), in place of
    /// `match_levels_factors.silver_factor` of the 0.69d data (1 / 1 / 2 / 3 / 3.5 / 4). Default = [`MATCH_SILVER_FACTORS`]
    /// (CHOICE: with the 0.55 repair prices a strong player stays clearly ahead at every level and an average one breaks
    /// even at level 5). Experience, reputation, the drop roll and the
    /// badge awards (the client computes those with its own data factors) are not touched. Empty = the data's factors.
    pub match_silver_factors: Vec<f64>,
}

/// The default [`LobbyConfig::match_silver_factors`]: level 0..5 = 1, 1, 3.5, 7, 10, 14 (the 0.69d data: 1, 1, 2, 3, 3.5, 4).
/// Worked out on 15-minute TDM matches (weak 4 kills 1 assist, average 8 / 3, strong 15 / 5) against
/// [`EQUIPMENT_USE_COSTS_055`]: a weak player keeps a little up to level 3 and loses at levels 4-5, an average one is about
/// even at level 5 (-130), a strong one nets +1235 / +2798 / +5245 / +5150 / +5940 at levels 1..5.
pub const MATCH_SILVER_FACTORS: [f64; 6] = [1.0, 1.0, 3.5, 7.0, 10.0, 14.0];

/// Patch 0.55 (Steam news 2018-10-31, OFFICIAL): "the price for repairs is now untied from the item of equipment or
/// weapons ... depends on the level of equipment, for example, if a player has put at least one item of equipment level 5,
/// he will pay for repairs the amount charged at the fifth level": level 1 0, 2 1000, 3 2000, 4 5000, 5 8000 silver.
/// Index = the equipment level, like the client's own `equipment_use_costs` (0 = nothing worn).
pub const EQUIPMENT_USE_COSTS_055: [u32; 6] = [0, 0, 1000, 2000, 5000, 8000];

impl LobbyConfig {
    /// The event whose window is open at `now` (unix seconds): its drops run while [`game_data::events::Schedule::active`].
    pub fn event_at(&self, now: i64) -> Option<&game_data::events::Schedule> {
        self.events.iter().find(|e| e.window_open(now))
    }

    /// Silver of the equipment use of a player of equipment level `level`: [`LobbyConfig::equipment_use_costs`], or the
    /// data's `equipment_use_costs` when the setting is empty; a level past the table takes its last value (CHOICE).
    pub fn equipment_use_cost(&self, gd: &game_data::GameData, level: u8) -> u32 {
        let t = &self.equipment_use_costs;
        if t.is_empty() {
            return gd.vouchers.equipment_use_cost(level);
        }
        t.get(usize::from(level)).or(t.last()).copied().unwrap_or(0)
    }

    /// The events' silver factor at reward level `level`: [`LobbyConfig::match_silver_factors`] (a level past the table
    /// takes its last value), None when the setting is empty (the data's `match_levels_factors`).
    pub fn match_silver_factor(&self, level: u32) -> Option<f64> {
        let t = &self.match_silver_factors;
        t.get(level as usize).or(t.last()).copied()
    }
}

impl Default for LobbyConfig {
    fn default() -> Self {
        LobbyConfig {
            match_min_time: Duration::from_secs(30),
            max_peers: 1024,
            achievement_rewards: true,
            daily_quests: true,
            treasure_hunt: true,
            faction_battles: None,
            league: game_data::LeagueSchedule::default(),
            login_calendar: Some(default_login_calendar()),
            online_queue: None,
            online_report_grace: Duration::from_secs(20),
            online_min_players: 2,
            online_bot_wait: Some(Duration::from_secs(30)),
            bots_per_team: 6,
            hero_of_the_day: true,
            store_tabs: crate::actions::default_tabs(),
            store_offers: crate::actions::default_offers(),
            premium_offers: crate::premium::default_offers(),
            game_servers: Vec::new(),
            claims_per_day: None,
            compliments_per_day: None,
            events: Vec::new(),
            equipment_use_costs: EQUIPMENT_USE_COSTS_055.to_vec(),
            match_silver_factors: MATCH_SILVER_FACTORS.to_vec(),
        }
    }
}

/// The server's own daily login calendar: boosters and vouchers. No game file or web source has the original table
/// (docs/notes/progression.md section 6), so the days are a CHOICE from the items of the window's own icon map (`BonusDayRenderer.dictIdToIconFrameMap`: 852, 340,
/// 339, 341, 1132, 342, 343, 860, 89; 852 / 860 / 89 are not in the 0.69d data) and the related per-match boosters
/// 344 / 853 / 857 / 861, escalating in value (silver boosters, cost 500 silver each, then the free stash map, then the
/// gold boosters, 5 gold each; `game.options`). A 7-day cycle: the window text says "On 7th day you will get"
/// (`st_login_reward_header`). The window draws each day's item icon and, as its amount text, the `labelValue` of the
/// item's own modifiers (`default_modifications`; PROVEN): the effect, not the count. A day may still
/// be a currency (214 / 925 / 215) in a configured table: `chests::grant` handles both. `quality` = Flash `award_quality`:
/// the star shows only for 1 (PROVEN, `BonusDayRenderer.setData`), on day 7.
///
/// | day | dict | item (`st_*` name) | effect for one match |
/// |---|---|---|---|
/// | 1 | 340 x1 | Tornado Energy Drink (`st_speed_booster_name`) | +5 % max movement speed |
/// | 2 | 341 x2 | Rations (`st_regen_booster_name`) | +2 health regeneration |
/// | 3 | 342 x2 | Radio (`st_respawn_booster_name`) | respawn time -0.30 |
/// | 4 | 1132 x1 | Stash map (`st_drop_card_free_name`) | free drop card of the treasure hunt |
/// | 5 | 861 x1 | Reputation booster (`st_rep_booster_24h_50_name`) | +50 % reputation |
/// | 6 | 853 x1 | Experience booster (`st_experience_booster_24h_50_name`) | +50 % experience |
/// | 7 | 857 x3 | Silver booster (`st_money_booster_24h_50_name`) | +50 % silver, three matches, star |
pub fn default_login_calendar() -> Vec<protocol::s2c::quests::BonusDay> {
    use protocol::s2c::quests::BonusDay;
    let day = |reward: u16, amount: u32| BonusDay { amount, reward, quality: 0 };
    vec![
        day(340, 1),
        day(341, 2),
        day(342, 2),
        day(1132, 1),
        day(861, 1),
        day(853, 1),
        BonusDay { amount: 3, reward: 857, quality: 1 },
    ]
}
