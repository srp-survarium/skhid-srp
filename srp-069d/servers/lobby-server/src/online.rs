//! Online matches from the lobby: queue group / custom room / squad play -> a match on the match
//! server -> `82` to every client -> the clients' win / loss reports -> results and rewards. Notes and open items:
//! `docs/notes/online.md`. Always on when a match server hook is attached (srp always attaches one).
//!
//! The lobby does not know the match server: it only sees the [`OnlineMatches`] hook (implemented by `srp` over
//! `match-server`). Everything the lobby decides is here:
//!
//! * **Start** ([`start`]): seats (account, name, team, login session, profile) -> `hook.create` -> one `82
//!   connect_to_game_server` per player (PROVEN layout, `protocol::s2c::ConnectToMatch`), the chat context of the match
//!   (channels 5..7) and the presence byte. A failed `create` leaves every player where it was.
//! * **Teams of the queue** ([`balance`]): a CHOICE, not the original's rule (nothing in the exe says): whole tickets
//!   (a squad stays together), the largest first, each to the team with fewer players, at most half of the mode's
//!   `max_players` per team.
//! * **Reports** ([`on_events`]): the client's end-of-match `53` flush carries event 5 (my team won) or 6 (lost, and also
//!   draw) with `value = match_id` online (PROVEN 0x20011695E..0x200116988). B8 enables them. Reports of a player of
//!   an online match are consumed here, never by the local-match path.
//! * **Verdict** ([`decide_winner`]): the only winner source until a referee client reports the world's own result.
//!   A team wins when a majority of its reports say 5 and the other team's majority does not; a majority of
//!   6 on both teams is a draw; anything else is unknown and pays nothing.
//! * **Finish** ([`tick`]): when the match server reports the end and every entered player has reported (or
//!   `online_report_grace` passed): per reporting player the data reward (`GameData::match_reward`), the drops,
//!   the facts for achievements / quests ([`stats::record_match`](crate::stats::record_match)) and `85 00`. The `97`
//!   results screen is sent only when the match server delivered real statistics ([`Ended::stats`]); kills, deaths and
//!   score are never invented (the stock client reports none: online_results 1).
//! * **PvE end** ([`pve_result`], docs/notes/pve.md): the clients end the mission themselves (pve.lua type 26 ->
//!   world state 4 / 5, 0x2002F39C7) and report event 5 / 6. When the reports are in (or the first is 30 s old) the lobby
//!   stores the run and pushes `99` and the drops `83 45 0B` to every player while their match is still loaded (both
//!   readers need it: 0x20018ED84, 0x2001ABD97), ends the relay match [`PVE_END_AFTER_RESULT`] later (its closing DF,
//!   code 2, closes the match and keeps the result screen) and sends `85 00` once the player left the screen (`5F 04`).
//!   Success = the majority of the reports; the time = the match server's clock from the enter to the first report;
//!   chests found, score, kills and deaths are world state no client reports: 0 (OPEN, never invented), so a won run
//!   gives the daily key and the achievements but no chest.
//!   The match server ends a mission itself on the script's world-free failures (pre-start cancel, 45-minute end,
//!   every human left; `match_server::PveEndRule`) and reports winner 1: [`finish`] sends `99` with success 0 and
//!   `83 45 0B` (CHOICE: the client's own failure screen), then `85 00`. With a world host the cancel and the 45-minute
//!   end come as an outcome first (below); this path covers "every human left".
//! * **PvE end from the world host**: the hook's
//!   [`OnlineMatches::take_outcomes`] gives the end of the mission as the exe's own world code saw it (the extraction,
//!   every human dead, the script's failures) with the world's counters. `99` (success = winner team 0, the host's
//!   mission time, kills / deaths / score per player, chests found = the humans' `pve_open_chest` events) and `83 45 0B`
//!   go out at once; [`PVE_END_AFTER_RESULT`] later the match server closes the match with DF code 2
//!   ([`OnlineMatches::end_after_result`]): the clients keep their world and the result screen gets the pointer; the
//!   `85 00` waits until every human left the screen (`5F 04`, its "Return to lobby") or the match, at most
//!   [`PVE_RESULT_SCREEN_CAP`] after the `99`. The match server's own failure ends (pre-start cancel, 45-minute end)
//!   come the same way (its hand-over outcome, winner 1). The clients' reports stay a cross-check (a disagreement is
//!   logged); they make the result only without a host outcome ([`HOST_OUTCOME_WAIT`]).
//!   Whenever a PvE match releases a player (menu leave, disconnect, end), his one-man PvE squad ends first (`AE` squad
//!   id 0 before the `85 00`, [`end_solo_pve_squad`]): while it exists the client's PvE preview shows no Cancel and
//!   hides the normal Play / mode panel.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use accounts::{public_id, AccountId, Profile, PveRun, PveRunPlayer};
use game_data::{GameData, MatchFacts};
use protocol::c2s::Event53;
use protocol::s2c::{ConnectToMatch, Denied, DeniedBody, MatchResult, PveResult, PveResultPlayer, QueueState, ResultPlayer, S2c, SystemMessage};
use protocol::Encode;

use crate::dispatch::Ctx;
use crate::local::MatchOutcome;
use crate::peer::Cross;
use crate::queue::Ticket;
use crate::social;

/// Match level when no player's kit can be read (the local match uses 1 too); online matches use [`match_level`].
pub const ONLINE_MATCH_LEVEL: u32 = 1;

/// One player of a match to create (roster order = slot order).
#[derive(Debug, Clone)]
pub struct Seat {
    pub account: AccountId,
    pub name: String,
    /// 0 or 1
    pub team: u8,
    /// The login session of the account (C9 carries it; None = not checked by the match server).
    pub session: Option<u32>,
    pub profile: Profile,
    /// hero-of-the-day title at the match start `(hero_place, nomination 0-based)` ([`crate::hero::match_bytes`])
    pub hero: Option<(u8, u8)>,
}

/// The AI players a PvP match gets besides its humans (the hook picks them: `local_match::pick_online_bots`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BotFill {
    /// AI players for team 0 / team 1
    pub need: [usize; 2],
    /// the level they are picked at: nearest `match_making_level` first ([`bot_level`])
    pub level: u32,
    /// a custom room's "bots ignore players" (the 98 / `62 1E` byte `ai_ignore_players`): the world host sets the exe's
    /// own harmless-bots byte, so the AI neither sees nor hears the humans (world host protocol 7); false for the queue
    pub ignore_players: bool,
}

/// At most this many players per team in a bot-filled match (CHOICE); bots fill each team up to it.
pub const BOT_TEAM_MAX: usize = 6;

/// Modes the exe's AI cannot play: Search and Destroy. A PvP bot claims a team patrol graph of the level project
/// (`server_objects`, loader 0x2003E3430); the S&D projects carry none (world host on level_02_day_sad_v3,
/// level_03_day_sad and sad_abandoned_base: "patrol graphs per team [0, 0]", PROVEN; level_03_day_sad_v2 LIKELY the
/// same), and a bot without a free graph is undefined in the exe, so the host refuses the
/// match. The client offers no S&D bots training either (`training_ai_data`). The gun race (Arms Race, mode 3) neither:
/// arena.lua spawns only records without an AI id (`has_entered_match` precondition `not(E20)`) and `arena_01/default`
/// has no patrol path either (no `patrol_path` string in its client project, PROVEN; docs/notes/gun-race.md). Bot fill
/// and room bots skip these modes.
pub const NO_BOT_MODES: [u32; 2] = [crate::queue::SAD_MODE, game_data::gunrace::MODE];

/// Can a match of this mode get AI players ([`NO_BOT_MODES`])?
pub fn bots_play(mode: u32) -> bool {
    !NO_BOT_MODES.contains(&mode)
}

/// The match server's answer to [`OnlineMatches::create`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ticket82 {
    pub match_id: u32,
    pub host: String,
    pub port: u16,
    /// `order_id` per seat, in seat order.
    pub orders: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndedPlayer {
    pub account: AccountId,
    pub team: u8,
    /// The client got into the running match.
    pub entered: bool,
    /// The player left with the menu "leave match" (the match server got his CE; docs/notes/online.md): a voluntary
    /// leave, even if his client still reports a result.
    pub left_by_menu: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EndedStat {
    pub account: AccountId,
    pub kills: u16,
    pub deaths: u16,
    pub score: u16,
    /// the world's own data of the match (world host protocol 3: event rows, streaks, inactive flag, badges, kill log),
    /// one shared value for every player of the match ([`crate::match_data`]); None = no world host data
    pub world: Option<crate::match_data::SharedWorld>,
    /// what he used in the world (world host protocol 6, `Used`): the after-match charge ([`crate::charge`]); None = not
    /// reported, nothing is charged
    pub used: Option<crate::charge::MatchUse>,
}

/// A match that ended on the match server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ended {
    pub match_id: u32,
    pub duration: Duration,
    pub players: Vec<EndedPlayer>,
    /// Real per-player counters and the two team scores, from a source that observed the world; None = unknown.
    pub stats: Option<(Vec<EndedStat>, [u32; 2])>,
    /// The winning team when the match server knows it without a world (an empty TDM team: the other team wins);
    /// None = unknown. No client message carries a winner online.
    pub winner: Option<u8>,
}

/// The end of a PvE mission as the world host saw it (the exe's own world code behind the relay): delivered once, while
/// the match still runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostOutcome {
    pub match_id: u32,
    /// world end state `W+0x180AE0` (4 / 5)
    pub state: u8,
    pub winner_team: u8,
    /// winner team 0: the extraction, the mission's only success (OPEN until a finished mission is recorded)
    pub success: bool,
    /// the host's world time from the enter DF to the end (OPEN: whether the result screen's time is this or pve.lua's
    /// own `match_time` of action 26)
    pub mission_ms: u32,
    /// the humans only (bot rows are dropped by the hook)
    pub players: Vec<HostPlayerStat>,
}

/// One human's counters of a [`HostOutcome`]: event rows 0 / 2 (kills / deaths), score `+0xA1C`, event row 42
/// `pve_open_chest` (chests) of his statistics block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostPlayerStat {
    pub account: AccountId,
    pub kills: u16,
    pub deaths: u16,
    pub score: u16,
    pub chests: u16,
    /// PvE paid respawns the world charged him: access keys and gold (world entry `+0x54` / `+0x58`); charged at the result
    pub keys_spent: u16,
    pub gold_spent: u32,
}

/// What a PvE match needs besides the roster (docs/notes/pve.md section 8). The roster of a PvE match is one team of
/// 1..=3 humans (team 0); the enemy bots belong to the mission script.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PveSetup {
    /// Mission id (the match options' map id, 15).
    pub mission: u32,
    /// Difficulty index 0..=2 (LIKELY options byte +0x1082, OPEN until the first capture).
    pub difficulty: u8,
    /// The map project, `pve_01_v{difficulty + 1}_maze{1|2}` (LIKELY mapping, OPEN).
    pub project: String,
    /// Humans in the match (`human_players_count` of the script).
    pub human_players: u32,
    /// Equipment level of the squad, the maximum `items_level` of its members (LIKELY options byte +0x1081); 0 = unknown.
    pub equipment_level: u32,
}

/// What the lobby needs from a match server.
pub trait OnlineMatches: Send + Sync {
    /// Can a match for this (map, mode) start (a world for it exists)?
    fn available(&self, map: u32, mode: u32) -> bool;
    /// Create a match; the options blob, the roster and the tickets are the hook's business. `match_type` is the options
    /// byte +0x107D ([`MatchKind::options_match_type`]).
    fn create(&self, data: &GameData, map: u32, mode: u32, weather: u32, match_type: u8, seats: &[Seat]) -> Result<Ticket82, String>;
    /// Can a PvP match on this row of the data (map, mode, variation, weather = one level project) get a start world?
    /// Default: a world for (map, mode) and the row is the (map, mode)'s own row of `resolve_map(map, mode, 0)` (variation
    /// 0, day), the row a stored world is captured from. A world host serves the projects of its config.
    fn serves(&self, data: &GameData, row: &game_data::ModeRow) -> bool {
        self.available(row.map, row.mode) && data.matches.resolve_map(row.map, row.mode, 0) == Some(row)
    }
    /// Create a PvP match on a given row of the data ([`serves`](Self::serves)), with AI players when `fill` is set.
    /// Default: [`create`](Self::create) / [`create_with_bots`](Self::create_with_bots) with the row's variation.
    fn create_row(&self, data: &GameData, row: &game_data::ModeRow, match_type: u8, seats: &[Seat], fill: Option<BotFill>) -> Result<Ticket82, String> {
        match fill {
            Some(f) => self.create_with_bots(data, row.map, row.mode, row.variation, match_type, seats, f),
            None => self.create(data, row.map, row.mode, row.variation, match_type, seats),
        }
    }
    /// Can this match server fill a PvP match with AI players (a world host that drives them)? Default: no.
    fn bots_available(&self) -> bool {
        false
    }
    /// [`OnlineMatches::create`] plus AI players after the humans ([`BotFill`]). Default: refused.
    #[allow(clippy::too_many_arguments)]
    fn create_with_bots(&self, _data: &GameData, _map: u32, _mode: u32, _weather: u32, _match_type: u8, _seats: &[Seat], _fill: BotFill) -> Result<Ticket82, String> {
        Err("this match server has no bots".into())
    }
    /// Create a PvE match. The default refuses: a match server that has no PvE world says so, the lobby then refuses the
    /// search (`84 5F` code 6) and nobody is charged.
    fn create_pve(&self, _data: &GameData, _setup: &PveSetup, _seats: &[Seat]) -> Result<Ticket82, String> {
        Err("this match server has no PvE world".into())
    }
    /// Matches that ended since the last call.
    fn take_ended(&self) -> Vec<Ended>;
    /// Ask the match server to end a running match now (its end then comes back through `take_ended`). The clients'
    /// own end sends nothing to the match server (`close_current_match(3)`: no CE, no FA, the connection stays), so the
    /// lobby ends it from the players' reports. Default: nothing to end.
    fn end(&self, _match_id: u32) -> bool {
        false
    }
    /// End a PvE match whose result (`99`, `83 45 0B`) went out: the mission script already ended the clients' worlds
    /// (the host's outcome, the match server's own failure end, or the players' end reports), so the closing DF keeps
    /// their world and HUD (DF code 2: the result screen gets the pointer; docs/notes/pve.md "End screen"). Default:
    /// [`OnlineMatches::end`].
    fn end_after_result(&self, match_id: u32) -> bool {
        self.end(match_id)
    }
    /// PvP end screen: the match server still runs the match's clients although it handed the result over (the lobby then
    /// keeps their `85 00` back until [`OnlineMatches::has_match`] is false). Default: false.
    fn holds_clients(&self, _match_id: u32) -> bool {
        false
    }
    /// The match server still has the match (running, or ended and draining its connections). Default: false.
    fn has_match(&self, _match_id: u32) -> bool {
        false
    }
    /// Players who left a running match since the last call (menu, closed game, no CD for 5 s): the lobby releases them
    /// at once instead of at the match end. Default: none.
    fn take_left(&self) -> Vec<Left> {
        Vec::new()
    }
    /// Match time of a running match since its enter (the mission clock every client runs), None when unknown. Default:
    /// unknown (the lobby then takes the time since the match was created).
    fn played(&self, _match_id: u32) -> Option<Duration> {
        None
    }
    /// The world host's outcomes since the last call ([`HostOutcome`]). Default: no world host.
    fn take_outcomes(&self) -> Vec<HostOutcome> {
        Vec::new()
    }
    /// Is a world host attached to this match and still in use (its outcome can still come)? Default: no.
    fn world_host(&self, _match_id: u32) -> bool {
        false
    }
    /// Rejoin: players dropped from a running match without the menu leave (no CD for 5 s,
    /// transport silence or disconnect) whose slot the match server keeps for them, since the last call. Default: none
    /// (every drop is a leave and comes through `take_left`).
    fn take_dropped(&self) -> Vec<Left> {
        Vec::new()
    }
    /// Rejoin: dropped players whose client is back in the match since the last call (match id, account).
    fn take_rejoined(&self) -> Vec<(u32, AccountId)> {
        Vec::new()
    }
    /// Rejoin: may the account's dropped slot still come back? `session` = the login session after a new sign-in (the
    /// match server checks C9 against it). Default: no.
    fn allow_rejoin(&self, _match_id: u32, _account: AccountId, _session: Option<u32>) -> bool {
        false
    }
    /// Rejoin declined: the dropped slot leaves for good (the leave then comes through `take_left`). Default: nothing.
    fn leave_dropped(&self, _match_id: u32, _account: AccountId) -> bool {
        false
    }
}

