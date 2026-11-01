//! What a finished 0.26g match gives its players, and the results screen
//! that shows it (`docs/notes/results.md`).
//!
//! The match server hands over a [`MatchResult`] (a plain copy of its own
//! `match_server::v026g::MatchResult`, field for field, so the adapter is a
//! move of each field). [`Results::settle`] changes each player's
//! [`Account`] and keeps a [`MatchReport`]: the results screen (`97`) for
//! each player, the equipment a click on a row shows (`93`), and the
//! notices. Nothing here knows of sockets: the lobby sends what
//! [`MatchReport::lobby_push`] lists when the player's client is in the
//! lobby again, or [`MatchReport::screen_for`] after a sign-in.
//!
//! # The rules
//!
//! The 0.26g data has no reward values, as 0.21d's had none. The numbers
//! are fitted to two results screens of the time and to the wiki and
//! patch notes of 0.24 to 0.27 (`progression_sources.md`, `results.md`
//! §10); they are all in [`super::pay`], each with its tag.
//!
//! - **Score.** The client counts 28 events a kit and weighs them
//!   (`score += round(count × weight × k)`, k = 0.01 for the three battery
//!   events counted in hundredths). At the end of a match the client adds,
//!   by itself, event 11 (participating) for those still in, event 12 (won)
//!   for the winning team and event 27 for a player under new-player
//!   protection, all to kit 0 (PROVEN: 0.26g `shared_statistics::
//!   finish_match` 0x2F4CF0; 0.21d's also added event 16, 0.26g's does
//!   not). The last snapshot the server holds is from before that, so the
//!   server adds the first two itself, as 0.21d's did (their weights are
//!   0).
//! - **The pay is by the screen's three rows**, each cell its plain
//!   amount times the match level's multiplier (`match_levels_dict`:
//!   `money_k`, `experience_k`, `reputation_k`), rounded down after
//!   multiplying (PROVEN by the second screen):
//!   - *Enemies killed*: [`pay::KILL`] a kill (25 silver, 3 experience,
//!     1 reputation). A team kill is in the row and pays nothing.
//!   - *Help ally*: [`pay::HELP`] (10 / 1 / 1) an assist, a teammate
//!     cured or a teammate rescued.
//!   - *Match Activity Rewards*: a fixed part for a match played to the
//!     end ([`pay::ACTIVITY_WON`]; [`pay::ACTIVITY_NOT_WON`] of it when
//!     the match was not won; none for a player who left early) and
//!     [`pay::ACTIVITY_PER_SCORE`] for each point of "other score": the
//!     score of the row's own events (every event that is no kill and no
//!     help), counted with the weights the match was played with
//!     ([`Terms::score_weights`], the ones the scoreboard used; a Research
//!     match's weigh its captures and its "kill invader": `static_info::
//!     RESEARCH_WEIGHTS`).
//! - **The rates** (the owner's choices): the three rows' silver times the
//!   percent of the match's level ([`pay::MONEY_RATES`]: 150 to level 5,
//!   rising to 220 from level 12; or `SRP_MONEY_RATE` at every level);
//!   their experience times `SRP_EXPERIENCE_RATE` (default 2); their
//!   reputation and the quests' times `SRP_REPUTATION_RATE` percent
//!   (default 120).
//! - **Reputation** goes to the faction whose quests the account takes;
//!   none until it chose one (PROVEN: patch note 0.25a).
//! - **Premium**: half again of the three rows, of all three kinds (PROVEN
//!   as what the screen promises: `VictoryScreen.as` adds the premium
//!   block with "+50%"; and by the second screen's numbers). It is half
//!   of the rows, not of the daily bonus. Without premium the block still
//!   says what premium would have given, and the screen's button buys it:
//!   [`Results::claim_premium`].
//! - **The daily bonus**: the first three victories of a day pay the
//!   rows' experience and reputation once more, and no silver
//!   ([`super::bonus`]; PROVEN by the first screen). So the total is rows
//!   + daily + premium.
//! - **Levels**: the account's table ([`Account::add_experience`]); each
//!   level pays [`pay::level_reward`].
//! - **Rating**: 0.21d's Elo between the teams (`v021d::rating`), from
//!   [`pay::START_RATING`].
//! - **The balance rating** (srp's own, for the sides of a match:
//!   [`balance`]): moved by a match that had `SRP_BALANCE_RATED_PLAYERS`
//!   players or more in it (default 4), each by its share of the match.
//! - **Quests**: [`quests::evaluate_scaled`] with the match's numbers:
//!   a quest's silver and reputation times the match level's multipliers,
//!   its reputation times `SRP_REPUTATION_RATE` percent too, and twice in
//!   one of the day's doubled matches.
//! - **A match against the server's bots** ("Player vs Bots",
//!   [`MatchResult::vs_bots`]; the owner's of 2026-10-16): its rows pay
//!   [`Terms::pvb_pay`] (`SRP_PVB_PAY`, default half) of their silver and
//!   experience, and so do the premium half and the items' bonus counted
//!   from them; reputation, the drop and the pay of a level reached are as
//!   in any match, but at most at match level 7
//!   ([`super::pvb::MAX_REWARD_LEVEL`] for the pay,
//!   [`super::pvb::MAX_DROP_LEVEL`] for the drop). It counts for no quest
//!   and no daily bonus, and, as every match with a bot of the server's,
//!   moves no rating and no balance rating.
//! - **A room with a PIN's match** ([`MatchResult::private`]; the
//!   owner's of 2026-10-20): pays nothing to anyone in it, late joiners
//!   too: no experience, silver, reputation, quest, daily bonus, drop or
//!   rating, and takes nothing (no wear, no supplies used, no autobuy or
//!   auto-repair after it). Its screen opens with the match's numbers and
//!   nothing paid, and takes no drop card.
//! - **Wear** (only with `SRP_ITEM_WEAR=on`; off by the owner's choice,
//!   [`pay::item_wear`]): every item of a kit the player used loses its
//!   `degrade_per_match`; a kit's firearms share `degrade_per_use` for each
//!   bullet the kit fired [choice: which weapon fired is not in the
//!   statistics]; `degrade_per_death` for each death (0 in every 0.26g
//!   item). Armour's `degrade_per_use` is not applied: how often it was hit
//!   is not in the statistics.
//! - **A drop**: one roll of `survarium::v026g::drops` for a player who
//!   stayed to the end: the new player's table while the account has no
//!   experience, else the generic one. The chance has three parts, each
//!   sent in its own place of the screen: the table's (40 %), with the play
//!   time bonus only when `SRP_DROP_PLAYTIME_BONUS=on`
//!   ([`pay::drop_playtime_bonus`]); [`pay::DROP_CHANCE_PER_BATTERY`] for each battery the
//!   player's own team has at the end (modes 2 and 3); [`pay::DROP_CHANCE_PREMIUM`] for a
//!   premium account. Together they are at most 1. A weapon or an armour
//!   piece found so arrives worn, with one defect
//!   ([`pay::drop_condition`], [`pay::drop_defect`]).
//! - **A drop card** (`drop_card`, used on the results screen) is one
//!   roll of `drop_card_drop` at the match's level, and only one for each
//!   player that finished the match: [`Results::use_drop_card`]
//!   (`docs/notes/drop_cards.md`).
//! - **Ammo and consumables used** are taken off the stacks as far as
//!   the last trusted snapshot proves them used, and the account's autobuy
//!   and auto-repair boxes buy them back and repair what the match wore:
//!   [`super::autobuy`]. The screen's money total is net of what they
//!   spent: the client takes it off itself, from the lists the inventory
//!   push carries (`VictoryScreen.updateMoney`); the rows here are gross.
//! - **A level reached** is told with important message 14
//!   ([`super::notices`]).
//!
//! # Who gets what
//!
//! | the player | gets |
//! |---|---|
//! | stayed to the end | everything above |
//! | left early | what its counters earned (kills, help, other score), a loss in the rating whatever its team did, wear, quests; nothing of the activity row's fixed part, no daily bonus and no drop |
//! | never got into the match | nothing; its rating stays (0.21d) |
//! | left by its own word and came into the match again (a new place) | its latest place's as above; the place it left counts for nothing ([`MatchResult::latest_places`]) |
//! | any, when play never began (`played_ms` is `None`) | nothing |
//! | any, when no snapshot was trusted (`trusted_by` 0) | no counters and no winner: the fixed part of a match not won, wear, a drop; the rating stays (0.21d's unknown result) |
//! | any, when one client alone vouched (`trusted_by` 1) | in full, as 0.21d paid a match one client was alone in (its numbers passed the match server's checks) |
//!
//! A match no client ever reached ([`MatchEnd::NoShow`]) has no report, nor
//! has a room's match called off before its countdown
//! ([`MatchEnd::NotMade`]): no screen, pay, rating, quest or wear.
//! A deserted match is a match all of whose players left early.
//!
//! # What the screen shows
//!
//! The three score lines are the game's own
//! (`gameplay/ui/victory_screen.options`: `categories` and
//! `event_categories`): kills, help, and everything else. A line's amount
//! is how many of its paying events the kit made (finishing and winning
//! count one each); the server sends nothing more for a column whose
//! amount is 0. Each line carries its own silver, experience and
//! reputation: the screen shows them row by row.
//! The daily block is the day's double pay ([`super::bonus`]); the
//! equipment block what the kit's items add (`items_rules::kit_bonuses`).

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use survarium::v021d;
use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::drops::{self, DropTable, Drops, Rng};
use survarium::v026g::item_params::item_params;
use survarium::v026g::lobby::{
    self, AccountBody, BonusColumn, FULL_CONDITION, FullResult, ItemDescr, ResultRow,
    ServerMessage, TitleHolder, WireError, limits,
};
use survarium::v026g::maps::mode;
use survarium::v026g::match_msgs::snapshot::{EVENTS, HUNDREDTHS, KITS, PlayerStatistics, event};
use survarium::v026g::match_msgs::static_info::{self, SCORE_WEIGHTS};
use survarium::v026g::modifications;
use survarium::v026g::quests::MatchStats as QuestStats;

use super::account::Account;
use super::level_stats::KillsDeaths;
use super::pay::{self, Pay};
use super::persist::GameRepository;
use super::quests::{self, Notice, REPUTATION_LEVEL_MESSAGE};
use super::{GameAccounts, balance, items_rules, rules, wire};
use crate::LockExt;

/// The winner value of a draw (`survarium::v026g::match_msgs::rules::DRAW`).
pub const DRAW: u8 = survarium::v026g::match_msgs::rules::DRAW;
/// The winner byte of a screen whose match has no known result (0.21d's
/// `game_team_id` for it). The client only compares the byte with the
/// player's team.
pub const NO_WINNER: u8 = 3;

/// The three lines of the score: their localization keys and whether the
/// amount is shown (`gameplay/ui/victory_screen.options`, `categories`).
pub const SCORE_LINES: [(&str, bool); 3] = [
    ("st_statistics_score_kills", true),
    ("st_statistics_score_help", true),
    ("st_statistics_score_game_events", false),
];
/// The line each event counts in (the same file, `event_categories`).
pub const EVENT_LINES: [usize; EVENTS] = [
    0, 1, 2, 2, 0, 2, 2, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
];
/// The line of the match activity: the fixed part for finishing and
/// winning, and the other score.
const ACTIVITY_LINE: usize = 2;
/// A bonus to [`drops::roll`]'s chance that makes any table give: the
/// chance is rolled here, with its three parts.
const CERTAIN_PERCENT: u32 = 100_000_000;

/// Event 25: a kill with a weapon of `item_category_grenades` (PROVEN:
/// `shared_statistics::on_player_killed` 0x2F5141 compares the weapon's
/// category with it and counts event 25).
pub const GRENADE_KILL: usize = 25;
/// Event 21: an enemy caught in a trap (0.21d's name for it; LIKELY kept).
pub const ENTRAP_ENEMY: usize = 21;

