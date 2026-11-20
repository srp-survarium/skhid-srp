//! The match history in srp's SQLite file (`docs/notes/match_history.md`):
//! every finished match ([`MatchRecord`], made by
//! [`crate::v026g::rewards::Results::settle`]) and its players, with the
//! counted public matches' totals by player and by clan added up as each is
//! written. [`MatchHistory`] reads them for the site, on read-only
//! connections of its own: only public matches are ever served.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard, TryLockError};

use rusqlite::{Connection, OpenFlags, OptionalExtension, Row, Transaction, params};
use survarium::v026g::maps::MapId;
use survarium::v026g::twist::LobbyMode;

use crate::LockExt;
use crate::persist::PersistError;
use crate::v026g::account::level_at;
use crate::v026g::clans::{self, status};
use crate::v026g::replays::ReplayId;

/// The schema step that adds the history. A database of before has no
/// matches: the history starts with the first match after it.
pub(super) const MIGRATION: &str = "
    CREATE TABLE g26_matches (
        -- The history's own number, counting up: the site's match id.
        id            INTEGER PRIMARY KEY,
        -- The match server's id: random, it can come again after a restart.
        game_match_id INTEGER NOT NULL,
        -- That id with the second the match was made (`ReplayId`): its
        -- recording's file stem, one match's alone.
        replay_id     TEXT NOT NULL UNIQUE,
        -- Unix seconds.
        ended_at      INTEGER NOT NULL,
        duration_ms   INTEGER NOT NULL,
        -- maps_dict's id.
        map_id        INTEGER NOT NULL,
        -- Its lobby number (`LobbyMode::number`): a mode's, or a
        -- free-for-all's twist's.
        game_mode     INTEGER NOT NULL,
        match_level   INTEGER NOT NULL,
        kind          TEXT NOT NULL CHECK (kind IN ('public', 'private', 'vs_bots')),
        -- A public match's: whether it counts in the totals and top lists
        -- (`Eligibility`); none for another kind.
        eligibility   TEXT CHECK (eligibility IN
            ('counted', 'with_bots', 'few_players', 'short', 'no_result')),
        -- `most_kills`: a free-for-all, which no team wins.
        winner        TEXT NOT NULL
            CHECK (winner IN ('team0', 'team1', 'draw', 'unknown', 'most_kills')),
        team0_points  INTEGER NOT NULL,
        team1_points  INTEGER NOT NULL,
        CHECK ((kind = 'public') = (eligibility IS NOT NULL))
    );
    CREATE INDEX g26_matches_public ON g26_matches (id) WHERE kind = 'public';
    CREATE INDEX g26_matches_by_end ON g26_matches (ended_at);
    -- One row a player who was in the match; no bot.
    CREATE TABLE g26_match_players (
        match_id      INTEGER NOT NULL REFERENCES g26_matches(id) ON DELETE CASCADE,
        account_id    INTEGER NOT NULL,
        -- The registered clan it was of then, if any.
        clan_id       INTEGER,
        team          INTEGER NOT NULL,
        -- In its team by score, from 1; of a free-for-all among everyone,
        -- by kills.
        place         INTEGER NOT NULL,
        -- Its place in the match's roster.
        seat          INTEGER NOT NULL,
        presence      TEXT NOT NULL CHECK (presence IN ('finished', 'left')),
        outcome       TEXT NOT NULL CHECK (outcome IN ('won', 'lost', 'neither')),
        rating        INTEGER NOT NULL,
        score         INTEGER NOT NULL,
        kills         INTEGER NOT NULL,
        deaths        INTEGER NOT NULL,
        assists       INTEGER NOT NULL,
        headshots     INTEGER NOT NULL,
        grenade_kills INTEGER NOT NULL,
        melee_kills   INTEGER NOT NULL,
        batteries     INTEGER NOT NULL,
        -- Research's captures, in hundredths of one.
        captures      INTEGER NOT NULL,
        -- Artefacts taken from an anomaly.
        artefacts     INTEGER NOT NULL,
        -- Bullets fired and bullets that hit (events 22 and 23).
        shots         INTEGER NOT NULL,
        hits          INTEGER NOT NULL,
        -- How long it was in the match.
        played_ms     INTEGER NOT NULL,
        PRIMARY KEY (match_id, account_id)
    ) WITHOUT ROWID;
    CREATE INDEX g26_match_players_by_account ON g26_match_players (account_id, match_id);
    CREATE INDEX g26_match_players_by_clan ON g26_match_players (clan_id, match_id)
        WHERE clan_id IS NOT NULL;
    -- The counted public matches added up, by player and by clan; a player
    -- of public matches none of which counted has a row of zeros.
    CREATE TABLE g26_player_totals (
        account_id    INTEGER PRIMARY KEY,
        matches       INTEGER NOT NULL,
        wins          INTEGER NOT NULL,
        losses        INTEGER NOT NULL,
        score         INTEGER NOT NULL,
        kills         INTEGER NOT NULL,
        deaths        INTEGER NOT NULL,
        assists       INTEGER NOT NULL,
        headshots     INTEGER NOT NULL,
        grenade_kills INTEGER NOT NULL,
        melee_kills   INTEGER NOT NULL,
        batteries     INTEGER NOT NULL,
        captures      INTEGER NOT NULL,
        artefacts     INTEGER NOT NULL,
        shots         INTEGER NOT NULL,
        hits          INTEGER NOT NULL,
        played_ms     INTEGER NOT NULL
    );
    CREATE INDEX g26_player_totals_matches ON g26_player_totals (matches);
    CREATE INDEX g26_player_totals_wins ON g26_player_totals (wins);
    CREATE INDEX g26_player_totals_kills ON g26_player_totals (kills);
    -- The average score top list: the players with at least 20 matches
    -- (`SCORE_AVG_MIN_MATCHES`, the same number).
    CREATE INDEX g26_player_totals_score_avg ON g26_player_totals (CAST(score AS REAL) / matches)
        WHERE matches >= 20;
    -- The accuracy top list: the players with at least 1000 shots
    -- (`ACCURACY_MIN_SHOTS`, the same number).
    CREATE INDEX g26_player_totals_accuracy ON g26_player_totals (CAST(hits AS REAL) / shots)
        WHERE shots >= 1000;
    -- A match counts once for a clan, however many of its members played:
    -- a win if members won it and none lost it; score, kills and the
    -- counters are the members' rows added up.
    CREATE TABLE g26_clan_totals (
        clan_id       INTEGER PRIMARY KEY,
        matches       INTEGER NOT NULL,
        wins          INTEGER NOT NULL,
        losses        INTEGER NOT NULL,
        score         INTEGER NOT NULL,
        kills         INTEGER NOT NULL,
        deaths        INTEGER NOT NULL,
        assists       INTEGER NOT NULL,
        headshots     INTEGER NOT NULL,
        grenade_kills INTEGER NOT NULL,
        melee_kills   INTEGER NOT NULL,
        batteries     INTEGER NOT NULL,
        captures      INTEGER NOT NULL,
        artefacts     INTEGER NOT NULL,
        shots         INTEGER NOT NULL,
        hits          INTEGER NOT NULL,
        played_ms     INTEGER NOT NULL
    );
    -- The kits a player used in a match ([`KitLine`]; no bot): one row a
    -- kit, with that kit's own counters.
    CREATE TABLE g26_match_kits (
        match_id      INTEGER NOT NULL REFERENCES g26_matches(id) ON DELETE CASCADE,
        account_id    INTEGER NOT NULL,
        -- Its place among the account's kits: the counters' kit.
        kit           INTEGER NOT NULL CHECK (kit BETWEEN 0 AND 2),
        kills         INTEGER NOT NULL,
        headshots     INTEGER NOT NULL,
        shots         INTEGER NOT NULL,
        hits          INTEGER NOT NULL,
        PRIMARY KEY (match_id, account_id, kit)
    ) WITHOUT ROWID;
    -- What a kit of `g26_match_kits` held: its weapons (profile slots 7
    -- and 10) and its armour (0..=6), by items_dict id.
    CREATE TABLE g26_match_kit_items (
        match_id      INTEGER NOT NULL REFERENCES g26_matches(id) ON DELETE CASCADE,
        account_id    INTEGER NOT NULL,
        kit           INTEGER NOT NULL,
        slot          INTEGER NOT NULL CHECK (slot BETWEEN 0 AND 7 OR slot = 10),
        dict_id       INTEGER NOT NULL,
        PRIMARY KEY (match_id, account_id, kit, slot)
    ) WITHOUT ROWID;
    -- The counted public matches in which a kit the player used held the
    -- item.
    CREATE TABLE g26_player_items (
        account_id    INTEGER NOT NULL,
        dict_id       INTEGER NOT NULL,
        gear          TEXT NOT NULL CHECK (gear IN ('weapon', 'armour')),
        matches       INTEGER NOT NULL,
        PRIMARY KEY (account_id, dict_id)
    ) WITHOUT ROWID;
    CREATE INDEX g26_player_items_by_matches ON g26_player_items (account_id, gear, matches);
    -- What the site looks a player up by, and a player's clan.
    CREATE INDEX g26_accounts_by_name ON g26_accounts (name COLLATE NOCASE);
    CREATE INDEX g26_accounts_by_rating ON g26_accounts (rating);
    CREATE INDEX g26_clan_members_by_account ON g26_clan_members (account_id);
    ";

/// The schema step that adds what a player did with each weapon, as the
/// games count it (exe patch 165). A match of before has none.
pub(super) const WEAPONS_MIGRATION: &str = "
    -- One row a weapon a player's games counted something of in the match
    -- ([`WeaponCounts`]), by items_dict id; 0: no weapon known.
    CREATE TABLE g26_match_weapons (
        match_id         INTEGER NOT NULL REFERENCES g26_matches(id) ON DELETE CASCADE,
        account_id       INTEGER NOT NULL,
        dict_id          INTEGER NOT NULL,
        kills            INTEGER NOT NULL,
        headshots        INTEGER NOT NULL,
        wall_kills       INTEGER NOT NULL,
        ricochet_kills   INTEGER NOT NULL,
        -- With it in the hands.
        deaths           INTEGER NOT NULL,
        shots            INTEGER NOT NULL,
        hits             INTEGER NOT NULL,
        -- In ten-thousandths of a pain pool.
        damage           INTEGER NOT NULL,
        -- The kills' distances added up, and the longest, in decimetres.
        kill_distance_dm INTEGER NOT NULL,
        longest_kill_dm  INTEGER NOT NULL,
        held_ms          INTEGER NOT NULL,
        PRIMARY KEY (match_id, account_id, dict_id)
    ) WITHOUT ROWID;
    -- The counted public matches added up by weapon (the longest kill: the
    -- longest of them); `matches`: those with a row of it.
    CREATE TABLE g26_player_weapons (
        account_id       INTEGER NOT NULL,
        dict_id          INTEGER NOT NULL,
        matches          INTEGER NOT NULL,
        kills            INTEGER NOT NULL,
        headshots        INTEGER NOT NULL,
        wall_kills       INTEGER NOT NULL,
        ricochet_kills   INTEGER NOT NULL,
        deaths           INTEGER NOT NULL,
        shots            INTEGER NOT NULL,
        hits             INTEGER NOT NULL,
        damage           INTEGER NOT NULL,
        kill_distance_dm INTEGER NOT NULL,
        longest_kill_dm  INTEGER NOT NULL,
        held_ms          INTEGER NOT NULL,
        PRIMARY KEY (account_id, dict_id)
    ) WITHOUT ROWID;
    ";

/// Who could join a match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchKind {
    /// Matchmaking or an open room: the only kind ever served; whether it
    /// counts in the totals and top lists.
    Public(Eligibility),
    /// A room with a PIN.
    Private,
    /// One player against the server's bots.
    VsBots,
}

impl MatchKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Public(_) => "public",
            Self::Private => "private",
            Self::VsBots => "vs_bots",
        }
    }

    fn eligibility(self) -> Option<Eligibility> {
        match self {
            Self::Public(eligibility) => Some(eligibility),
            Self::Private | Self::VsBots => None,
        }
    }
}

/// Whether a public match counts in the totals and the top lists: a real
/// match, not a few players' warm-up or one abandoned at its start. The
/// match itself is served either way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Eligibility {
    Counted,
    /// A test bot or a server bot played in it (it moved no rating either).
    WithBots,
    /// Fewer than [`Eligibility::MIN_HUMANS`] accounts were in it.
    FewPlayers,
    /// Played for less than [`Eligibility::MIN_PLAYED_MS`], or never began.
    Short,
    /// No snapshot of it was trusted: it has no winner and no numbers, and
    /// paid for none (`rewards`).
    NoResult,
}

impl Eligibility {
    /// The accounts that must have been in it, late joiners and leavers
    /// counted: the recordings' default minimum (`SRP_RECORD_MIN_PLAYERS`)
    /// and a default room's smallest match (two sides of three).
    pub const MIN_HUMANS: usize = 6;
    /// Its least play time (from the match clock's start, the warm-up
    /// left out): five minutes. srp has no rule of a match's length to
    /// take; a match given up at its start ends sooner.
    pub const MIN_PLAYED_MS: u32 = 5 * 60_000;

    /// A public match with `humans` accounts in it, `bots` whether a bot
    /// played, `played_ms` its play time (`None`: play never began),
    /// `trusted` whether its numbers are a trusted snapshot's.
    pub fn of(bots: bool, humans: usize, played_ms: Option<u32>, trusted: bool) -> Self {
        if bots {
            Self::WithBots
        } else if humans < Self::MIN_HUMANS {
            Self::FewPlayers
        } else if played_ms.is_none_or(|played| played < Self::MIN_PLAYED_MS) {
            Self::Short
        } else if !trusted {
            Self::NoResult
        } else {
            Self::Counted
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Counted => "counted",
            Self::WithBots => "with_bots",
            Self::FewPlayers => "few_players",
            Self::Short => "short",
            Self::NoResult => "no_result",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "counted" => Self::Counted,
            "with_bots" => Self::WithBots,
            "few_players" => Self::FewPlayers,
            "short" => Self::Short,
            "no_result" => Self::NoResult,
            _ => return None,
        })
    }
}