/// A player who left a running online match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Left {
    pub match_id: u32,
    pub account: AccountId,
    pub by_menu: bool,
}

/// After the first player's report (53 event 5 / 6) the lobby ends the match at the latest this long after it: the
/// clients reach the end screen at the same simulated time, so the others' reports follow within seconds.
pub const END_AFTER_FIRST_REPORT: std::time::Duration = std::time::Duration::from_secs(30);

/// A PvE match is ended on the match server this long after its `99` / `83 45 0B` went out (CHOICE, 3 s: the end's
/// closing DF closes the client's match connection, and both readers need the match still loaded, 0x20018ED84 /
/// 0x2001ABD97; the lobby and the match server send on different connections, so the result goes first with a margin).
/// The closing DF has code 2 ([`OnlineMatches::end_after_result`]), which turns the result screen usable.
pub const PVE_END_AFTER_RESULT: Duration = Duration::from_secs(3);

/// PvP end screen: this long after a PvP match's `83 45 0A` + `97` went out the lobby asks
/// the match server to close it after its result ([`OnlineMatches::end_after_result`]). When the match server handed the
/// result over while its clients were still in the match (a world host's end), the closing DF then carries code 2 and the
/// client shows its in-match victory screen with the 97 (the 97 handler fills the HUD only while the match is loaded,
/// 0x20018B2F1..0x20018B473; the two connections are separate, hence the margin, as for PvE). A match the match server
/// already closed (every other end) ignores it.
pub const PVP_END_AFTER_RESULT: Duration = Duration::from_secs(3);

/// PvP end screen: the longest the lobby keeps a player's `85 00` back while the match server still has the match (the
/// relay closes it at most `MatchConfig::pvp_end_screen` after the result and drains its connections a few seconds more).
/// An `85 00` that reaches a client whose match world is still loaded unloads that world (85 handler 0x200175590, world
/// unload 0x200130F20, lobby return 0x200117E60; seen with the real client: no victory splash, then a crash on the match
/// connection's timeout). After the client's `close_current_match 3` (the closing DF) an `85 00` leaves it alone.
pub const PVP_RELEASE_CAP: Duration = Duration::from_secs(30);

/// The `85 00` of a PvE match closed with its result screen on (DF code 2, [`OnlineMatches::end_after_result`]) waits
/// until every human still in it left the screen (`5F 04`) or the match, at most this long after the `99`. The client's
/// "Return to lobby" (`close_pve_stats_clicked`, 0x20019E167) unloads the world and returns to the lobby itself; the
/// `85 00` (queue state 0) then frees Play. OPEN: what an `85 00` does while the result screen is still open (not
/// traced; CHOICE to keep it until the screen is left). The match connection is NOT kept open for the screen: with it
/// open the client stays in its in-match controls (no pointer, seen with the real client).
pub const PVE_RESULT_SCREEN_CAP: Duration = Duration::from_secs(120);

/// With a world host in use, the clients' reports wait this long after the first one for the host's outcome before
/// they make the result themselves (CHOICE: the host reads its outcome from a final tick, 300 ms behind the newest DF,
/// so the clients' `53` usually comes first).
pub const HOST_OUTCOME_WAIT: Duration = Duration::from_secs(10);

/// What kind of match is created: the queue's random or rated matches, or a clan battle (custom match type 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchKind {
    Random,
    Ranked,
    /// A custom TRAINING match (custom room type 0). OFFICIAL 0.65b notes: "custom training matches do not yield rewards
    /// (like regular PvP matches). Also, the character's experience and reputation with factions are not awarded,
    /// equipment progress is not tracked." So it pays nothing and records no progress (`pay`).
    Training,
    /// A clan battle: the clan of team 0 and the clan of team 1 (different, both non-zero). Results go to the clan
    /// experience (`clans::record`) and the faction battle only; see `docs/notes/clan-battles.md`.
    Clan([u32; 2]),
    /// A PvE mission of the given difficulty on the given maze variant (1 or 2): one team of humans against the script's
    /// bots. Results and chests are paid by `pve.rs`, never by the PvP reward path.
    Pve { difficulty: u8, maze: u8 },
}

impl MatchKind {
    pub fn is_pve(self) -> bool {
        matches!(self, MatchKind::Pve { .. })
    }

    pub fn rated(self) -> bool {
        self == MatchKind::Ranked
    }

    /// The match-type byte of the match options (+0x107D): random 1, ranked 3, clan battle 6, everything else 0
    /// (`protocol::s2c::online::MATCH_TYPE_*`; the client turns the killcam and the hit / kill markers off for 3 and 6,
    /// and tracks inactivity for 1 and 3; custom rooms and PvE stay 0: no AFK penalty there, OFFICIAL 0.56b).
    pub fn options_match_type(self) -> u8 {
        use protocol::s2c::online::{MATCH_TYPE_CLAN, MATCH_TYPE_PLAIN, MATCH_TYPE_RANDOM, MATCH_TYPE_RANKED};
        match self {
            MatchKind::Random => MATCH_TYPE_RANDOM,
            MatchKind::Ranked => MATCH_TYPE_RANKED,
            MatchKind::Clan(_) => MATCH_TYPE_CLAN,
            _ => MATCH_TYPE_PLAIN,
        }
    }
    /// `MatchFacts.match_type`: 1 random, 2 rated (the achievement `match_type_mask` bits); 3 = clan battle (CHOICE, no
    /// achievement mask uses it).
    pub fn match_type(self) -> u8 {
        match self {
            MatchKind::Random | MatchKind::Training => 1,
            MatchKind::Ranked => 2,
            MatchKind::Clan(_) => 3,
            // CHOICE: no achievement mask uses it; PvE facts are not recorded by this path
            MatchKind::Pve { .. } => 4,
        }
    }
}

#[derive(Debug, Clone)]
struct SeatState {
    account: AccountId,
    team: u8,
    in_squad: bool,
    /// in a squad with at least one clanmate (never in a clan battle)
    in_clan_squad: bool,
    /// hero title at the match start (the 97 player row shows it)
    hero: Option<(u8, u8)>,
    /// the squad (of two or more) the player queued with (achievement 621: the squad's medals)
    squad: Option<u32>,
    /// the seat's `82` (rejoin: sent again to bring a dropped player back)
    connect: ConnectToMatch,
    /// achievements 415-417: the most random modifiers on one item per options profile of the loadout the match was
    /// started with ([`crate::achievements::start_mods`])
    start_mods: Vec<u32>,
}

pub(crate) struct Running {
    map: u32,
    mode: u32,
    kind: MatchKind,
    seats: Vec<SeatState>,
    /// account -> reported "my team won" (event 5) / lost or draw (event 6)
    reports: BTreeMap<AccountId, bool>,
    /// When the first report arrived, and whether the lobby already asked the match server to end the match.
    first_report: Option<Instant>,
    end_asked: bool,
    /// accounts already released because they left (the match end does not push their lobby state again)
    released: std::collections::BTreeSet<AccountId>,
    ended: Option<(Ended, Instant)>,
    /// when the lobby created the match
    created: Instant,
    /// PvE: the match server's mission clock at the first report (seconds), and when the result (`99`, `83 45 0B`) went out
    pve_time: Option<u32>,
    pve_sent: Option<Instant>,
    /// PvE: the world host's outcome, the result's source when present
    host: Option<HostOutcome>,
    /// PvE with a host outcome: the players who closed the result screen (`5F 04`)
    closed_stats: std::collections::BTreeSet<AccountId>,
    /// PvE: what the start took from each payer (keys, gold): given back when nobody ever entered the match (the world
    /// host gave no start world, nobody loaded, ...)
    pve_receipts: BTreeMap<AccountId, crate::pve::Receipt>,
    /// what the start took from each account besides the PvE entry (the per-match boosters' stock or silver, the service
    /// boosters' units, a custom room's tokens): given back when nobody ever entered the match
    start_charges: BTreeMap<AccountId, StartCharge>,
    /// the boosters each account's start charged (stock or money): their reward bonus is paid with this match
    boosters: BTreeMap<AccountId, Vec<u16>>,
    /// rejoin: seats the match server keeps for a dropped player (not released: no `85 00`); a seat still here at the end
    /// left early (paid nothing when the winner is known, not fined)
    dropped: std::collections::BTreeSet<AccountId>,
}

/// What a match start took from one account, measured on its profile before and after the charge: stock units by dict
/// id (per-match and service boosters), silver and gold (a booster paid in silver, a custom room's tokens).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct StartCharge {
    pub items: BTreeMap<u16, u32>,
    pub silver: u32,
    pub gold: u32,
}

impl StartCharge {
    /// The units and money `after` has less than `before` (nothing for what it has more of).
    pub fn between(before: &Profile, after: &Profile) -> StartCharge {
        let count = |p: &Profile| {
            let mut m: BTreeMap<u16, u32> = BTreeMap::new();
            for i in &p.items {
                *m.entry(i.dict_id).or_default() += i.count;
            }
            m
        };
        let (b, a) = (count(before), count(after));
        let items = b.iter().filter_map(|(d, n)| n.checked_sub(a.get(d).copied().unwrap_or(0)).filter(|k| *k > 0).map(|k| (*d, k))).collect();
        StartCharge { items, silver: before.silver.saturating_sub(after.silver), gold: before.gold.saturating_sub(after.gold) }
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty() && self.silver == 0 && self.gold == 0
    }

    pub fn merge(&mut self, o: StartCharge) {
        for (d, n) in o.items {
            *self.items.entry(d).or_default() += n;
        }
        self.silver += o.silver;
        self.gold += o.gold;
    }

    /// Put it back: the units onto a stack of the same dict id (a new row when none is left), the money credited.
    pub fn give_back(&self, p: &mut Profile) {
        for (&d, &n) in &self.items {
            match p.items.iter_mut().find(|i| i.dict_id == d) {
                Some(row) => row.count += n,
                None => {
                    p.add_item(d, n);
                }
            }
        }
        p.credit(accounts::Currency::Silver, u64::from(self.silver));
        p.credit(accounts::Currency::Gold, u64::from(self.gold));
    }
}

/// What the start of match `id` took from `account` (custom rooms add their tokens after the match exists).
pub(crate) fn add_start_charge(cx: &mut Ctx, id: u32, account: AccountId, c: StartCharge) {
    if c.is_empty() {
        return;
    }
    if let Some(m) = cx.registry.online.matches.get_mut(&id) {
        m.start_charges.entry(account).or_default().merge(c);
    }
}

/// A match that ended before anybody entered it: every account gets back what its start took, and its client its
/// inventory (`89`), money (`8B`) and booster records (`94`) as they are now.
fn refund_start_charges(cx: &mut Ctx, id: u32, charges: &BTreeMap<AccountId, StartCharge>) {
    for (a, c) in charges.iter().filter(|(_, c)| !c.is_empty()) {
        if crate::dispatch::update_account(cx, *a, |p| c.give_back(p)).is_none() {
            log::error!("online match {id:#x}: account {a}: the start's charge could not be given back ({c:?})");
            continue;
        }
        log::info!("online match {id:#x} ended before anybody entered: account {a} gets back {:?}, {} silver, {} gold", c.items, c.silver, c.gold);
        if let Ok(p) = cx.lobby.store.load_profile(*a) {
            cx.fx.push(crate::peer::Cross { to: *a, msg: S2c::Inventory(crate::build::inventory_89(&cx.lobby.data, &p)) });
            cx.fx.push(crate::peer::Cross { to: *a, msg: S2c::Money(crate::build::money_8b(&p)) });
            for msg in crate::premium::status_msg(cx, *a) {
                cx.fx.push(crate::peer::Cross { to: *a, msg });
            }
        }
    }
}

/// What a PvE start took (lobby pve.rs), kept with the match until it is over.
pub(crate) fn set_pve_receipts(cx: &mut Ctx, id: u32, receipts: BTreeMap<AccountId, crate::pve::Receipt>) {
    if let Some(m) = cx.registry.online.matches.get_mut(&id) {
        m.pve_receipts = receipts;
    }
}

/// The online matches the lobby knows (in the [`Registry`](crate::Registry)).
#[derive(Default)]
pub struct OnlineBook {
    matches: BTreeMap<u32, Running>,
    of: BTreeMap<AccountId, u32>,
    /// A bot-filled start failed; the next try waits until then (the tick would retry it every pass)
    bot_retry: Option<Instant>,
    /// PvP matches whose result went out: the match server closes them after their result at that time
    /// ([`PVP_END_AFTER_RESULT`])
    close_after: Vec<(u32, Instant)>,
    /// PvP end screen: players whose `85 00` waits until the match server let go of the match (match id, account, cap)
    release_after: Vec<(u32, AccountId, Instant)>,
    /// per player: the match id and the world data (its bots) of his last bot-filled PvP result, for the result
    /// window's `38 00` on a bot row (details.rs)
    result_bots: BTreeMap<AccountId, (u32, crate::match_data::SharedWorld)>,
}

impl OnlineBook {
    /// The match id and the world data of the player's last bot-filled PvP result.
    pub fn result_bots(&self, a: AccountId) -> Option<(u32, crate::match_data::SharedWorld)> {
        self.result_bots.get(&a).cloned()
    }

    /// The match the account plays or just played (until its rewards are paid).
    pub fn match_of(&self, a: AccountId) -> Option<u32> {
        self.of.get(&a).copied()
    }

    pub fn len(&self) -> usize {
        self.matches.len()
    }

    pub fn is_empty(&self) -> bool {
        self.matches.is_empty()
    }
}

/// Result of the vote of the clients' reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Winner(u8),
    Draw,
    Unknown,
}

/// The winner from the clients' reports `(team, reported_my_team_won)`; rules in the module docs.
pub fn decide_winner(reports: &[(u8, bool)]) -> Verdict {
    let count = |team: u8, won: bool| reports.iter().filter(|r| r.0 == team && r.1 == won).count();
    let (w, l) = ([count(0, true), count(1, true)], [count(0, false), count(1, false)]);
    let claims_win = [w[0] > l[0], w[1] > l[1]];
    let claims_lose = [l[0] > w[0], l[1] > w[1]];
    // a draw is reported as "lost" by both sides (the client sets 6 for both teams)
    if claims_lose[0] && claims_lose[1] {
        return Verdict::Draw;
    }
    match (claims_win[0], claims_win[1]) {
        (true, false) => Verdict::Winner(0),
        (false, true) => Verdict::Winner(1),
        _ => Verdict::Unknown,
    }
}