/// The silver and the gold as dropped items. The client keeps neither in
/// its inventory (`results.md` §3: dict ids 0xD6 and 0xD7 are never added).
pub const GENERIC_MONEY: u16 = 214;
pub const PREMIUM_MONEY: u16 = 215;
/// The drop card's item name (`use_drop_card`).
pub const DROP_CARD: &str = "drop_card";

/// The reports kept for players asking again (`41 00`, `3D 00`).
const KEPT: usize = 64;

/// How long a report is kept past [`KEPT`] for an account that was shown
/// its screen: the results screen asks for a row's player when it is
/// clicked (`3D 00 <player> <match>`), for as long as it stays open, and
/// the client has nothing to show for a report that is gone (no "empty"
/// 0x93: `results.md` §2). [srp's own choice: two hours; the client does
/// not say when the screen is closed.]
const SHOWN_FOR_SECONDS: u64 = 2 * 60 * 60;

/// Why a match was over (the match server's `MatchEnd`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MatchEnd {
    ModeRule,
    TimeLimit,
    Deserted,
    NoShow,
    /// A room's match called off before its countdown.
    NotMade,
}

/// One roster place of a finished match (the match server's
/// `PlayerResult`).
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerResult {
    pub place: u8,
    pub session_id: u32,
    pub account_id: u32,
    pub name: String,
    pub team: u8,
    /// The match server's own bot (a match against bots): no account (0),
    /// no session; it is shown and paid nothing.
    pub driven: bool,
    /// Whether the player was ever in the match.
    pub entered: bool,
    pub present_at_end: bool,
    pub left: bool,
    pub time_in_match_ms: u32,
    /// Its counters in the last trusted snapshot, by event and kit.
    pub statistics: Option<PlayerStatistics>,
    /// Payment-only counters after anti-farming limits. `None` pays the
    /// ordinary statistics; scores, quests and rating never use this copy.
    pub pay_statistics: Option<PlayerStatistics>,
    /// The scoreboard's score from those counters.
    pub score: u32,
    pub kills: u16,
    pub deaths: u16,
    pub longest_killstreak: u16,
    /// Its kit choices for a tick before play began (the match server's
    /// `PlayerResult::kit_changes`): each may have been a respawn in place
    /// with the new kit's supplies (patch 31), a life more than its deaths
    /// for what the match used.
    pub kit_changes: u16,
    /// What the player had left in the last trusted snapshot: alive or
    /// not, the kit in use, and what is left of every item of its profile
    /// (`Supplies::left` adds a stack item up). `None`: not known.
    pub supplies: Option<survarium::v026g::match_msgs::players::Supplies>,
}

/// How a match ended (the match server's `MatchResult`).
#[derive(Clone, Debug, PartialEq)]
pub struct MatchResult {
    pub match_id: u32,
    pub map_id: u8,
    /// `survarium::v026g::maps::mode`.
    pub game_mode: u8,
    pub match_level: u16,
    pub end: MatchEnd,
    /// 0 or 1, [`DRAW`], or `None` when no trusted snapshot gave a standing.
    pub winner: Option<u8>,
    /// Batteries home (modes 2, 3), tickets left (mode 4) or Research's
    /// points (mode 5).
    pub team_points: [u16; 2],
    pub duration_ms: u32,
    /// `None` if play never began.
    pub played_ms: Option<u32>,
    pub trusted_snapshot_ms: Option<u32>,
    /// How many clients vouched for the snapshot the numbers are from; 0
    /// with none.
    pub trusted_by: u8,
    /// By roster place.
    pub players: Vec<PlayerResult>,
    /// A match of one player against the server's bots ("Player vs Bots",
    /// the match server's `MatchSetup::vs_bots`): it pays
    /// [`Terms::pvb_pay`] of its silver and experience, and counts for no
    /// quest and no daily bonus.
    pub vs_bots: bool,
    /// A match of a room with a PIN (the match server's
    /// `MatchSetup::private`): it pays nothing to anyone in it, late
    /// joiners too, and its screen takes no drop card (the owner's of
    /// 2026-10-20; `docs/notes/rooms.md`).
    pub private: bool,
    /// A Blackout match (`docs/notes/blackout.md`): every point a player
    /// earns in it counts twice, in its score and in what the score pays
    /// ([`BLACKOUT_FACTOR`]); the counts (kills, batteries) are as played.
    pub blackout: bool,
}

/// What a Blackout match's score and pay are worth: the owner's of
/// 2026-10-25, "in blackout everything should give twice as much match
/// points".
pub const BLACKOUT_FACTOR: f64 = 2.0;

impl MatchResult {
    /// The result with each account's latest place only: a player that
    /// left the match by its own word and came into it again is a new
    /// place, and the place it left counts for nothing, as if it was not
    /// there (the owner's of 2026-10-19). The server's own bots (account
    /// 0) are all kept.
    pub fn latest_places(&self) -> MatchResult {
        let players = self
            .players
            .iter()
            .enumerate()
            .filter(|(index, player)| {
                player.account_id == 0
                    || !self.players[index + 1..]
                        .iter()
                        .any(|later| later.account_id == player.account_id)
            })
            .map(|(_, player)| player.clone())
            .collect();
        MatchResult {
            players,
            ..self.clone()
        }
    }
}

/// What the rewards of a match are counted with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Terms {
    /// Unix seconds.
    pub now: u64,
    /// What a match's experience is multiplied by.
    pub experience_rate: u32,
    /// The percent of the original's silver the match's rows pay, by the
    /// match's level.
    pub money_rate: pay::MoneyRate,
    /// The percent of the original's reputation the match's rows and its
    /// quests pay.
    pub reputation_rate: u32,
    /// The score weights of the matches of modes 2 to 4; a Research
    /// match's are these with its own three ([`static_info::weights_of`]),
    /// as the match server sends them.
    pub score_weights: [f32; EVENTS],
    /// Whether the match wears the items of the kits played
    /// ([`pay::item_wear`]).
    pub wear: bool,
    /// Whether the first games of a play period raise the drop chance
    /// ([`pay::drop_playtime_bonus`]).
    pub playtime_bonus: bool,
    /// The fewest players a match must have had in it to move the balance
    /// rating ([`balance::players_in`]). 1: every match does.
    pub rated_players: u32,
    /// What a match against the server's bots ([`MatchResult::vs_bots`])
    /// pays of the silver and the experience of its rows, 0 to 1
    /// (`SRP_PVB_PAY`, default 0.5; [`super::pvb::pay`]).
    pub pvb_pay: f64,
}

impl Terms {
    /// srp's terms at `now`: `SRP_EXPERIENCE_RATE` (default 2),
    /// `SRP_MONEY_RATE` (default: by the match's level),
    /// `SRP_REPUTATION_RATE` (default 120), `SRP_ITEM_WEAR` and
    /// `SRP_DROP_PLAYTIME_BONUS` (both off unless set),
    /// `SRP_BALANCE_RATED_PLAYERS` (default 4), `SRP_PVB_PAY` (default
    /// 0.5) and the weights srp's matches are made with.
    pub fn at(now: u64) -> Self {
        Self {
            now,
            experience_rate: pay::experience_rate(),
            money_rate: pay::money_rate(),
            reputation_rate: pay::reputation_rate(),
            score_weights: SCORE_WEIGHTS,
            wear: pay::item_wear(),
            playtime_bonus: pay::drop_playtime_bonus(),
            rated_players: balance::rated_players(),
            pvb_pay: super::pvb::pay(),
        }
    }
}

/// How a player's match went, for the rewards.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Standing {
    /// It was in the match when it ended.
    Finished,
    /// It was in the match and left before the end.
    Left,
    /// Its game never got into the match.
    #[default]
    Absent,
    /// Play never began.
    NotPlayed,
}

/// The item a match dropped.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Dropped {
    pub dict_id: u16,
    /// Units of a stack (silver, gold, vouchers); 1 for anything else.
    pub count: u32,
    /// The instance in the inventory; 0 for silver and gold, which go to
    /// the balances.
    pub item_id: u32,
    /// What the screen is told: a stack's rarity as rolled; for a weapon
    /// or armour its rank, the modifications it was given (as many as its
    /// rarity, `items_rules::roll_modifications`).
    pub rarity: u8,
    /// What the table rolled.
    pub rolled_rarity: u8,
}

/// What a match gave one player.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Reward {
    pub standing: Standing,
    pub won: bool,
    /// The score with the end-of-match events.
    pub score: u32,
    /// Whether the premium account held when the match was settled.
    pub premium: bool,
    /// What the account got for the match itself, the premium half
    /// included when it held: not the quests' pay, not the levels' rewards.
    pub experience: u32,
    pub money: u32,
    /// 0: no faction chosen, no reputation.
    pub reputation_faction: u8,
    pub reputation: u32,
    /// What premium adds: `(money, experience, reputation)`. Part of the
    /// three above when `premium`; else what [`Results::claim_premium`]
    /// gives.
    pub premium_bonus: (u32, u32, u32),
    /// The levels reached, and what they paid.
    pub levels: Vec<u8>,
    pub level_money: u32,
    pub level_gold: u32,
    /// The reputation levels the match's own reputation reached.
    pub reputation_levels: Vec<u8>,
    pub quest_money: u32,
    /// `(faction, points)`.
    pub quest_reputation: Vec<(u8, u32)>,
    pub rating_before: u16,
    pub rating_after: u16,
    pub drop: Option<Dropped>,
    /// The chance the drop was rolled with, all its parts together; 0
    /// with no roll.
    pub drop_chance: f32,
    /// `(item id, condition before, condition after)`.
    pub worn: Vec<(u32, u32, u32)>,
    /// `(item id, defect)`: the defects the match's wear gave.
    pub defects: Vec<(u32, u32)>,
    /// The match was one of the day's first three victories: its
    /// experience and reputation were paid once more ([`super::bonus`]).
    pub daily_bonus: bool,
    /// The seven days' reward, given by this victory.
    pub daily_reward: Option<super::bonus::Given>,
    /// `(dict id, units)` of ammo and other stacks the match is known to
    /// have used ([`super::autobuy`]).
    pub supplies_used: Vec<(u16, u32)>,
    /// What autobuy and auto-repair spent after the match: the lists the
    /// results screen shows.
    pub spent: lobby::SpentMoney,
}

/// One player of a [`MatchReport`].
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PlayerReport {
    pub account_id: u32,
    /// The id the screen knows the player by: its roster place + 1.
    pub player_id: u32,
    pub reward: Reward,
    /// The player's own part of `97`.
    pub full: FullResult,
    /// Its kits as the match left them, for `93`.
    pub body: AccountBody,
    /// Important messages for its client: `(type, text)`. The lobby gives
    /// each its message id.
    pub notices: Vec<Notice>,
}

/// A finished match as the lobby shows it.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MatchReport {
    pub match_id: u32,
    pub end: MatchEnd,
    pub match_level: u16,
    /// A room with a PIN's match ([`MatchResult::private`]): nothing was
    /// paid, and its screen takes no drop card. Absent from a report kept
    /// before it was known: not private.
    #[serde(default)]
    pub private: bool,
    /// A match against the server's bots ([`MatchResult::vs_bots`]): a
    /// drop card on its screen rolls at its capped drop level
    /// ([`super::pvb::drop_level`]). Absent from a report kept before it
    /// was known: not against bots.
    #[serde(default)]
    pub vs_bots: bool,
    /// The part of `97` every player gets.
    pub stats: lobby::MatchStats,
    /// By roster place.
    pub players: Vec<PlayerReport>,
}

impl MatchReport {
    pub fn index_of(&self, account_id: u32) -> Option<usize> {
        self.players
            .iter()
            .position(|player| player.account_id == account_id)
    }

    pub fn player(&self, account_id: u32) -> Option<&PlayerReport> {
        self.index_of(account_id).map(|index| &self.players[index])
    }