/// How a match ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Winner {
    /// Team 0 won.
    FirstTeam,
    /// Team 1 won.
    SecondTeam,
    Draw,
    /// No trusted snapshot said.
    Unknown,
    /// A free-for-all: no team wins it, its lines say who did.
    MostKills,
}

impl Winner {
    /// The match server's winner byte: 0 or 1, the draw, else unknown.
    pub fn of(winner: Option<u8>, draw: u8) -> Self {
        match winner {
            Some(0) => Self::FirstTeam,
            Some(1) => Self::SecondTeam,
            Some(value) if value == draw => Self::Draw,
            _ => Self::Unknown,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::FirstTeam => "team0",
            Self::SecondTeam => "team1",
            Self::Draw => "draw",
            Self::Unknown => "unknown",
            Self::MostKills => "most_kills",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "team0" => Self::FirstTeam,
            "team1" => Self::SecondTeam,
            "draw" => Self::Draw,
            "unknown" => Self::Unknown,
            "most_kills" => Self::MostKills,
            _ => return None,
        })
    }
}

/// What a match was for one of its players, said once, when it is settled
/// (`rewards`), as its rating took it: one who left a match that had a
/// winner lost it, whatever its team did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Won,
    Lost,
    /// A draw, or a match without a known winner.
    Neither,
}

impl Outcome {
    fn as_str(self) -> &'static str {
        match self {
            Self::Won => "won",
            Self::Lost => "lost",
            Self::Neither => "neither",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "won" => Self::Won,
            "lost" => Self::Lost,
            "neither" => Self::Neither,
            _ => return None,
        })
    }
}

/// Whether a player was still in the match at its end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Presence {
    Finished,
    Left,
}

impl Presence {
    fn as_str(self) -> &'static str {
        match self {
            Self::Finished => "finished",
            Self::Left => "left",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        match text {
            "finished" => Some(Self::Finished),
            "left" => Some(Self::Left),
            _ => None,
        }
    }
}

/// The game events counted for the site, of one match or added up.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counters {
    pub assists: u64,
    pub headshots: u64,
    pub grenade_kills: u64,
    pub melee_kills: u64,
    /// Batteries put into a container.
    pub batteries: u64,
    /// Research's captures, in hundredths of one.
    pub capture_hundredths: u64,
    /// Artefacts taken from an anomaly (event 5, every pickup).
    pub artefacts: u64,
    /// Bullets fired (event 22) and bullets that hit (event 23).
    pub shots: u64,
    pub hits: u64,
}

/// The fewest shots a player's totals need for the accuracy top list, so
/// that a handful of lucky ones does not top it (`g26_player_totals_accuracy`
/// is the same number).
pub const ACCURACY_MIN_SHOTS: u64 = 1000;
/// The fewest counted matches a player's totals need for the average score
/// top list, so that one good match does not top it
/// (`g26_player_totals_score_avg` is the same number).
pub const SCORE_AVG_MIN_MATCHES: u64 = 20;

/// What a kit's counters say of one match, or of the matches added up.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KitCounts {
    pub kills: u64,
    pub headshots: u64,
    pub shots: u64,
    pub hits: u64,
}

/// What an item in a [`GearSlot`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gear {
    Weapon,
    Armour,
}

impl Gear {
    fn as_str(self) -> &'static str {
        match self {
            Self::Weapon => "weapon",
            Self::Armour => "armour",
        }
    }
}

/// A profile slot of a kit the history keeps: a weapon's (7 and 10,
/// `rules::WEAPON_SLOTS`) or an armour piece's (0..=6). Ammo and the
/// pockets are not kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct GearSlot(u8);

impl GearSlot {
    /// The kit's slot `slot`, if the history keeps it.
    pub fn of(slot: usize) -> Option<Self> {
        match slot {
            0..=7 | 10 => Some(Self(slot as u8)),
            _ => None,
        }
    }

    /// The profile slot.
    pub fn number(self) -> u8 {
        self.0
    }

    pub fn gear(self) -> Gear {
        match self.0 {
            7 | 10 => Gear::Weapon,
            _ => Gear::Armour,
        }
    }
}

/// An item a kit held: items_dict's id, by slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KitItem {
    pub slot: GearSlot,
    pub dict_id: u16,
}

/// A kit a player used in a match: its own counters and what it held
/// (`docs/notes/match_history.md`, "What is kept": which kits count as used).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KitLine {
    /// Its place among the account's kits, 0..=2: the counters' kit.
    pub kit: u8,
    pub counts: KitCounts,
    /// By slot.
    pub items: Vec<KitItem>,
}

/// How many of a player's counted public matches a kit it used held the
/// item in. No kills or shots: the game counts those by kit, not by item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemTotals {
    pub dict_id: u16,
    pub matches: u64,
}

/// What a player did with one weapon, as its games counted it (exe patch
/// 165, `snapshot::WeaponCounts`): of one match, or of matches added up.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WeaponCounts {
    /// Enemies killed with it; of them, those by a headshot, by a bullet
    /// that had pierced something, and by one that had bounced.
    pub kills: u64,
    pub headshots: u64,
    pub wall_kills: u64,
    pub ricochet_kills: u64,
    /// The player's deaths with it in the hands.
    pub deaths: u64,
    /// Bullets fired with it and those that hit an enemy, a pellet each.
    pub shots: u64,
    pub hits: u64,
    /// The pain its hits took off enemies, in ten-thousandths of a pain
    /// pool.
    pub damage: u64,
    /// Its kills' distances added up, in decimetres.
    pub kill_distance_dm: u64,
    /// Its longest kill, in decimetres.
    pub longest_kill_dm: u64,
    /// How long it was in the hands of the player alive.
    pub held_ms: u64,
}

impl WeaponCounts {
    /// The counters in the order of [`WEAPON`].
    fn values(&self) -> [u64; WEAPON_COLUMNS] {
        [
            self.kills,
            self.headshots,
            self.wall_kills,
            self.ricochet_kills,
            self.deaths,
            self.shots,
            self.hits,
            self.damage,
            self.kill_distance_dm,
            self.longest_kill_dm,
            self.held_ms,
        ]
    }

    fn of_values(values: [u64; WEAPON_COLUMNS]) -> Self {
        let [
            kills,
            headshots,
            wall_kills,
            ricochet_kills,
            deaths,
            shots,
            hits,
            damage,
            kill_distance_dm,
            longest_kill_dm,
            held_ms,
        ] = values;
        Self {
            kills,
            headshots,
            wall_kills,
            ricochet_kills,
            deaths,
            shots,
            hits,
            damage,
            kill_distance_dm,
            longest_kill_dm,
            held_ms,
        }
    }
}

/// What a player's weapons sort by, the largest first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeaponRanking {
    Kills,
    /// Kills by the deaths with it in the hands.
    Kd,
    /// Headshots by kills, hits by shots, the kills' average distance: a
    /// weapon without a kill (a shot) comes last.
    HeadshotRate,
    Accuracy,
    KillDistance,
    WallKills,
    RicochetKills,
    Held,
}

impl WeaponRanking {
    /// The number it sorts by, of `g26_player_weapons`; NULL (the last)
    /// where nothing divides.
    fn order(self) -> &'static str {
        match self {
            Self::Kills => "kills",
            Self::Kd => "CAST(kills AS REAL) / max(deaths, 1)",
            Self::HeadshotRate => "CAST(headshots AS REAL) / nullif(kills, 0)",
            Self::Accuracy => "CAST(hits AS REAL) / nullif(shots, 0)",
            Self::KillDistance => "CAST(kill_distance_dm AS REAL) / nullif(kills, 0)",
            Self::WallKills => "wall_kills",
            Self::RicochetKills => "ricochet_kills",
            Self::Held => "held_ms",
        }
    }
}

/// A weapon of a player's counted public matches added up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WeaponTotals {
    pub dict_id: u16,
    /// The matches its games counted something of it in.
    pub matches: u64,
    pub counts: WeaponCounts,
}

/// A player's line of one match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerLine {
    pub team: u8,
    /// In its team by score, from 1; of a free-for-all among everyone, by
    /// kills.
    pub place: u8,
    /// Its place in the match's roster.
    pub seat: u8,
    pub presence: Presence,
    pub outcome: Outcome,
    /// The visible rating after the match.
    pub rating: u16,
    pub score: u64,
    pub kills: u64,
    pub deaths: u64,
    pub counters: Counters,
    /// How long it was in the match.
    pub played_ms: u32,
}

/// One player of a [`MatchRecord`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerRecord {
    pub account_id: u32,
    /// The registered clan it is of.
    pub clan_id: Option<u32>,
    pub line: PlayerLine,
    /// The kits it used, by place.
    pub kits: Vec<KitLine>,
    /// What it did with each weapon, by items_dict id (0: no weapon known);
    /// empty when its games counted none.
    pub weapons: BTreeMap<u16, WeaponCounts>,
}

/// What a match was, whoever played it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MatchInfo {
    /// Unix seconds.
    pub ended_at: u64,
    pub duration_ms: u32,
    pub map_id: MapId,
    /// The mode as the lobby calls it: a free-for-all's twist is one.
    pub mode: LobbyMode,
    pub match_level: u16,
    pub winner: Winner,
    pub team_points: [u16; 2],
}

/// A finished match as the history keeps it: no bots in it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchRecord {
    /// The match server's id with when it was made.
    pub replay_id: ReplayId,
    pub kind: MatchKind,
    pub info: MatchInfo,
    pub players: Vec<PlayerRecord>,
}

/// Public matches added up.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Totals {
    pub matches: u64,
    pub wins: u64,
    /// The rest were draws or had no known winner.
    pub losses: u64,
    pub score: u64,
    pub kills: u64,
    pub deaths: u64,
    pub counters: Counters,
    /// Time in the matches.
    pub played_ms: u64,
}

/// A total the top lists sort by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TotalsKey {
    Matches,
    Wins,
    Kills,
}

impl TotalsKey {
    /// The column, of `g26_player_totals` and `g26_clan_totals` alike.
    fn column(self) -> &'static str {
        match self {
            Self::Matches => "matches",
            Self::Wins => "wins",
            Self::Kills => "kills",
        }
    }
}

/// What the players' top list sorts by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerRanking {
    /// The visible rating.
    Rating,
    Totals(TotalsKey),
    /// Hits by shots, of the players with [`ACCURACY_MIN_SHOTS`] or more.
    Accuracy,
    /// Score a match, of the players with [`SCORE_AVG_MIN_MATCHES`] or more.
    ScoreAvg,
    /// Wins by matches, kills by deaths and headshots by kills, of the same
    /// players.
    WinRate,
    Kd,
    HeadshotRate,
}

/// A registered clan, as a player's tag links it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClanRef {
    pub id: u32,
    pub tag: String,
}

/// A player as the site names it: its in-game name and current clan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Named {
    pub account_id: u32,
    pub name: String,
    pub clan: Option<ClanRef>,
}

/// A player's card: who it is now and its totals.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerCard {
    pub player: Named,
    pub level: u8,
    pub experience: u32,
    pub rating: u16,
    pub totals: Totals,
}

/// A public match of the history.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchSummary {
    /// The history's number.
    pub id: u64,
    /// The match for good: its recording's name.
    pub replay_id: ReplayId,
    pub info: MatchInfo,
    /// Whether it counts in the totals and top lists.
    pub eligibility: Eligibility,
    /// How many players the history has of it.
    pub players: u32,
}

/// A player's line of a match with who it is now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchPlayer {
    pub player: Named,
    pub level: u8,
    pub line: PlayerLine,
    pub kits: Vec<KitLine>,
}

/// A public match of a player's: the match, its line and its kits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerMatch {
    pub summary: MatchSummary,
    pub line: PlayerLine,
    pub kits: Vec<KitLine>,
}

/// A player's most used items of its public matches, most matches first.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Favourites {
    pub weapons: Vec<ItemTotals>,
    pub armour: Vec<ItemTotals>,
}

/// One line of the players' top list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TopPlayer {
    pub player: Named,
    pub level: u8,
    pub rating: u16,
    pub totals: Totals,
}

/// One line of the best single-match scores.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BestLine {
    pub player: Named,
    pub summary: MatchSummary,
    pub line: PlayerLine,
}

/// Everything counted, added up: how many players have a counted match, how
/// many counted matches there are, and the players' totals together (their
/// `matches` are lines: a match once a player).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AllTime {
    pub players: u64,
    pub matches: u64,
    pub totals: Totals,
}

/// A registered clan with its totals.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClanLine {
    pub id: u32,
    pub name: String,
    pub tag: String,
    pub level: u8,
    pub members: u32,
    pub totals: Totals,
}

/// A member of a [`ClanCard`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClanMember {
    pub player: Named,
    pub commander: bool,
    pub level: u8,
    pub rating: u16,
    /// All its public matches, in the clan or not.
    pub totals: Totals,
}

/// A registered clan: its totals and its members with a public match (a
/// name is most often still the login), commander first, then by score.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClanCard {
    pub clan: ClanLine,
    /// `None` when the commander has no public match.
    pub leader: Option<Named>,
    pub members: Vec<ClanMember>,
}

/// How the site names a clan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClanKey<'a> {
    Id(u32),
    Tag(&'a str),
}