/// A gun race winner byte that is a result (docs/notes/gun-race.md): a record index 0..7 (the race winner) or
/// arena.lua's 9 (the 10-minute end: a played match nobody won). Anything else (the relay's `GUNRACE_NO_RESULT`: the
/// pre-game cancel) is none.
pub fn gunrace_result(w: u8) -> bool {
    usize::from(w) < game_data::gunrace::MAX_PLAYERS || w == game_data::gunrace::NO_WINNER
}

/// The verdict of a gun race match: the match server's winner byte when it is a result ([`gunrace_result`]:
/// `Winner(record index)` or `Winner(9)`, which no seat's team is: nobody won); else the clients' reports, each
/// seat its own team (a client reports "won" when the world's winner byte is its team, 0x20011695E): one seat claiming
/// the win and every other reporting seat a loss = that seat; reports and none claiming = `Winner(9)`; else unknown.
pub fn gunrace_verdict(winner: Option<u8>, reports: &[(u8, bool)]) -> Verdict {
    match winner {
        Some(w) if gunrace_result(w) => Verdict::Winner(w),
        Some(_) => Verdict::Unknown,
        None => {
            let won: Vec<u8> = reports.iter().filter(|r| r.1).map(|r| r.0).collect();
            match won.as_slice() {
                [t] => Verdict::Winner(*t),
                [] if !reports.is_empty() => Verdict::Winner(game_data::gunrace::NO_WINNER),
                _ => Verdict::Unknown,
            }
        }
    }
}

/// Split whole tickets over two teams of at most `cap` players: largest first, each to the smaller team (ties: team 0).
/// Returns the accounts of both teams and the order ids of the tickets that fit; a ticket that fits in neither team is
/// left out (it stays in the queue).
pub fn balance(tickets: &[Ticket], cap: usize) -> ([Vec<AccountId>; 2], Vec<u32>) {
    let mut order: Vec<&Ticket> = tickets.iter().collect();
    order.sort_by_key(|t| (std::cmp::Reverse(t.accounts.len()), t.order));
    let mut teams: [Vec<AccountId>; 2] = [vec![], vec![]];
    let mut used = vec![];
    for t in order {
        let n = t.accounts.len();
        let mut pick = if teams[0].len() <= teams[1].len() { [0, 1] } else { [1, 0] };
        if teams[pick[0]].len() + n > cap {
            pick.swap(0, 1);
        }
        let team = pick[0];
        if teams[team].len() + n > cap {
            continue;
        }
        teams[team].extend(t.accounts.iter().copied());
        used.push(t.order);
    }
    (teams, used)
}

fn push(cx: &mut Ctx, to: AccountId, msg: S2c) {
    cx.fx.push(Cross { to, msg });
}

/// What a group of the queue became.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueStart {
    /// a match was created, the tickets are used up
    Started,
    /// worlds exist, but no group of waiting players can play together yet: they keep searching
    Wait,
    /// no way to start (online off, no world, the server failed): the caller refuses a full group as before
    Fallback,
}

/// The rows of the data (map, mode, variation, weather = one level project) the match server can start a PvP match on
/// ([`OnlineMatches::serves`]: a world host serves the projects of its config), each once, sorted.
pub fn playable_rows(cx: &Ctx) -> Vec<crate::queue::Pick> {
    let Some(hook) = cx.lobby.online.as_ref() else { return vec![] };
    let gd = &cx.lobby.data;
    let mut rows: Vec<crate::queue::Pick> = gd.matches.modes().iter().filter(|r| hook.serves(gd, r)).map(crate::queue::Pick::of).collect();
    rows.sort_unstable();
    rows.dedup();
    rows
}

/// The data row of a pick.
fn row_of(cx: &Ctx, p: &crate::queue::Pick) -> Option<game_data::ModeRow> {
    cx.lobby.data.matches.modes().iter().find(|r| crate::queue::Pick::of(r) == *p).cloned()
}

/// The queue's group and data row: with `online_queue` (a test-only override) every match is that one row
/// and any group of the queue; otherwise a group whose tickets share a playable mode
/// ([`crate::queue::PvpQueue::group_for_kinds`]; `unrated_only` for a bot fill) and the queue's pick among the rows the
/// match server serves ([`crate::queue::PvpQueue::pick`]: mode, map, then the variation and weather drawn).
/// Err(true) = nobody can play yet (keep searching), Err(false) = no way to start.
fn queue_group(cx: &Ctx, needed: u32, unrated_only: bool, now_unix: i64) -> Result<QueueMatch, bool> {
    let hook = cx.lobby.online.as_ref().ok_or(false)?;
    let q = &cx.registry.queue;
    if let Some((map, mode, weather)) = cx.lobby.cfg.online_queue {
        if !hook.available(map, mode) {
            return Err(false);
        }
        let row = cx.lobby.data.matches.resolve_map(map, mode, weather).cloned().ok_or(false)?;
        if unrated_only && !bots_play(mode) {
            return Err(true);
        }
        let g = if unrated_only { q.group_for_unrated(needed, |_| true).map(|g| g.0) } else { Some(q.group(needed)) };
        return g.filter(|g| !g.is_empty()).map(|group| QueueMatch { group, row, pinned: Some(weather) }).ok_or(true);
    }
    let mut rows = playable_rows(cx);
    if unrated_only {
        // a bot fill only on modes the AI can play (NO_BOT_MODES): an S&D-only search waits for ten players
        rows.retain(|r| bots_play(r.mode));
    }
    if rows.is_empty() {
        return Err(unrated_only);
    }
    let playable = |m: u32, rated: bool| q.choose(m, rated, &rows, now_unix).is_some();
    let (g, mask) = if unrated_only { q.group_for_unrated(needed, |m| playable(m, false)) } else { q.group_for_kinds(needed, playable) }.ok_or(true)?;
    let rated = g.first().is_some_and(|t| t.rated);
    let p = cx.lobby.roll(|d| q.pick_random(mask, rated, &rows, now_unix, |n| d.below(n.max(1)))).ok_or(true)?;
    let row = row_of(cx, &p).ok_or(false)?;
    Ok(QueueMatch { group: g, row, pinned: None })
}

/// A queue group and the data row of its match.
struct QueueMatch {
    group: Vec<Ticket>,
    row: game_data::ModeRow,
    /// the test override's third value: the match is created as before the pick existed (`create(map, mode, value)`)
    pinned: Option<u32>,
}

impl QueueMatch {
    /// The third value and the row for [`start_with`]: the override's own value and no row, else the picked row.
    fn target(&self) -> (u32, Option<&game_data::ModeRow>) {
        match self.pinned {
            Some(w) => (w, None),
            None => (self.row.variation, Some(&self.row)),
        }
    }
}

/// A queue group is ready: form a match ([`queue_group`]; docs/notes/online.md section 7).
pub fn queue_start(cx: &mut Ctx, now_unix: i64) -> QueueStart {
    if cx.lobby.online.is_none() {
        return QueueStart::Fallback;
    }
    let min = cx.lobby.cfg.online_min_players.max(2);
    let mut qm = match queue_group(cx, min, false, now_unix) {
        Ok(x) => x,
        // fewer than `online_min_players` share a playable mode so far: they keep searching (the fallback would refuse
        // them at the smallest row minimum, which can be below the operator's group size)
        Err(true) => return QueueStart::Wait,
        Err(false) => return QueueStart::Fallback,
    };
    let rated = qm.group[0].rated;
    let split = |row: &game_data::ModeRow| {
        let (teams, used) = balance(&qm.group, (match_size(row) as usize / 2).max(1));
        (!teams[0].is_empty() && !teams[1].is_empty() && teams[0].len() + teams[1].len() >= min as usize).then_some((teams, used))
    };
    let (teams, used) = match split(&qm.row) {
        Some(x) => x,
        // ranked: the slot's mode cannot form this match (e.g. Search and Destroy's 5 + 5 seats under the operator's
        // group size): the next rotation mode that can (queue::ranked_order, CHOICE)
        None if rated && qm.pinned.is_none() => match ranked_fall_through(cx, qm.row.mode, now_unix, &split) {
            Some((row, x)) => {
                log::info!("queue: ranked slot mode {} cannot form this match ({} tickets): falls through to mode {} ({})", qm.row.mode, qm.group.len(), row.mode, row.project);
                qm.row = row;
                x
            }
            None => return QueueStart::Wait,
        },
        // the tickets do not split into two teams yet: wait for more players
        None => return QueueStart::Wait,
    };
    let (third, row_arg) = qm.target();
    let (row, group) = (qm.row.clone(), qm.group.clone());
    let (map, mode, weather) = (row.map, row.mode, row.weather);
    let kind = if rated { MatchKind::Ranked } else { MatchKind::Random };
    match start_with(cx, map, mode, third, kind, teams, None, row_arg) {
        Ok(id) => {
            let taken: Vec<Ticket> = group.into_iter().filter(|t| used.contains(&t.order)).collect();
            cx.registry.queue.record_start(&taken, now_unix);
            cx.registry.queue.note_pick(crate::queue::Pick::of(&row));
            log::info!("online match {id:#x} started from the queue ({} tickets): {} (map {map} mode {mode} weather {weather})", taken.len(), row.project);
            QueueStart::Started
        }
        Err(e) => {
            log::warn!("online match from the queue not started: {e}");
            QueueStart::Fallback
        }
    }
}

/// Two teams of accounts and the order ids of the tickets they hold ([`balance`]).
type Split = ([Vec<AccountId>; 2], Vec<u32>);

/// The ranked fall-through of [`queue_start`] (CHOICE, [`crate::queue::ranked_order`]): the modes after `failed` in the
/// slot's order, each with its ranked rows the match server serves; the first whose drawn row `split`s the group into
/// two teams. None = no ranked mode can form this match yet (the group keeps searching).
fn ranked_fall_through(cx: &Ctx, failed: u32, now_unix: i64, split: &dyn Fn(&game_data::ModeRow) -> Option<Split>) -> Option<(game_data::ModeRow, Split)> {
    let rows = playable_rows(cx);
    let q = &cx.registry.queue;
    let order = crate::queue::ranked_order(now_unix);
    let after = order.iter().position(|m| *m == failed).map_or(0, |i| i + 1);
    for &m in &order[after..] {
        let of_mode: Vec<crate::queue::Pick> = rows.iter().filter(|r| r.mode == m).copied().collect();
        let Some(p) = cx.lobby.roll(|d| q.pick_random(0, true, &of_mode, now_unix, |n| d.below(n.max(1)))) else { continue };
        let Some(row) = row_of(cx, &p) else { continue };
        if let Some(x) = split(&row) {
            return Some((row, x));
        }
    }
    None
}

/// Create a random or rated match for two teams of accounts and send every player `82`. On an error nothing was sent.
pub fn start(cx: &mut Ctx, map: u32, mode: u32, weather: u32, rated: bool, teams: [Vec<AccountId>; 2]) -> Result<u32, String> {
    start_kind(cx, map, mode, weather, if rated { MatchKind::Ranked } else { MatchKind::Random }, teams)
}

/// [`start`] for any [`MatchKind`] (the clan battle of `rooms.rs` uses it).
pub fn start_kind(cx: &mut Ctx, map: u32, mode: u32, weather: u32, kind: MatchKind, teams: [Vec<AccountId>; 2]) -> Result<u32, String> {
    start_with(cx, map, mode, weather, kind, teams, None, None)
}

/// A custom room's Start (rooms.rs): the match on the room's own row (map, mode, variation; the hook must serve it) with
/// the room's AI players when `bots` is set.
pub fn start_room(cx: &mut Ctx, row: &game_data::ModeRow, kind: MatchKind, teams: [Vec<AccountId>; 2], bots: Option<BotFill>) -> Result<u32, String> {
    start_with(cx, row.map, row.mode, row.variation, kind, teams, bots, Some(row))
}

/// The plan of a bot-filled match: the humans of `group` split by [`balance`] over two teams of at most `cap`, then
/// bots up to `cap` on each team. None when no human fits.
pub fn bot_fill_plan(group: &[Ticket], cap: usize) -> Option<([Vec<AccountId>; 2], Vec<u32>, [usize; 2])> {
    let (teams, used) = balance(group, cap);
    if teams[0].is_empty() && teams[1].is_empty() {
        return None;
    }
    let need = [cap - teams[0].len(), cap - teams[1].len()];
    Some((teams, used, need))
}

/// Bot level (CHOICE): the humans' average equipped `match_making_level`. Per human the mean over the items worn in his
/// current profile that carry a level (pocket slots excluded, as the client's equipment level does), then the mean over
/// the humans, rounded, 1..=5; 1 when nobody wears a levelled item.
pub fn bot_level(cx: &Ctx, humans: &[AccountId]) -> u32 {
    let gd = &cx.lobby.data;
    let per: Vec<f64> = humans
        .iter()
        .filter_map(|a| cx.lobby.store.load_profile(*a).ok())
        .filter_map(|p| {
            let lv: Vec<u32> = p
                .profile_slots(p.equip.current)
                .iter()
                .filter(|(s, _)| !crate::items::level_ignores_slot(**s))
                .filter_map(|(_, uid)| p.item(*uid).and_then(|r| gd.items.get(r.dict_id)))
                .map(|it| it.match_making_level)
                .filter(|l| *l > 0)
                .collect();
            (!lv.is_empty()).then(|| lv.iter().sum::<u32>() as f64 / lv.len() as f64)
        })
        .collect();
    if per.is_empty() {
        return 1;
    }
    ((per.iter().sum::<f64>() / per.len() as f64).round() as u32).clamp(1, 5)
}

/// A queue search that waited [`crate::LobbyConfig::online_bot_wait`] without a full group starts a match with the
/// humans that share a playable mode with it (at least one; no minimum of [`queue_start`]) and AI players up to
/// [`BOT_TEAM_MAX`] per team, on any mode the humans ticked. Only unrated searches: ranked stays humans-only
/// (CHOICE). Only when the hook can drive bots. Called from [`tick`].
pub fn queue_bot_fill(cx: &mut Ctx, now_unix: i64) {
    let Some(wait) = cx.lobby.cfg.online_bot_wait else { return };
    if cx.lobby.online.is_none() || cx.registry.queue.is_empty() {
        return;
    }
    let Some(hook) = cx.lobby.online.clone() else { return };
    if !hook.bots_available() || cx.registry.online.bot_retry.is_some_and(|t| cx.now < t) {
        return;
    }
    let due = |g: &[Ticket]| g.iter().map(|t| t.since).min().is_some_and(|s| now_unix - s >= wait.as_secs() as i64);
    // nobody waited long enough: no row scan (this runs every lobby tick)
    if !cx.registry.queue.oldest_since(false).is_some_and(|s| now_unix - s >= wait.as_secs() as i64) {
        return;
    }
    let Ok(qm) = queue_group(cx, 1, true, now_unix) else { return };
    let (third, row_arg) = qm.target();
    let (row, group) = (qm.row.clone(), qm.group.clone());
    if group.is_empty() || group.iter().any(|t| t.rated) || !due(&group) {
        return;
    }
    let (map, mode) = (row.map, row.mode);
    let cap = (match_size(&row) as usize / 2).clamp(1, BOT_TEAM_MAX);
    let Some((teams, used, need)) = bot_fill_plan(&group, cap) else { return };
    let humans: Vec<AccountId> = teams.iter().flatten().copied().collect();
    let level = bot_level(cx, &humans);
    let kind = MatchKind::Random;
    log::info!(
        "queue: {} human(s) waited {} s or more: a match on {} (map {map} mode {mode}) with {} + {} AI players at level {level}",
        humans.len(),
        wait.as_secs(),
        row.project,
        need[0],
        need[1]
    );
    match start_with(cx, map, mode, third, kind, teams, Some(BotFill { need, level, ignore_players: false }), row_arg) {
        Ok(id) => {
            let taken: Vec<Ticket> = group.into_iter().filter(|t| used.contains(&t.order)).collect();
            cx.registry.queue.record_start(&taken, now_unix);
            cx.registry.queue.note_pick(crate::queue::Pick::of(&row));
            cx.registry.online.bot_retry = None;
            log::info!("online match {id:#x} started from the queue with AI players ({} tickets)", taken.len());
        }
        Err(e) => {
            log::warn!("bot-filled match not started: {e}; next try in 10 s");
            cx.registry.online.bot_retry = Some(cx.now + Duration::from_secs(10));
        }
    }
}