    /// `97` for the account's player: it opens the results screen. The
    /// answer to `41 00 <match id>` too.
    pub fn screen_for(&self, account_id: u32) -> Option<Result<ServerMessage, WireError>> {
        let player = self.player(account_id)?;
        Some(lobby::match_stats(&self.stats, &player.full))
    }

    /// `93`, the answer to `3D 00 <player id> <match id>`: the kits of the
    /// player a row of the screen names.
    pub fn player_details(&self, player_id: u32) -> Option<Result<ServerMessage, WireError>> {
        let player = self
            .players
            .iter()
            .find(|player| player.player_id == player_id)?;
        Some(lobby::player_details(
            &FullResult::empty(player_id),
            &player.body,
        ))
    }

    /// `89` with the money autobuy and auto-repair spent for the account's
    /// player after the match, for the results screen's lists; `None` when
    /// nothing was spent. Send it before the screen (`97`): a client that
    /// signed in after the match has its inventory without the lists.
    pub fn spent_inventory(
        &self,
        account_id: u32,
        account: &Account,
    ) -> Option<Result<ServerMessage, WireError>> {
        let spent = &self.player(account_id)?.reward.spent;
        (*spent != lobby::SpentMoney::default()).then(|| wire::inventory_with_spent(account, spent))
    }

    /// The notices of the account's player.
    pub fn notices_for(&self, account_id: u32) -> &[Notice] {
        self.player(account_id)
            .map_or(&[], |player| &player.notices)
    }

    /// Everything a client that sat through the match needs when it is in
    /// the lobby again, in order: what the match changed of the account
    /// (inventory before the account, then money, level, reputations,
    /// quests and, with its `place` in the ranking, the rating), then the
    /// screen. The account goes first because the screen's experience and
    /// reputation bars are read from what the client holds when `97`
    /// arrives (`LobbyMenu.fill_victory_screen`). The notices go after, by
    /// the lobby's own numbering. After the screen the client sends `3C 00`
    /// (answer: the quests).
    pub fn lobby_push(
        &self,
        account_id: u32,
        account: &Account,
        now: u64,
        place: Option<u32>,
    ) -> Option<Result<Vec<ServerMessage>, WireError>> {
        let screen = self.screen_for(account_id)?;
        Some((|| -> Result<Vec<ServerMessage>, WireError> {
            let mut push = wire::after_change(account, now)?;
            // The inventory carries what autobuy and auto-repair spent.
            if let Some(spent) = self.spent_inventory(account_id, account) {
                push[0] = spent?;
            }
            push.push(wire::level_and_skills(account)?);
            push.push(wire::reputations(account)?);
            push.push(wire::quests(account, now));
            if let Some(place) = place {
                push.push(wire::player_elo(account, place));
            }
            push.push(screen?);
            Ok(push)
        })())
    }
}

/// What a signed-in client has not been shown yet.
#[derive(Clone, Debug, PartialEq)]
pub struct Unseen {
    /// The last match it has not seen the screen of.
    pub report: Arc<MatchReport>,
    /// The notices of every match since it was last shown one, in order.
    pub notices: Vec<Notice>,
}

/// The repository row with who has not seen what, the play periods and the
/// premium bonuses claimed ([`State`]).
const STATE_KEY: &str = "state";
/// A report's row is under this and its match id.
const REPORT_KEY: &str = "report:";

fn report_key(match_id: u32) -> String {
    format!("{REPORT_KEY}{match_id}")
}

/// Takes off the upgrade records of a kept report's items that the game data
/// no longer has (a dict id of no item) or no longer fits (a scope variant
/// that is not made for its item any more, `survarium::v026g::scopes`): a
/// release that withdraws a scope pair leaves old results screens behind.
/// Returns how many were taken off.
pub(crate) fn drop_stale_upgrades(report: &mut MatchReport) -> usize {
    let dictionary = Dictionary::shared();
    let scopes = survarium::v026g::scopes::ScopeFits::shared();
    let mut taken = 0;
    let mut clean = |item: &mut lobby::ItemDescr| {
        let host = item.dict_id;
        let before = item.upgrades.len();
        item.upgrades.retain(|record| {
            u16::try_from(record.a).ok().is_some_and(|dict_id| {
                dictionary.item(dict_id).is_some()
                    && (!scopes.is_variant(dict_id) || scopes.fits(host, dict_id))
            })
        });
        taken += before - item.upgrades.len();
    };
    for player in &mut report.players {
        if let Some(dropped) = &mut player.full.dropped {
            clean(dropped);
        }
        for item in &mut player.body.items {
            clean(item);
        }
    }
    taken
}

/// The reports of a repository's rows that read ([`Results::restored`]
/// keeps those a state lists), for the start's check of what they name
/// (`super::known`).
pub(crate) fn kept_reports(rows: &[(String, String)]) -> Vec<MatchReport> {
    rows.iter()
        .filter(|(key, _)| key.starts_with(REPORT_KEY))
        .filter_map(|(_, body)| serde_json::from_str(body).ok())
        .collect()
}

/// What of [`Inner`] is written beside the reports, as JSON under
/// [`STATE_KEY`]. Lists, sorted: the same state is the same text.
#[derive(Default, serde::Serialize, serde::Deserialize)]
struct State {
    /// The matches whose reports are kept, oldest first.
    reports: Vec<u32>,
    /// `(account, the matches it has yet to be shown)`.
    unseen: Vec<(u32, Vec<u32>)>,
    /// `(account, notices of reports no longer kept)`.
    stray: Vec<(u32, Vec<Notice>)>,
    /// `(account, when its last match ended, games of the play period)`.
    periods: Vec<(u32, u64, u32)>,
    /// `(match, account)`.
    claimed: Vec<(u32, u32)>,
}

/// What changed since the repository was last written.
#[derive(Default)]
struct Changes {
    /// Anything of [`State`].
    state: bool,
    /// Reports made.
    saved: Vec<u32>,
    /// Reports no longer kept.
    deleted: Vec<u32>,
}

impl Changes {
    fn is_empty(&self) -> bool {
        !self.state && self.saved.is_empty() && self.deleted.is_empty()
    }
}

#[derive(Default)]
struct Inner {
    changes: Changes,
    reports: VecDeque<Arc<MatchReport>>,
    /// The matches each account has yet to be shown, oldest first.
    unseen: HashMap<u32, Vec<u32>>,
    /// Notices of reports no longer kept.
    stray: HashMap<u32, Vec<Notice>>,
    /// `(when its last match ended, games of the play period)`.
    periods: HashMap<u32, (u64, u32)>,
    /// `(match, account)`: premium bonuses given after the match.
    claimed: Vec<(u32, u32)>,
    /// The match whose results screen each account's client was last sent,
    /// and when (Unix seconds): the screen may still be open, so the
    /// report stays ([`SHOWN_FOR_SECONDS`]).
    shown: HashMap<u32, (u32, u64)>,
    /// The equipment level of each match being played, by the id its
    /// players were sent with: the highest gear level of its players
    /// ([`Results::raise_match_gear`]). Not kept across a restart, which
    /// ends the matches.
    gear: HashMap<u32, u8>,
}

impl Inner {
    fn find(&self, match_id: u32) -> Option<Arc<MatchReport>> {
        self.reports
            .iter()
            .rev()
            .find(|report| report.match_id == match_id)
            .cloned()
    }

    /// Make room for one more report: all but the newest `KEPT - 1` go,
    /// except those whose screen an account was sent less than
    /// [`SHOWN_FOR_SECONDS`] before `now` (they are not counted).
    fn make_room(&mut self, now: u64) {
        self.shown
            .retain(|_, &mut (_, at)| now.saturating_sub(at) < SHOWN_FOR_SECONDS);
        let open: Vec<u32> = self.shown.values().map(|&(match_id, _)| match_id).collect();
        let mut free = KEPT - 1;
        let mut gone = Vec::new();
        let mut kept = VecDeque::with_capacity(self.reports.len());
        for report in std::mem::take(&mut self.reports).into_iter().rev() {
            if open.contains(&report.match_id) {
                kept.push_front(report);
            } else if free > 0 {
                free -= 1;
                kept.push_front(report);
            } else {
                gone.push(report);
            }
        }
        self.reports = kept;
        for gone in gone {
            self.changes.state = true;
            self.changes.deleted.push(gone.match_id);
            // Its screen is lost to who has not seen it; its notices are not.
            for player in &gone.players {
                let waits = self
                    .unseen
                    .get(&player.account_id)
                    .is_some_and(|ids| ids.contains(&gone.match_id));
                if waits && !player.notices.is_empty() {
                    self.stray
                        .entry(player.account_id)
                        .or_default()
                        .extend(player.notices.iter().cloned());
                }
            }
            self.claimed.retain(|&(id, _)| id != gone.match_id);
        }
    }

    /// Which game of its play period this is for the account, from 1. A
    /// period ends when the account finishes no match for `play_period`
    /// minutes [GUESS: the data has the number 60 and no unit or rule].
    fn game_of_period(&mut self, account_id: u32, now: u64) -> u32 {
        let period = Self::play_period();
        self.changes.state = true;
        let entry = self.periods.entry(account_id).or_insert((now, 0));
        if now.saturating_sub(entry.0) >= period {
            entry.1 = 0;
        }
        *entry = (now, entry.1.saturating_add(1));
        entry.1
    }

    /// A play period's length in seconds.
    fn play_period() -> u64 {
        u64::from(Drops::shared().playtime.play_period) * 60
    }

    /// What is written under [`STATE_KEY`].
    fn state(&self) -> State {
        let mut unseen: Vec<(u32, Vec<u32>)> = self
            .unseen
            .iter()
            .map(|(&account, ids)| (account, ids.clone()))
            .collect();
        unseen.sort_unstable();
        let mut stray: Vec<(u32, Vec<Notice>)> = self
            .stray
            .iter()
            .map(|(&account, notices)| (account, notices.clone()))
            .collect();
        stray.sort_unstable_by_key(|&(account, _)| account);
        // A period that ended a whole period before the newest match did
        // is over whenever its account plays next: not kept.
        let newest = self.periods.values().map(|&(at, _)| at).max().unwrap_or(0);
        let mut periods: Vec<(u32, u64, u32)> = self
            .periods
            .iter()
            .filter(|&(_, &(at, _))| newest.saturating_sub(at) < Self::play_period())
            .map(|(&account, &(at, games))| (account, at, games))
            .collect();
        periods.sort_unstable();
        State {
            reports: self.reports.iter().map(|report| report.match_id).collect(),
            unseen,
            stray,
            periods,
            claimed: self.claimed.clone(),
        }
    }
}

/// The finished matches of a running srp: their reports, who has not seen
/// its own yet, and each account's play period.
///
/// [`Results::new`] keeps them in memory only. [`Results::attached`] keeps
/// them in the accounts' repository as well (`g26_results`), so that a
/// restart forgets neither a results screen not shown yet (with its
/// notices, and the premium bonus that can still be claimed from it) nor
/// which game of its play period an account's next match is.
///
/// The last [`KEPT`] reports stay, and past them any whose screen a client
/// may still have open ([`SHOWN_FOR_SECONDS`]): a click on a row of an open
/// results screen is always answered. (A restart is no gap either: the
/// lobby link of a client does not outlive srp, so no results screen does,
/// and which screens were open is not kept.)
#[derive(Default)]
pub struct Results {
    inner: Mutex<Inner>,
}

impl Results {
    pub fn new() -> Self {
        Self::default()
    }

    /// The equipment level of the match being played that its players
    /// were sent to as `match_id`: the highest gear level of its players
    /// so far ([`Self::raise_match_gear`]); `None` for one it does not
    /// know.
    pub fn match_gear(&self, match_id: u32) -> Option<u8> {
        self.inner.locked().gear.get(&match_id).copied()
    }