/// Writes a match and adds a counted public one to the totals, in one
/// transaction. A match of a replay id kept already is not written again:
/// a failed write is retried.
pub(super) fn record(
    connection: &mut Connection,
    record: &MatchRecord,
) -> Result<(), PersistError> {
    let transaction = connection.transaction()?;
    let kept = transaction
        .query_row(
            "SELECT 1 FROM g26_matches WHERE replay_id = ?1",
            [record.replay_id.as_str()],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    if kept {
        return Ok(());
    }
    let info = &record.info;
    transaction.execute(
        "INSERT INTO g26_matches (game_match_id, replay_id, ended_at, duration_ms, map_id,
             game_mode, match_level, kind, eligibility, winner, team0_points, team1_points)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            record.replay_id.match_id(),
            record.replay_id.as_str(),
            info.ended_at.min(i64::MAX as u64) as i64,
            info.duration_ms,
            info.map_id.get(),
            info.mode.number(),
            info.match_level,
            record.kind.as_str(),
            record.kind.eligibility().map(Eligibility::as_str),
            info.winner.as_str(),
            info.team_points[0],
            info.team_points[1],
        ],
    )?;
    let match_id = transaction.last_insert_rowid();
    // A public match that does not count still names its players (a row of
    // zeros): the site names whoever has a public match.
    let counted = record.kind.eligibility();
    // Each clan's line of a counted match: one match, whoever of it played.
    let mut clans: BTreeMap<u32, Totals> = BTreeMap::new();
    for player in &record.players {
        let line = &player.line;
        let counters = &line.counters;
        transaction.execute(
            "INSERT INTO g26_match_players (match_id, account_id, clan_id, team, place, presence,
                 rating, score, kills, deaths, assists, headshots, grenade_kills, melee_kills,
                 batteries, captures, artefacts, shots, hits, seat, outcome, played_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16,
                 ?17, ?18, ?19, ?20, ?21, ?22)",
            params![
                match_id,
                player.account_id,
                player.clan_id,
                line.team,
                line.place,
                line.presence.as_str(),
                line.rating,
                line.score as i64,
                line.kills as i64,
                line.deaths as i64,
                counters.assists as i64,
                counters.headshots as i64,
                counters.grenade_kills as i64,
                counters.melee_kills as i64,
                counters.batteries as i64,
                counters.capture_hundredths as i64,
                counters.artefacts as i64,
                counters.shots as i64,
                counters.hits as i64,
                line.seat,
                line.outcome.as_str(),
                line.played_ms,
            ],
        )?;
        record_kits(&transaction, match_id, player)?;
        record_weapons(
            &transaction,
            match_id,
            player,
            counted == Some(Eligibility::Counted),
        )?;
        match counted {
            None => {}
            Some(Eligibility::Counted) => {
                let own = Totals::of_match(line);
                add_totals(
                    &transaction,
                    "g26_player_totals",
                    "account_id",
                    player.account_id,
                    &own,
                )?;
                add_items(&transaction, player)?;
                if let Some(clan_id) = player.clan_id {
                    clans
                        .entry(clan_id)
                        .and_modify(|clan| *clan = clan.with_member(&own))
                        .or_insert(own);
                }
            }
            Some(
                Eligibility::WithBots
                | Eligibility::FewPlayers
                | Eligibility::Short
                | Eligibility::NoResult,
            ) => {
                add_totals(
                    &transaction,
                    "g26_player_totals",
                    "account_id",
                    player.account_id,
                    &Totals::default(),
                )?;
            }
        }
    }
    for (clan_id, mut totals) in clans {
        // Its members on both sides: it neither won nor lost.
        if totals.wins > 0 && totals.losses > 0 {
            (totals.wins, totals.losses) = (0, 0);
        }
        add_totals(&transaction, "g26_clan_totals", "clan_id", clan_id, &totals)?;
    }
    transaction.commit()?;
    Ok(())
}

impl Totals {
    /// A player's line of one match.
    fn of_match(line: &PlayerLine) -> Self {
        Self {
            matches: 1,
            wins: u64::from(line.outcome == Outcome::Won),
            losses: u64::from(line.outcome == Outcome::Lost),
            score: line.score,
            kills: line.kills,
            deaths: line.deaths,
            counters: line.counters,
            played_ms: u64::from(line.played_ms),
        }
    }

    /// A clan's one match with another member's line of it added: still
    /// one match, won if any member won it and lost if any lost it (both:
    /// [`record`] counts neither), the rest added up.
    fn with_member(&self, member: &Self) -> Self {
        let (own, more) = (&self.counters, &member.counters);
        Self {
            matches: 1,
            wins: self.wins.max(member.wins),
            losses: self.losses.max(member.losses),
            score: self.score + member.score,
            kills: self.kills + member.kills,
            deaths: self.deaths + member.deaths,
            counters: Counters {
                assists: own.assists + more.assists,
                headshots: own.headshots + more.headshots,
                grenade_kills: own.grenade_kills + more.grenade_kills,
                melee_kills: own.melee_kills + more.melee_kills,
                batteries: own.batteries + more.batteries,
                capture_hundredths: own.capture_hundredths + more.capture_hundredths,
                artefacts: own.artefacts + more.artefacts,
                shots: own.shots + more.shots,
                hits: own.hits + more.hits,
            },
            played_ms: self.played_ms + member.played_ms,
        }
    }
}

/// Writes the kits `player` used in the match `match_id` and what they
/// held.
fn record_kits(
    transaction: &Transaction,
    match_id: i64,
    player: &PlayerRecord,
) -> Result<(), PersistError> {
    for kit in &player.kits {
        let counts = &kit.counts;
        transaction.execute(
            "INSERT INTO g26_match_kits (match_id, account_id, kit, kills, headshots, shots, hits)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                match_id,
                player.account_id,
                kit.kit,
                counts.kills as i64,
                counts.headshots as i64,
                counts.shots as i64,
                counts.hits as i64,
            ],
        )?;
        for item in &kit.items {
            transaction.execute(
                "INSERT INTO g26_match_kit_items (match_id, account_id, kit, slot, dict_id)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    match_id,
                    player.account_id,
                    kit.kit,
                    item.slot.number(),
                    item.dict_id
                ],
            )?;
        }
    }
    Ok(())
}

/// The counters of a [`WeaponCounts`], as both weapon tables name them
/// ([`WEAPON_COLUMNS`] of them, [`WeaponCounts::values`]' order).
const WEAPON: &str = "kills, headshots, wall_kills, ricochet_kills, deaths, shots, hits, damage,
     kill_distance_dm, longest_kill_dm, held_ms";
const WEAPON_COLUMNS: usize = 11;

/// Writes what `player` did with each weapon in the match `match_id`, and
/// adds it to the player's weapons when the match is `counted`.
fn record_weapons(
    transaction: &Transaction,
    match_id: i64,
    player: &PlayerRecord,
    counted: bool,
) -> Result<(), PersistError> {
    for (&dict_id, counts) in &player.weapons {
        let [a, b, c, d, e, f, g, h, i, j, k] = counts
            .values()
            .map(|value| value.min(i64::MAX as u64) as i64);
        transaction.execute(
            &format!(
                "INSERT INTO g26_match_weapons (match_id, account_id, dict_id, {WEAPON})
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)"
            ),
            params![
                match_id,
                player.account_id,
                dict_id,
                a,
                b,
                c,
                d,
                e,
                f,
                g,
                h,
                i,
                j,
                k
            ],
        )?;
        if counted {
            transaction.execute(
                &format!(
                    "INSERT INTO g26_player_weapons (account_id, dict_id, matches, {WEAPON})
                     VALUES (?1, ?2, 1, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
                     ON CONFLICT(account_id, dict_id) DO UPDATE SET
                         matches = matches + 1, kills = kills + excluded.kills,
                         headshots = headshots + excluded.headshots,
                         wall_kills = wall_kills + excluded.wall_kills,
                         ricochet_kills = ricochet_kills + excluded.ricochet_kills,
                         deaths = deaths + excluded.deaths, shots = shots + excluded.shots,
                         hits = hits + excluded.hits, damage = damage + excluded.damage,
                         kill_distance_dm = kill_distance_dm + excluded.kill_distance_dm,
                         longest_kill_dm = max(longest_kill_dm, excluded.longest_kill_dm),
                         held_ms = held_ms + excluded.held_ms"
                ),
                params![player.account_id, dict_id, a, b, c, d, e, f, g, h, i, j, k],
            )?;
        }
    }
    Ok(())
}

/// Adds a counted public match to `player`'s item totals: each item the
/// kits it used held, once.
fn add_items(transaction: &Transaction, player: &PlayerRecord) -> Result<(), PersistError> {
    let held: BTreeMap<u16, Gear> = player
        .kits
        .iter()
        .flat_map(|kit| &kit.items)
        .map(|item| (item.dict_id, item.slot.gear()))
        .collect();
    for (dict_id, gear) in held {
        transaction.execute(
            "INSERT INTO g26_player_items (account_id, dict_id, gear, matches)
             VALUES (?1, ?2, ?3, 1)
             ON CONFLICT(account_id, dict_id) DO UPDATE SET matches = matches + 1",
            params![player.account_id, dict_id, gear.as_str()],
        )?;
    }
    Ok(())
}

/// Adds one match's `totals` to the row of `id` in `table`.
fn add_totals(
    transaction: &Transaction,
    table: &str,
    key: &str,
    id: u32,
    totals: &Totals,
) -> Result<(), PersistError> {
    let counters = &totals.counters;
    transaction.execute(
        &format!(
            "INSERT INTO {table} ({key}, matches, wins, losses, score, kills, deaths, assists,
                 headshots, grenade_kills, melee_kills, batteries, captures, artefacts, shots, hits,
                 played_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
             ON CONFLICT({key}) DO UPDATE SET
                 matches = matches + excluded.matches, wins = wins + excluded.wins,
                 losses = losses + excluded.losses,
                 score = score + excluded.score, kills = kills + excluded.kills,
                 deaths = deaths + excluded.deaths, assists = assists + excluded.assists,
                 headshots = headshots + excluded.headshots,
                 grenade_kills = grenade_kills + excluded.grenade_kills,
                 melee_kills = melee_kills + excluded.melee_kills,
                 batteries = batteries + excluded.batteries,
                 captures = captures + excluded.captures,
                 artefacts = artefacts + excluded.artefacts,
                 shots = shots + excluded.shots, hits = hits + excluded.hits,
                 played_ms = played_ms + excluded.played_ms"
        ),
        params![
            id,
            totals.matches as i64,
            totals.wins as i64,
            totals.losses as i64,
            totals.score as i64,
            totals.kills as i64,
            totals.deaths as i64,
            counters.assists as i64,
            counters.headshots as i64,
            counters.grenade_kills as i64,
            counters.melee_kills as i64,
            counters.batteries as i64,
            counters.capture_hundredths as i64,
            counters.artefacts as i64,
            counters.shots as i64,
            counters.hits as i64,
            totals.played_ms as i64,
        ],
    )?;
    Ok(())
}

/// The columns a [`Named`] is read from, `a` being `g26_accounts`: id,
/// name, clan id, clan tag (four).
const NAMED: &str = "a.account_id, a.name, c.clan_id, c.tag";
const _: () = assert!(status::ACCEPTED == 0);
/// Joins the account's current registered clan to `a` (status 0:
/// accepted).
const CLAN_JOIN: &str = "LEFT JOIN g26_clan_members cm
         ON cm.account_id = a.account_id AND cm.status = 0
     LEFT JOIN g26_clans c ON c.clan_id = cm.clan_id AND c.registered != 0";
/// The columns of a [`MatchSummary`] of a public match `m`
/// ([`SUMMARY_COLUMNS`]).
const SUMMARY: &str = "m.id, m.ended_at, m.duration_ms, m.map_id, m.game_mode, m.match_level,
     m.winner, m.team0_points, m.team1_points,
     (SELECT COUNT(*) FROM g26_match_players n WHERE n.match_id = m.id), m.eligibility,
     m.replay_id";
const SUMMARY_COLUMNS: usize = 12;
/// The columns of a [`PlayerLine`] of `p` (nineteen).
const LINE: &str = "p.team, p.place, p.presence, p.rating, p.score, p.kills, p.deaths, p.assists,
     p.headshots, p.grenade_kills, p.melee_kills, p.batteries, p.captures, p.artefacts, p.shots,
     p.hits, p.seat, p.outcome, p.played_ms";
/// The columns of [`Totals`] of `t` ([`TOTALS_COLUMNS`]).
const TOTALS_COLUMNS: usize = 16;
const TOTALS: &str = "t.matches, t.wins, t.losses, t.score, t.kills, t.deaths, t.assists, t.headshots,
     t.grenade_kills, t.melee_kills, t.batteries, t.captures, t.artefacts, t.shots, t.hits,
     t.played_ms";
/// A match's kits and what they held, of `k` and `i`, by account and
/// place, then slot (eight columns: account, kit, its four counters, slot,
/// dict id); `{filter}` narrows `k` further.
fn kits_query(filter: &str) -> String {
    format!(
        "SELECT k.account_id, k.kit, k.kills, k.headshots, k.shots, k.hits, i.slot, i.dict_id
         FROM g26_match_kits k LEFT JOIN g26_match_kit_items i
             ON i.match_id = k.match_id AND i.account_id = k.account_id AND i.kit = k.kit
         WHERE k.match_id = ?1 {filter}
         ORDER BY k.account_id, k.kit, i.slot"
    )
}
/// A player's items of a gear, most matches first.
const FAVOURITES: &str = "SELECT dict_id, matches
     FROM g26_player_items WHERE account_id = ?1 AND gear = ?2
     ORDER BY matches DESC, dict_id DESC LIMIT ?3";

/// The rows of [`kits_query`], by account.
fn kits_of(
    statement: &mut rusqlite::CachedStatement,
    params: impl rusqlite::Params,
) -> rusqlite::Result<BTreeMap<u32, Vec<KitLine>>> {
    let mut kits: BTreeMap<u32, Vec<KitLine>> = BTreeMap::new();
    let mut rows = statement.query(params)?;
    while let Some(row) = rows.next()? {
        let account_id: u32 = row.get(0)?;
        let kit: u8 = row.get(1)?;
        let lines = kits.entry(account_id).or_default();
        if lines.last().is_none_or(|line| line.kit != kit) {
            lines.push(KitLine {
                kit,
                counts: counts_at(row, 2)?,
                items: Vec::new(),
            });
        }
        if let Some(slot) = row.get::<_, Option<i64>>(6)? {
            let slot = usize::try_from(slot)
                .ok()
                .and_then(GearSlot::of)
                .ok_or_else(|| bad("slot", &slot.to_string()))?;
            let line = lines.last_mut().expect("pushed above");
            line.items.push(KitItem {
                slot,
                dict_id: row.get(7)?,
            });
        }
    }
    Ok(kits)
}