/// [`start_kind`] with an optional [`BotFill`] and an optional data row of the PvP queue (its variation and weather:
/// [`OnlineMatches::create_row`]; `map` / `mode` are the row's).
#[allow(clippy::too_many_arguments)]
fn start_with(cx: &mut Ctx, map: u32, mode: u32, weather: u32, kind: MatchKind, teams: [Vec<AccountId>; 2], bots: Option<BotFill>, row: Option<&game_data::ModeRow>) -> Result<u32, String> {
    let hook = cx.lobby.online.clone().ok_or("no match server is attached")?;
    if let Some(r) = row {
        if !hook.serves(&cx.lobby.data, r) {
            return Err(format!("no start world for {} (map {map} mode {mode})", r.project));
        }
    } else if !hook.available(map, mode) {
        return Err(format!("no world is cached for map {map} mode {mode}"));
    }
    // a premium profile that ended is not used for the loadout (profiles.rs); each player's booster cost row is his
    // booster level ([`booster_level`]: the client's own rule)
    for &a in teams.iter().flatten() {
        crate::profiles::ensure_playable(&cx.lobby.store, a);
    }
    let mut seats = vec![];
    for (team, accounts) in teams.iter().enumerate() {
        for &a in accounts {
            if !cx.registry.is_online(a) {
                return Err(format!("account {a} is not online"));
            }
            if cx.registry.online.of.contains_key(&a) {
                return Err(format!("account {a} is still in a match"));
            }
            let mut profile = cx.lobby.store.load_profile(a).map_err(|e| format!("profile of {a}: {e:?}"))?;
            // the per-match boosters: the options must carry exactly the boosters the player will pay for, so the charge is
            // run on this copy now (an unaffordable booster is off in the options) and saved after the match server accepted
            if !kind.is_pve() {
                crate::boosters::charge_profile(&cx.lobby.data, &mut profile, booster_level(cx, a));
            }
            let name = cx.lobby.store.account(a).ok().flatten().map(|x| x.name).unwrap_or_default();
            let session = cx.registry.session_of(a);
            let hero = crate::hero::match_bytes(&profile, crate::premium::unix_now());
            seats.push(Seat { account: a, name, team: team as u8, session, profile, hero });
        }
    }
    // Arms Race (gun race, mode 3): all versus all (docs/notes/gun-race.md). Every seat is its own team, its record index
    // (the options carry the same: local_match::gunrace), so arena.lua's winner (`player_id`, written to the winner
    // byte) is the winner's team; at most eight players and no AI players (arena.lua spawns humans only)
    if mode == game_data::gunrace::MODE {
        if seats.len() > game_data::gunrace::MAX_PLAYERS || bots.is_some() {
            return Err(format!("a gun race match takes 1..={} humans and no AI players", game_data::gunrace::MAX_PLAYERS));
        }
        for (i, s) in seats.iter_mut().enumerate() {
            s.team = i as u8;
        }
    }
    let t = match kind {
        MatchKind::Pve { difficulty, maze } => {
            let project = cx.lobby.data.pve.project(difficulty, maze).ok_or("no map project for this PvE difficulty")?;
            let setup = PveSetup { mission: map, difficulty, project, human_players: seats.len() as u32, equipment_level: 0 };
            hook.create_pve(&cx.lobby.data, &setup, &seats)?
        }
        _ => match (row, bots) {
            (Some(r), fill) => hook.create_row(&cx.lobby.data, r, kind.options_match_type(), &seats, fill)?,
            (None, Some(fill)) => hook.create_with_bots(&cx.lobby.data, map, mode, weather, kind.options_match_type(), &seats, fill)?,
            (None, None) => hook.create(&cx.lobby.data, map, mode, weather, kind.options_match_type(), &seats)?,
        },
    };
    if t.orders.len() != seats.len() {
        return Err("the match server returned a wrong number of tickets".into());
    }
    // every match has its own id (the match server's world seed): a running or ended entry with this id cannot exist
    if cx.registry.online.matches.contains_key(&t.match_id) {
        return Err(format!("the match server returned match id {:#x}, which the lobby still holds", t.match_id));
    }
    let mut states = vec![];
    let mut charges: BTreeMap<AccountId, StartCharge> = BTreeMap::new();
    let mut carried: BTreeMap<AccountId, Vec<u16>> = BTreeMap::new();
    for (s, order) in seats.iter().zip(&t.orders) {
        // the match exists: charge the boosters for real (stock first, else silver), then show the new stock and money
        if !kind.is_pve() {
            let level = booster_level(cx, s.account);
            log::info!("account {}: booster level {level}", s.account);
            let (msgs, taken, active) = crate::boosters::charge_account(cx, s.account, level);
            if !taken.is_empty() {
                charges.insert(s.account, taken);
            }
            if !active.is_empty() {
                carried.insert(s.account, active);
            }
            for msg in msgs {
                push(cx, s.account, msg);
            }
        }
        let connect = ConnectToMatch { host: t.host.clone(), port: t.port, match_id: t.match_id, side: s.team, order_id: *order };
        let msg = connect.message().map_err(|e| e.to_string())?;
        push(cx, s.account, msg);
        cx.registry.set_match_context(s.account, t.match_id, s.team);
        social::announce(cx, s.account, social::ST_INGAME);
        cx.registry.online.of.insert(s.account, t.match_id);
        let (in_squad, in_clan_squad) = squad_flags(&cx.lobby.store, &cx.registry.squads, s.account);
        let squad = cx.registry.squads.squad_of(s.account).filter(|q| q.members.len() > 1).map(|q| q.id);
        let start_mods = crate::achievements::start_mods(&s.profile, crate::premium::unix_now());
        states.push(SeatState { account: s.account, team: s.team, in_squad, in_clan_squad, hero: s.hero, squad, connect, start_mods });
    }
    cx.registry.online.matches.insert(
        t.match_id,
        Running { map, mode, kind, seats: states, reports: BTreeMap::new(), first_report: None, end_asked: false, released: Default::default(), ended: None, created: cx.now, pve_time: None, pve_sent: None, host: None, closed_stats: Default::default(), pve_receipts: BTreeMap::new(), start_charges: charges, boosters: carried, dropped: Default::default() },
    );
    log::info!("online match {:#x}: map {map} mode {mode}, {} players, 82 sent ({}:{})", t.match_id, seats.len(), t.host, t.port);
    Ok(t.match_id)
}

/// `(in_squad, in_clan_squad)` of an account for the match facts. A squad is two or more members; "in a squad with
/// your clanmates" (achievement 418, `is_in_clan_squad`) is read as: the account has a clan and at least one other
/// member of his squad is in the same clan (CHOICE: the data only has the name and the precondition flag).
pub fn squad_flags(store: &accounts::Store, squads: &crate::squads::Squads, account: AccountId) -> (bool, bool) {
    let Some(q) = squads.squad_of(account).filter(|q| q.members.len() > 1) else { return (false, false) };
    let mine = store.clan_id_of(account).ok().flatten();
    let clan = mine.is_some() && q.members.iter().any(|m| m.account != account && store.clan_id_of(m.account).ok().flatten() == mine);
    (true, clan)
}

/// The `53` flush of a player: true when the player is in an online match (the flush is consumed). Events 5 / 6 with
/// `value = match_id` are the player's report; anything else in the flush is ignored.
pub fn on_events(cx: &mut Ctx, account: AccountId, ev: &[Event53]) -> bool {
    let Some(id) = cx.registry.online.of.get(&account).copied() else { return false };
    let hook = cx.lobby.online.clone();
    let Some(m) = cx.registry.online.matches.get_mut(&id) else { return false };
    for e in ev.iter().take(40) {
        if matches!(e.id, 5 | 6) && e.value == id {
            let first = m.reports.insert(account, e.id == 5).is_none();
            if m.first_report.is_none() {
                m.first_report = Some(cx.now);
                // the mission time of the top 100: the match server's clock since the enter, at the first report
                if m.kind.is_pve() {
                    m.pve_time = hook.as_ref().and_then(|h| h.played(id)).map(|d| d.as_secs().min(u64::from(u32::MAX)) as u32);
                }
            }
            if first {
                log::info!("online match {id:#x}: account {account} reports {}", if e.id == 5 { "won" } else { "lost or draw" });
                cross_check(id, account, e.id == 5, m.host.as_ref());
            }
        }
    }
    true
}

/// The clients' `53` 5 / 6 against the world host's outcome: the live cross-check of the two sources.
/// The humans are team 0, so a "won" report means the extraction. A disagreement is logged loudly, nothing else.
fn cross_check(id: u32, account: AccountId, won: bool, host: Option<&HostOutcome>) {
    let Some(h) = host else { return };
    if won != h.success {
        log::error!(
            "online PvE match {id:#x}: HOST AND CLIENT DISAGREE: account {account} reports {}, the world host says success {} (state {}, winner {})",
            if won { "won" } else { "lost" },
            h.success,
            h.state,
            h.winner_team
        );
    }
}

/// `5F 04` (the player left the PvE result screen with "Return to lobby", builder 0x2001973E0): the match's `85 00` waits
/// until every human still in it did so or left ([`PVE_RESULT_SCREEN_CAP`] at most).
pub fn on_close_stats(cx: &mut Ctx, account: AccountId) {
    let Some(id) = cx.registry.online.of.get(&account).copied() else { return };
    let Some(m) = cx.registry.online.matches.get_mut(&id) else { return };
    if m.kind.is_pve() && m.pve_sent.is_some() && m.closed_stats.insert(account) {
        log::info!("online PvE match {id:#x}: account {account} closed the result screen (5F 04)");
    }
}

/// The world host's outcome of a match: for PvE the result goes out at once ([`pve_result`]), the match stays open for
/// the result screen. PvP outcomes are only logged here (the match end carries them).
fn on_host_outcome(cx: &mut Ctx, o: HostOutcome) {
    let id = o.match_id;
    let Some(m) = cx.registry.online.matches.get_mut(&id) else {
        log::warn!("world host outcome of match {id:#x}, which the lobby does not know");
        return;
    };
    if !m.kind.is_pve() {
        // PvP: the match server ends the match from it and its end carries the winner and the counters
        log::info!("online match {id:#x}: world host outcome (winner {}); the match end carries it", o.winner_team);
        return;
    }
    if m.host.is_some() {
        return;
    }
    log::info!(
        "online PvE match {id:#x}: the world host's outcome: success {}, winner {}, state {}, mission {} ms, {} human row(s)",
        o.success,
        o.winner_team,
        o.state,
        o.mission_ms,
        o.players.len()
    );
    for (a, won) in m.reports.iter() {
        cross_check(id, *a, *won, Some(&o));
    }
    if m.pve_sent.is_some() {
        // the clients' reports made the result first (the host came after HOST_OUTCOME_WAIT): only the cross-check
        log::warn!("online PvE match {id:#x}: the world host's outcome came after the result went out from the reports");
        return;
    }
    m.host = Some(o);
    pve_result(cx, id);
}

/// A player who left a running match is free at once: `85 00` (Play works again), his presence back to the lobby.
fn release_left(cx: &mut Ctx, l: &Left) {
    let Some(m) = cx.registry.online.matches.get_mut(&l.match_id) else { return };
    if !m.seats.iter().any(|s| s.account == l.account) || !m.released.insert(l.account) {
        return;
    }
    m.dropped.remove(&l.account);
    log::info!("online match {:#x}: account {} left ({}), released", l.match_id, l.account, if l.by_menu { "menu" } else { "disconnected or silent" });
    let pve = m.kind.is_pve();
    cx.registry.online.of.remove(&l.account);
    cx.registry.clear_match_context(l.account);
    if pve {
        end_solo_pve_squad(cx, l.account, l.match_id);
    }
    push(cx, l.account, S2c::QueueState(QueueState { state: 0 }));
    social::announce(cx, l.account, social::ST_LOBBY);
}