    /// A player of gear level `gear` is in the match `match_id` (made, or
    /// let in late): the match's equipment level is the highest of its
    /// players' (the owner's of 2026-11-01), what its kills, deaths and
    /// ratings are counted at when it is settled (`level_stats`). The level
    /// now.
    pub fn raise_match_gear(&self, match_id: u32, gear: u8) -> u8 {
        let mut inner = self.inner.locked();
        let level = inner.gear.entry(match_id).or_insert(gear);
        *level = (*level).max(gear);
        *level
    }

    /// The results kept in `accounts`' repository: what it had of them
    /// when it was opened ([`GameAccounts::take_restored_results`]), and
    /// every change from here written at each [`GameAccounts::flush`]
    /// (the saver's passes, and the one after a match is settled). With
    /// accounts kept in memory it is [`Results::new`].
    pub fn attached(accounts: &GameAccounts) -> Arc<Self> {
        let results = Arc::new(Self::restored(accounts.take_restored_results()));
        let kept = Arc::downgrade(&results);
        accounts.on_flush(Box::new(move |repository| {
            if let Some(results) = kept.upgrade() {
                results.save_changes(repository);
            }
        }));
        results
    }

    /// The results a repository's rows hold ([`Results::save_changes`]
    /// wrote them). A row that does not read (written by other code, or
    /// damaged) is left out with a warning, and a report nothing lists is
    /// deleted at the next save: what is lost is a screen, never anything
    /// an account was given.
    pub fn restored(rows: Vec<(String, String)>) -> Self {
        let mut inner = Inner::default();
        let mut state = State::default();
        let mut bodies: HashMap<u32, String> = HashMap::new();
        for (key, body) in rows {
            if key == STATE_KEY {
                match serde_json::from_str(&body) {
                    Ok(read) => state = read,
                    Err(error) => {
                        log::warn!(
                            "The kept results' state does not read ({error}): the screens not \
                             shown and the play periods of before the restart are forgotten"
                        );
                        inner.changes.state = true;
                    }
                }
            } else if let Some(match_id) =
                key.strip_prefix(REPORT_KEY).and_then(|id| id.parse().ok())
            {
                bodies.insert(match_id, body);
            }
        }
        for match_id in state.reports {
            let Some(body) = bodies.remove(&match_id) else {
                // Listed and not there (its write failed, or the id came twice).
                inner.changes.state = true;
                continue;
            };
            match serde_json::from_str::<MatchReport>(&body) {
                Ok(mut report) if report.match_id == match_id => {
                    let stale = drop_stale_upgrades(&mut report);
                    if stale > 0 {
                        log::warn!(
                            "The kept report of match {match_id:#x} named {stale} upgrade \
                             record(s) the game data no longer has or fits (a withdrawn scope \
                             pair); taken off"
                        );
                        inner.changes.saved.push(match_id);
                    }
                    inner.reports.push_back(Arc::new(report));
                }
                Ok(_) => {
                    log::warn!(
                        "The kept report of match {match_id:#x} is another match's; dropped"
                    );
                    inner.changes.state = true;
                    inner.changes.deleted.push(match_id);
                }
                Err(error) => {
                    log::warn!(
                        "The kept report of match {match_id:#x} does not read ({error}); dropped"
                    );
                    inner.changes.state = true;
                    inner.changes.deleted.push(match_id);
                }
            }
        }
        // Reports no state lists: written, and srp ended before the state was.
        inner.changes.deleted.extend(bodies.into_keys());
        inner.unseen = state.unseen.into_iter().collect();
        inner.stray = state.stray.into_iter().collect();
        inner.periods = state
            .periods
            .into_iter()
            .map(|(account, at, games)| (account, (at, games)))
            .collect();
        inner.claimed = state.claimed;
        if !inner.reports.is_empty() || !inner.unseen.is_empty() || !inner.periods.is_empty() {
            log::info!(
                "Restored {} match reports, {} accounts with a results screen not shown yet, {} \
                 play periods",
                inner.reports.len(),
                inner.unseen.len(),
                inner.periods.len()
            );
        }
        Self {
            inner: Mutex::new(inner),
        }
    }

    /// Write what changed since the last call to `repository`: the reports
    /// no longer kept are deleted, the new ones written, then the state
    /// that lists them. Each row is its own write: srp ending between two
    /// leaves rows [`Results::restored`] puts right. A write that fails is
    /// tried again at the next call.
    pub fn save_changes(&self, repository: &mut dyn GameRepository) {
        let (state, saved, deleted) = {
            let mut inner = self.inner.locked();
            if inner.changes.is_empty() {
                return;
            }
            let changes = std::mem::take(&mut inner.changes);
            let state = changes.state.then(|| serde_json::to_string(&inner.state()));
            let saved: Vec<(u32, Result<String, serde_json::Error>)> = changes
                .saved
                .iter()
                .filter_map(|&match_id| {
                    let report = inner.find(match_id)?;
                    Some((match_id, serde_json::to_string(&*report)))
                })
                .collect();
            (state, saved, changes.deleted)
        };
        let mut again = Changes::default();
        for match_id in deleted {
            if let Err(error) = repository.delete_result(&report_key(match_id)) {
                log::error!("Match {match_id:#x}'s report not deleted: {error}; retrying");
                again.deleted.push(match_id);
            }
        }
        for (match_id, body) in saved {
            match body {
                Ok(body) => {
                    if let Err(error) = repository.save_result(&report_key(match_id), &body) {
                        log::error!("Match {match_id:#x}'s report not saved: {error}; retrying");
                        again.saved.push(match_id);
                    }
                }
                Err(error) => log::error!(
                    "Match {match_id:#x}'s report cannot be written ({error}): a restart forgets it"
                ),
            }
        }
        match state {
            Some(Ok(body)) => {
                if let Err(error) = repository.save_result(STATE_KEY, &body) {
                    log::error!("The results' state not saved: {error}; retrying");
                    again.state = true;
                }
            }
            Some(Err(error)) => log::error!("The results' state cannot be written: {error}"),
            None => {}
        }
        if !again.is_empty() {
            let mut inner = self.inner.locked();
            inner.changes.state |= again.state;
            inner.changes.saved.extend(again.saved);
            inner.changes.deleted.extend(again.deleted);
        }
    }

    /// A match is over: give each player what it earned and keep the
    /// report. Every account changed is marked for saving
    /// ([`GameAccounts::mark_dirty`]); call [`GameAccounts::flush`] after,
    /// with no account locked. `None` for a match nobody reached. Call it
    /// once for a match: a second call pays again.
    ///
    /// It locks one account at a time; call it with none locked.
    pub fn settle(
        &self,
        result: &MatchResult,
        accounts: &GameAccounts,
        terms: &Terms,
        rng: &mut Rng,
    ) -> Option<Arc<MatchReport>> {
        // The match's equipment level is of no use past its end.
        let gear = self.inner.locked().gear.remove(&result.match_id);
        if matches!(result.end, MatchEnd::NoShow | MatchEnd::NotMade) {
            return None;
        }
        // An account's place it left and came back from counts for
        // nothing: its latest one is its result.
        let latest = result.latest_places();
        let result = &latest;
        if result.private {
            log::info!("Match {:#x}: private room, no rewards", result.match_id);
        }
        let mut shared = Shared::of(result, terms);
        // Test bots (`super::bots`; none unless `SRP_BOT_ACCOUNTS` names
        // them) and the match server's own (in every match against bots):
        // a match with one in it moves no rating, and
        // a bot is paid no daily bonus (nor is the player of a match
        // against bots, `settle_player`). Nor does a room with a PIN's
        // match move any.
        let bots: Vec<bool> = result
            .players
            .iter()
            .map(|player| player.driven || accounts.is_bot(player.account_id))
            .collect();
        shared.unrated = bots.contains(&true) || result.private;
        shared.money_floor = shared.money_floor(&bots);

        // What the screen's rows and the rating need of every account.
        let held: Vec<Option<(u16, u8, String)>> = result
            .players
            .iter()
            .map(|player| {
                accounts.get(player.account_id).map(|account| {
                    let account = account.locked();
                    (account.rating, account.level(), account.clan_tag.clone())
                })
            })
            .collect();
        let changes = shared.rating_changes(&held);
        let balance_changes = shared.balance_changes(accounts, |account| account.balance);
        // The match's equipment level, and how its rating there moves: a
        // match that moves the balance rating is one of its K/D and rating
        // at that level (`level_stats`), and no other is.
        let level_changes = gear.map(|gear| {
            let changes =
                shared.balance_changes(accounts, |account| account.level_stats.at(gear).rating);
            (gear, changes)
        });

        let mut players = Vec::with_capacity(result.players.len());
        for (index, player) in result.players.iter().enumerate() {
            let standing = shared.standing(player);
            // Only a player who stayed has a game of its play period; a
            // private match is nobody's.
            let game = if standing == Standing::Finished && held[index].is_some() && !result.private
            {
                self.inner
                    .locked()
                    .game_of_period(player.account_id, terms.now)
            } else {
                0
            };
            let report = match accounts.get(player.account_id) {
                Some(account) => {
                    let mut account = account.locked();
                    if let Some(change) = balance_changes[index] {
                        account.balance = account.balance.moved(change);
                    }
                    if let Some((gear, changes)) = &level_changes
                        && let Some(change) = changes[index]
                    {
                        let played = KillsDeaths {
                            kills: u32::from(player.kills),
                            deaths: u32::from(player.deaths),
                        };
                        account.level_stats.played(*gear, played, change);
                    }
                    let (reward, full, notices) = shared.settle_player(
                        &mut account,
                        player,
                        changes[index],
                        game,
                        bots[index],
                        rng,
                    );
                    let body = wire::account_body(&account, terms.now);
                    drop(account);
                    accounts.mark_dirty(player.account_id);
                    PlayerReport {
                        account_id: player.account_id,
                        player_id: player_id(player),
                        reward,
                        full,
                        body,
                        notices,
                    }
                }
                // No 0.26g account: a row of the screen and nothing else.
                None => PlayerReport {
                    account_id: player.account_id,
                    player_id: player_id(player),
                    reward: Reward::default(),
                    full: shared.empty_full(player_id(player)),
                    body: AccountBody {
                        team: wire::LOBBY_TEAM,
                        name: player.name.clone(),
                        clan_tag: String::new(),
                        protection: lobby::NO_PROTECTION,
                        kits: Vec::new(),
                        items: Vec::new(),
                    },
                    notices: Vec::new(),
                },
            };
            log::info!(
                "Match {:#x}: {} ({:?}) gets {} experience, {} money, {} reputation with faction \
                 {}, quests {} money, drop {:?}, rating {} -> {} (score {})",
                result.match_id,
                player.name,
                report.reward.standing,
                report.reward.experience,
                report.reward.money,
                report.reward.reputation,
                report.reward.reputation_faction,
                report.reward.quest_money,
                report.reward.drop,
                report.reward.rating_before,
                report.reward.rating_after,
                report.reward.score
            );
            players.push(report);
        }

        let stats = shared.match_stats(&held);
        let report = Arc::new(MatchReport {
            match_id: result.match_id,
            end: result.end,
            match_level: result.match_level,
            private: result.private,
            vs_bots: result.vs_bots,
            stats,
            players,
        });
        let mut inner = self.inner.locked();
        // The server's own bots have no account (0) to show it to.
        for player in report
            .players
            .iter()
            .filter(|player| player.account_id != 0)
        {
            inner
                .unseen
                .entry(player.account_id)
                .or_default()
                .push(report.match_id);
        }
        inner.make_room(super::quests::now());
        inner.reports.push_back(report.clone());
        inner.changes.state = true;
        inner.changes.saved.push(report.match_id);
        Some(report)
    }