fn counts_at(row: &Row, at: usize) -> rusqlite::Result<KitCounts> {
    Ok(KitCounts {
        kills: u64_at(row, at)?,
        headshots: u64_at(row, at + 1)?,
        shots: u64_at(row, at + 2)?,
        hits: u64_at(row, at + 3)?,
    })
}

fn bad(what: &str, text: &str) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        0,
        rusqlite::types::Type::Text,
        format!("{what} {text:?}").into(),
    )
}

fn u64_at(row: &Row, index: usize) -> rusqlite::Result<u64> {
    Ok(row.get::<_, Option<i64>>(index)?.unwrap_or(0).max(0) as u64)
}

fn named_at(row: &Row, at: usize) -> rusqlite::Result<Named> {
    let clan = match (
        row.get::<_, Option<u32>>(at + 2)?,
        row.get::<_, Option<String>>(at + 3)?,
    ) {
        (Some(id), Some(tag)) => Some(ClanRef { id, tag }),
        _ => None,
    };
    Ok(Named {
        account_id: row.get(at)?,
        name: row.get(at + 1)?,
        clan,
    })
}

fn summary_at(row: &Row, at: usize) -> rusqlite::Result<MatchSummary> {
    let winner: String = row.get(at + 6)?;
    let mode: u8 = row.get(at + 4)?;
    let eligibility: String = row.get(at + 10)?;
    let replay_id: String = row.get(at + 11)?;
    Ok(MatchSummary {
        id: u64_at(row, at)?,
        replay_id: ReplayId::parse(&replay_id).ok_or_else(|| bad("replay id", &replay_id))?,
        info: MatchInfo {
            ended_at: u64_at(row, at + 1)?,
            duration_ms: row.get(at + 2)?,
            map_id: MapId::new(row.get(at + 3)?),
            mode: LobbyMode::of_number(mode)
                .ok_or_else(|| bad("game mode", &mode.to_string()))?,
            match_level: row.get(at + 5)?,
            winner: Winner::parse(&winner).ok_or_else(|| bad("winner", &winner))?,
            team_points: [row.get(at + 7)?, row.get(at + 8)?],
        },
        players: row.get(at + 9)?,
        eligibility: Eligibility::parse(&eligibility)
            .ok_or_else(|| bad("eligibility", &eligibility))?,
    })
}

fn line_at(row: &Row, at: usize) -> rusqlite::Result<PlayerLine> {
    let presence: String = row.get(at + 2)?;
    let outcome: String = row.get(at + 17)?;
    Ok(PlayerLine {
        team: row.get(at)?,
        place: row.get(at + 1)?,
        seat: row.get(at + 16)?,
        presence: Presence::parse(&presence).ok_or_else(|| bad("presence", &presence))?,
        outcome: Outcome::parse(&outcome).ok_or_else(|| bad("outcome", &outcome))?,
        rating: row.get(at + 3)?,
        score: u64_at(row, at + 4)?,
        kills: u64_at(row, at + 5)?,
        deaths: u64_at(row, at + 6)?,
        counters: Counters {
            assists: u64_at(row, at + 7)?,
            headshots: u64_at(row, at + 8)?,
            grenade_kills: u64_at(row, at + 9)?,
            melee_kills: u64_at(row, at + 10)?,
            batteries: u64_at(row, at + 11)?,
            capture_hundredths: u64_at(row, at + 12)?,
            artefacts: u64_at(row, at + 13)?,
            shots: u64_at(row, at + 14)?,
            hits: u64_at(row, at + 15)?,
        },
        played_ms: row.get(at + 18)?,
    })
}

/// [`Totals`] at `at`.
fn totals_at(row: &Row, at: usize) -> rusqlite::Result<Totals> {
    Ok(Totals {
        matches: u64_at(row, at)?,
        wins: u64_at(row, at + 1)?,
        losses: u64_at(row, at + 2)?,
        score: u64_at(row, at + 3)?,
        kills: u64_at(row, at + 4)?,
        deaths: u64_at(row, at + 5)?,
        counters: Counters {
            assists: u64_at(row, at + 6)?,
            headshots: u64_at(row, at + 7)?,
            grenade_kills: u64_at(row, at + 8)?,
            melee_kills: u64_at(row, at + 9)?,
            batteries: u64_at(row, at + 10)?,
            capture_hundredths: u64_at(row, at + 11)?,
            artefacts: u64_at(row, at + 12)?,
            shots: u64_at(row, at + 13)?,
            hits: u64_at(row, at + 14)?,
        },
        played_ms: u64_at(row, at + 15)?,
    })
}

fn level_of(experience: u32) -> u8 {
    level_at(experience).0
}

/// The history as the site reads it: public matches only, on read-only
/// connections of its own. The file is in WAL mode
/// ([`super::SqliteGameRepository::open`]), so a read never waits for the
/// writer nor the writer for a read. Every query is an indexed lookup of at
/// most [`MatchHistory::MAX_ROWS`] rows (a clan card's members: up to
/// `clans::MAX_MEMBERS`; a list a `keep` filters reads up to
/// [`MatchHistory::MAX_READ`]).
pub struct MatchHistory {
    /// A slow request holds one; when all are busy the next waits.
    connections: Vec<Mutex<Connection>>,
    /// The connection a read waits for when all are busy, in turn.
    next: AtomicUsize,
}

impl MatchHistory {
    /// The most rows a list gives.
    pub const MAX_ROWS: usize = 50;
    /// The most rows a list that a `keep` filters (`top`, `best`) reads to
    /// find its [`Self::MAX_ROWS`]: the ones `keep` leaves out (the banned,
    /// which the accounts' store knows) are read past.
    pub const MAX_READ: usize = 500;
    /// How many requests read at once: the host has one CPU, the rest wait.
    pub const READERS: usize = 4;

    /// Opens the file at `path`, which [`super::SqliteGameRepository`]
    /// made, put in WAL mode and brought to this schema.
    pub(super) fn open(path: &Path) -> Result<Self, PersistError> {
        let connections = (0..Self::READERS)
            .map(|_| {
                let connection = Connection::open_with_flags(
                    path,
                    OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
                )?;
                connection.execute_batch("PRAGMA busy_timeout = 2000;")?;
                Ok(Mutex::new(connection))
            })
            .collect::<Result<_, PersistError>>()?;
        Ok(Self {
            connections,
            next: AtomicUsize::new(0),
        })
    }