/// Called by the server loop: take ended matches from the hook and pay the ones whose reports are in.
pub fn tick(cx: &mut Ctx) {
    let Some(hook) = cx.lobby.online.clone() else { return };
    // a search that waited long enough gets AI players
    queue_bot_fill(cx, crate::premium::unix_now());
    // a new ranked mode slot can make waiting ranked searches playable (nobody joins to trigger the queue then)
    let now = crate::premium::unix_now();
    if cx.lobby.online.is_some() && cx.registry.queue.ranked_slot_changed(now) {
        let r = queue_start(cx, now);
        log::debug!("queue: ranked mode slot {} (mode {}): {r:?}", now.div_euclid(crate::queue::RANKED_SLOT_SECS), crate::queue::ranked_mode(now));
    }
    // the world host's outcomes first: a PvE result made from them carries the world's counters
    for o in hook.take_outcomes() {
        on_host_outcome(cx, o);
    }
    // a player who left is free at once: Play works again, the queue timer stops (the match goes on for the others)
    for l in hook.take_left() {
        release_left(cx, &l);
    }
    // rejoin: a dropped player stays in the match, unreleased: no `85 00`, so his client keeps its in-match lobby state and
    // shows its own `st_system_message_connection_lost` ("Lost connection to server. Reconnecting ...") when its match
    // connection times out (close_current_match(0) -> 0x2001783C0, shown only while the 85 state is not 0). His seat's
    // `82` goes out again: the 82 handler closes a match still loaded and connects (0x20018E276..0x20018E30B).
    for l in hook.take_dropped() {
        if !offer_rejoin(cx, &*hook, l.match_id, l.account, None, "dropped (silence or disconnect)") {
            // no rejoin after all (the match is ending, the slot is gone): free him as a leaver
            release_left(cx, &l);
        }
    }
    for (id, a) in hook.take_rejoined() {
        if let Some(m) = cx.registry.online.matches.get_mut(&id) {
            if m.dropped.remove(&a) {
                log::info!("online match {id:#x}: account {a} is back in the match (rejoin)");
            }
        }
    }
    for e in hook.take_ended() {
        match cx.registry.online.matches.get_mut(&e.match_id) {
            Some(m) => m.ended = Some((e, cx.now)),
            None => log::warn!("online match {:#x} ended but the lobby does not know it", e.match_id),
        }
    }
    // PvP end screen: the results went out PVP_END_AFTER_RESULT ago: the match server closes the match after its result
    let now0 = cx.now;
    let (due, wait): (Vec<(u32, Instant)>, Vec<(u32, Instant)>) = std::mem::take(&mut cx.registry.online.close_after).into_iter().partition(|(_, at)| now0 >= *at);
    cx.registry.online.close_after = wait;
    // PvP end screen: the 85 00 once the match server let go of the clients (their closing DFs went out and the
    // connections drained), or at the cap
    let pending = std::mem::take(&mut cx.registry.online.release_after);
    for (id, a, cap) in pending {
        let gone = !hook.has_match(id);
        if gone || now0 >= cap {
            log::info!("online match {id:#x}: account {a} released after the end screen ({})", if gone { "the match server let go of the match" } else { "cap" });
            push(cx, a, S2c::QueueState(QueueState { state: 0 }));
            social::announce(cx, a, social::ST_LOBBY);
        } else {
            cx.registry.online.release_after.push((id, a, cap));
        }
    }
    for (id, _) in due {
        let ok = hook.end_after_result(id);
        log::info!("online match {id:#x}: the result went out {PVP_END_AFTER_RESULT:?} ago: the match server closes it after its result: {}", if ok { "ok (code 2 when it handed the result over in the match)" } else { "already closed there" });
    }
    // every seat reported, or the first report is old: the match is over for the clients, end it on the match server (a
    // PvE match first gets its result, and is ended PVE_END_AFTER_RESULT later)
    let now = cx.now;
    let over = |m: &Running| {
        let all = m.seats.iter().all(|s| m.reports.contains_key(&s.account));
        let old = m.first_report.is_some_and(|t| now.saturating_duration_since(t) >= END_AFTER_FIRST_REPORT);
        (all || old).then_some(all)
    };
    // the clients' reports make a PvE result when no world host is in use for the match, or when its outcome did not come
    // within HOST_OUTCOME_WAIT of the first report (the host was lost)
    let pve_due: Vec<u32> = cx
        .registry
        .online
        .matches
        .iter()
        .filter(|(_, m)| m.kind.is_pve() && m.pve_sent.is_none() && m.ended.is_none() && m.host.is_none() && over(m).is_some())
        .filter(|(id, m)| !hook.world_host(**id) || m.first_report.is_some_and(|t| now.saturating_duration_since(t) >= HOST_OUTCOME_WAIT))
        .map(|(id, _)| *id)
        .collect();
    for id in pve_due {
        pve_result(cx, id);
    }
    let to_end: Vec<(u32, bool)> = cx
        .registry
        .online
        .matches
        .iter()
        .filter(|(_, m)| m.ended.is_none() && !m.end_asked && !m.kind.is_pve())
        .filter_map(|(id, m)| over(m).map(|all| (*id, all)))
        .collect();
    for (id, all) in to_end {
        let ok = hook.end(id);
        log::info!("online match {id:#x}: {}, asking the match server to end it: {}", if all { "every player reported" } else { "the first report is 30 s old" }, if ok { "ok" } else { "unknown there" });
        if let Some(m) = cx.registry.online.matches.get_mut(&id) {
            m.end_asked = true;
        }
    }
    // PvE: PVE_END_AFTER_RESULT after the 99 / 83 45 0B (from the world host, the match server's hand-over or the
    // reports) the match server closes the match with DF code 2: the clients keep their ended world and the result
    // screen gets the pointer (docs/notes/pve.md "End screen"). The 85 00 waits for the screen (`screen_left` below).
    let pve_end: Vec<u32> = cx
        .registry
        .online
        .matches
        .iter()
        .filter(|(_, m)| m.kind.is_pve() && m.ended.is_none() && !m.end_asked)
        .filter(|(_, m)| m.pve_sent.is_some_and(|t| now.saturating_duration_since(t) >= PVE_END_AFTER_RESULT))
        .filter(|(_, m)| m.host.is_some() || over(m).is_some())
        .map(|(id, _)| *id)
        .collect();
    for id in pve_end {
        let ok = hook.end_after_result(id);
        log::info!("online PvE match {id:#x}: the result went out {PVE_END_AFTER_RESULT:?} ago: the match server closes it with the result screen on: {}", if ok { "ok" } else { "unknown there" });
        if let Some(m) = cx.registry.online.matches.get_mut(&id) {
            m.end_asked = true;
        }
    }
    let grace = cx.lobby.cfg.online_report_grace;
    // a PvE match closed with its result screen on (the 99 went out before the end): every human still in it left the
    // screen (5F 04) or the match, or PVE_RESULT_SCREEN_CAP since the 99
    let screen_left = |m: &Running| match m.pve_sent {
        Some(sent) if m.kind.is_pve() => {
            m.seats.iter().all(|s| m.released.contains(&s.account) || m.closed_stats.contains(&s.account)) || now.saturating_duration_since(sent) >= PVE_RESULT_SCREEN_CAP
        }
        _ => true,
    };
    let ready: Vec<u32> = cx
        .registry
        .online
        .matches
        .iter()
        .filter_map(|(id, m)| {
            let (e, at) = m.ended.as_ref()?;
            let all = e.players.iter().filter(|p| p.entered).all(|p| m.reports.contains_key(&p.account));
            // a winner from the match server needs no reports (the clients' 5 / 6 only echo the lobby's 97)
            ((e.winner.is_some() || all || cx.now.saturating_duration_since(*at) >= grace) && screen_left(m)).then_some(*id)
        })
        .collect();
    for id in ready {
        finish(cx, id);
    }
}

/// The result of a PvE mission (module docs, "PvE end"): the run is stored (top 100, daily key, achievements, chests:
/// `pve::record_run_notify`, which pushes the `83 45 0B` drops), `99` goes to every player still in the match.
fn pve_result(cx: &mut Ctx, id: u32) {
    pve_result_from(cx, id, None);
}

/// [`pve_result`]. The source of success, time and counters, first that applies:
/// * the world host's outcome ([`HostOutcome`]): success = winner team 0, time = its mission time, per
///   player kills / deaths / score from the world's statistics block, chests found = the sum of the humans' event row 42
///   (`pve_open_chest`; the HUD's `set_collected_chests` sums the same row over the players, 0x2001A8EF0). The `99`
///   fields are the PROVEN layout (reader 0x200180D67); the score goes out bit for bit (the block's u16 in the wire's
///   u16). OPEN until a finished mission is recorded: the success value of a real extraction, the time the screen shows,
///   and that row 42 is the count the HUD shows;
/// * `failed`: the match server's own failure end without a host (CHOICE: `99` with success 0, the same sequence as a
///   success), time = `failed` (seconds);
/// * the clients' reports: success = their majority, time = the match server's clock, counters and chests 0 (no client
///   reports them; never invented).
fn pve_result_from(cx: &mut Ctx, id: u32, failed: Option<u32>) {
    let Some(m) = cx.registry.online.matches.get_mut(&id) else { return };
    let MatchKind::Pve { difficulty, .. } = m.kind else { return };
    // one result per match: every call stores a new run (its own run key, `accounts::pve_run_key`)
    if m.pve_sent.is_some() {
        log::warn!("online PvE match {id:#x}: the result went out before, not sent again");
        return;
    }
    m.pve_sent = Some(cx.now);
    let reports: Vec<(u8, bool)> = m.seats.iter().filter_map(|s| m.reports.get(&s.account).map(|w| (s.team, *w))).collect();
    let since_created = |t: Instant| t.saturating_duration_since(m.created).as_secs().min(u64::from(u32::MAX)) as u32;
    let (success, time_secs, source) = match (&m.host, failed) {
        (Some(h), _) => (h.success, h.mission_ms / 1000, "the world host"),
        (None, Some(t)) => (false, t, "the match server's failure end"),
        // the humans are team 0: success = the majority of their reports says "won" (event 5)
        (None, None) => (decide_winner(&reports) == Verdict::Winner(0), m.pve_time.unwrap_or_else(|| since_created(m.first_report.unwrap_or(cx.now))), "the clients' reports"),
    };
    let host_stat = |a: AccountId| m.host.as_ref().and_then(|h| h.players.iter().find(|p| p.account == a)).copied();
    // without a host: no client reports chests (never invented)
    let chests_found = m.seats.iter().filter_map(|s| host_stat(s.account)).map(|p| u32::from(p.chests)).sum::<u32>().min(255) as u8;
    let started_at = crate::premium::unix_now() - i64::from(since_created(cx.now));
    let seats: Vec<(AccountId, bool)> = m.seats.iter().map(|s| (s.account, m.released.contains(&s.account))).collect();
    let stats: Vec<(u16, u16, u16)> = seats.iter().map(|(a, _)| host_stat(*a).map_or((0, 0, 0), |p| (p.score, p.kills, p.deaths))).collect();
    let spends: Vec<(AccountId, u32, u32)> =
        seats.iter().filter_map(|(a, _)| host_stat(*a).map(|p| (*a, u32::from(p.keys_spent), p.gold_spent))).filter(|x| x.1 > 0 || x.2 > 0).collect();
    let mission = m.map;
    let gd = cx.lobby.data.clone();
    let chests_total = gd.pve.mission(mission).map_or(0, |x| x.chests_total.min(255) as u8);
    let now_unix = crate::premium::unix_now();
    let mut players = Vec::with_capacity(seats.len());
    let mut rows = Vec::with_capacity(seats.len());
    for (i, (a, _)) in seats.iter().enumerate() {
        let premium = cx.lobby.store.load_profile(*a).is_ok_and(|p| p.premium_left(now_unix) > 0);
        // score, kills and deaths live only in the worlds: the host's values, else 0 (never invented)
        let (score, kills, deaths) = stats[i];
        players.push(PveRunPlayer { account: *a, score, kills, deaths, sort: i as u8, premium });
        let name = cx.lobby.store.account(*a).ok().flatten().map(|x| x.name).unwrap_or_default();
        rows.push(PveResultPlayer { name, clan: cx.lobby.store.clan_abbr_of(*a), account: public_id(*a), score: score as i16, kills, deaths, sort: i as u8, premium: u8::from(premium) });
    }
    let run = PveRun { match_id: id, mission, difficulty, success, time_secs, chests_found, chests_total, started_at, players };
    log::info!(
        "online PvE match {id:#x}: result from {source} ({} report(s)): success {success}, mission time {time_secs} s, chests {chests_found}/{chests_total}: 99 and 83 45 0B to the players{}",
        reports.len(),
        if failed.is_some() { String::new() } else { format!(", the match closes in {PVE_END_AFTER_RESULT:?} with the result screen on (85 00 after 5F 04 / leave / {PVE_RESULT_SCREEN_CAP:?})") }
    );
    for (i, (a, released)) in seats.iter().enumerate() {
        if *released {
            continue;
        }
        match (PveResult { match_id: id, time_secs, chests_found, chests_total, success, players: rows.clone(), me: i as u8 }).message() {
            Ok(msg) => push(cx, *a, msg),
            Err(e) => log::error!("99 of match {id:#x}: {e}"),
        }
    }
    // the paid respawns the world charged (only a world host knows them; without one no budget was given): before the
    // run's 83 45 0B, so a 5F 05 cannot spend a key the player already used
    for (a, keys, gold) in spends {
        crate::pve_end::charge_respawns(cx, id, a, keys, gold);
    }
    // the run (under its own run key: the wire id repeats for every run of a maze), the daily keys, the achievements
    // and the end screen's 83 45 0B
    if let Err(e) = crate::pve::record_run_notify(cx, &run, now_unix) {
        log::error!("online PvE match {id:#x}: the run was not stored: {e}");
    }
}

/// The mission script's failure, as the match server computed it without a world: winner 1 (pve.lua action 26
/// `winner_team` 1 of the pre-start cancel, the 45-minute end and "every human left"; the humans are team 0).
fn pve_failed(m: &Running, e: &Ended) -> bool {
    m.kind.is_pve() && e.winner == Some(1)
}