    /// What the account's client has not been shown, once: call it when the
    /// client is in the lobby (after the match, or after its sign-in's
    /// pushes) and send [`MatchReport::lobby_push`] or
    /// [`MatchReport::screen_for`], then the notices.
    pub fn take_unseen(&self, account_id: u32) -> Option<Unseen> {
        let mut inner = self.inner.locked();
        let ids = inner.unseen.remove(&account_id).unwrap_or_default();
        let mut notices = inner.stray.remove(&account_id).unwrap_or_default();
        if !ids.is_empty() || !notices.is_empty() {
            inner.changes.state = true;
        }
        let mut last = None;
        for match_id in ids {
            if let Some(report) = inner.find(match_id) {
                notices.extend(report.notices_for(account_id).iter().cloned());
                last = Some(report);
            }
        }
        // The lobby sends its screen now: kept while it may be open.
        if let Some(report) = &last {
            inner
                .shown
                .insert(account_id, (report.match_id, super::quests::now()));
        }
        last.map(|report| Unseen { report, notices })
    }

    /// The account's client was sent the screen of `match_id` again (the
    /// answer to `41 00 <match id>`): its report is kept while the screen
    /// may be open, as after [`Results::take_unseen`].
    pub fn shown(&self, account_id: u32, match_id: u32) {
        self.inner
            .locked()
            .shown
            .insert(account_id, (match_id, super::quests::now()));
    }

    /// Whether the account has a match it was not shown.
    pub fn has_unseen(&self, account_id: u32) -> bool {
        self.inner.locked().unseen.contains_key(&account_id)
    }

    /// A kept report, for `41 00 <match id>` and `3D 00 <player> <match>`.
    pub fn find(&self, match_id: u32) -> Option<Arc<MatchReport>> {
        self.inner.locked().find(match_id)
    }

    /// The account bought premium from a match's results screen
    /// (`47 02 <offer> <match id>`): give it the premium half the screen
    /// promised, once. Returns `(money, experience, reputation)` and the
    /// notices of the reputation levels reached; `None` if the match is not
    /// kept, was paid with premium already, or the account has no premium
    /// at `now`. Mark the account dirty after.
    pub fn claim_premium(
        &self,
        match_id: u32,
        account_id: u32,
        account: &mut Account,
        now: u64,
    ) -> Option<((u32, u32, u32), Vec<Notice>)> {
        let mut inner = self.inner.locked();
        let report = inner.find(match_id)?;
        let reward = &report.player(account_id)?.reward;
        if reward.premium
            || !account.is_premium(now)
            || inner.claimed.contains(&(match_id, account_id))
        {
            return None;
        }
        inner.claimed.push((match_id, account_id));
        inner.changes.state = true;
        drop(inner);
        let (money, experience, reputation) = reward.premium_bonus;
        account.generic_money = account.generic_money.saturating_add(money);
        let mut notices = Vec::new();
        let (levels, _, _) = pay_levels(account, experience);
        notices.extend(super::notices::levels_reached(&levels));
        if reward.reputation_faction != 0 {
            let faction = reward.reputation_faction;
            for level in account.add_reputation(faction, reputation) {
                notices.push((REPUTATION_LEVEL_MESSAGE, format!("#ru[{faction}:{level}]")));
            }
        }
        Some(((money, experience, reputation), notices))
    }

    /// `46 03`: the account uses a drop card on the results screen of
    /// `match_id` ([`use_drop_card`], at the match's drop level,
    /// [`super::pvb::drop_level`]). One card for
    /// each player that finished the match, once (the owner's rule,
    /// 2026-10-20; the original's screen takes one card too:
    /// `docs/notes/drop_cards.md`). Refused, and nothing changed, for a
    /// match not kept, one the account was not in or left before its end,
    /// a room with a PIN's match (it gives nothing), one it used a card
    /// for already, an item that is no drop card of the account, or a roll
    /// that gave nothing. Lock the account before calling it, as for
    /// [`Results::claim_premium`]; mark it dirty after.
    /// Returns the level the card rolled at and what the card did.
    ///
    /// The mark is the account's own ([`Account::drop_cards`]): it is
    /// checked and made under the account's lock, which the caller holds
    /// from the check to the grant, and it is written in the same
    /// transaction as the card taken and the item given. So a card is
    /// never taken twice for a match, across a crash too. The results'
    /// lock is held only to read the report and which marks are of reports
    /// still kept, never over the roll.
    pub fn use_drop_card(
        &self,
        match_id: u32,
        account_id: u32,
        account: &mut Account,
        card_item_id: u32,
        rng: &mut Rng,
    ) -> Result<(u16, Result<DropCardUse, WireError>), DropCardRefusal> {
        self.use_drop_card_from(
            &Drops::shared().drop_card,
            match_id,
            account_id,
            account,
            card_item_id,
            rng,
        )
    }

    /// [`Results::use_drop_card`] with the table rolled.
    fn use_drop_card_from(
        &self,
        table: &DropTable,
        match_id: u32,
        account_id: u32,
        account: &mut Account,
        card_item_id: u32,
        rng: &mut Rng,
    ) -> Result<(u16, Result<DropCardUse, WireError>), DropCardRefusal> {
        let (report, kept) = {
            let inner = self.inner.locked();
            let report = inner.find(match_id).ok_or(DropCardRefusal::NoMatch)?;
            // The marks of reports no longer kept go with the next card: a
            // card for a match not kept is refused anyway.
            let kept: Vec<u32> = account
                .drop_cards
                .iter()
                .copied()
                .filter(|&id| inner.find(id).is_some())
                .collect();
            (report, kept)
        };
        let standing = report
            .player(account_id)
            .filter(|_| account_id != 0)
            .ok_or(DropCardRefusal::NotAPlayer)?
            .reward
            .standing;
        if standing != Standing::Finished {
            return Err(DropCardRefusal::NotFinished(standing));
        }
        if report.private {
            return Err(DropCardRefusal::Private);
        }
        if account.drop_cards.contains(&match_id) {
            return Err(DropCardRefusal::Used);
        }
        let level = super::pvb::drop_level(report.match_level, report.vs_bots);
        let used = use_card(account, card_item_id, level, table, rng)?;
        // Marked even when its `98` cannot be written: the card is spent
        // and the item given.
        account.drop_cards = kept;
        if let Err(at) = account.drop_cards.binary_search(&match_id) {
            account.drop_cards.insert(at, match_id);
        }
        Ok((level, used))
    }
}

/// Why [`Results::use_drop_card`] took no card.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropCardRefusal {
    /// No report of the match is kept.
    NoMatch,
    /// The account was not in the match.
    NotAPlayer,
    /// The account was not in the match at its end: it left early, or its
    /// game never got into it.
    NotFinished(Standing),
    /// The account used a card for the match already.
    Used,
    /// A room with a PIN's match: it gives nothing
    /// ([`MatchReport::private`]).
    Private,
    /// The item is no drop card of the account.
    NoCard,
    /// The roll gave nothing (`drop_card_drop` always gives: a table the
    /// game data broke). The card is kept.
    NoRoll,
}

impl std::fmt::Display for DropCardRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoMatch => f.write_str("no such match kept"),
            Self::NotAPlayer => f.write_str("not a player of the match"),
            Self::NotFinished(standing) => {
                write!(f, "did not finish the match ({standing:?})")
            }
            Self::Used => f.write_str("a card was used for the match already"),
            Self::Private => f.write_str("private match"),
            Self::NoCard => f.write_str("no drop card of the account"),
            Self::NoRoll => f.write_str("the drop card table rolled nothing; card kept"),
        }
    }
}

/// The id a screen's row names a player by.
fn player_id(player: &PlayerResult) -> u32 {
    u32::from(player.place) + 1
}

/// Adds experience and pays the levels it reaches: `(levels, silver,
/// gold)`, by [`pay::level_reward`].
pub(super) fn pay_levels(account: &mut Account, experience: u32) -> (Vec<u8>, u32, u32) {
    let levels = account.add_experience(experience);
    let (mut silver, mut gold) = (0_u32, 0_u32);
    for &level in &levels {
        match pay::level_reward(level) {
            pay::LevelReward::Silver(amount) => silver = silver.saturating_add(amount),
            pay::LevelReward::Gold(amount) => gold = gold.saturating_add(amount),
        }
    }
    account.generic_money = account.generic_money.saturating_add(silver);
    account.premium_money = account.premium_money.saturating_add(gold);
    (levels, silver, gold)
}

/// A player's counters by event and kit; none without statistics.
fn counters(player: &PlayerResult) -> [[u16; KITS]; EVENTS] {
    player
        .statistics
        .as_ref()
        .map_or([[0; KITS]; EVENTS], |statistics| statistics.events)
}

fn total(events: &[[u16; KITS]; EVENTS], event: usize) -> u32 {
    events[event].iter().map(|&count| u32::from(count)).sum()
}

fn clamp_u16(value: u32) -> u16 {
    value.min(u32::from(u16::MAX)) as u16
}

/// What every player of a match is settled with.
struct Shared<'a> {
    result: &'a MatchResult,
    terms: &'a Terms,
    /// Whether the numbers of the result can be used: play began and a
    /// snapshot was trusted.
    trusted: bool,
    /// 0 or 1, [`DRAW`], or `None`.
    winner: Option<u8>,
    /// Whether the match is of a mode with batteries (2 and 3): the
    /// battery quests, the drop chance a battery and a robbed base count in
    /// it only.
    batteries: bool,
    /// Which teams took a battery from the other's base.
    stole: [bool; 2],
    /// The weights its score was counted with: the terms', with Research's
    /// own in mode 5 ([`static_info::weights_of`]).
    weights: [f32; EVENTS],
    /// Reward-table level, capped at seven for PvE. The match's actual
    /// level and equipment remain unchanged.
    reward_level: u16,
    /// The reward level's multipliers: money, experience, reputation.
    multipliers: (f64, f64, f64),
    /// Drop-table level, capped at seven for PvE. The match's actual
    /// level and equipment remain unchanged.
    drop_level: u16,
    /// A test bot was in the match, or it is a room with a PIN's
    /// ([`MatchResult::private`]): nobody's rating moves.
    unrated: bool,
    /// The silver of the rows a player who takes the floor is paid at
    /// least ([`Shared::money_floor`]); 0 for none.
    money_floor: u32,
}

impl<'a> Shared<'a> {
    fn of(result: &'a MatchResult, terms: &'a Terms) -> Self {
        let trusted = result.played_ms.is_some() && result.trusted_by > 0;
        let batteries = mode::WITH_BATTERIES.contains(&result.game_mode);
        let mut stole = [false; 2];
        if trusted && batteries {
            for player in &result.players {
                if player.team < 2 && total(&counters(player), event::STEAL_VICTORY_ITEM) > 0 {
                    stole[usize::from(player.team)] = true;
                }
            }
        }
        let reward_level = if result.vs_bots {
            result.match_level.min(super::pvb::MAX_REWARD_LEVEL)
        } else {
            result.match_level
        };
        let drop_level = super::pvb::drop_level(result.match_level, result.vs_bots);
        let multipliers = Dictionary::shared()
            .match_level(usize::from(reward_level))
            .map_or((1.0, 1.0, 1.0), |level| {
                (
                    pay::multiplier(level.money_k),
                    pay::multiplier(level.experience_k),
                    pay::multiplier(level.reputation_k),
                )
            });
        Self {
            result,
            terms,
            trusted,
            winner: result.winner.filter(|&winner| trusted && winner <= DRAW),
            batteries,
            stole,
            weights: static_info::weights_of(result.game_mode, terms.score_weights),
            reward_level,
            multipliers,
            drop_level,
            unrated: false,
            money_floor: 0,
        }
    }

    /// The share of the rows' silver the match pays (its level's rate, a
    /// match against the server's bots' share), times the level's
    /// multiplier.
    fn money_k(&self) -> f64 {
        let pvb_pay = if self.result.vs_bots {
            self.terms.pvb_pay.clamp(0.0, 1.0)
        } else {
            1.0
        };
        self.multipliers.0 * f64::from(self.terms.money_rate.at(self.reward_level)) / 100.0
            * pvb_pay
    }