    /// A free connection, else the next one in turn once it is free.
    fn reader(&self) -> MutexGuard<'_, Connection> {
        for connection in &self.connections {
            match connection.try_lock() {
                Ok(connection) => return connection,
                Err(TryLockError::Poisoned(poisoned)) => return poisoned.into_inner(),
                Err(TryLockError::WouldBlock) => {}
            }
        }
        let turn = self.next.fetch_add(1, Ordering::Relaxed) % self.connections.len();
        self.connections[turn].locked()
    }

    /// The account with this in-game name (Latin letters in any case), of
    /// those with a public match: a name is most often still the login, so
    /// no other account is ever found.
    pub fn account_by_name(&self, name: &str) -> Result<Option<u32>, PersistError> {
        Ok(self
            .reader()
            .query_row(
                "SELECT a.account_id FROM g26_accounts a
                 JOIN g26_player_totals t ON t.account_id = a.account_id
                 WHERE a.name = ?1 COLLATE NOCASE
                 ORDER BY a.account_id LIMIT 1",
                [name],
                |row| row.get(0),
            )
            .optional()?)
    }

    /// The account's card; `None` for no account or one without a public
    /// match (its name is most often still the login).
    pub fn player(&self, account_id: u32) -> Result<Option<PlayerCard>, PersistError> {
        Ok(self
            .reader()
            .query_row(
                &format!(
                    "SELECT {NAMED}, a.exp_total, a.rating, {TOTALS}
                     FROM g26_accounts a {CLAN_JOIN}
                     JOIN g26_player_totals t ON t.account_id = a.account_id
                     WHERE a.account_id = ?1"
                ),
                [account_id],
                |row| {
                    let experience: u32 = row.get(4)?;
                    Ok(PlayerCard {
                        player: named_at(row, 0)?,
                        level: level_of(experience),
                        experience,
                        rating: row.get(5)?,
                        totals: totals_at(row, 6)?,
                    })
                },
            )
            .optional()?)
    }

    /// The account's public matches before `before` (the newest with
    /// `None`), newest first, at most `limit`, among its newest
    /// [`MatchHistory::MAX_READ`] before it (its private ones and those
    /// against bots are read past).
    pub fn player_matches(
        &self,
        account_id: u32,
        before: Option<u64>,
        limit: usize,
    ) -> Result<Vec<PlayerMatch>, PersistError> {
        let connection = self.reader();
        let mut statement = connection.prepare_cached(&format!(
            "SELECT {SUMMARY}, {LINE}
             FROM (SELECT * FROM g26_match_players
                   WHERE account_id = ?1 AND match_id < ?2
                   ORDER BY match_id DESC LIMIT 500) p
             JOIN g26_matches m ON m.id = p.match_id
             WHERE m.kind = 'public'
             ORDER BY p.match_id DESC LIMIT ?3"
        ))?;
        let rows = statement
            .query_map(params![account_id, upper(before), bounded(limit)], |row| {
                Ok((summary_at(row, 0)?, line_at(row, SUMMARY_COLUMNS)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        // Each match's kits of the player: by the primary key.
        let mut kits = connection.prepare_cached(&kits_query("AND k.account_id = ?2"))?;
        let mut matches = Vec::with_capacity(rows.len());
        for (summary, line) in rows {
            let kits = kits_of(&mut kits, params![summary.id as i64, account_id])?
                .remove(&account_id)
                .unwrap_or_default();
            matches.push(PlayerMatch {
                summary,
                line,
                kits,
            });
        }
        Ok(matches)
    }

    /// The account's most used weapons and armour of its public matches,
    /// at most `limit` of each.
    pub fn favourites(&self, account_id: u32, limit: usize) -> Result<Favourites, PersistError> {
        let connection = self.reader();
        let mut statement = connection.prepare_cached(FAVOURITES)?;
        let mut of = |gear: Gear| -> rusqlite::Result<Vec<ItemTotals>> {
            statement
                .query_map(params![account_id, gear.as_str(), bounded(limit)], |row| {
                    Ok(ItemTotals {
                        dict_id: row.get(0)?,
                        matches: u64_at(row, 1)?,
                    })
                })?
                .collect()
        };
        Ok(Favourites {
            weapons: of(Gear::Weapon)?,
            armour: of(Gear::Armour)?,
        })
    }

    /// The weapons the account fired or killed with in its counted public
    /// matches, the largest `by` first (then most kills), at most `limit`.
    /// Not what the games counted under no weapon (dict id 0), nor an item
    /// only held.
    pub fn player_weapons(
        &self,
        account_id: u32,
        by: WeaponRanking,
        limit: usize,
    ) -> Result<Vec<WeaponTotals>, PersistError> {
        let connection = self.reader();
        let mut statement = connection.prepare_cached(&format!(
            "SELECT dict_id, matches, {WEAPON} FROM g26_player_weapons
             WHERE account_id = ?1 AND dict_id != 0 AND (kills > 0 OR shots > 0)
             ORDER BY {} DESC, kills DESC, shots DESC, dict_id LIMIT ?2",
            by.order()
        ))?;
        let weapons = statement
            .query_map(params![account_id, bounded(limit)], |row| {
                let mut values = [0; WEAPON_COLUMNS];
                for (index, value) in values.iter_mut().enumerate() {
                    *value = u64_at(row, 2 + index)?;
                }
                Ok(WeaponTotals {
                    dict_id: row.get(0)?,
                    matches: u64_at(row, 1)?,
                    counts: WeaponCounts::of_values(values),
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(weapons)
    }

    /// The account's counted public matches that ended at `since` or
    /// later, added up by UTC day (`YYYY-MM-DD`), oldest first.
    pub fn player_days(
        &self,
        account_id: u32,
        since: u64,
    ) -> Result<Vec<(String, Totals)>, PersistError> {
        let connection = self.reader();
        let mut statement = connection.prepare_cached(
            "SELECT date(m.ended_at, 'unixepoch') AS day, COUNT(*),
                 SUM(p.outcome = 'won'), SUM(p.outcome = 'lost'), SUM(p.score), SUM(p.kills),
                 SUM(p.deaths), SUM(p.assists), SUM(p.headshots), SUM(p.grenade_kills),
                 SUM(p.melee_kills), SUM(p.batteries), SUM(p.captures), SUM(p.artefacts),
                 SUM(p.shots), SUM(p.hits), SUM(p.played_ms)
             FROM g26_match_players p JOIN g26_matches m ON m.id = p.match_id
             WHERE p.account_id = ?1 AND m.eligibility = 'counted' AND m.ended_at >= ?2
               AND p.match_id >= IFNULL((SELECT MIN(id) FROM g26_matches WHERE ended_at >= ?2),
                   9223372036854775807)
             GROUP BY day ORDER BY day",
        )?;
        let rows = statement.query_map(
            params![account_id, since.min(i64::MAX as u64) as i64],
            |row| Ok((row.get(0)?, totals_at(row, 1)?)),
        )?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Public matches before `before`, newest first, at most `limit`; with
    /// `clan`, those a player of that clan played in.
    pub fn matches(
        &self,
        before: Option<u64>,
        limit: usize,
        clan: Option<u32>,
    ) -> Result<Vec<MatchSummary>, PersistError> {
        let connection = self.reader();
        let rows: Vec<MatchSummary> = match clan {
            None => {
                let mut statement = connection.prepare_cached(&format!(
                    "SELECT {SUMMARY} FROM g26_matches m
                     WHERE m.kind = 'public' AND m.id < ?1 ORDER BY m.id DESC LIMIT ?2"
                ))?;
                let rows = statement.query_map(params![upper(before), bounded(limit)], |row| {
                    summary_at(row, 0)
                })?;
                rows.collect::<Result<_, _>>()?
            }
            Some(clan_id) => {
                let mut statement = connection.prepare_cached(&format!(
                    "SELECT {SUMMARY}
                     FROM g26_match_players p JOIN g26_matches m ON m.id = p.match_id
                     WHERE p.clan_id = ?1 AND p.match_id < ?2 AND m.kind = 'public'
                     GROUP BY p.match_id ORDER BY p.match_id DESC LIMIT ?3"
                ))?;
                let rows = statement
                    .query_map(params![clan_id, upper(before), bounded(limit)], |row| {
                        summary_at(row, 0)
                    })?;
                rows.collect::<Result<_, _>>()?
            }
        };
        Ok(rows)
    }

    /// A public match and its players, by team, then place (a
    /// free-for-all's: each its own team, by roster place).
    pub fn game_match(
        &self,
        id: u64,
    ) -> Result<Option<(MatchSummary, Vec<MatchPlayer>)>, PersistError> {
        let connection = self.reader();
        let summary = connection
            .query_row(
                &format!(
                    "SELECT {SUMMARY} FROM g26_matches m WHERE m.id = ?1 AND m.kind = 'public'"
                ),
                [id.min(i64::MAX as u64) as i64],
                |row| summary_at(row, 0),
            )
            .optional()?;
        let Some(summary) = summary else {
            return Ok(None);
        };
        let mut statement = connection.prepare_cached(&format!(
            "SELECT {NAMED}, a.exp_total, {LINE}
             FROM g26_match_players p JOIN g26_accounts a ON a.account_id = p.account_id
             {CLAN_JOIN}
             WHERE p.match_id = ?1 ORDER BY p.team, p.place"
        ))?;
        let mut kits = kits_of(
            &mut connection.prepare_cached(&kits_query(""))?,
            [summary.id as i64],
        )?;
        let rows = statement.query_map([summary.id as i64], |row| {
            let player = named_at(row, 0)?;
            Ok(MatchPlayer {
                kits: kits.remove(&player.account_id).unwrap_or_default(),
                player,
                level: level_of(row.get(4)?),
                line: line_at(row, 5)?,
            })
        })?;
        Ok(Some((summary, rows.collect::<Result<_, _>>()?)))
    }

    /// The top `limit` players by `by`, of those with a counted public
    /// match and that `keep` keeps (it is asked of each in order until enough,
    /// of the first [`MatchHistory::MAX_READ`]).
    pub fn top(
        &self,
        by: PlayerRanking,
        limit: usize,
        keep: &dyn Fn(u32) -> bool,
    ) -> Result<Vec<TopPlayer>, PersistError> {
        let connection = self.reader();
        let mut statement = connection.prepare_cached(&top_query(by))?;
        let mut rows = statement.query([MatchHistory::MAX_READ as i64])?;
        let mut top = Vec::new();
        while top.len() < bounded(limit) as usize
            && let Some(row) = rows.next()?
        {
            let player = named_at(row, 0)?;
            if !keep(player.account_id) {
                continue;
            }
            top.push(TopPlayer {
                player,
                level: level_of(row.get(4)?),
                rating: row.get(5)?,
                totals: totals_at(row, 6)?,
            });
        }
        Ok(top)
    }

    /// The best `limit` single-match scores of counted public matches that
    /// ended at `since` or later, of the players that `keep` keeps (it is asked
    /// of each in order until enough, of the first [`MatchHistory::MAX_READ`]).
    pub fn best(
        &self,
        since: u64,
        limit: usize,
        keep: &dyn Fn(u32) -> bool,
    ) -> Result<Vec<BestLine>, PersistError> {
        let connection = self.reader();
        let mut statement = connection.prepare_cached(&best_query())?;
        let mut rows = statement.query(params![
            since.min(i64::MAX as u64) as i64,
            MatchHistory::MAX_READ as i64
        ])?;
        let mut best = Vec::new();
        while best.len() < bounded(limit) as usize
            && let Some(row) = rows.next()?
        {
            let player = named_at(row, 0)?;
            if !keep(player.account_id) {
                continue;
            }
            best.push(BestLine {
                player,
                summary: summary_at(row, 4)?,
                line: line_at(row, 4 + SUMMARY_COLUMNS)?,
            });
        }
        Ok(best)
    }

    /// Each player's counted public matches that ended at `since` or later,
    /// added up, of the players that `keep` keeps: the day's leaders are
    /// read from it. As many rows as players of the period (a day's lines
    /// are read: its matches by `g26_matches_by_end`, each one's players by
    /// the primary key).
    pub fn totals_since(
        &self,
        since: u64,
        keep: &dyn Fn(u32) -> bool,
    ) -> Result<Vec<(Named, Totals)>, PersistError> {
        let connection = self.reader();
        let mut statement = connection.prepare_cached(&format!(
            "SELECT {NAMED}, COUNT(*), SUM(p.outcome = 'won'), SUM(p.outcome = 'lost'),
                 SUM(p.score), SUM(p.kills), SUM(p.deaths), SUM(p.assists), SUM(p.headshots),
                 SUM(p.grenade_kills), SUM(p.melee_kills), SUM(p.batteries), SUM(p.captures),
                 SUM(p.artefacts), SUM(p.shots), SUM(p.hits), SUM(p.played_ms)
             FROM g26_matches m CROSS JOIN g26_match_players p ON p.match_id = m.id
             JOIN g26_accounts a ON a.account_id = p.account_id
             {CLAN_JOIN}
             WHERE m.ended_at >= ?1 AND m.eligibility = 'counted'
             GROUP BY a.account_id"
        ))?;
        let rows = statement.query_map([since.min(i64::MAX as u64) as i64], |row| {
            Ok((named_at(row, 0)?, totals_at(row, 4)?))
        })?;
        let mut all = Vec::new();
        for row in rows {
            let (player, totals) = row?;
            if keep(player.account_id) {
                all.push((player, totals));
            }
        }
        Ok(all)
    }

    /// Everything counted, added up ([`AllTime`]): one pass over the
    /// players' totals and one over the matches.
    pub fn all_time(&self) -> Result<AllTime, PersistError> {
        let connection = self.reader();
        let columns = TOTALS
            .split(',')
            .map(|column| format!("SUM({})", column.trim()))
            .collect::<Vec<_>>()
            .join(", ");
        let (players, totals) = connection.query_row(
            &format!("SELECT COUNT(*), {columns} FROM g26_player_totals t WHERE t.matches > 0"),
            [],
            |row| Ok((u64_at(row, 0)?, totals_at(row, 1)?)),
        )?;
        let matches = connection.query_row(
            "SELECT COUNT(*) FROM g26_matches WHERE eligibility = 'counted'",
            [],
            |row| u64_at(row, 0),
        )?;
        Ok(AllTime {
            players,
            matches,
            totals,
        })
    }

    /// The top `limit` registered clans by `by`.
    pub fn clans(&self, by: TotalsKey, limit: usize) -> Result<Vec<ClanLine>, PersistError> {
        let connection = self.reader();
        let mut statement = connection.prepare_cached(&format!(
            "SELECT c.clan_id, c.name, c.tag, c.experience,
                 (SELECT COUNT(*) FROM g26_clan_members cm
                  WHERE cm.clan_id = c.clan_id AND cm.status = 0),
                 {TOTALS}
             FROM g26_clan_totals t JOIN g26_clans c ON c.clan_id = t.clan_id
             WHERE c.registered != 0
             ORDER BY t.{} DESC, t.clan_id LIMIT ?1",
            by.column()
        ))?;
        let rows = statement.query_map([bounded(limit)], clan_line)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// A registered clan's card.
    pub fn clan(&self, key: ClanKey) -> Result<Option<ClanCard>, PersistError> {
        let connection = self.reader();
        let (filter, value) = match key {
            ClanKey::Id(id) => ("c.clan_id = ?1", rusqlite::types::Value::from(id)),
            ClanKey::Tag(tag) => ("c.tag = ?1 COLLATE NOCASE", tag.to_owned().into()),
        };
        let found = connection
            .query_row(
                &format!(
                    "SELECT c.clan_id, c.name, c.tag, c.experience,
                         (SELECT COUNT(*) FROM g26_clan_members cm
                          WHERE cm.clan_id = c.clan_id AND cm.status = 0),
                         {TOTALS}, c.leader
                     FROM g26_clans c LEFT JOIN g26_clan_totals t ON t.clan_id = c.clan_id
                     WHERE {filter} AND c.registered != 0
                     ORDER BY c.clan_id LIMIT 1"
                ),
                [value],
                // c.leader: after the five of the clan and TOTALS.
                |row| Ok((clan_line(row)?, row.get::<_, u32>(5 + TOTALS_COLUMNS)?)),
            )
            .optional()?;
        let Some((clan, leader_id)) = found else {
            return Ok(None);
        };
        // Its accepted members and its commander (who is one of them), of
        // those with a public match.
        let mut statement = connection.prepare_cached(&format!(
            "SELECT {NAMED}, a.exp_total, a.rating, {TOTALS}
             FROM g26_clan_members m JOIN g26_accounts a ON a.account_id = m.account_id
             {CLAN_JOIN}
             JOIN g26_player_totals t ON t.account_id = a.account_id
             WHERE m.clan_id = ?1 AND (m.status = ?2 OR m.account_id = ?3)
             ORDER BY a.account_id != ?3, t.score DESC, a.account_id"
        ))?;
        let rows = statement.query_map(params![clan.id, status::ACCEPTED, leader_id], |row| {
            let player = named_at(row, 0)?;
            Ok(ClanMember {
                commander: player.account_id == leader_id,
                player,
                level: level_of(row.get(4)?),
                rating: row.get(5)?,
                totals: totals_at(row, 6)?,
            })
        })?;
        let members: Vec<ClanMember> = rows.collect::<Result<_, _>>()?;
        let leader = members
            .iter()
            .find(|member| member.commander)
            .map(|member| member.player.clone());
        Ok(Some(ClanCard {
            clan,
            leader,
            members,
        }))
    }
}

fn clan_line(row: &Row) -> rusqlite::Result<ClanLine> {
    let experience: u32 = row.get(3)?;
    Ok(ClanLine {
        id: row.get(0)?,
        name: row.get(1)?,
        tag: row.get(2)?,
        level: clans::level_of(experience),
        members: row.get(4)?,
        totals: totals_at(row, 5)?,
    })
}

/// [`MatchHistory::top`]'s query by `by`. Accuracy and the average score
/// read their partial indexes (`g26_player_totals_accuracy`,
/// `g26_player_totals_score_avg`): the expressions and the bounds are the
/// migration's, word for word. The other ratios sort the players with
/// their matches: a few thousand rows at most.
fn top_query(by: PlayerRanking) -> String {
    let ratio = |of: &str| {
        (
            format!("WHERE t.matches >= {SCORE_AVG_MIN_MATCHES}"),
            format!("{of} DESC, t.account_id"),
        )
    };
    let (filter, order) = match by {
        PlayerRanking::Rating => (
            "WHERE t.matches > 0".to_owned(),
            "a.rating DESC, a.account_id".to_owned(),
        ),
        PlayerRanking::Totals(key) => (
            "WHERE t.matches > 0".to_owned(),
            format!("t.{} DESC, t.account_id", key.column()),
        ),
        PlayerRanking::Accuracy => (
            format!("WHERE t.shots >= {ACCURACY_MIN_SHOTS}"),
            "CAST(t.hits AS REAL) / t.shots DESC, t.account_id".to_owned(),
        ),
        PlayerRanking::ScoreAvg => (
            format!("WHERE t.matches >= {SCORE_AVG_MIN_MATCHES}"),
            "CAST(t.score AS REAL) / t.matches DESC, t.account_id".to_owned(),
        ),
        PlayerRanking::WinRate => ratio("CAST(t.wins AS REAL) / t.matches"),
        PlayerRanking::Kd => ratio("CAST(t.kills AS REAL) / max(t.deaths, 1)"),
        PlayerRanking::HeadshotRate => ratio("CAST(t.headshots AS REAL) / max(t.kills, 1)"),
    };
    format!(
        "SELECT {NAMED}, a.exp_total, a.rating, {TOTALS}
         FROM g26_player_totals t JOIN g26_accounts a ON a.account_id = t.account_id
         {CLAN_JOIN}
         {filter}
         ORDER BY {order} LIMIT ?1"
    )
}

/// [`MatchHistory::best`]'s query: from the matches that ended in the
/// period (`g26_matches_by_end`) to their players, never the other way
/// round, which reads every player row (`CROSS JOIN` keeps the order).
fn best_query() -> String {
    format!(
        "SELECT {NAMED}, {SUMMARY}, {LINE}
         FROM g26_matches m CROSS JOIN g26_match_players p ON p.match_id = m.id
         JOIN g26_accounts a ON a.account_id = p.account_id
         {CLAN_JOIN}
         WHERE m.ended_at >= ?1 AND m.eligibility = 'counted'
         ORDER BY p.score DESC, m.id DESC LIMIT ?2"
    )
}

/// A page's upper bound: everything with `None`.
fn upper(before: Option<u64>) -> i64 {
    before.map_or(i64::MAX, |id| id.min(i64::MAX as u64) as i64)
}

/// A list's length, 1 to [`MatchHistory::MAX_ROWS`].
fn bounded(limit: usize) -> i64 {
    limit.clamp(1, MatchHistory::MAX_ROWS) as i64
}

#[cfg(test)]
mod tests {
    use super::super::{GameRepository, MIGRATIONS, SqliteGameRepository};
    use super::*;
    use crate::persist::DB_FILE;
    use crate::v026g::account::Account;
    use crate::v026g::clans::{Clan, Member};
    use survarium::v026g::maps::GameMode;

    struct TempDir(std::path::PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "srp-g26-history-{name}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn db(&self) -> std::path::PathBuf {
            self.0.join(DB_FILE)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// A line of a match team 0 won.
    fn line(team: u8, place: u8, score: u64, kills: u64, deaths: u64) -> PlayerLine {
        PlayerLine {
            team,
            place,
            seat: place,
            presence: Presence::Finished,
            outcome: if team == 0 {
                Outcome::Won
            } else {
                Outcome::Lost
            },
            rating: 100,
            score,
            kills,
            deaths,
            counters: Counters {
                assists: 1,
                headshots: 2,
                grenade_kills: 0,
                melee_kills: 0,
                batteries: 1,
                capture_hundredths: 50,
                artefacts: 3,
                shots: 100,
                hits: 40,
            },
            played_ms: 600_000,
        }
    }

    /// The AK (dict 155) in slot 7, the pistol (dict 60) in 10 and a
    /// helmet (dict 300) in 0.
    const AK: u16 = 155;
    const PISTOL: u16 = 60;
    const HELMET: u16 = 300;

    fn kit(place: u8, kills: u64, held: &[(usize, u16)]) -> KitLine {
        KitLine {
            kit: place,
            counts: KitCounts {
                kills,
                headshots: 1,
                shots: 100,
                hits: 40,
            },
            items: held
                .iter()
                .map(|&(slot, dict_id)| KitItem {
                    slot: GearSlot::of(slot).unwrap(),
                    dict_id,
                })
                .collect(),
        }
    }

    /// A grenade (dict 400): it kills, and fires no bullet.
    const GRENADE: u16 = 400;

    /// A player's weapons of a match: the AK with `shots` bullets, the
    /// pistol, and what its games counted under no weapon.
    fn weapons(kills: u64, shots: u64) -> BTreeMap<u16, WeaponCounts> {
        BTreeMap::from([
            (
                0,
                WeaponCounts {
                    deaths: 2,
                    hits: 2,
                    damage: 5_000,
                    held_ms: 50_000,
                    ..WeaponCounts::default()
                },
            ),
            (
                PISTOL,
                WeaponCounts {
                    kills: 4,
                    shots: 20,
                    hits: 8,
                    held_ms: 100_000,
                    ..WeaponCounts::default()
                },
            ),
            (
                AK,
                WeaponCounts {
                    kills,
                    headshots: 2,
                    wall_kills: 1,
                    ricochet_kills: 1,
                    deaths: 4,
                    shots,
                    hits: 30,
                    damage: 95_000,
                    kill_distance_dm: 2_500,
                    longest_kill_dm: 620,
                    held_ms: 400_000,
                },
            ),
        ])
    }

    /// A public match that counts.
    const PUBLIC: MatchKind = MatchKind::Public(Eligibility::Counted);

    /// Alice (10, of clan 7) beats Bob (11) on team 0's win; a match of
    /// its own (its replay id is new each call).
    fn record(kind: MatchKind, ended_at: u64) -> MatchRecord {
        static MATCHES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);
        MatchRecord {
            replay_id: ReplayId::new(
                MATCHES.fetch_add(1, Ordering::Relaxed),
                std::time::UNIX_EPOCH + std::time::Duration::from_secs(ended_at),
            ),
            kind,
            info: MatchInfo {
                ended_at,
                duration_ms: 845_000,
                map_id: MapId::new(3),
                mode: LobbyMode::Plain(GameMode::Batteries),
                match_level: 12,
                winner: Winner::FirstTeam,
                team_points: [5, 3],
            },
            players: vec![
                PlayerRecord {
                    account_id: 10,
                    clan_id: Some(7),
                    line: line(0, 1, 300, 14, 6),
                    kits: vec![
                        kit(0, 10, &[(0, HELMET), (7, AK), (10, PISTOL)]),
                        kit(2, 4, &[(7, PISTOL)]),
                    ],
                    weapons: weapons(10, 80),
                },
                PlayerRecord {
                    account_id: 11,
                    clan_id: None,
                    line: line(1, 1, 100, 6, 14),
                    kits: vec![kit(1, 6, &[(7, AK)])],
                    weapons: BTreeMap::new(),
                },
            ],
        }
    }

    /// A repository with Alice, Bob and Alice's clan, and its reader.
    fn repository(dir: &TempDir) -> (SqliteGameRepository, MatchHistory) {
        let mut repository = SqliteGameRepository::open(&dir.db()).unwrap();
        let mut alice = Account::new("Alice");
        alice.rating = 150;
        repository.save(10, &alice).unwrap();
        repository.save(11, &Account::new("Bob")).unwrap();
        repository
            .save_clan(&Clan {
                id: 7,
                name: "Skhid".to_owned(),
                tag: "SKH".to_owned(),
                registered: true,
                leader: 10,
                ranks: Vec::new(),
                members: vec![Member {
                    account_id: 10,
                    rank: 0,
                    status: status::ACCEPTED,
                }],
                daily_message: String::new(),
                experience: 0,
                money: 0,
            })
            .unwrap();
        let history = repository.history().unwrap().unwrap();
        (repository, history)
    }

    #[test]
    fn a_database_of_before_gets_the_tables_and_keeps_its_accounts() {
        let dir = TempDir::new("upgrade");
        let own = MIGRATIONS
            .iter()
            .position(|step| *step == MIGRATION)
            .expect("the step is in the list");
        {
            let connection = Connection::open(dir.db()).unwrap();
            connection
                .execute_batch(
                    "CREATE TABLE g26_meta (key TEXT PRIMARY KEY, value INTEGER NOT NULL);",
                )
                .unwrap();
            for earlier in &MIGRATIONS[..own] {
                connection.execute_batch(earlier).unwrap();
            }
            connection
                .execute(
                    "INSERT INTO g26_meta (key, value) VALUES ('schema', ?1)",
                    [own as i64],
                )
                .unwrap();
            // The columns of the first step; the later ones have defaults.
            connection
                .execute(
                    "INSERT INTO g26_accounts (
                        account_id, name, clan_tag, generic_money, premium_money, premium_until,
                        exp_total, quest_faction, rating, rating_before, active_kit, autobuy,
                        next_item_id, next_quest_instance, tutorial_step, tutorial_done
                     ) VALUES (10, 'Alice', '', 777, 0, 0, 0, 0, 1000, 1000, 1, 0, 1, 1, 0, 0)",
                    [],
                )
                .unwrap();
        }
        let mut repository = SqliteGameRepository::open(&dir.db()).unwrap();
        assert_eq!(repository.version().unwrap(), MIGRATIONS.len() as i64);
        assert_eq!(repository.load_all().unwrap().len(), 1);
        repository.record_match(&record(PUBLIC, 1000)).unwrap();
        let history = repository.history().unwrap().unwrap();
        let card = history.player(10).unwrap().unwrap();
        assert_eq!(card.totals.matches, 1);
        assert_eq!(
            (card.totals.counters.shots, card.totals.counters.hits),
            (100, 40)
        );
        assert_eq!(
            history.player_matches(10, None, 1).unwrap()[0].kits.len(),
            2
        );
        assert_eq!(history.favourites(10, 10).unwrap().weapons.len(), 2);
        assert_eq!(history.player_weapons(10, WeaponRanking::Kills, 10).unwrap().len(), 2);
    }

    #[test]
    fn a_players_weapons_add_up_over_its_counted_public_matches() {
        let dir = TempDir::new("weapons");
        let (mut repository, history) = repository(&dir);
        repository.record_match(&record(PUBLIC, 100)).unwrap();
        // A longer kill with the AK, and a grenade's (one kill, no shot).
        let mut second = record(PUBLIC, 200);
        let alice = &mut second.players[0].weapons;
        alice.get_mut(&AK).unwrap().longest_kill_dm = 900;
        alice.remove(&PISTOL);
        alice.insert(
            GRENADE,
            WeaponCounts {
                kills: 1,
                ..WeaponCounts::default()
            },
        );
        // An item only held.
        alice.insert(
            HELMET,
            WeaponCounts {
                held_ms: 4_000,
                ..WeaponCounts::default()
            },
        );
        repository.record_match(&second).unwrap();
        // Kept, never added up.
        for kind in [
            MatchKind::Private,
            MatchKind::VsBots,
            MatchKind::Public(Eligibility::Short),
        ] {
            repository.record_match(&record(kind, 300)).unwrap();
        }
        let first = weapons(10, 80);
        assert_eq!(
            history.player_weapons(10, WeaponRanking::Kills, 10).unwrap(),
            [
                WeaponTotals {
                    dict_id: AK,
                    matches: 2,
                    counts: WeaponCounts {
                        kills: 20,
                        headshots: 4,
                        wall_kills: 2,
                        ricochet_kills: 2,
                        deaths: 8,
                        shots: 160,
                        hits: 60,
                        damage: 190_000,
                        kill_distance_dm: 5_000,
                        longest_kill_dm: 900,
                        held_ms: 800_000,
                    },
                },
                WeaponTotals {
                    dict_id: PISTOL,
                    matches: 1,
                    counts: first[&PISTOL],
                },
                WeaponTotals {
                    dict_id: GRENADE,
                    matches: 1,
                    counts: WeaponCounts {
                        kills: 1,
                        ..WeaponCounts::default()
                    },
                },
            ]
        );
        assert_eq!(
            history
                .player_weapons(10, WeaponRanking::Kills, 1)
                .unwrap()
                .len(),
            1
        );
        assert!(
            history
                .player_weapons(99, WeaponRanking::Kills, 10)
                .unwrap()
                .is_empty()
        );
        // By any of its numbers, the largest first.
        let number = |by: WeaponRanking, counts: &WeaponCounts| -> f64 {
            let ratio = |of: u64, by: u64| {
                if by == 0 {
                    -1.0
                } else {
                    of as f64 / by as f64
                }
            };
            match by {
                WeaponRanking::Kills => counts.kills as f64,
                WeaponRanking::Kd => ratio(counts.kills, counts.deaths.max(1)),
                WeaponRanking::HeadshotRate => ratio(counts.headshots, counts.kills),
                WeaponRanking::Accuracy => ratio(counts.hits, counts.shots),
                WeaponRanking::KillDistance => ratio(counts.kill_distance_dm, counts.kills),
                WeaponRanking::WallKills => counts.wall_kills as f64,
                WeaponRanking::RicochetKills => counts.ricochet_kills as f64,
                WeaponRanking::Held => counts.held_ms as f64,
            }
        };
        for by in [
            WeaponRanking::Kills,
            WeaponRanking::Kd,
            WeaponRanking::HeadshotRate,
            WeaponRanking::Accuracy,
            WeaponRanking::KillDistance,
            WeaponRanking::WallKills,
            WeaponRanking::RicochetKills,
            WeaponRanking::Held,
        ] {
            let listed = history.player_weapons(10, by, 10).unwrap();
            assert_eq!(listed.len(), 3, "{by:?}");
            assert!(
                listed
                    .windows(2)
                    .all(|pair| number(by, &pair[0].counts) >= number(by, &pair[1].counts)),
                "{by:?}: {listed:?}"
            );
        }
        let rows = |table: &str, account_id: u32| -> i64 {
            history
                .reader()
                .query_row(
                    &format!("SELECT COUNT(*) FROM {table} WHERE account_id = ?1"),
                    [account_id],
                    |row| row.get(0),
                )
                .unwrap()
        };
        // Three weapons and no weapon in four matches, four in the second.
        assert_eq!(rows("g26_match_weapons", 10), 3 * 4 + 4);
        // The same with what was counted under no weapon, and the item held.
        assert_eq!(rows("g26_player_weapons", 10), 5);
    }

    #[test]
    fn a_read_in_progress_blocks_neither_the_writer_nor_other_reads() {
        let dir = TempDir::new("readers");
        let (mut repository, history) = repository(&dir);
        repository.record_match(&record(PUBLIC, 100)).unwrap();
        let count = |connection: &Connection| -> i64 {
            connection
                .query_row("SELECT COUNT(*) FROM g26_matches", [], |row| row.get(0))
                .unwrap()
        };
        let slow = history.reader();
        slow.execute_batch("BEGIN").unwrap();
        assert_eq!(count(&slow), 1);
        repository.record_match(&record(PUBLIC, 200)).unwrap();
        assert_eq!(history.player(10).unwrap().unwrap().totals.matches, 2);
        // The slow one still reads what it began with.
        assert_eq!(count(&slow), 1);
        slow.execute_batch("COMMIT").unwrap();
    }

    #[test]
    fn a_public_match_that_does_not_count_is_served_and_names_its_players_only() {
        let dir = TempDir::new("uncounted");
        let (mut repository, history) = repository(&dir);
        let short = MatchKind::Public(Eligibility::Short);
        repository.record_match(&record(short, 100)).unwrap();
        let (summary, players) = history.game_match(1).unwrap().unwrap();
        assert_eq!(summary.eligibility, Eligibility::Short);
        assert_eq!(players.len(), 2);
        assert_eq!(history.player_matches(10, None, 10).unwrap().len(), 1);
        assert_eq!(history.account_by_name("Alice").unwrap(), Some(10));
        assert_eq!(
            history.player(10).unwrap().unwrap().totals,
            Totals::default()
        );
        let everything = |_| true;
        assert!(
            history
                .top(PlayerRanking::Totals(TotalsKey::Kills), 10, &everything)
                .unwrap()
                .is_empty()
        );
        assert!(
            history
                .top(PlayerRanking::Rating, 10, &everything)
                .unwrap()
                .is_empty()
        );
        assert!(history.best(0, 10, &everything).unwrap().is_empty());
        assert!(history.player_days(10, 0).unwrap().is_empty());
        assert!(history.clans(TotalsKey::Matches, 10).unwrap().is_empty());
        assert_eq!(history.favourites(10, 10).unwrap(), Favourites::default());
        // The next one counts.
        repository.record_match(&record(PUBLIC, 200)).unwrap();
        assert_eq!(history.player(10).unwrap().unwrap().totals.matches, 1);
    }

    #[test]
    fn the_kits_used_are_kept_with_what_they_held() {
        let dir = TempDir::new("kits");
        let (mut repository, history) = repository(&dir);
        repository.record_match(&record(PUBLIC, 100)).unwrap();
        let alice_kits = vec![
            kit(0, 10, &[(0, HELMET), (7, AK), (10, PISTOL)]),
            kit(2, 4, &[(7, PISTOL)]),
        ];
        let matches = history.player_matches(10, None, 10).unwrap();
        assert_eq!(matches[0].kits, alice_kits);
        assert_eq!(
            (
                matches[0].line.counters.shots,
                matches[0].line.counters.hits
            ),
            (100, 40)
        );
        let (_, players) = history.game_match(1).unwrap().unwrap();
        assert_eq!(players[0].kits, alice_kits);
        assert_eq!(players[1].kits, [kit(1, 6, &[(7, AK)])]);
        // No kit kept: an empty list, the line still there.
        let mut bare = record(PUBLIC, 200);
        bare.players[1].kits.clear();
        repository.record_match(&bare).unwrap();
        let (_, players) = history.game_match(2).unwrap().unwrap();
        assert!(players[1].kits.is_empty());
        assert_eq!(players[1].line.kills, 6);
    }

    #[test]
    fn favourites_count_the_matches_an_item_was_held_in() {
        let dir = TempDir::new("favourites");
        let (mut repository, history) = repository(&dir);
        for (kind, at) in [
            (PUBLIC, 100),
            (PUBLIC, 200),
            // Not public: kept, not added up.
            (MatchKind::Private, 300),
        ] {
            repository.record_match(&record(kind, at)).unwrap();
        }
        let favourites = history.favourites(10, 10).unwrap();
        // The pistol is in both of Alice's kits: one match each time.
        let item = |dict_id| ItemTotals {
            dict_id,
            matches: 2,
        };
        assert_eq!(favourites.weapons, [item(AK), item(PISTOL)]);
        assert_eq!(favourites.armour.len(), 1);
        assert_eq!(
            (favourites.armour[0].dict_id, favourites.armour[0].matches),
            (HELMET, 2)
        );
        assert_eq!(history.favourites(10, 1).unwrap().weapons.len(), 1);
        assert_eq!(history.favourites(11, 10).unwrap().weapons[0].matches, 2);
        assert_eq!(history.favourites(99, 10).unwrap(), Favourites::default());
    }

    #[test]
    fn the_lists_read_past_more_banned_than_they_hold() {
        let dir = TempDir::new("banned");
        let (mut repository, history) = repository(&dir);
        repository.record_match(&record(PUBLIC, 100)).unwrap();
        let banned = 100..100 + MatchHistory::MAX_ROWS as u32 + 10;
        let mut theirs = record(PUBLIC, 200);
        theirs.players = banned
            .clone()
            .map(|account_id| {
                repository
                    .save(account_id, &Account::new(&format!("Banned{account_id}")))
                    .unwrap();
                PlayerRecord {
                    account_id,
                    clan_id: None,
                    line: line(0, 1, 1000 + u64::from(account_id), 50, 0),
                    kits: Vec::new(),
                    weapons: BTreeMap::new(),
                }
            })
            .collect();
        repository.record_match(&theirs).unwrap();
        let keep = |account_id| !banned.contains(&account_id);
        let names = |top: Vec<TopPlayer>| -> Vec<String> {
            top.into_iter().map(|top| top.player.name).collect()
        };
        let by_score = PlayerRanking::Totals(TotalsKey::Kills);
        let top = |limit| names(history.top(by_score, limit, &keep).unwrap());
        assert_eq!(top(10), ["Alice", "Bob"]);
        assert_eq!(top(1), ["Alice"]);
        let best: Vec<String> = history
            .best(0, 10, &keep)
            .unwrap()
            .into_iter()
            .map(|best| best.player.name)
            .collect();
        assert_eq!(best, ["Alice", "Bob"]);
        // Unfiltered, the banned fill the list.
        assert_eq!(
            history.top(by_score, 100, &|_| true).unwrap().len(),
            MatchHistory::MAX_ROWS
        );
    }

    #[test]
    fn the_accuracy_list_needs_its_shots() {
        let dir = TempDir::new("accuracy");
        let (mut repository, history) = repository(&dir);
        // Alice: 100 shots, 40 hits a match; Bob 20 hits of the same.
        let mut played = record(PUBLIC, 100);
        played.players[1].line.counters.hits = 20;
        for _ in 0..9 {
            repository
                .record_match(&MatchRecord {
                    replay_id: record(PUBLIC, 100).replay_id,
                    ..played.clone()
                })
                .unwrap();
        }
        let top = |history: &MatchHistory| -> Vec<String> {
            history
                .top(PlayerRanking::Accuracy, 10, &|_| true)
                .unwrap()
                .into_iter()
                .map(|top| top.player.name)
                .collect()
        };
        assert!(top(&history).is_empty(), "900 shots are not enough");
        repository.record_match(&played).unwrap();
        repository.record_match(&played).unwrap();
        assert_eq!(top(&history), ["Alice", "Bob"]);
        let accurate = history.top(PlayerRanking::Accuracy, 1, &|id| id != 10)
            .unwrap();
        assert_eq!(accurate[0].player.name, "Bob");
        assert_eq!(accurate[0].totals.matches, 10, "a match is kept once");
    }

    #[test]
    fn public_matches_are_served_and_added_up_the_others_only_kept() {
        let dir = TempDir::new("kinds");
        let (mut repository, history) = repository(&dir);
        repository.record_match(&record(PUBLIC, 1000)).unwrap();
        repository
            .record_match(&record(MatchKind::Private, 2000))
            .unwrap();
        repository
            .record_match(&record(MatchKind::VsBots, 3000))
            .unwrap();
        repository.record_match(&record(PUBLIC, 4000)).unwrap();

        let kept: i64 = Connection::open(dir.db())
            .unwrap()
            .query_row("SELECT COUNT(*) FROM g26_matches", [], |row| row.get(0))
            .unwrap();
        assert_eq!(kept, 4, "every match is kept");

        let card = history.player(10).unwrap().unwrap();
        assert_eq!(card.player.name, "Alice");
        assert_eq!(
            card.player.clan,
            Some(ClanRef {
                id: 7,
                tag: "SKH".to_owned()
            })
        );
        assert_eq!(card.rating, 150);
        assert_eq!(
            card.totals,
            Totals {
                matches: 2,
                wins: 2,
                losses: 0,
                score: 600,
                kills: 28,
                deaths: 12,
                counters: Counters {
                    assists: 2,
                    headshots: 4,
                    grenade_kills: 0,
                    melee_kills: 0,
                    batteries: 2,
                    capture_hundredths: 100,
                    artefacts: 6,
                    shots: 200,
                    hits: 80,
                },
                played_ms: 1_200_000,
            }
        );
        let bob = history.player(11).unwrap().unwrap();
        assert_eq!(
            (bob.totals.matches, bob.totals.wins, bob.totals.losses),
            (2, 0, 2)
        );
        assert_eq!(bob.player.clan, None);

        let ids: Vec<u64> = history.matches(None, 50, None)
            .unwrap()
            .iter()
            .map(|summary| summary.id)
            .collect();
        assert_eq!(ids, [4, 1]);
        assert!(history.game_match(2).unwrap().is_none());
        assert!(history.game_match(3).unwrap().is_none());
        let (summary, players) = history.game_match(4).unwrap().unwrap();
        assert_eq!(summary.players, 2);
        assert_eq!(summary.info.winner, Winner::FirstTeam);
        assert_eq!(summary.info.team_points, [5, 3]);
        assert_eq!(players[0].player.name, "Alice");
        assert_eq!(players[0].line, line(0, 1, 300, 14, 6));
        assert_eq!(players[1].line.team, 1);
        assert!(history.player(99).unwrap().is_none());
    }

    #[test]
    fn a_players_matches_come_by_page_newest_first() {
        let dir = TempDir::new("pages");
        let (mut repository, history) = repository(&dir);
        for at in 1..=5 {
            repository.record_match(&record(PUBLIC, at * 100)).unwrap();
        }
        repository
            .record_match(&record(MatchKind::Private, 600))
            .unwrap();
        let page: Vec<u64> = history.player_matches(10, None, 2)
            .unwrap()
            .iter()
            .map(|played| played.summary.id)
            .collect();
        assert_eq!(page, [5, 4]);
        let next: Vec<u64> = history.player_matches(10, Some(4), 2)
            .unwrap()
            .iter()
            .map(|played| played.summary.id)
            .collect();
        assert_eq!(next, [3, 2]);
        // A limit is 1 to 50.
        assert_eq!(history.player_matches(10, None, 0).unwrap().len(), 1);
        assert_eq!(history.matches(Some(3), 50, None).unwrap().len(), 2);
        // The clan's: Alice's matches, not Bob's alone.
        assert_eq!(history.matches(None, 50, Some(7)).unwrap().len(), 5);
        assert!(history.matches(None, 50, Some(8)).unwrap().is_empty());
    }

    #[test]
    fn days_add_up_by_utc_day() {
        let dir = TempDir::new("days");
        let (mut repository, history) = repository(&dir);
        const DAY: u64 = 86_400;
        for at in [DAY + 10, DAY + 20, 3 * DAY + 5] {
            repository.record_match(&record(PUBLIC, at)).unwrap();
        }
        let days = history.player_days(10, 0).unwrap();
        assert_eq!(days.len(), 2);
        assert_eq!(days[0].0, "1970-01-02");
        assert_eq!(
            (days[0].1.matches, days[0].1.wins, days[0].1.kills),
            (2, 2, 28)
        );
        assert_eq!(days[1].0, "1970-01-04");
        let since = history.player_days(10, 2 * DAY).unwrap();
        assert_eq!(since.len(), 1);
        let bob = history.player_days(11, 0).unwrap()[0].1;
        assert_eq!((bob.wins, bob.losses), (0, 2));
    }

    /// A line's outcome, said when the match is settled, is what the totals
    /// and the days count: one who left lost, and a draw is neither.
    #[test]
    fn wins_and_losses_are_the_lines_outcomes() {
        let dir = TempDir::new("outcomes");
        let (mut repository, history) = repository(&dir);
        // Alice left the match her team won.
        let mut left = record(PUBLIC, 100);
        left.players[0].line.presence = Presence::Left;
        left.players[0].line.outcome = Outcome::Lost;
        repository.record_match(&left).unwrap();
        let mut draw = record(PUBLIC, 200);
        draw.info.winner = Winner::Draw;
        for player in &mut draw.players {
            player.line.outcome = Outcome::Neither;
        }
        repository.record_match(&draw).unwrap();
        // A free-for-all: Bob had the most kills.
        let mut ffa = record(PUBLIC, 300);
        // A twist of it, kept as the lobby calls it.
        ffa.info.mode = LobbyMode::Racket;
        ffa.info.winner = Winner::MostKills;
        ffa.players[0].line.outcome = Outcome::Lost;
        ffa.players[1].line.outcome = Outcome::Won;
        repository.record_match(&ffa).unwrap();
        let of = |id| {
            let totals = history.player(id).unwrap().unwrap().totals;
            (totals.matches, totals.wins, totals.losses)
        };
        assert_eq!(of(10), (3, 0, 2));
        assert_eq!(of(11), (3, 1, 1));
        let day = history.player_days(10, 0).unwrap()[0].1;
        assert_eq!((day.matches, day.wins, day.losses), (3, 0, 2));
        let (summary, players) = history.game_match(3).unwrap().unwrap();
        assert_eq!(summary.info.winner, Winner::MostKills);
        assert_eq!(summary.info.mode, LobbyMode::Racket);
        assert_eq!(players[1].line.outcome, Outcome::Won);
    }

    #[test]
    fn a_periods_totals_and_all_time_add_up_the_counted_matches() {
        let dir = TempDir::new("periods");
        let (mut repository, history) = repository(&dir);
        for (kind, at) in [
            (PUBLIC, 100),
            (PUBLIC, 5_000),
            (PUBLIC, 6_000),
            // Neither counts: too short, and not public.
            (MatchKind::Public(Eligibility::Short), 7_000),
            (MatchKind::Private, 8_000),
        ] {
            repository.record_match(&record(kind, at)).unwrap();
        }
        let since = |from, keep: &dyn Fn(u32) -> bool| -> Vec<(String, u64, u64)> {
            let mut rows: Vec<_> = history
                .totals_since(from, keep)
                .unwrap()
                .into_iter()
                .map(|(player, totals)| (player.name, totals.matches, totals.kills))
                .collect();
            rows.sort();
            rows
        };
        assert_eq!(
            since(5_000, &|_| true),
            [("Alice".to_owned(), 2, 28), ("Bob".to_owned(), 2, 12)]
        );
        assert_eq!(since(5_000, &|id| id != 10), [("Bob".to_owned(), 2, 12)]);
        assert!(since(9_000, &|_| true).is_empty());
        let all = history.all_time().unwrap();
        assert_eq!((all.players, all.matches), (2, 3));
        assert_eq!(
            (all.totals.matches, all.totals.kills, all.totals.deaths),
            (6, 60, 60)
        );
        assert_eq!(all.totals.played_ms, 6 * 600_000);
    }

    #[test]
    fn the_top_lists_skip_who_is_left_out() {
        let dir = TempDir::new("top");
        let (mut repository, history) = repository(&dir);
        repository.record_match(&record(PUBLIC, 100)).unwrap();
        let by_kills = history.top(PlayerRanking::Totals(TotalsKey::Kills), 10, &|_| true)
            .unwrap();
        let names: Vec<&str> = by_kills
            .iter()
            .map(|top| top.player.name.as_str())
            .collect();
        assert_eq!(names, ["Alice", "Bob"]);
        let by_rating = history.top(PlayerRanking::Rating, 10, &|id| id != 10)
            .unwrap();
        assert_eq!(by_rating.len(), 1);
        assert_eq!(by_rating[0].player.name, "Bob");
        assert_eq!(
            history
                .top(PlayerRanking::Rating, 1, &|_| true)
                .unwrap()
                .len(),
            1
        );

        let best = history.best(0, 20, &|_| true).unwrap();
        assert_eq!(best[0].player.name, "Alice");
        assert_eq!(best[0].line.score, 300);
        assert!(history.best(101, 20, &|_| true).unwrap().is_empty());
        let without_alice = history.best(0, 1, &|id| id != 10).unwrap();
        assert_eq!(without_alice.len(), 1);
        assert_eq!(without_alice[0].player.name, "Bob");
    }

    #[test]
    fn a_clan_has_its_members_rows_added_up_and_its_members() {
        let dir = TempDir::new("clan");
        let (mut repository, history) = repository(&dir);
        repository.record_match(&record(PUBLIC, 100)).unwrap();
        repository
            .record_match(&record(MatchKind::Private, 200))
            .unwrap();
        let clans = history.clans(TotalsKey::Matches, 10).unwrap();
        assert_eq!(clans.len(), 1);
        assert_eq!(
            (clans[0].tag.as_str(), clans[0].level, clans[0].members),
            ("SKH", 1, 1)
        );
        assert_eq!((clans[0].totals.matches, clans[0].totals.score), (1, 300));

        for key in [ClanKey::Id(7), ClanKey::Tag("skh")] {
            let card = history.clan(key).unwrap().unwrap();
            assert_eq!(card.clan.name, "Skhid");
            assert_eq!(card.leader.unwrap().name, "Alice");
            assert_eq!(card.members.len(), 1);
            assert!(card.members[0].commander);
            assert_eq!(card.members[0].totals.kills, 14);
        }
        assert!(history.clan(ClanKey::Id(8)).unwrap().is_none());

        // Carol (12), its commander now, has no public match: counted, but
        // neither named nor listed.
        repository.save(12, &Account::new("Carol")).unwrap();
        let member = |account_id| Member {
            account_id,
            rank: 0,
            status: status::ACCEPTED,
        };
        repository
            .save_clan(&Clan {
                id: 7,
                name: "Skhid".to_owned(),
                tag: "SKH".to_owned(),
                registered: true,
                leader: 12,
                ranks: Vec::new(),
                members: vec![member(10), member(12)],
                daily_message: String::new(),
                experience: 0,
                money: 0,
            })
            .unwrap();
        let card = history.clan(ClanKey::Id(7)).unwrap().unwrap();
        assert_eq!(card.clan.members, 2);
        assert_eq!(card.leader, None);
        let names: Vec<&str> = card
            .members
            .iter()
            .map(|member| member.player.name.as_str())
            .collect();
        assert_eq!(names, ["Alice"]);
        assert!(!card.members[0].commander);
    }

    #[test]
    fn a_match_counts_once_for_a_clan_however_many_members_played() {
        let dir = TempDir::new("clan-once");
        let (mut repository, history) = repository(&dir);
        // Carol (12) and Dave (13) join Alice in clan 7.
        repository.save(12, &Account::new("Carol")).unwrap();
        repository.save(13, &Account::new("Dave")).unwrap();
        let member = |account_id| Member {
            account_id,
            rank: 0,
            status: status::ACCEPTED,
        };
        repository
            .save_clan(&Clan {
                id: 7,
                name: "Skhid".to_owned(),
                tag: "SKH".to_owned(),
                registered: true,
                leader: 10,
                ranks: Vec::new(),
                members: vec![member(10), member(12), member(13)],
                daily_message: String::new(),
                experience: 0,
                money: 0,
            })
            .unwrap();
        let of_clan = |account_id, line| PlayerRecord {
            account_id,
            clan_id: Some(7),
            line,
            kits: Vec::new(),
            weapons: BTreeMap::new(),
        };
        let with = |winner, players: Vec<PlayerRecord>| {
            let mut played = record(PUBLIC, 100);
            played.info.winner = winner;
            played.players = players;
            played
        };
        let bob = |team| PlayerRecord {
            account_id: 11,
            clan_id: None,
            line: line(team, 1, 100, 6, 14),
            kits: Vec::new(),
            weapons: BTreeMap::new(),
        };
        for played in [
            // Three members on the winning team: one match, one win.
            with(
                Winner::FirstTeam,
                vec![
                    of_clan(10, line(0, 1, 300, 14, 6)),
                    of_clan(12, line(0, 2, 200, 8, 5)),
                    of_clan(13, line(0, 3, 100, 3, 7)),
                    bob(1),
                ],
            ),
            // Members on both teams: one match, neither won nor lost.
            with(
                Winner::FirstTeam,
                vec![
                    of_clan(10, line(0, 1, 300, 10, 4)),
                    of_clan(12, line(1, 1, 250, 9, 9)),
                ],
            ),
            // Two members, both on the losing team: one match, no win.
            with(
                Winner::FirstTeam,
                vec![
                    bob(0),
                    of_clan(10, line(1, 1, 50, 1, 10)),
                    of_clan(13, line(1, 2, 40, 2, 11)),
                ],
            ),
            // A draw: one match, no win.
            with(
                Winner::Draw,
                vec![of_clan(
                    12,
                    PlayerLine {
                        outcome: Outcome::Neither,
                        ..line(0, 1, 10, 0, 0)
                    },
                )],
            ),
            // Not public: never counted.
            MatchRecord {
                kind: MatchKind::Private,
                ..with(Winner::FirstTeam, vec![of_clan(10, line(0, 1, 999, 99, 0))])
            },
        ] {
            repository.record_match(&played).unwrap();
        }

        let totals = history.clan(ClanKey::Id(7)).unwrap().unwrap().clan.totals;
        assert_eq!(
            totals,
            Totals {
                matches: 4,
                wins: 1,
                losses: 1,
                score: 300 + 200 + 100 + 300 + 250 + 50 + 40 + 10,
                kills: 14 + 8 + 3 + 10 + 9 + 1 + 2,
                deaths: 6 + 5 + 7 + 4 + 9 + 10 + 11,
                counters: Counters {
                    assists: 8,
                    headshots: 16,
                    grenade_kills: 0,
                    melee_kills: 0,
                    batteries: 8,
                    capture_hundredths: 400,
                    artefacts: 24,
                    shots: 800,
                    hits: 320,
                },
                played_ms: 4_800_000,
            }
        );
        // The list of clans and the clan's matches agree with the card.
        assert_eq!(
            history.clans(TotalsKey::Matches, 10).unwrap()[0].totals,
            totals
        );
        let listed = history.matches(None, 50, Some(7)).unwrap();
        assert_eq!(listed.len() as u64, totals.matches);
        let ids: Vec<u64> = listed.iter().map(|summary| summary.id).collect();
        assert_eq!(ids, [4, 3, 2, 1]);
        // Each member keeps its own matches.
        let matches_of = |id| history.player(id).unwrap().unwrap().totals;
        assert_eq!((matches_of(10).matches, matches_of(10).wins), (3, 2));
        assert_eq!((matches_of(12).matches, matches_of(12).wins), (3, 1));
        assert_eq!((matches_of(13).matches, matches_of(13).wins), (2, 1));
    }

    #[test]
    fn a_player_is_found_by_its_name_once_it_played_a_public_match() {
        let dir = TempDir::new("name");
        let (mut repository, history) = repository(&dir);
        // No public match yet: names are most often logins, none is found.
        assert_eq!(history.account_by_name("alice").unwrap(), None);
        assert!(history.player(10).unwrap().is_none());
        repository
            .record_match(&record(MatchKind::Private, 100))
            .unwrap();
        assert_eq!(history.account_by_name("alice").unwrap(), None);
        assert!(history.player(10).unwrap().is_none());
        repository.record_match(&record(PUBLIC, 200)).unwrap();
        assert_eq!(history.account_by_name("alice").unwrap(), Some(10));
        assert_eq!(history.account_by_name("Bob").unwrap(), Some(11));
        assert_eq!(history.account_by_name("Carol").unwrap(), None);
        // Carol is an account, but has played no public match.
        repository.save(12, &Account::new("Carol")).unwrap();
        assert_eq!(history.account_by_name("Carol").unwrap(), None);
        assert!(history.player(12).unwrap().is_none());
    }

    #[test]
    fn the_queries_use_their_indexes() {
        let dir = TempDir::new("plans");
        let (_repository, _history) = repository(&dir);
        let connection = Connection::open(dir.db()).unwrap();
        let plan = |sql: &str| -> String {
            let mut statement = connection
                .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
                .unwrap();
            let rows = statement
                .query_map([], |row| row.get::<_, String>(3))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            rows.join("; ")
        };
        for (sql, index) in [
            (
                "SELECT * FROM g26_match_players p WHERE p.account_id = 1 AND p.match_id < 9
                 ORDER BY p.match_id DESC LIMIT 5",
                "g26_match_players_by_account",
            ),
            (
                "SELECT id FROM g26_matches m WHERE m.kind = 'public' AND m.id < 9
                 ORDER BY m.id DESC LIMIT 5",
                "",
            ),
            (
                "SELECT * FROM g26_player_totals t ORDER BY t.kills DESC LIMIT 5",
                "g26_player_totals_kills",
            ),
            (
                "SELECT a.account_id FROM g26_accounts a
                 JOIN g26_player_totals t ON t.account_id = a.account_id
                 WHERE a.name = 'x' COLLATE NOCASE ORDER BY a.account_id LIMIT 1",
                "g26_accounts_by_name",
            ),
            (
                "SELECT id FROM g26_matches WHERE ended_at >= 5 ORDER BY ended_at LIMIT 1",
                "g26_matches_by_end",
            ),
        ] {
            let plan = plan(sql);
            assert!(
                !plan.contains("SCAN") || plan.contains("USING"),
                "{sql}: {plan}"
            );
            assert!(!plan.contains("TEMP B-TREE"), "{sql}: {plan}");
            assert!(plan.contains(index), "{sql}: {plan}");
        }
    }

    #[test]
    fn the_best_scores_start_from_the_matches_of_the_period() {
        let dir = TempDir::new("best-plan");
        let (_repository, _history) = repository(&dir);
        let connection = Connection::open(dir.db()).unwrap();
        let mut statement = connection
            .prepare(&format!("EXPLAIN QUERY PLAN {}", best_query()))
            .unwrap();
        let plan = statement
            .query_map(params![0, 20], |row| row.get::<_, String>(3))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
            .join("; ");
        assert!(
            plan.contains("SEARCH m USING INDEX g26_matches_by_end (ended_at>?)"),
            "{plan}"
        );
        // The players of those matches only, by the primary key.
        assert!(!plan.contains("SCAN p"), "{plan}");
        assert!(plan.contains("SEARCH p USING PRIMARY KEY"), "{plan}");
    }

    fn plan_of(dir: &TempDir, sql: &str, values: impl rusqlite::Params) -> String {
        let connection = Connection::open(dir.db()).unwrap();
        let mut statement = connection
            .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
            .unwrap();
        statement
            .query_map(values, |row| row.get::<_, String>(3))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
            .join("; ")
    }

    #[test]
    fn the_kits_and_items_queries_use_their_keys() {
        let dir = TempDir::new("kit-plans");
        let (_repository, _history) = repository(&dir);
        for (sql, values) in [
            (kits_query(""), vec![5_i64]),
            (kits_query("AND k.account_id = ?2"), vec![5, 10]),
        ] {
            let plan = plan_of(&dir, &sql, rusqlite::params_from_iter(values));
            assert!(
                plan.contains("SEARCH k USING PRIMARY KEY (match_id=?"),
                "{plan}"
            );
            assert!(
                plan.contains("SEARCH i USING PRIMARY KEY (match_id=? AND account_id=? AND kit=?)"),
                "{plan}"
            );
            assert!(!plan.contains("SCAN"), "{plan}");
            assert!(!plan.contains("TEMP B-TREE"), "{plan}");
        }
        let plan = plan_of(&dir, FAVOURITES, params![10, "weapon", 10]);
        assert!(
            plan.contains("INDEX g26_player_items_by_matches"),
            "{plan}"
        );
        assert!(!plan.contains("TEMP B-TREE"), "{plan}");
    }

    #[test]
    fn the_accuracy_and_average_score_lists_read_their_partial_indexes() {
        let dir = TempDir::new("accuracy-plan");
        let (_repository, _history) = repository(&dir);
        for (by, index) in [
            (PlayerRanking::Accuracy, "g26_player_totals_accuracy"),
            (PlayerRanking::ScoreAvg, "g26_player_totals_score_avg"),
        ] {
            let plan = plan_of(&dir, &top_query(by), [10]);
            assert!(plan.contains(&format!("SCAN t USING INDEX {index}")), "{plan}");
            // Only the ties are sorted further, by account.
            assert!(!plan.contains("USE TEMP B-TREE FOR ORDER BY"), "{plan}");
        }
    }

    #[test]
    fn the_average_score_list_needs_its_matches() {
        let dir = TempDir::new("score-avg");
        let (mut repository, history) = repository(&dir);
        let top = |history: &MatchHistory| -> Vec<String> {
            history
                .top(PlayerRanking::ScoreAvg, 10, &|_| true)
                .unwrap()
                .into_iter()
                .map(|top| top.player.name)
                .collect()
        };
        for at in 1..SCORE_AVG_MIN_MATCHES {
            repository.record_match(&record(PUBLIC, at * 100)).unwrap();
        }
        assert!(top(&history).is_empty(), "one match short");
        repository.record_match(&record(PUBLIC, 9_000)).unwrap();
        assert_eq!(top(&history), ["Alice", "Bob"]);
        // The other ratios list the same players, the better first.
        for by in [
            PlayerRanking::WinRate,
            PlayerRanking::Kd,
            PlayerRanking::HeadshotRate,
        ] {
            let listed = history.top(by, 10, &|_| true).unwrap();
            assert_eq!(listed.len(), 2, "{by:?}");
            let of = |totals: &Totals| match by {
                PlayerRanking::WinRate => totals.wins as f64 / totals.matches as f64,
                PlayerRanking::Kd => totals.kills as f64 / totals.deaths.max(1) as f64,
                _ => totals.counters.headshots as f64 / totals.kills.max(1) as f64,
            };
            assert!(of(&listed[0].totals) >= of(&listed[1].totals), "{by:?}");
        }
    }
}