fn finish(cx: &mut Ctx, id: u32) {
    // a PvE match that ended before its result went out (the match server ended it first): the result now. A failure end
    // of the match server (the script's cancel / 45-minute end / everybody left, winner 1) is a failed mission whatever
    // the reports say: `99` with success 0 and `83 45 0B` (CHOICE), unless nobody ever entered.
    let pending = cx.registry.online.matches.get(&id).and_then(|m| {
        let (e, _) = m.ended.as_ref()?;
        if !m.kind.is_pve() || m.pve_sent.is_some() {
            return None;
        }
        if m.host.is_some() {
            Some(None)
        } else if pve_failed(m, e) {
            // OPEN: this `99` comes after the match server's fin DF (the relay closed the clients at its end); whether the
            // client still takes it then (`99` reader gate 0x20018ED84..0x20018EDAE: match client active, running match
            // id loaded) is not traced; a recorded failed mission answers it.
            let entered = e.players.iter().any(|p| p.entered);
            entered.then(|| Some(m.pve_time.unwrap_or(e.duration.as_secs().min(u64::from(u32::MAX)) as u32)))
        } else if !m.reports.is_empty() {
            Some(None)
        } else {
            None
        }
    });
    if let Some(failed) = pending {
        pve_result_from(cx, id, failed);
    }
    let Some(m) = cx.registry.online.matches.remove(&id) else { return };
    let Some((e, _)) = m.ended.clone() else { return };
    // PvP end screen: the clients are still in the match (the result came before the close): their 85 00 waits
    let hold = !m.kind.is_pve() && cx.lobby.online.as_ref().is_some_and(|h| h.holds_clients(id));
    // PvP: after its 83 45 0A + 97 (below) the match server closes the match after its result (the end screen)
    if !m.kind.is_pve() {
        cx.registry.online.close_after.push((id, cx.now + PVP_END_AFTER_RESULT));
        // the bots of the result window (its 38 00 on a bot row): kept per player until his next bot-filled result
        if let Some(w) = e.stats.as_ref().and_then(|(v, _)| v.iter().find_map(|x| x.world.clone())).filter(|w| !w.bots.is_empty()) {
            for s in &m.seats {
                cx.registry.online.result_bots.insert(s.account, (id, w.clone()));
            }
        }
    }
    // nobody ever got into the match (no start world from the world host, nobody loaded): the PvE start's charge goes
    // back, and the clients get their inventory and money again
    if !e.players.iter().any(|p| p.entered) {
        if !m.pve_receipts.is_empty() {
            crate::pve::refund_unplayed(cx, id, &m.pve_receipts);
        }
        if !m.start_charges.is_empty() {
            refund_start_charges(cx, id, &m.start_charges);
        }
    }
    let reports: Vec<(u8, bool)> = m.seats.iter().filter_map(|s| m.reports.get(&s.account).map(|w| (s.team, *w))).collect();
    // the match server's winner (its own computation, or the world host's winner byte `S+0xEBC0`): 0 / 1 = that team,
    // 2 = draw (the tdm rule: both teams empty, or equal tickets, 0x200366B4B..0x200366B96), 3 = none (the statistics
    // core's initial value): the reports then
    let gun = m.mode == game_data::gunrace::MODE;
    let verdict = match e.winner {
        _ if gun => gunrace_verdict(e.winner, &reports),
        Some(w @ 0..=1) => Verdict::Winner(w),
        Some(2) => Verdict::Draw,
        Some(_) | None => decide_winner(&reports),
    };
    log::info!("online match {id:#x} over after {:?}: {} reports, verdict {verdict:?}", e.duration, reports.len());
    let now_unix = crate::premium::unix_now();
    for s in &m.seats {
        cx.registry.online.of.remove(&s.account);
        cx.registry.clear_match_context(s.account);
        let entered = e.players.iter().any(|p| p.account == s.account && p.entered);
        let reported = m.reports.contains_key(&s.account);
        let by_menu = e.players.iter().any(|p| p.account == s.account && p.left_by_menu);
        // a player who never got in, left with the menu or reported nothing is paid nothing (like a local match left early).
        // With the match server's winner (the world host's or its own) no report is needed, but a player who left before
        // the end in any way (menu, disconnect, silence: `released`) is paid nothing either (CHOICE)
        let server_verdict = e.winner.is_some_and(|w| if gun { gunrace_result(w) } else { w <= 2 });
        let dropped = m.dropped.contains(&s.account);
        let left_early = m.released.contains(&s.account) || dropped;
        let paid = entered
            && (reported || server_verdict)
            && !by_menu
            && !(server_verdict && left_early)
            && verdict != Verdict::Unknown
            && !m.kind.is_pve();
        if m.kind.is_pve() {
            // A failed mission (the script's cancel, 45-minute end or everybody left, or the world host's failure) got
            // `99` with success 0 and `83 45 0B` above (CHOICE: the client's own failure screen, HUD
            // `victory = match_result == 1`). Nothing is paid or refunded.
            let why = if m.pve_sent.is_some() && m.host.is_some() {
                "result sent (world host)"
            } else if m.pve_sent.is_some() && pve_failed(&m, &e) {
                "mission failed (match server end, winner 1): 99 success 0 sent"
            } else if m.pve_sent.is_some() {
                "result sent"
            } else {
                "no report, no result"
            };
            log::info!("online PvE match {id:#x} over: account {} is released ({why})", s.account);
            // the premium ammo boxes the world applied in the mission (won or failed): one per spawned profile with a
            // box in slot 11 / 12, from the world host's Used
            let boxes = e
                .stats
                .as_ref()
                .and_then(|(v, _)| v.iter().find(|x| x.account == s.account))
                .and_then(|x| x.used.as_ref())
                .map(|u| crate::charge::boxes(&cx.lobby.data, u))
                .unwrap_or_default();
            take_boxes(cx, id, s.account, &boxes);
        }
        if paid {
            pay(cx, &m, &e, s, verdict, now_unix);
        } else if entered && (by_menu || (!reported && !server_verdict)) && verdict != Verdict::Unknown && matches!(m.kind, MatchKind::Random | MatchKind::Ranked) {
            // he got in and left with the menu, or the others finished and reported and he did not: a leaver (none in
            // custom, clan or PvE matches). With the server's winner only a menu leave is penalised: a disconnect or a
            // silence can be a crash (CHOICE)
            penalize_leaver(cx, s.account, now_unix);
        }
        // a PvP player who got in but is not paid (left early, no verdict) used his premium ammo boxes too: one per box slot
        // (charge::match_boxes); the custom training and the gun race take none (as when paid)
        if entered && !paid && !m.kind.is_pve() && m.kind != MatchKind::Training && m.mode != game_data::gunrace::MODE {
            let used = e.stats.as_ref().and_then(|(v, _)| v.iter().find(|x| x.account == s.account)).and_then(|x| x.used.as_ref());
            let boxes = pvp_box(cx, id, s.account, used);
            take_boxes(cx, id, s.account, &boxes);
        }
        if dropped && entered && !paid && verdict != Verdict::Unknown && !m.kind.is_pve() {
            dropped_result(cx, &m, &e, s, verdict);
        }
        if !m.released.contains(&s.account) {
            if m.kind.is_pve() {
                end_solo_pve_squad(cx, s.account, id);
            }
            // a player still in the match world (not dropped) gets his 85 00 only after the match server let go of it
            if hold && !dropped {
                log::info!("online match {id:#x}: account {} keeps its 85 00 until the match server closed the end screen", s.account);
                cx.registry.online.release_after.push((id, s.account, cx.now + PVP_RELEASE_CAP));
                continue;
            }
            push(cx, s.account, S2c::QueueState(QueueState { state: 0 }));
            social::announce(cx, s.account, social::ST_LOBBY);
        }
    }
}

/// The end of a PvE match for one player: his one-man PvE squad (the PvE window's seat locks, `squads.rs`
/// [`Squads::set_pve_state`](crate::squads::Squads::set_pve_state)) ends with it, and his client gets `AE` with squad id 0
/// before the `85 00`. PROVEN (lobby movie and exe): back in the lobby the client shows the PvE preview, which hides the
/// normal Play panel and offers Cancel only while the player is in no squad (85 handler 0x200175639); a one-man squad
/// would keep him there until a re-login. An `AE` with squad id 0 empties the client's squad (0x20018DC33), so Cancel
/// comes back (either order of `AE` and `85 00`). A real squad of 2 or 3 stays a squad (OPEN: what the original did).
fn end_solo_pve_squad(cx: &mut Ctx, account: AccountId, match_id: u32) {
    if !cx.registry.squads.solo_pve(account) {
        return;
    }
    let changed = cx.registry.squads.remove(account);
    log::info!("online PvE match {match_id:#x}: account {account}'s one-man PvE squad ends with the match (AE squad id 0, then 85 00)");
    crate::squads::notify(cx, changed);
}

/// The client's own queue-block notice (PROVEN 0x200176908): `84 2F` numeric code 0,
/// `a` = 0 -> `st_cant_play_leave` ("You have left a match. You will be able to queue to the next match in: ", every
/// language) with a live mm:ss countdown of `b + 1` seconds (`a` = 1 would be the inactivity text `st_cant_play_afk`).
/// The fine amount has no client key and no number slot: the 8B wallet update shows it.
pub fn queue_block_notice(secs: u32) -> Denied {
    Denied { request: 0x2F, body: DeniedBody::Numeric { code: 0, a: 0, b: secs.saturating_sub(1) } }
}

/// The client's own fine notice (PROVEN, `important_message_arrived` 0x2001C2140, wire type 0x19 at 0x2001C458B): the text
/// is two bracketed integers read by 0x1C4750, `[kind][amount]`; kind 0 = `st_leave_match_fine_received` ("Penalty for
/// leaving match: "), any other = `st_inactivity_fine_received`, then " %d" with the amount (0x20096A584), shown by
/// `root.add_important_message` with Flash type 12; the client answers the read (`51`) itself.
pub const KIND_FINE: u8 = 0x19;

pub fn leave_fine_notice(silver: u32) -> SystemMessage {
    SystemMessage { kind: KIND_FINE, text: format!("[0][{silver}]"), message_id: crate::notices::next_message_id(), ..Default::default() }
}

/// Rejoin: the seat's `82` to a dropped player (at once, or after his new sign-in with the new login session), when the
/// match server still keeps his slot. True = sent.
fn offer_rejoin(cx: &mut Ctx, hook: &dyn OnlineMatches, id: u32, account: AccountId, session: Option<u32>, why: &str) -> bool {
    let Some(m) = cx.registry.online.matches.get_mut(&id) else { return false };
    let Some(seat) = m.seats.iter().find(|s| s.account == account) else { return false };
    if m.released.contains(&account) || m.ended.is_some() || !hook.allow_rejoin(id, account, session) {
        return false;
    }
    let connect = seat.connect.clone();
    m.dropped.insert(account);
    match connect.message() {
        Ok(msg) => {
            log::info!("online match {id:#x}: account {account} {why}: the seat is kept, its 82 goes out again (rejoin)");
            push(cx, account, msg);
            true
        }
        Err(e) => {
            log::error!("online match {id:#x}: account {account}: 82 not encodable: {e}");
            false
        }
    }
}

/// Rejoin after a new sign-in (the game was closed or crashed): an account whose seat is kept gets its `82` once the
/// client sends its first request after the loading messages (the lobby screen is up then).
pub fn offer_rejoin_after_login(cx: &mut Ctx, account: AccountId) {
    let Some(id) = cx.registry.online.of.get(&account).copied() else { return };
    if !cx.registry.online.matches.get(&id).is_some_and(|m| m.dropped.contains(&account)) {
        return;
    }
    let Some(hook) = cx.lobby.online.clone() else { return };
    let session = cx.registry.session_of(account);
    offer_rejoin(cx, &*hook, id, account, session, "signed in again");
}

/// `31` from a player whose seat is kept for a rejoin: he declines it; the slot leaves for good (its leave then releases
/// him with `85 00`). True = consumed. OPEN: whether the client's in-match lobby state offers a button that sends `31`.
pub fn decline_rejoin(cx: &mut Ctx, account: AccountId) -> bool {
    let Some(id) = cx.registry.online.of.get(&account).copied() else { return false };
    if !cx.registry.online.matches.get(&id).is_some_and(|m| m.dropped.contains(&account)) {
        return false;
    }
    let Some(hook) = cx.lobby.online.clone() else { return false };
    let ok = hook.leave_dropped(id, account);
    log::info!("online match {id:#x}: account {account} declines the rejoin (31): {}", if ok { "the slot leaves" } else { "unknown at the match server" });
    ok
}

/// Rejoin: a dropped player who did not come back before the end is paid nothing when the winner is known (CHOICE) but
/// sees the match's results screen (`97`, every reward 0; CHOICE: the client's own screen tells
/// him who won), then `85 00` with the others.
fn dropped_result(cx: &mut Ctx, m: &Running, e: &Ended, s: &SeatState, verdict: Verdict) {
    let Ok(before) = cx.lobby.store.load_profile(s.account) else { return };
    let results97 = results97(cx, m, e, s, verdict, (0, 0, 0), None, &before, &Extra97::default());
    log::info!("online match {:#x}: account {} did not come back before the end: results screen, no reward", e.match_id, s.account);
    let outcome = MatchOutcome { results97, exp: 0, silver: 0, reputation: 0, first_game: false };
    for msg in crate::play::reward_messages(cx, s.account, outcome, Default::default()) {
        push(cx, s.account, msg);
    }
}

/// The leaver penalty (game_data::fines, OFFICIAL 0.56b): silver by the equipment tier (what he has, never below 0) and a
/// queue block that grows with the leaves of the UTC day; shown with 8B and an `84 2F` text.
fn penalize_leaver(cx: &mut Ctx, account: AccountId, now: i64) {
    let gd = &cx.lobby.data;
    let day = now.div_euclid(86_400);
    let res = crate::dispatch::update_account(cx, account, |p| {
        if p.penalty.day != day {
            p.penalty.day = day;
            p.penalty.count = 0;
        }
        let tier = crate::items::equipment_level(gd, p, now);
        let fine = game_data::fines::leaving_fine(tier, p.penalty.count);
        let ban = game_data::fines::ban_secs(p.penalty.count);
        let taken = fine.min(p.silver);
        p.silver -= taken;
        p.penalty.count += 1;
        p.penalty.blocked_until = p.penalty.blocked_until.max(now + i64::from(ban));
        (taken, ban, p.penalty.count, crate::build::money_8b(p))
    });
    if let Some((taken, ban, n, money)) = res {
        log::info!("account {account} left an online match: leave {n} today, fine {taken} silver, queue block {ban} s");
        push(cx, account, S2c::Money(money));
        if taken > 0 {
            push(cx, account, S2c::SystemMessage(leave_fine_notice(taken)));
        }
        if ban > 0 {
            push(cx, account, S2c::Denied(queue_block_notice(ban)));
        }
    }
}