    /// The silver of a player's rows, before premium and every bonus.
    fn rows_money(&self, player: &PlayerResult, main: usize) -> u32 {
        let k = self.money_k();
        self.lines(player, main)
            .iter()
            .flatten()
            .filter(|(amount, _)| *amount > 0.0)
            .map(|(_, base)| pay::cell(base.money, k).min(f64::from(u32::MAX)) as u32)
            .fold(0, u32::saturating_add)
    }

    /// The floor of the match's lowest: [`pay::FLOOR_OF_MEDIAN`] of the
    /// rows' silver of its median player, of those who stayed to the end
    /// (bots not counted; the lower of two middle ones).
    fn money_floor(&self, bots: &[bool]) -> u32 {
        if self.result.private || !self.trusted {
            return 0;
        }
        let mut paid: Vec<u32> = self
            .result
            .players
            .iter()
            .zip(bots)
            .filter(|(player, bot)| !**bot && self.standing(player) == Standing::Finished)
            .map(|(player, _)| self.rows_money(player, 0))
            .collect();
        if paid.is_empty() {
            return 0;
        }
        paid.sort_unstable();
        (f64::from(paid[(paid.len() - 1) / 2]) * pay::FLOOR_OF_MEDIAN) as u32
    }

    fn standing(&self, player: &PlayerResult) -> Standing {
        if self.result.played_ms.is_none() {
            Standing::NotPlayed
        } else if !player.entered {
            Standing::Absent
        } else if player.present_at_end {
            Standing::Finished
        } else {
            Standing::Left
        }
    }

    fn won(&self, player: &PlayerResult) -> bool {
        matches!(self.standing(player), Standing::Finished | Standing::Left)
            && player.team < 2
            && self.winner == Some(player.team)
    }

    /// A player's counters, if the result's numbers can be used.
    fn events(&self, player: &PlayerResult) -> [[u16; KITS]; EVENTS] {
        if self.trusted {
            counters(player)
        } else {
            [[0; KITS]; EVENTS]
        }
    }

    /// The score with what the client adds at the end of a match.
    fn score(&self, player: &PlayerResult) -> u32 {
        let weights = &self.weights;
        let bonus = |given: bool, event: usize| {
            if given {
                f64::from(weights[event]).round().max(0.0) as u32
            } else {
                0
            }
        };
        let base = if self.trusted { player.score } else { 0 };
        let score = base
            .saturating_add(bonus(
                self.standing(player) == Standing::Finished,
                event::PARTICIPATING,
            ))
            .saturating_add(bonus(self.won(player), event::WON));
        (f64::from(score) * self.points_factor()).min(f64::from(u32::MAX)) as u32
    }

    /// What a point is worth: twice in a Blackout match.
    fn points_factor(&self) -> f64 {
        if self.result.blackout {
            BLACKOUT_FACTOR
        } else {
            1.0
        }
    }

    /// How the match moves each rating (0.21d's rule at 0.26g's scale,
    /// [`pay::RATING_STEP`];
    /// `match-server/src/server.rs` `rating_changes`): Elo between the
    /// teams of those who were in the match; one who left loses whatever
    /// its team did; one who never got in is left out; no known winner, or
    /// one team, moves nothing.
    fn rating_changes(&self, held: &[Option<(u16, u8, String)>]) -> Vec<i32> {
        let mut changes = vec![0; self.result.players.len()];
        let Some(winner) = self.winner else {
            return changes;
        };
        let rated: Vec<usize> = (0..self.result.players.len())
            .filter(|&index| {
                held[index].is_some()
                    && matches!(
                        self.standing(&self.result.players[index]),
                        Standing::Finished | Standing::Left
                    )
            })
            .collect();
        let list: Vec<(u8, u16, bool)> = rated
            .iter()
            .map(|&index| {
                let player = &self.result.players[index];
                let rating = held[index]
                    .as_ref()
                    .map_or(pay::START_RATING, |held| held.0);
                (player.team, rating, player.present_at_end)
            })
            .collect();
        for (&index, change) in rated.iter().zip(v021d::rating::changes_scaled(
            &list,
            winner,
            pay::RATING_STEP,
            pay::RATING_SPREAD,
        )) {
            changes[index] = change;
        }
        changes
    }

    /// How the match moves each balance rating ([`balance::changes`]), by
    /// roster place; `None` for a player it does not move. The players
    /// are the ranking's: those who were in the match, one who left with
    /// its team's result and nothing more. A match with a test bot, a room
    /// with a PIN's, one with no known winner, or one left by a whole side
    /// ([`MatchEnd::Deserted`]: its winner is who stayed) moves none. Nor
    /// does a match of few players ([`Terms::rated_players`], by default
    /// one against one and two against one): it is not one more match of anybody's either, so
    /// it adds nothing to how far a rating is trusted.
    ///
    /// Each counts for the share of the match it was in
    /// ([`balance::changes_by_share`]): its time in the match against the
    /// longest anybody's was. One who came late or left early is that part
    /// of a player to its side, and is moved by that part.
    ///
    /// `rating` is the rating moved: the overall one, or the one at the
    /// match's equipment level (`level_stats`).
    fn balance_changes(
        &self,
        accounts: &GameAccounts,
        rating: impl Fn(&Account) -> balance::Balance,
    ) -> Vec<Option<i32>> {
        let mut changes = vec![None; self.result.players.len()];
        let Some(winner) = self.winner else {
            return changes;
        };
        if self.unrated || self.result.end == MatchEnd::Deserted {
            return changes;
        }
        let rated: Vec<(usize, u8, balance::Balance, u32)> = self
            .result
            .players
            .iter()
            .enumerate()
            .filter(|(_, player)| {
                player.team < 2
                    && matches!(self.standing(player), Standing::Finished | Standing::Left)
            })
            .filter_map(|(index, player)| {
                let account = accounts.get(player.account_id)?;
                let held = rating(&account.locked());
                Some((index, player.team, held, player.time_in_match_ms))
            })
            .collect();
        // The whole match is the longest stay: a result that names no
        // times (0 for all) counts everybody in full.
        let longest = rated.iter().map(|rated| rated.3).max().unwrap_or(0);
        let list: Vec<(u8, balance::Balance, f64)> = rated
            .iter()
            .map(|&(_, team, held, time)| {
                let share = if longest == 0 {
                    1.0
                } else {
                    f64::from(time) / f64::from(longest)
                };
                (team, held, share)
            })
            .collect();
        // One side only: no match to learn from, and none counted.
        if !(0..2).all(|team| list.iter().any(|player| player.0 == team)) {
            return changes;
        }
        let players = balance::players_in(&list);
        if players < self.terms.rated_players {
            log::info!(
                "Match {:#x} moves no balance rating: {players} players were in it for half of \
                 it or more, and it takes {}",
                self.result.match_id,
                self.terms.rated_players
            );
            return changes;
        }
        for (&(index, ..), change) in rated.iter().zip(balance::changes_by_share(&list, winner)) {
            changes[index] = Some(change);
        }
        changes
    }

    /// A full result with the score's lines named and nothing in them.
    fn empty_full(&self, player_id: u32) -> FullResult {
        let mut full = FullResult::empty(player_id);
        for (component, (key, shown)) in full.components.iter_mut().zip(SCORE_LINES) {
            component.name_key = key.to_owned();
            component.show_amount = shown;
        }
        full
    }

    /// The score's lines for a player: for each line and kit, how many
    /// paying events and what they pay before the rates and the match
    /// level ([`pay`]).
    fn lines(&self, player: &PlayerResult, main: usize) -> [[(f64, Pay); KITS]; 3] {
        let events = if self.trusted {
            player
                .pay_statistics
                .as_ref()
                .map_or_else(|| counters(player), |s| s.events)
        } else {
            [[0; KITS]; EVENTS]
        };
        let weights = &self.weights;
        let factor = self.points_factor();
        let mut lines = [[(0.0, Pay::NONE); KITS]; 3];
        for (event, counts) in events.iter().enumerate() {
            let line = EVENT_LINES[event];
            // What one of the event pays. A kill and a help pay by the
            // event; an event of the activity line by its score, with the
            // weight the scoreboard counted it with. An event that scores
            // nothing is no part of a line's amount (a team kill is in the
            // kills' line and is no enemy killed); the two end-of-match
            // events are the fixed part, added below.
            let each = match event {
                event::KILL => pay::KILL,
                event::ASSIST | event::TEAMMATE_CURE | event::TEAMMATE_RESCUE => pay::HELP,
                event::PARTICIPATING | event::WON => continue,
                _ if line == ACTIVITY_LINE && weights[event] > 0.0 => {
                    pay::ACTIVITY_PER_SCORE.times(f64::from(weights[event]))
                }
                _ => continue,
            };
            let unit = if HUNDREDTHS.contains(&event) {
                0.01
            } else {
                1.0
            };
            for (kit, &count) in counts.iter().enumerate() {
                let cell = &mut lines[line][kit];
                let units = f64::from(count) * unit;
                cell.0 += units;
                cell.1 = cell.1.plus(each.times(units * factor));
            }
        }
        // The fixed part: for one who stayed to the end, in full when its
        // team won (`pay`'s block has what is known of it).
        if self.standing(player) == Standing::Finished {
            let cell = &mut lines[ACTIVITY_LINE][main];
            let won = self.won(player);
            cell.0 += if won { 2.0 } else { 1.0 };
            let share = if won { 1.0 } else { pay::ACTIVITY_NOT_WON };
            cell.1 = cell.1.plus(pay::ACTIVITY_WON.times(share * factor));
        }
        lines
    }

    /// What the match's quests ask of a player.
    fn quest_stats(&self, player: &PlayerResult) -> QuestStats {
        let events = self.events(player);
        let count = |event: usize| clamp_u16(total(&events, event));
        let batteries = self.batteries;
        let (kills, deaths, streak) = if self.trusted {
            (player.kills, player.deaths, player.longest_killstreak)
        } else {
            (0, 0, 0)
        };
        QuestStats {
            won: self.won(player),
            kills,
            deaths,
            headshots: count(event::HEADSHOT_KILL),
            max_killstreak: streak,
            grenade_kills: count(GRENADE_KILL),
            melee_kills: count(event::MELEE_KILL),
            trapped: count(ENTRAP_ENEMY),
            shots: total(&events, event::BULLET_FIRE),
            hits: total(&events, event::BULLET_HIT),
            artefacts: count(event::ARTEFACT),
            // The game's own count for "Find and bring batteries", not
            // every battery put into the container (`quests::MatchStats`).
            delivered: count(event::SOLO_DELIVERY),
            // The game's own count for "Steal a battery", by ones; not
            // the shared credit for a stolen battery (event 17).
            stolen: count(event::STOLEN_AND_DELIVERED),
            team_batteries: if batteries && self.trusted && player.team < 2 {
                self.result.team_points[usize::from(player.team)]
            } else {
                0
            },
            base_robbed: batteries && player.team < 2 && self.stole[usize::from(1 - player.team)],
            battery_mode: batteries,
        }
    }

    /// Wears the items of the kits the player used; returns `(item id,
    /// condition before, after)` of those that lost any.
    fn wear(
        &self,
        account: &mut Account,
        player: &PlayerResult,
        main: usize,
    ) -> Vec<(u32, u32, u32)> {
        let events = self.events(player);
        // (item, fraction): each item once, whatever kits wear it.
        let mut wear: Vec<(u32, f32)> = Vec::new();
        for (index, kit) in account.kits.iter().enumerate().take(KITS) {
            let used = index == main || events.iter().any(|counts| counts[index] > 0);
            if !used {
                continue;
            }
            let worn: Vec<(u32, &'static survarium::v026g::item_params::ItemParams)> = kit
                .slots
                .iter()
                .flatten()
                .filter_map(|slot| {
                    let item = account.item(slot.item_id)?;
                    Some((item.item_id, item_params(item.dict_id)?))
                })
                .collect();
            let firearms = worn
                .iter()
                .filter(|(_, params)| params.kind.starts_with("wpn_"))
                .count();
            let shots = f32::from(events[event::BULLET_FIRE][index]);
            let deaths = f32::from(events[event::DEATH][index]);
            for (item_id, params) in worn {
                let mut fraction = deaths * params.degrade_per_death;
                if params.kind.starts_with("wpn_") {
                    fraction += shots / firearms as f32 * params.degrade_per_use;
                }
                match wear.iter_mut().find(|(id, _)| *id == item_id) {
                    Some(entry) => entry.1 += fraction,
                    None => wear.push((item_id, params.degrade_per_match + fraction)),
                }
            }
        }
        let mut changed = Vec::new();
        for (item_id, fraction) in wear {
            let Some(before) = account.item(item_id).map(|item| item.condition_or_stack) else {
                continue;
            };
            // Less with a "reliability" modification on the item.
            let fraction = fraction * items_rules::wear_factor(account, item_id);
            let after = rules::wear(account, item_id, fraction).unwrap_or(before);
            if after != before {
                changed.push((item_id, before, after));
            }
        }
        changed
    }

    /// Give one player what the match earned. `rating_change` is its part
    /// of [`Self::rating_changes`]; `game` which game of its play period
    /// this is (0: no drop is rolled); `bot` whether the account is a test
    /// bot (no daily bonus). A room with a PIN's match
    /// ([`MatchResult::private`]) gives and takes nothing: only the
    /// screen's numbers are filled.
    fn settle_player(
        &self,
        account: &mut Account,
        player: &PlayerResult,
        rating_change: i32,
        game: u32,
        bot: bool,
        rng: &mut Rng,
    ) -> (Reward, FullResult, Vec<Notice>) {
        let now = self.terms.now;
        let standing = self.standing(player);
        let mut full = self.empty_full(player_id(player));
        let mut reward = Reward {
            standing,
            won: self.won(player),
            score: self.score(player),
            premium: account.is_premium(now),
            reputation_faction: account.quest_faction,
            rating_before: account.rating,
            rating_after: account.rating,
            ..Reward::default()
        };
        full.faction = account.quest_faction;
        full.elo_before = account.rating;
        full.elo_after = account.rating;
        let mut notices = Vec::new();
        if !matches!(standing, Standing::Finished | Standing::Left) {
            return (reward, full, notices);
        }

        // The kit the bonuses and the play time are shown with: the one
        // the account plays (the result does not say how long each was
        // used).
        let main = account
            .kit_index(account.active_kit)
            .unwrap_or(0)
            .min(KITS - 1);
        full.play_seconds[main] = clamp_u16(player.time_in_match_ms / 1000);
        // A room with a PIN's match pays nothing and takes nothing: no
        // experience, silver, reputation, quest, daily bonus, drop or
        // rating, and no wear, supplies used or autobuy after it (the
        // owner's of 2026-10-20). Its screen shows the match with nothing
        // paid.
        if self.result.private {
            return (reward, full, notices);
        }

        // The rating.
        if self.winner.is_some() && !self.unrated {
            account.rating_before = account.rating;
            account.rating = v021d::rating::moved(account.rating, rating_change);
            reward.rating_after = account.rating;
            full.elo_after = account.rating;
        }

        if self.terms.wear {
            reward.worn = self.wear(account, player, main);
        }
        // A worn item may get a defect (`items_rules`): the inventory sent
        // with the results shows it.
        reward.defects = items_rules::after_wear(account, &reward.worn, rng);
        // What the match used of the account's stacks (`autobuy`), counted
        // before its pay or drop adds to one.
        if super::premium::settings().ammo_use {
            let events = self.events(player);
            let shots: Vec<u32> = (0..KITS)
                .map(|kit| u32::from(events[event::BULLET_FIRE][kit]))
                .collect();
            let respawns =
                total(&events, event::DEATH).saturating_add(u32::from(player.kit_changes));
            reward.supplies_used =
                super::autobuy::used(account, player.supplies.as_ref(), &shots, respawns);
        }
        // A new player is one with no experience from a match yet (what a
        // new account starts with at most, `tutorial::experience`): its
        // first drop is from the new player's table.
        let new_player = account.exp_total <= super::tutorial::experience();

        // The score's lines, and the premium half of each kit's column. A
        // match against the server's bots pays a share of the silver and
        // the experience (`SRP_PVB_PAY`): of every cell, so of the premium
        // half and the items' bonus too, which are counted from them.
        let vs_bots = self.result.vs_bots;
        let pvb_pay = if vs_bots {
            self.terms.pvb_pay.clamp(0.0, 1.0)
        } else {
            1.0
        };
        let rate = f64::from(self.terms.experience_rate) * pvb_pay;
        let reputation_share = f64::from(self.terms.reputation_rate) / 100.0;
        let cell_money_k = self.money_k();
        let (money_k, experience_k, reputation_k) = self.multipliers;
        let faction = account.quest_faction;
        let lines = self.lines(player, main);
        // The floor of the match's lowest (`pay::FLOOR_OF_MEDIAN`): what
        // its rows lack of it goes into the main kit's activity cell, so
        // premium's half and the items' bonus count it too.
        let top_up =
            if standing == Standing::Finished && !bot && reward.score >= pay::FLOOR_MIN_SCORE {
                self.money_floor
                    .saturating_sub(self.rows_money(player, main))
            } else {
                0
            };
        let (mut money, mut experience, mut reputation) = (0_u32, 0_u32, 0_u32);
        let mut bonus = (0_u32, 0_u32, 0_u32);
        // The daily bonus: the account is seen today, and a victory it
        // stayed for may be one of the day's first three. A match against
        // the server's bots counts for none of it.
        if standing == Standing::Finished && !bot && !vs_bots {
            super::bonus::visit(account, now, rng);
            if reward.won {
                let win = super::bonus::count_win(account);
                reward.daily_bonus = win.doubled;
                reward.daily_reward = win.given;
            }
        }
        let mut daily_pay = (0_u32, 0_u32);
        for kit in 0..KITS {
            let mut column = (0_u32, 0_u32, 0_u32);
            for (line, component) in full.components.iter_mut().enumerate() {
                let (amount, base) = lines[line][kit];
                if amount <= 0.0 {
                    continue;
                }
                let mut cell_money =
                    pay::cell(base.money, cell_money_k).min(f64::from(u32::MAX)) as u32;
                if kit == main && line == ACTIVITY_LINE {
                    cell_money = cell_money.saturating_add(top_up);
                }
                // What the wire's s16 holds.
                let cell = |plain: f64, k: f64| pay::cell(plain, k).min(f64::from(i16::MAX)) as i16;
                let cell_experience = cell(base.experience, experience_k * rate);
                let cell_reputation = if faction == 0 {
                    0
                } else {
                    cell(base.reputation, reputation_k * reputation_share)
                };
                component.columns[kit] =
                    (amount as f32, cell_money, cell_experience, cell_reputation);
                column.0 = column.0.saturating_add(cell_money);
                column.1 += cell_experience as u32;
                column.2 += cell_reputation as u32;
            }
            let half = |value: u32| v021d::premium::with_bonus(value) - value;
            let premium = BonusColumn {
                money: half(column.0),
                experience: clamp_u16(half(column.1)),
                reputation: clamp_u16(half(column.2)),
                ..BonusColumn::default()
            };
            full.premium.columns[kit] = premium;
            // The day's double pay (`bonus`): the column's experience and
            // reputation once more, the screen's "daily" block with its
            // "(+100%)", paid with or without premium.
            if reward.daily_bonus {
                let daily = BonusColumn {
                    experience: clamp_u16(column.1),
                    reputation: clamp_u16(column.2),
                    bonus_experience: if column.1 > 0 { 100 } else { 0 },
                    bonus_reputation: if column.2 > 0 { 100 } else { 0 },
                    ..BonusColumn::default()
                };
                full.daily.columns[kit] = daily;
                daily_pay.0 += u32::from(daily.experience);
                daily_pay.1 += u32::from(daily.reputation);
            }
            // What the kit's items add (the premium copies of weapons and
            // armour, `items_rules::kit_bonuses`): the screen's "premium
            // items" column, paid whatever the account is. [GUESS: the
            // three bytes are the percents.]
            let gear = items_rules::kit_bonuses(account, kit);
            let gear_reputation = match faction {
                1..=4 => gear.reputation[usize::from(faction) - 1],
                _ => 0.0,
            };
            let share = |value: u32, bonus: f32| (f64::from(value) * f64::from(bonus)) as u32;
            let percent = |bonus: f32| (bonus * 100.0).round().clamp(0.0, 255.0) as u8;
            let items = BonusColumn {
                money: share(column.0, gear.money),
                experience: clamp_u16(share(column.1, gear.experience)),
                reputation: clamp_u16(share(column.2, gear_reputation)),
                bonus_money: percent(gear.money),
                bonus_experience: percent(gear.experience),
                bonus_reputation: percent(gear_reputation),
            };
            if items.money != 0 || items.experience != 0 || items.reputation != 0 {
                full.premium_items.columns[kit] = items;
            }
            money = money.saturating_add(column.0).saturating_add(items.money);
            experience += column.1 + u32::from(items.experience);
            reputation += column.2 + u32::from(items.reputation);
            bonus.0 = bonus.0.saturating_add(premium.money);
            bonus.1 += u32::from(premium.experience);
            bonus.2 += u32::from(premium.reputation);
        }
        experience += daily_pay.0;
        reputation += daily_pay.1;
        reward.premium_bonus = bonus;
        if reward.premium {
            money = money.saturating_add(bonus.0);
            experience += bonus.1;
            reputation += bonus.2;
        }
        reward.money = money;
        reward.experience = experience;
        reward.reputation = reputation;
        account.generic_money = account.generic_money.saturating_add(money);
        (reward.levels, reward.level_money, reward.level_gold) = pay_levels(account, experience);
        notices.extend(super::notices::levels_reached(&reward.levels));
        if faction != 0 {
            reward.reputation_levels = account.add_reputation(faction, reputation);
            for &level in &reward.reputation_levels {
                notices.push((REPUTATION_LEVEL_MESSAGE, format!("#ru[{faction}:{level}]")));
            }
        }

        // The quests; a match against the server's bots counts for none.
        if !vs_bots {
            let scale = quests::Scale {
                money_k,
                reputation_k,
                reputation_share,
                doubled: reward.daily_bonus,
            };
            let pay = quests::evaluate_scaled(account, &self.quest_stats(player), &scale, now, rng);
            reward.quest_money = pay.money;
            full.quest_money = pay.money;
            full.quest_reputation = clamp_u16(
                pay.reputation
                    .iter()
                    .find(|&&(id, _)| id == faction)
                    .map_or(0, |&(_, points)| points),
            );
            reward.quest_reputation = pay.reputation;
            notices.extend(pay.notices);
        }

        // The drop.
        if standing == Standing::Finished && game > 0 {
            let tables = Drops::shared();
            let (table, percent) = if new_player {
                (&tables.new_player, 0)
            } else if self.terms.playtime_bonus {
                (&tables.generic, tables.playtime.bonus_percent(game))
            } else {
                (&tables.generic, 0)
            };
            // The chance's parts, as the screen shows them: the table's
            // (with the play time bonus when that is on); the match's result (the batteries
            // of the player's own team at the end); the premium account.
            // Together at most 1, the later parts cut.
            let base = (table.item_chance() * (1.0 + percent as f32 / 100.0)).min(1.0);
            let batteries = if self.trusted && self.batteries && player.team < 2 {
                self.result.team_points[usize::from(player.team)]
            } else {
                0
            };
            let from_result = (f32::from(batteries) * pay::DROP_CHANCE_PER_BATTERY).min(1.0 - base);
            let from_premium = if reward.premium {
                pay::DROP_CHANCE_PREMIUM
                    .min(1.0 - base - from_result)
                    .max(0.0)
            } else {
                0.0
            };
            let chance = base + from_result + from_premium;
            reward.drop_chance = chance;
            // The fourth part is a penalty ("for idleness"); srp has none.
            // The third's label is the client's choice: it shows "Premium
            // account" only for a raw value over 12.5 and "Premium players
            // in match" under it, and prints either × 100 (`results.md`
            // §9), so a fraction always has the second label.
            full.drop_chances = [base, from_result, from_premium, 0.0];
            let level = self.drop_level.min(u16::from(u8::MAX)) as u8;
            let found = (rng.below(1 << 24) as f32 / (1 << 24) as f32) < chance;
            let rolled = if found {
                drops::roll(
                    table,
                    Dictionary::shared(),
                    &account.owned_dict_ids(),
                    level,
                    CERTAIN_PERCENT,
                    rng,
                )
            } else {
                None
            };
            if let Some(rolled) = rolled {
                let (dropped, shown) = give_drop(account, &rolled, true, rng);
                full.dropped = Some(shown);
                full.drop_rarity = dropped.rarity;
                reward.drop = Some(dropped);
            }
        }
        // The supplies used are taken, and the account's autobuy and
        // auto-repair boxes buy them back and repair what the match wore,
        // paid from what it has now (the match's pay included).
        let worn: Vec<u32> = reward.worn.iter().map(|&(item_id, _, _)| item_id).collect();
        reward.spent =
            super::autobuy::settle(account, &reward.supplies_used, &worn, self.terms.now).spent;
        account.sync_kits();
        (reward, full, notices)
    }

    /// The part of the screen every player gets: the winner, the teams'
    /// points, the four titles and a row a player.
    fn match_stats(&self, held: &[Option<(u16, u8, String)>]) -> lobby::MatchStats {
        let result = self.result;
        struct Line<'a> {
            player: &'a PlayerResult,
            tag: &'a str,
            score: u32,
            shots: u32,
            accuracy: u32,
            support: u32,
            artefacts: u32,
            items: u32,
        }
        let lines: Vec<Line> = result
            .players
            .iter()
            .zip(held)
            .map(|(player, held)| {
                let events = self.events(player);
                let shots = total(&events, event::BULLET_FIRE);
                let hits = total(&events, event::BULLET_HIT);
                Line {
                    player,
                    tag: held.as_ref().map_or("", |held| held.2.as_str()),
                    score: self.score(player),
                    shots,
                    accuracy: (hits * 100).checked_div(shots).unwrap_or(0).min(100),
                    support: total(&events, event::TEAMMATE_CURE)
                        + total(&events, event::TEAMMATE_RESCUE),
                    artefacts: total(&events, event::ARTEFACT),
                    items: total(&events, event::VICTORY_ITEM_FOUND),
                }
            })
            .collect();
        // The best of a title, if anyone has any of it (0.21d's titles).
        let best = |key: &dyn Fn(&Line) -> u32| {
            lines
                .iter()
                .filter(|line| line.player.entered && key(line) > 0)
                .max_by_key(|line| key(line))
        };
        let holder = |line: Option<&Line>| {
            line.map_or_else(TitleHolder::default, |line| TitleHolder {
                name: line.player.name.clone(),
                clan_tag: line.tag.to_owned(),
            })
        };
        let player = best(&|line| line.score);
        let shooter = best(&|line| line.accuracy * 0x1_0000 + line.shots.min(0xFFFF));
        let support = best(&|line| line.support);
        let explorer = best(&|line| line.artefacts + line.items);
        let team_size = |team: u8| {
            result
                .players
                .iter()
                .filter(|player| player.team == team)
                .count() as u32
        };
        lobby::MatchStats {
            match_id: result.match_id,
            unknown: 0,
            winner_team: self.winner.unwrap_or(NO_WINNER),
            team_scores: if self.trusted {
                result
                    .team_points
                    .map(|points| points.min(u16::from(u8::MAX)) as u8)
            } else {
                [0, 0]
            },
            // Read by the screen only in Team Deathmatch with both scores
            // 0, as the top of a bar whose value is the team's rows [GUESS:
            // the players a team had].
            team_params: [team_size(0), team_size(1)],
            game_mode: result.game_mode,
            best_player: holder(player),
            best_player_score: clamp_u16(player.map_or(0, |line| line.score)),
            best_shooter: holder(shooter),
            shooter_accuracy_percent: shooter.map_or(0, |line| line.accuracy) as u8,
            shooter_shots: clamp_u16(shooter.map_or(0, |line| line.shots)),
            best_support: holder(support),
            support_value: clamp_u16(support.map_or(0, |line| line.support)),
            best_explorer: holder(explorer),
            explorer_artefacts: clamp_u16(explorer.map_or(0, |line| line.artefacts)),
            explorer_items: clamp_u16(explorer.map_or(0, |line| line.items)),
            rows: lines
                .iter()
                .zip(held)
                // A team past 1 would corrupt the client's stack.
                .filter(|(line, _)| line.player.team < 2)
                .take(limits::RESULT_ROWS)
                .map(|(line, held)| ResultRow {
                    name: line.player.name.clone(),
                    clan_tag: line.tag.to_owned(),
                    player_id: player_id(line.player),
                    score: clamp_u16(line.score),
                    kills: if self.trusted { line.player.kills } else { 0 },
                    deaths: if self.trusted { line.player.deaths } else { 0 },
                    team: line.player.team,
                    level: held.as_ref().map_or(0, |held| u16::from(held.1)),
                })
                .collect(),
        }
    }
}