fn pay(cx: &mut Ctx, m: &Running, e: &Ended, s: &SeatState, verdict: Verdict, now_unix: i64) {
    if m.kind == MatchKind::Training {
        // no rewards, no experience / reputation, no progress of any kind (OFFICIAL 0.65b, see MatchKind::Training); the
        // results screen still shows who played and who won, with a zero reward
        log::info!("training match {:#x}: account {} is paid nothing (custom training)", e.match_id, s.account);
        if let Ok(before) = cx.lobby.store.load_profile(s.account) {
            let results97 = results97(cx, m, e, s, verdict, (0, 0, 0), None, &before, &Extra97::default());
            let outcome = MatchOutcome { results97, exp: 0, silver: 0, reputation: 0, first_game: false };
            for msg in crate::play::reward_messages(cx, s.account, outcome, Default::default()) {
                push(cx, s.account, msg);
            }
        }
        return;
    }
    let won = verdict == Verdict::Winner(s.team);
    let stat = e.stats.as_ref().and_then(|(v, _)| v.iter().find(|x| x.account == s.account)).cloned();
    let Ok(before) = cx.lobby.store.load_profile(s.account) else { return };
    let gd = &cx.lobby.data;
    let mut facts = MatchFacts {
        mode: m.mode,
        map: m.map,
        won,
        finished: true,
        faction: before.faction,
        duration_secs: Some(e.duration.as_secs().min(u32::MAX as u64) as u32),
        match_type: Some(m.kind.match_type()),
        in_squad: Some(s.in_squad),
        first_game: !before.claimed(accounts::CLAIM_FIRST_GAME),
        in_clan_squad: Some(s.in_clan_squad),
        clan_battle: matches!(m.kind, MatchKind::Clan(_)),
        ..MatchFacts::default()
    };
    if let Some(x) = &stat {
        facts.deaths = Some(u32::from(x.deaths));
        facts.score = Some(u32::from(x.score));
        // the kill list (weapon, victim, ...) is not known: `kills` stays None, only the counters are
        // the place in his team by score (0 = best), what the ranked rating reads (league::record): teammates with a
        // higher score come first, equal scores share a place
        let all = e.stats.as_ref().map(|(v, _)| v.as_slice()).unwrap_or_default();
        // the gun race is all versus all: the place among every player (score = 10 x race progress, the gunrace
        // events.options rows: rank progress +10, rank loss -10, kills 0)
        let gun = m.mode == game_data::gunrace::MODE;
        let better = m
            .seats
            .iter()
            .filter(|o| (gun || o.team == s.team) && o.account != s.account)
            .filter_map(|o| all.iter().find(|y| y.account == o.account))
            .filter(|y| y.score > x.score)
            .count();
        facts.team_place = Some(better.min(usize::from(u8::MAX)) as u8);
    }
    // the world's own data (world host protocol 3, match_data.rs): event row, kill streak, badges, inactive flag, kills
    let world = stat.as_ref().and_then(|x| x.world.clone());
    let mut extra = Extra97::default();
    if let Some(w) = world.as_deref() {
        let squad: Vec<AccountId> = match s.squad {
            Some(q) => m.seats.iter().filter(|o| o.squad == Some(q)).map(|o| o.account).collect(),
            None => vec![],
        };
        crate::match_data::fill_facts(&mut facts, w, s.account, &squad);
        extra.kinds = w.slot_of(s.account).and_then(|r| w.detail(r.slot)).map(crate::match_data::badge_kinds).unwrap_or_default();
        extra.medals = game_data::badges::MEDALS.map(|t| w.medal_holder(t));
    }
    // the winning team's lead in points (achievements 613-615 / 627): the two team values of the world
    if let (Verdict::Winner(t), Some((_, ts)), false) = (verdict, e.stats.as_ref(), m.mode == game_data::gunrace::MODE) {
        facts.score_diff = Some(ts[usize::from(t & 1)].saturating_sub(ts[usize::from(1 - (t & 1))]));
    }
    // the data's reward (events.options x match level factors + the drop_after_combat silver roll), like a local match; the
    // level is the player's OWN equipment level ([`reward_level`])
    let own_level = crate::items::equipment_level(gd, &before, now_unix);
    // the gun race: "The list of rewards does not depend on your maximum equipment tier" (OFFICIAL, Guns Race notes 0.61b /
    // 0.65d / 0.69b): one level for everybody, [`ONLINE_MATCH_LEVEL`] (CHOICE: the lowest; the events pay no level factor
    // in mode 3 anyway, game_data::progression)
    let level = if m.mode == game_data::gunrace::MODE { ONLINE_MATCH_LEVEL } else { reward_level(own_level) };
    log::info!("online match {:#x}: account {} equipment level {own_level}: reward level {level}, silver factor {:?}", e.match_id, s.account, cx.lobby.cfg.match_silver_factor(level));
    // the events' silver factor is the server's setting (LobbyConfig::match_silver_factors, default above the 0.69d data)
    let silver_factor = cx.lobby.cfg.match_silver_factor(level);
    let (r, parts) = cx.lobby.roll(|rng| gd.match_reward_parts_with(&facts, level, silver_factor, rng));
    // a match that ended before its time: by duration x the 0.62b multiplier (game_data::progression::short_match_factor)
    let k = game_data::progression::short_match_factor(e.duration.as_secs().min(u64::from(u32::MAX)) as u32);
    let short = |v: u32| (f64::from(v) * k).round() as u32;
    let (exp0, silver0, rep0) = (short(r.exp), short(r.silver), short(r.rep));
    if k != 1.0 {
        log::info!("online match {:#x}: lasted {} s, reward factor {k:.3}", e.match_id, e.duration.as_secs());
    }
    // the result window's three reward rows: each `ui_category`'s share of the paid base;
    // the drop_after_combat silver roll has no event row and goes with the third row ("Activity Rewards", CHOICE)
    extra.rows = event_rows(&parts, exp0, silver0, rep0);
    extra.level = level;
    let carried = m.boosters.get(&s.account).map(Vec::as_slice).unwrap_or_default();
    let pparts = crate::boosters::reward_percent_parts(gd, &before, now_unix, carried);
    if !carried.is_empty() {
        log::info!("online match {:#x}: account {} carried boosters {carried:?}: booster bonus {:?} %", e.match_id, s.account, pparts[crate::boosters::BONUS_BOOSTERS]);
    }
    let pct: [u32; 3] = [0, 1, 2].map(|i| pparts.iter().map(|x| x[i]).sum());
    let bonus = |v: u32, p: u32| (v as u64 * p as u64 / 100) as u32;
    let bonus_total = [bonus(exp0, pct[0]), bonus(silver0, pct[1]), bonus(rep0, pct[2])];
    let (mut exp, mut silver, mut rep) = (exp0 + bonus_total[0], silver0 + bonus_total[1], rep0 + bonus_total[2]);
    // the bonus split over its sources (premium, equipment, decals, hero of the day, boosters), each in its own row of
    // the window; the parts add up to the bonus paid
    extra.bonus = bonus_rows(bonus_total, &pparts);
    // the premium row (score block 2) of a player without premium: what the premium account would have added (+50 % of
    // the base). The window draws that row dimmed and its "total premium" = "total" + this row, while "total" (every row
    // but block 2) is what he was paid (AS VictoryScreenPlayerMatchDetails.setData, PREMIUM_COLOR_SECONDARY). Shown,
    // never paid.
    if before.premium_left(now_unix) == 0 {
        extra.bonus[crate::boosters::BONUS_PREMIUM] = crate::premium::match_bonus([exp0, silver0, rep0]);
    }
    // the badge awards: exactly what the results screen computes for them (one profile:
    // trunc(amount x match_levels_factors[level]) per earned instance; no premium, booster, hero or short-match term),
    // paid on top; the screen shows them in its own row, so they stay out of the game-mode component (results97)
    let badge = match world.as_deref().and_then(|w| w.slot_of(s.account).and_then(|r| w.detail(r.slot))) {
        Some(d) => gd.badges.reward(&d.badges, &gd.match_level_factors, level, m.mode),
        None => Default::default(),
    };
    exp += badge.exp;
    silver += badge.silver;
    rep += badge.rep;
    extra.badge = [badge.exp, badge.silver, badge.rep];
    // an inactive player (the world's flag) loses the match reward and the drops; no fine, no kick (the world never
    // removes him); the screen says why (97 `has_inactive_fine`, `83 45 0A` flag 1 = `st_reject_drop_for_inactivity`).
    // CHOICE: no Winner's Contract day either (below), no achievements and no daily quests (stats::record_match)
    let inactive = facts.inactive == Some(true);
    if inactive {
        log::info!("online match {:#x}: account {} was inactive in the world: no reward, no drops", e.match_id, s.account);
        (exp, silver, rep, extra.badge) = (0, 0, 0, [0; 3]);
        (extra.rows, extra.bonus) = Default::default();
        extra.inactive = true;
    }
    // the after-match charge (charge.rs): the equipment use of his equipment level (LobbyConfig, default the 0.55 prices)
    // -> "Match costs", the pocket units of the world host's Used row (protocol 6) -> "Special items"; the 97 shows the
    // lists, the silver above stays gross (the screen subtracts them), the money goes after the reward
    let charge = if inactive {
        log::info!("online match {:#x}: account {} was inactive: nothing charged", e.match_id, s.account);
        Default::default()
    } else if m.mode == game_data::gunrace::MODE {
        // the gun race plays the race kits of the match, not the player's own equipment or pockets
        // (local_match::gunrace): no equipment use, no pocket units (CHOICE)
        log::info!("online match {:#x}: account {} played the gun race kits: nothing charged", e.match_id, s.account);
        Default::default()
    } else {
        let cost = cx.lobby.cfg.equipment_use_cost(gd, own_level);
        log::info!("online match {:#x}: account {} equipment level {own_level}: equipment use {cost} silver", e.match_id, s.account);
        crate::charge::price(gd, cost, stat.as_ref().and_then(|x| x.used.as_ref()))
    };
    extra.spent = charge.spent.clone();
    // the premium ammo boxes the world applied: one per box slot per match (charge::match_boxes), also for an inactive player: the box
    // was used in the world
    let boxes = if m.mode == game_data::gunrace::MODE { Vec::new() } else { pvp_box(cx, e.match_id, s.account, stat.as_ref().and_then(|x| x.used.as_ref())) };
    let league_row = crate::league::results_row(cx, &before, &facts);
    let results97 = results97(cx, m, e, s, verdict, (exp, silver, rep), league_row, &before, &extra);
    let premium = before.premium_left(now_unix) > 0;
    let parts_pct = crate::boosters::parts_percent(gd, &before);
    let mut drops = if inactive { Default::default() } else { cx.lobby.roll(|r| cx.lobby.data.drops.roll_with_parts_bonus(level, won, true, premium, parts_pct, r)) };
    // event currency (events.rs): the place in the team by score, only known with a referee's counters
    let place = stat.as_ref().map(|x| {
        let gun = m.mode == game_data::gunrace::MODE;
        let team = e.stats.iter().flat_map(|(v, _)| v.iter()).filter(|y| m.seats.iter().any(|t| t.account == y.account && (gun || t.team == s.team)));
        game_data::events::place_by_score(x.score, team.map(|y| y.score))
    });
    if !inactive {
        crate::events::add_to_drops(cx, &before, m.mode, won, place, now_unix, &mut drops);
    }
    let outcome = MatchOutcome { results97, exp, silver, reputation: rep, first_game: facts.first_game && !inactive };
    if inactive {
        push(cx, s.account, S2c::Reply(protocol::s2c::Reply::Inventory(protocol::s2c::Inventory45::MatchDrops { flag: 1, items: vec![] })));
    }
    for msg in crate::play::reward_messages(cx, s.account, outcome, drops) {
        push(cx, s.account, msg);
    }
    take_charge(cx, e.match_id, s.account, &charge);
    take_boxes(cx, e.match_id, s.account, &boxes);
    // the Winner's Contract pays the first won online PvP match of each contract day (random, ranked, clan battle; custom
    // training returned above, PvE is never paid here; contract.rs); an inactive player's win does not count
    // (CHOICE)
    if won && !inactive && matches!(m.kind, MatchKind::Random | MatchKind::Ranked | MatchKind::Clan(_)) {
        crate::contract::on_won_match(cx, s.account, now_unix);
    }
    facts.reputation = Some(rep);
    // achievements 415-417: the loadout he played the match with (the profiles he spawned with when the world host sent
    // its Used row, else the start profile of the options)
    let (mods, src) = crate::achievements::match_mods(&s.start_mods, stat.as_ref().and_then(|x| x.used.as_ref()).map(|u| u.profiles));
    log::info!("online match {:#x}: account {} item modifiers {mods} (source {src:?}, start profiles {:?})", e.match_id, s.account, s.start_mods);
    facts.item_mods = Some(mods);
    crate::stats::record_match(cx, s.account, &facts);
}

/// The premium ammo boxes an online PvP match takes from `account` ([`crate::charge::match_boxes`]: one per box slot), from his
/// world host `Used` row; logged either way.
fn pvp_box(cx: &Ctx, match_id: u32, account: AccountId, used: Option<&crate::charge::MatchUse>) -> Vec<(u32, u16, u32)> {
    let Some(u) = used else {
        log::info!("online match {match_id:#x}: account {account}: no Used row from the world host, no premium ammo box taken");
        return vec![];
    };
    let all = crate::charge::boxes(&cx.lobby.data, u);
    let taken = crate::charge::match_boxes(&cx.lobby.data, u);
    if !all.is_empty() {
        log::info!("online match {match_id:#x}: account {account} spawned with premium ammo boxes {all:?} (profiles {:#x}): one per box slot, takes {taken:?}", u.profiles);
    }
    taken
}

/// Take the used premium ammo boxes from their stacks and mirror it: one `83 45 03 <uid>` per box (the client lowers the
/// stack by one, as for the booster stock), then `87` with a new profile version when a stack ran out (its slot 11 / 12
/// is empty now).
fn take_boxes(cx: &mut Ctx, match_id: u32, account: AccountId, boxes: &[(u32, u16, u32)]) {
    if boxes.is_empty() {
        return;
    }
    let gd = &cx.lobby.data;
    let name = cx.lobby.store.account(account).ok().flatten().map(|a| a.name).unwrap_or_default();
    let done = crate::dispatch::update_account(cx, account, |p| {
        let mut taken = Vec::new();
        let mut emptied = false;
        for &(uid, dict, n) in boxes {
            if !p.item(uid).is_some_and(|i| i.dict_id == dict) {
                continue;
            }
            if let Some(c) = p.consume(uid, n) {
                taken.push((uid, dict, c.taken, c.left));
                emptied |= c.left == 0;
            }
        }
        if emptied {
            p.bump_version();
        }
        (taken, emptied.then(|| crate::build::account_87(gd, p, &name)))
    });
    let Some((taken, acct)) = done else { return };
    for &(uid, dict, n, left) in &taken {
        log::info!("online match {match_id:#x}: account {account} used {n} premium ammo box(es) dict {dict} (uid {uid}), {left} left");
        for _ in 0..n {
            push(cx, account, S2c::Reply(protocol::s2c::Reply::Inventory(protocol::s2c::Inventory45::Used { item: uid })));
        }
    }
    if let Some(a) = acct {
        push(cx, account, S2c::Account(a));
    }
}

/// Take the after-match charge from the silver (never below 0; a shortfall is logged) and push `8B`.
fn take_charge(cx: &mut Ctx, match_id: u32, account: AccountId, charge: &crate::charge::Charge) {
    let total = charge.total();
    if total == 0 {
        return;
    }
    let taken = crate::dispatch::update_account(cx, account, |p| {
        let take = total.min(u64::from(p.silver));
        let _ = p.spend(accounts::Currency::Silver, take);
        take
    });
    let Some(taken) = taken else { return };
    log::info!(
        "online match {match_id:#x}: account {account} charged {taken} of {total} silver after the match (match costs {:?}, special items {:?})",
        charge.spent[protocol::s2c::SPENT_REPAIR],
        charge.spent[protocol::s2c::SPENT_POCKETS]
    );
    if taken < total {
        log::warn!("online match {match_id:#x}: account {account} could not pay {} silver of the after-match charge", total - taken);
    }
    if let Ok(p) = cx.lobby.store.load_profile(account) {
        push(cx, account, S2c::Money(crate::build::money_8b(&p)));
    }
}

/// The level the reward tables (`match_levels_factors`, `drop_after_combat`, the badge awards) are indexed by for one
/// player: his OWN equipment level (`items::equipment_level`, the client's 0x2003330C0; the number the repairs charge
/// uses), clamped to the tables' 1..=5 (nothing worn = [`ONLINE_MATCH_LEVEL`]). OFFICIAL: the Guns Race notes 0.61b /
/// 0.65d / 0.69b say its rewards do not depend on the equipment tier (so the other modes do), and the developers
/// (2022-01-27) tie rewards to the tier played; the `97` carries a level per profile (R +0x2E).
pub(crate) fn reward_level(own: u8) -> u32 {
    if own == 0 {
        ONLINE_MATCH_LEVEL
    } else {
        u32::from(own).clamp(1, 5)
    }
}

/// The booster cost row of a player at the match start, as the client prices the boosters it shows
/// (`booster_participation_costs` 0x2001BF460; levels from 0x2003330C0, lobby bytes +0x39C / +0x3A0 written by
/// 0x2001C75CF, the AE `items_level`): the highest `items::equipment_level` of him and his squad; the level is the row
/// itself (`Vouchers::match_cost`).
pub(crate) fn booster_level(cx: &Ctx, account: AccountId) -> u32 {
    let squad: Vec<AccountId> = cx.registry.squads.squad_of(account).map(|q| q.members.iter().map(|m| m.account).collect()).unwrap_or_default();
    equip_level_of(cx, std::iter::once(account).chain(squad)).map_or(0, u32::from)
}

fn equip_level_of(cx: &Ctx, accounts: impl Iterator<Item = AccountId>) -> Option<u8> {
    // the players' own equipment levels as the client computes them (items::equipment_level: worn items of the playable
    // profiles; pockets and empty stacks do not count); None when nobody wears anything
    let now = crate::premium::unix_now();
    accounts
        .filter_map(|a| cx.lobby.store.load_profile(a).ok())
        .map(|p| crate::items::equipment_level(&cx.lobby.data, &p, now))
        .filter(|l| *l > 0)
        .max()
}

/// What the world's own data adds to a player's `97` (world host protocol 3, [`crate::match_data`]).
#[derive(Debug, Clone, Default)]
struct Extra97 {
    /// the receiver's badge counts `(type, [state 1, 2, 3])` (R +0x560)
    kinds: Vec<(u8, [u8; 3])>,
    /// the holders of the post-match medals 23..26 (header strings)
    medals: [Option<AccountId>; 4],
    /// `[experience, silver, reputation]` of the badge awards inside the totals (kept out of the game-mode component: the
    /// screen shows them in their own row)
    badge: [u32; 3],
    /// the world's inactive flag: no reward (R +0x92E `has_inactive_fine`)
    inactive: bool,
    /// the after-match charge's lists: the `97` tail `spent_for_autobuy_money` ([`crate::charge`])
    spent: [Vec<protocol::s2c::Spent>; 3],
    /// the three reward rows (R +0x38 `score_components`), one per `ui_category`
    rows: [Row97; 3],
    /// the bonus rows `[experience, silver, reputation]` per source, [`crate::boosters::BONUS_PREMIUM`] ..
    bonus: [[u32; 3]; 5],
    /// the receiver's reward level ([`reward_level`]): R +0x2E of his profile and the header's match level
    level: u32,
}