/// Gives the account a rolled drop: silver and gold to the balances, a
/// stack onto the one owned, anything else as a new item with the
/// modifications of its rarity (`items_rules::roll_modifications`).
/// `found_in_match`: a weapon or an armour piece arrives worn
/// ([`pay::drop_condition`]) and with one defect it may have
/// ([`pay::drop_defect`]); an item of a drop card arrives whole, and so
/// does anything that has no condition to lose (a cosmetic).
/// Returns what was given and the item as a message describes it (a
/// stack's value is the units dropped, not the stack's size).
fn give_drop(
    account: &mut Account,
    rolled: &drops::Drop,
    found_in_match: bool,
    rng: &mut Rng,
) -> (Dropped, ItemDescr) {
    let dictionary = Dictionary::shared();
    let mut shown = None;
    let (item_id, value, rarity) = match rolled.dict_id {
        GENERIC_MONEY => {
            account.generic_money = account.generic_money.saturating_add(rolled.count);
            (0, rolled.count, rolled.rarity)
        }
        PREMIUM_MONEY => {
            account.premium_money = account.premium_money.saturating_add(rolled.count);
            (0, rolled.count, rolled.rarity)
        }
        dict_id if dictionary.is_stack(dict_id) => (
            account.add_to_stack(dict_id, rolled.count),
            rolled.count,
            rolled.rarity,
        ),
        dict_id => {
            let worn = found_in_match && is_gear(dict_id);
            let condition = if worn {
                FULL_CONDITION / 100 * pay::drop_condition().min(100)
            } else {
                FULL_CONDITION
            };
            let item_id = account.add_item(dict_id, condition);
            let rank = items_rules::roll_modifications(account, item_id, rolled.rarity, rng);
            if worn && pay::drop_defect() {
                add_defect(account, item_id, rng);
            }
            shown = account.item(item_id).map(wire::item_descr);
            (item_id, condition, rank)
        }
    };
    (
        Dropped {
            dict_id: rolled.dict_id,
            count: rolled.count,
            item_id,
            rarity,
            rolled_rarity: rolled.rarity,
        },
        shown.unwrap_or_else(|| ItemDescr::plain(item_id, rolled.dict_id, value)),
    )
}

/// Whether the item is a weapon or an armour piece: what takes
/// modifications and defects (`items_rules`), and so has a condition.
fn is_gear(dict_id: u16) -> bool {
    modifications::item_mods(dict_id).is_some_and(|mods| mods.modifications != 0)
}

/// One defect for a new item, among those the client knows for it and its
/// modifications allow (`Modifications::defect_candidates`, the list a
/// worn item's defect is rolled from), by drop weight. None if the item
/// can have none.
fn add_defect(account: &mut Account, item_id: u32, rng: &mut Rng) -> Option<u32> {
    let item = account.item_mut(item_id)?;
    if item.modifications.len() >= limits::MODIFICATIONS {
        return None;
    }
    let candidates = modifications::Modifications::shared().defect_candidates(
        item.dict_id,
        &items_rules::own_modifications(item),
        &items_rules::defects(item),
    );
    let weights: Vec<u32> = candidates
        .iter()
        .map(|row| (row.drop_weight * 1000.0).round().max(0.0) as u32)
        .collect();
    let defect = candidates[rng.pick(&weights)?].id;
    item.modifications.push((defect, true));
    Some(defect)
}

/// What using a drop card did.
#[derive(Clone, Debug, PartialEq)]
pub struct DropCardUse {
    pub dropped: Dropped,
    /// The cards left.
    pub cards_left: u32,
    /// `98`: the client adds the item to its inventory and sets the card's
    /// count. Send the account's inventory, account and money after it
    /// ([`wire::after_change`]): a card stack used up is still in the
    /// client's inventory with a count of 0, and silver or gold is not in
    /// `98` at all.
    pub message: ServerMessage,
}

/// Use a drop card (the voucher `drop_card`, the screen's stash map): one
/// card for one roll of the `drop_card_drop` table, which always gives.
/// `match_level` is the drop level of the match the screen shows
/// ([`super::pvb::drop_level`]). Refused, and nothing changed, when the
/// item is no drop card of the account ([`DropCardRefusal::NoCard`]) or
/// the roll gives nothing ([`DropCardRefusal::NoRoll`]): deny with the
/// game's `st_cannot_use_drop_card`. The lobby goes through
/// [`Results::use_drop_card`], which allows one card a finisher of a match.
pub fn use_drop_card(
    account: &mut Account,
    card_item_id: u32,
    match_level: u16,
    rng: &mut Rng,
) -> Result<Result<DropCardUse, WireError>, DropCardRefusal> {
    use_card(
        account,
        card_item_id,
        match_level,
        &Drops::shared().drop_card,
        rng,
    )
}

/// [`use_drop_card`] with the table rolled.
fn use_card(
    account: &mut Account,
    card_item_id: u32,
    match_level: u16,
    table: &DropTable,
    rng: &mut Rng,
) -> Result<Result<DropCardUse, WireError>, DropCardRefusal> {
    let dictionary = Dictionary::shared();
    let card = dictionary
        .item_named(DROP_CARD)
        .ok_or(DropCardRefusal::NoCard)?
        .dict_id;
    let owned = account.item(card_item_id).ok_or(DropCardRefusal::NoCard)?;
    if owned.dict_id != card || owned.condition_or_stack == 0 {
        return Err(DropCardRefusal::NoCard);
    }
    let level = match_level.min(u16::from(u8::MAX)) as u8;
    // Rolled before anything changes: a roll that gives nothing keeps the
    // card.
    let rolled = drops::roll(table, dictionary, &account.owned_dict_ids(), level, 0, rng)
        .ok_or(DropCardRefusal::NoRoll)?;
    let (dropped, shown) = give_drop(account, &rolled, false, rng);
    let cards_left = match account
        .inventory
        .iter()
        .position(|item| item.item_id == card_item_id)
    {
        Some(index) if account.inventory[index].condition_or_stack > 1 => {
            account.inventory[index].condition_or_stack -= 1;
            account.inventory[index].condition_or_stack
        }
        Some(index) => {
            account.inventory.remove(index);
            0
        }
        None => 0,
    };
    account.sync_kits();
    Ok(
        lobby::drop(&shown, card_item_id, cards_left, dropped.rarity).map(|message| DropCardUse {
            dropped,
            cards_left,
            message,
        }),
    )
}

#[cfg(test)]
mod tests;