/// One reward row of the `97`: the paying events' count and what they paid.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Row97 {
    amount: u32,
    money: u32,
    exp: u32,
    rep: u32,
}

/// The three reward rows: the short-match scaled base `(exp0, silver0, rep0)` split by the categories' shares
/// (`game_data::progression::apportion`), the silver roll with the third row. The rows add up to the base exactly.
fn event_rows(parts: &game_data::progression::RewardParts, exp0: u32, silver0: u32, rep0: u32) -> [Row97; 3] {
    use game_data::progression::apportion;
    let w = |f: fn(&game_data::progression::Reward) -> u32| parts.rows.map(|r| i64::from(f(&r)));
    let ex = apportion(exp0, &w(|r| r.exp));
    let rp = apportion(rep0, &w(|r| r.rep));
    let s = w(|r| r.silver);
    let si = apportion(silver0, &[s[0], s[1], s[2], i64::from(parts.drop_silver)]);
    [0, 1, 2].map(|c| Row97 { amount: parts.amounts[c], money: si[c] + if c == 2 { si[3] } else { 0 }, exp: ex[c], rep: rp[c] })
}

/// The bonus `[experience, silver, reputation]` split over its sources by their percentages (adds up exactly).
fn bonus_rows(total: [u32; 3], pct: &[[u32; 3]; 5]) -> [[u32; 3]; 5] {
    let mut out = [[0u32; 3]; 5];
    for i in 0..3 {
        let w: Vec<i64> = pct.iter().map(|p| i64::from(p[i])).collect();
        // no source (cannot happen with a bonus): nothing is lost, the last row takes it
        for (j, v) in game_data::progression::apportion(total[i], &w).into_iter().enumerate() {
            out[j][i] = v;
        }
    }
    out
}

/// The `97` screen of an online match (docs/notes/online.md section 9): who played (names,
/// public ids, teams), who won (the verdict of the clients' reports), the paid amounts, and the numbers a source
/// delivered. Kills, deaths, score and the team scores are what [`Ended::stats`] holds; without a source they are 0
/// (never invented), as they are for a player the source did not list. The client's lobby turns a `97` into a clickable
/// match notification (`LobbyMenu.fill_victory_screen`); the window opens on its click.
///
/// * Rows: the humans, then a bot-filled match's AI players ([`crate::match_data::BotRow`]) with ids 1.. (the window
///   treats ids below `2 * players + 2` as bots, no report / friend menu) and their name in the receiver's language
///   (the window shows the row name raw: 0x2001412BB..0x2001412DB; the real server's sample lists localized bot names).
///   Each human row's tail carries the title `[hero_place, nomination, 0, 0]` (none = `0, 255`).
/// * The personal tab: the three reward rows (R +0x38, `score_components`: name = the client key the exe localizes,
///   a show-amount byte, per profile `f32 amount` + `u32 money, u16 experience, u16 reputation`) carry the match's base
///   reward per `ui_category` (the sample: kills row with amount = kills, help row, actions row without amount); the
///   bonus rows are the score blocks 1 equipment, 2 premium, 3 boosters, 8 decals, 9 hero of the day; the badge row is
///   the client's own computation from R +0x560 (paid as [`Extra97::badge`]). Block 10 `game_mode` is not drawn by the
///   window (VictoryScreenPlayerMatchDetails.setData reads no `game_mode_*` key): 0, as in the sample. The window's
///   "total" is every row but the premium one, "total premium" adds it: a premium player was paid "total premium" (block
///   2 = his premium share), any other player "total" (block 2 = what premium would have added, drawn dimmed). Every
///   per-profile value sits at the receiver's current equipment profile, whose play time (R +0x1C) is the match length.
/// * A ranked match sets header `b3` = 3 (`rating_match`) and `league` fills the league progress row
///   ([`crate::league::results_row`]).
#[allow(clippy::too_many_arguments)]
fn results97(
    cx: &Ctx,
    m: &Running,
    e: &Ended,
    me: &SeatState,
    verdict: Verdict,
    (exp, silver, rep): (u32, u32, u32),
    league: Option<(u8, u16, u8, i8)>,
    before: &Profile,
    extra: &Extra97,
) -> Option<Vec<u8>> {
    use protocol::s2c::ScoreComponent;
    let stats = e.stats.as_ref().map(|(v, _)| v.as_slice()).unwrap_or(&[]);
    let team_scores = e.stats.as_ref().map(|(_, t)| *t).unwrap_or([0, 0]);
    let mut players = vec![];
    for s in &m.seats {
        let st = stats.iter().find(|x| x.account == s.account).cloned().unwrap_or(EndedStat { account: s.account, ..Default::default() });
        let name = cx.lobby.store.account(s.account).ok().flatten().map(|a| a.name).unwrap_or_default();
        let mut row = ResultPlayer::new(&name, public_id(s.account), i16::try_from(st.score).unwrap_or(i16::MAX), st.kills, st.deaths, s.team);
        row.tail = match s.hero {
            Some((place, nomination)) => [place, nomination, 0, 0],
            None => [0, 255, 0, 0],
        };
        players.push(row);
    }
    // the AI players of the match (the world's roster; one shared value for every player of the match)
    let language = cx.registry.language(me.account);
    if let Some(w) = stats.iter().find_map(|x| x.world.clone()) {
        for (k, b) in w.bots.iter().enumerate() {
            let fallback = || format!("Bot {}", k + 1);
            let name = b.names.and_then(|(f, s)| cx.lobby.data.bot_names.name(usize::from(f), usize::from(s), language)).unwrap_or_else(fallback);
            let mut row = ResultPlayer::new(&name, k as u64 + 1, i16::try_from(b.score).unwrap_or(i16::MAX), b.kills, b.deaths, b.team);
            row.tail = [0, 255, 0, 0];
            players.push(row);
        }
    }
    players.truncate(protocol::s2c::MAX_RESULT_PLAYERS);
    // the place byte: rows with a higher score, whole table, ties share (protocol::s2c::set_places, PROVEN)
    protocol::s2c::set_places(&mut players);
    let p = usize::from(before.equip.current).min(8);
    let mut full = protocol::s2c::ResultFull {
        my_id: public_id(me.account),
        exp_total: before.xp.wrapping_add(exp),
        exp_delta: exp as u16,
        rep_total: before.points(before.faction).wrapping_add(rep),
        rep_delta: rep as u16,
        faction: before.faction,
        // the victory splash's silver: the whole credited amount, bonuses and badge awards included (0x2001ACCFC)
        f14: silver,
        ..Default::default()
    };
    // the three reward rows (VictoryScreenPlayerMatchDetails rows 1..3); the amount is shown for the first two
    for (c, r) in extra.rows.iter().enumerate() {
        let a = &mut full.awards[c];
        a.name = game_data::result_texts::ROW_LABELS[c].to_string();
        a.flag = u8::from(c < 2);
        // the per-profile triple is on the wire only after a non-zero f32 (reader 0x200181C90..0x200181CEB)
        let carries = r.money != 0 || r.exp != 0 || r.rep != 0;
        a.values[p] = if r.amount > 0 { r.amount as f32 } else if carries { 1.0 } else { 0.0 };
        a.extras[p] = (r.money, r.exp.min(u32::from(u16::MAX)) as u16, r.rep.min(u32::from(u16::MAX)) as u16);
    }
    // the bonus rows: score block -> source
    use crate::boosters::{BONUS_BOOSTERS, BONUS_DECALS, BONUS_HERO, BONUS_ITEMS, BONUS_PREMIUM};
    for (block, src) in [(1, BONUS_ITEMS), (2, BONUS_PREMIUM), (3, BONUS_BOOSTERS), (8, BONUS_DECALS), (9, BONUS_HERO)] {
        let [x, s, r] = extra.bonus[src];
        full.score[block][p] = ScoreComponent { money: s, rep: r.min(u32::from(u16::MAX)) as u16, exp: x.min(u32::from(u16::MAX)) as u16 };
    }
    if let Some((f928, progress, stars, delta)) = league {
        (full.f928, full.stars_progress, full.stars, full.stars_delta) = (f928, progress, stars, delta);
    }
    // the play time and match level of the profile played (R +0x1C / +0x2E): the profile column's time and the weight
    // and level factor of the client's badge amounts
    full.counts[p] = e.duration.as_secs().min(u64::from(u16::MAX)) as u16;
    full.bytes[p] = extra.level.min(255) as u8;
    // the receiver's badges: R +0x560 = his slot counts
    full.kinds = extra.kinds.clone();
    // has_inactive_fine (R +0x92E, read by fill_match_statistic 0x20013EF50): the screen's inactivity line
    full.flag92e = u8::from(extra.inactive);
    // the four post-match medals in the header, (name, clan) per medal in the order 23 / 24 / 26 / 25 (best player,
    // shooter, hero, killer): byte-equal to the winner's row (rows carry no clan tag)
    let mut texts: [String; 8] = Default::default();
    for (k, t) in [0usize, 1, 3, 2].into_iter().enumerate() {
        if let Some(a) = extra.medals[t] {
            texts[2 * k] = cx.lobby.store.account(a).ok().flatten().map(|x| x.name).unwrap_or_default();
        }
    }
    let msg = MatchResult {
        match_id: e.match_id,
        map_id: m.map,
        // the client's own convention for the winner byte: 0 / 1 = team, 2 = draw (online_results 1, LIKELY for 97)
        winner_team: match verdict {
            Verdict::Winner(t) => t,
            _ => 2,
        },
        // the screen draws the u8 scores (a score above 255 shows 255); the u32 params stay 0
        team0_score: team_scores[0].min(255) as u8,
        team1_score: team_scores[1].min(255) as u8,
        // the receiver's own reward level (each 97 is personal; CHOICE, the window reads the level per profile at R +0x2E)
        match_level: extra.level.min(255) as u8,
        mode: m.mode as u8,
        // 3 = a rating match (0x200142749: the league rows; the shown hero place is 0)
        b3: if m.kind.rated() { 3 } else { 0 },
        texts,
        players,
        full,
        spent: extra.spent.clone(),
        ..Default::default()
    };
    msg.to_bytes().ok()
}

/// Players of a match on `row`: `max_players`; 0 = not stated (most `match_making_params` have only `min_players`), then
/// the row's minimum (as the training options do), else 20.
pub(crate) fn match_size(row: &game_data::matches::ModeRow) -> u32 {
    if row.max_players > 0 {
        row.max_players
    } else if row.min_players > 0 {
        row.min_players
    } else {
        20
    }
}

/// Shared handle of the hook (the lobby keeps one).
pub type Hook = Arc<dyn OnlineMatches>;

#[cfg(test)]
mod tests {
    use super::*;

    fn t(order: u32, accounts: &[i64]) -> Ticket {
        Ticket { order, accounts: accounts.to_vec(), mask: 0, rated: false, since: 0 }
    }

    #[test]
    fn a_squad_with_a_clanmate_is_a_clan_squad() {
        let dir = tempfile::tempdir().unwrap();
        let store = accounts::Store::open(&dir.path().join("s.sqlite")).unwrap();
        store.set_hash_cost(accounts::HashCost::FAST_FOR_TESTS);
        let ids: Vec<AccountId> = ["amy", "ben", "cat", "dan"].iter().map(|n| store.create_account(n, "pw").unwrap()).collect();
        let mut squads = crate::squads::Squads::default();
        // nobody has a clan: a squad of two is a squad, not a clan squad
        let squad = |squads: &mut crate::squads::Squads, a: AccountId, b: AccountId| {
            squads.record_invite(a, b, 0);
            squads.accept_invite(a, b);
            squads.join(a, b).unwrap();
        };
        squad(&mut squads, ids[0], ids[1]);
        assert_eq!(squad_flags(&store, &squads, ids[0]), (true, false));
        assert_eq!(squad_flags(&store, &squads, ids[2]), (false, false), "no squad");
        // amy founds a clan and takes ben in: the squad has a clanmate
        store.reserve_clan(ids[0], "Iron Wolves", "IW").unwrap();
        store.mark_clan_registration_paid(ids[0]).unwrap();
        let clan = store.create_clan(ids[0]).unwrap();
        store.clan_invite(ids[0], ids[1]).unwrap();
        store.clan_accept(ids[1], clan).unwrap();
        assert_eq!(squad_flags(&store, &squads, ids[0]), (true, true));
        assert_eq!(squad_flags(&store, &squads, ids[1]), (true, true));
        // cat is in the squad but not in the clan: for cat there is no clanmate, for amy there still is
        squad(&mut squads, ids[0], ids[2]);
        assert_eq!(squad_flags(&store, &squads, ids[2]), (true, false));
        assert_eq!(squad_flags(&store, &squads, ids[0]), (true, true));
    }

    #[test]
    fn verdict_rules() {
        // both teams agree: 0 won (team 0 says 5, team 1 says 6)
        assert_eq!(decide_winner(&[(0, true), (0, true), (1, false), (1, false)]), Verdict::Winner(0));
        assert_eq!(decide_winner(&[(0, false), (1, true)]), Verdict::Winner(1));
        // a draw is 6 from both sides
        assert_eq!(decide_winner(&[(0, false), (1, false), (1, false)]), Verdict::Draw);
        // both claim the win: no consensus
        assert_eq!(decide_winner(&[(0, true), (1, true)]), Verdict::Unknown);
        // one cheater inside a team is outvoted
        assert_eq!(decide_winner(&[(0, true), (0, true), (0, false), (1, false), (1, false)]), Verdict::Winner(0));
        assert_eq!(decide_winner(&[(0, true), (0, false), (0, false), (1, false), (1, false)]), Verdict::Draw);
        // only the losing side reported, or nobody: unknown
        assert_eq!(decide_winner(&[(1, false)]), Verdict::Unknown);
        assert_eq!(decide_winner(&[]), Verdict::Unknown);
        // only the winning side reported: accepted (the other team left without a report)
        assert_eq!(decide_winner(&[(0, true)]), Verdict::Winner(0));
    }

    #[test]
    fn balance_keeps_tickets_whole_and_teams_even() {
        let tickets = [t(1, &[1]), t(2, &[2, 3]), t(3, &[4]), t(4, &[5, 6, 7])];
        let (teams, used) = balance(&tickets, 4);
        assert_eq!(used.len(), 4);
        // largest first: 5,6,7 -> team 0; 2,3 -> team 1; singles fill the smaller team
        assert_eq!(teams[0].iter().filter(|a| [5, 6, 7].contains(*a)).count(), 3);
        assert!(teams[1].contains(&2) && teams[1].contains(&3));
        assert!(teams[0].len().abs_diff(teams[1].len()) <= 1);
        // a ticket that fits in neither team is left out
        let (teams, used) = balance(&[t(1, &[1, 2, 3]), t(2, &[4, 5, 6])], 2);
        assert!(teams[0].is_empty() && teams[1].is_empty() && used.is_empty());
    }
}
