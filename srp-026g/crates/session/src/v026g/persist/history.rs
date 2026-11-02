//! The match history in srp's SQLite file (`docs/notes/match_history.md`):
//! every finished match ([`MatchRecord`], made by
//! [`crate::v026g::rewards::Results::settle`]) and its players, with the
//! public matches' totals by player and by clan added up as each is
//! written. [`MatchHistory`] reads them for the site, on a connection of its
//! own: only public matches are ever read back.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Mutex;

use rusqlite::{Connection, OpenFlags, OptionalExtension, Row, Transaction, params};

use crate::LockExt;
use crate::persist::PersistError;
use crate::v026g::account::level_at;
use crate::v026g::clans::{self, status};

/// The schema step that adds the history. A database of before has no
/// matches: the history starts with the first match after it.
pub(super) const MIGRATION: &str = "
    CREATE TABLE g26_matches (
        -- The history's own number, counting up: the site's match id.
        id            INTEGER PRIMARY KEY,
        -- The match server's id: random, it can come again after a restart.
        game_match_id INTEGER NOT NULL,
        -- Unix seconds.
        ended_at      INTEGER NOT NULL,
        duration_ms   INTEGER NOT NULL,
        -- maps_dict's id.
        map_id        INTEGER NOT NULL,
        game_mode     INTEGER NOT NULL,
        match_level   INTEGER NOT NULL,
        kind          TEXT NOT NULL CHECK (kind IN ('public', 'private', 'vs_bots')),
        winner        TEXT NOT NULL CHECK (winner IN ('team0', 'team1', 'draw', 'unknown')),
        team0_points  INTEGER NOT NULL,
        team1_points  INTEGER NOT NULL
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
        -- In its team by score, from 1.
        place         INTEGER NOT NULL,
        presence      TEXT NOT NULL CHECK (presence IN ('finished', 'left')),
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
        PRIMARY KEY (match_id, account_id)
    ) WITHOUT ROWID;
    CREATE INDEX g26_match_players_by_account ON g26_match_players (account_id, match_id);
    CREATE INDEX g26_match_players_by_clan ON g26_match_players (clan_id, match_id)
        WHERE clan_id IS NOT NULL;
    -- The public matches added up, by player and by clan.
    CREATE TABLE g26_player_totals (
        account_id    INTEGER PRIMARY KEY,
        matches       INTEGER NOT NULL,
        wins          INTEGER NOT NULL,
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
        hits          INTEGER NOT NULL
    );
    CREATE INDEX g26_player_totals_matches ON g26_player_totals (matches);
    CREATE INDEX g26_player_totals_wins ON g26_player_totals (wins);
    CREATE INDEX g26_player_totals_kills ON g26_player_totals (kills);
    CREATE INDEX g26_player_totals_score ON g26_player_totals (score);
    CREATE INDEX g26_player_totals_headshots ON g26_player_totals (headshots);
    CREATE INDEX g26_player_totals_batteries ON g26_player_totals (batteries);
    CREATE INDEX g26_player_totals_artefacts ON g26_player_totals (artefacts);
    -- The accuracy top list: the players with at least 1000 shots
    -- (`ACCURACY_MIN_SHOTS`, the same number).
    CREATE INDEX g26_player_totals_accuracy ON g26_player_totals (CAST(hits AS REAL) / shots)
        WHERE shots >= 1000;
    -- A match counts once for a clan, however many of its members played:
    -- a win if one of them was on the winning team; score, kills and the
    -- counters are the members' rows added up.
    CREATE TABLE g26_clan_totals (
        clan_id       INTEGER PRIMARY KEY,
        matches       INTEGER NOT NULL,
        wins          INTEGER NOT NULL,
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
        hits          INTEGER NOT NULL
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
    -- The public matches added up by player and item: the matches in
    -- which a kit it used held the item, and those kits' counters.
    CREATE TABLE g26_player_items (
        account_id    INTEGER NOT NULL,
        dict_id       INTEGER NOT NULL,
        gear          TEXT NOT NULL CHECK (gear IN ('weapon', 'armour')),
        matches       INTEGER NOT NULL,
        kills         INTEGER NOT NULL,
        headshots     INTEGER NOT NULL,
        shots         INTEGER NOT NULL,
        hits          INTEGER NOT NULL,
        PRIMARY KEY (account_id, dict_id)
    ) WITHOUT ROWID;
    CREATE INDEX g26_player_items_by_matches ON g26_player_items (account_id, gear, matches);
    -- What the site looks a player up by, and a player's clan.
    CREATE INDEX g26_accounts_by_name ON g26_accounts (name COLLATE NOCASE);
    CREATE INDEX g26_accounts_by_rating ON g26_accounts (rating);
    CREATE INDEX g26_clan_members_by_account ON g26_clan_members (account_id);
    ";

/// Who could join a match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchKind {
    /// Matchmaking or an open room: the only kind ever served.
    Public,
    /// A room with a PIN.
    Private,
    /// One player against the server's bots.
    VsBots,
}

impl MatchKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Private => "private",
            Self::VsBots => "vs_bots",
        }
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

    /// The team that won, if one did.
    pub fn team(self) -> Option<u8> {
        match self {
            Self::FirstTeam => Some(0),
            Self::SecondTeam => Some(1),
            Self::Draw | Self::Unknown => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::FirstTeam => "team0",
            Self::SecondTeam => "team1",
            Self::Draw => "draw",
            Self::Unknown => "unknown",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "team0" => Self::FirstTeam,
            "team1" => Self::SecondTeam,
            "draw" => Self::Draw,
            "unknown" => Self::Unknown,
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

/// What a kit's counters say of one match, or of the matches added up.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KitCounts {
    pub kills: u64,
    pub headshots: u64,
    pub shots: u64,
    pub hits: u64,
}

impl KitCounts {
    fn plus(self, more: Self) -> Self {
        Self {
            kills: self.kills + more.kills,
            headshots: self.headshots + more.headshots,
            shots: self.shots + more.shots,
            hits: self.hits + more.hits,
        }
    }
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
/// (`docs/notes/match_history.md`, "Kits": which kits count as used).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KitLine {
    /// Its place among the account's kits, 0..=2: the counters' kit.
    pub kit: u8,
    pub counts: KitCounts,
    /// By slot.
    pub items: Vec<KitItem>,
}

/// An item's public matches added up for a player: the matches in which a
/// kit it used held it, and those kits' counters (a kit's, not the item's
/// own: a kit of two weapons gives both its kills).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemTotals {
    pub dict_id: u16,
    pub matches: u64,
    pub counts: KitCounts,
}

/// A player's line of one match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerLine {
    pub team: u8,
    /// In its team by score, from 1.
    pub place: u8,
    pub presence: Presence,
    /// The visible rating after the match.
    pub rating: u16,
    pub score: u64,
    pub kills: u64,
    pub deaths: u64,
    pub counters: Counters,
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
}

/// What a match was, whoever played it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MatchInfo {
    /// Unix seconds.
    pub ended_at: u64,
    pub duration_ms: u32,
    /// maps_dict's id.
    pub map_id: u8,
    pub game_mode: u8,
    pub match_level: u16,
    pub winner: Winner,
    pub team_points: [u16; 2],
}

/// A finished match as the history keeps it: no bots in it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchRecord {
    pub game_match_id: u32,
    pub kind: MatchKind,
    pub info: MatchInfo,
    pub players: Vec<PlayerRecord>,
}

/// Public matches added up.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Totals {
    pub matches: u64,
    pub wins: u64,
    pub score: u64,
    pub kills: u64,
    pub deaths: u64,
    pub counters: Counters,
}

/// A total the top lists sort by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TotalsKey {
    Matches,
    Wins,
    Kills,
    Score,
    Headshots,
    Batteries,
    Artefacts,
}

impl TotalsKey {
    /// The column, of `g26_player_totals` and `g26_clan_totals` alike.
    fn column(self) -> &'static str {
        match self {
            Self::Matches => "matches",
            Self::Wins => "wins",
            Self::Kills => "kills",
            Self::Score => "score",
            Self::Headshots => "headshots",
            Self::Batteries => "batteries",
            Self::Artefacts => "artefacts",
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MatchSummary {
    /// The history's number.
    pub id: u64,
    pub info: MatchInfo,
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

/// Writes a match and adds a public one to the totals, in one
/// transaction.
pub(super) fn record(
    connection: &mut Connection,
    record: &MatchRecord,
) -> Result<(), PersistError> {
    let transaction = connection.transaction()?;
    let info = &record.info;
    transaction.execute(
        "INSERT INTO g26_matches (game_match_id, ended_at, duration_ms, map_id, game_mode,
             match_level, kind, winner, team0_points, team1_points)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            record.game_match_id,
            info.ended_at.min(i64::MAX as u64) as i64,
            info.duration_ms,
            info.map_id,
            info.game_mode,
            info.match_level,
            record.kind.as_str(),
            info.winner.as_str(),
            info.team_points[0],
            info.team_points[1],
        ],
    )?;
    let match_id = transaction.last_insert_rowid();
    // Each clan's line of a public match: one match, whoever of it played.
    let mut clans: BTreeMap<u32, Totals> = BTreeMap::new();
    for player in &record.players {
        let line = &player.line;
        let counters = &line.counters;
        transaction.execute(
            "INSERT INTO g26_match_players (match_id, account_id, clan_id, team, place, presence,
                 rating, score, kills, deaths, assists, headshots, grenade_kills, melee_kills,
                 batteries, captures, artefacts, shots, hits)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16,
                 ?17, ?18, ?19)",
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
            ],
        )?;
        record_kits(&transaction, match_id, player)?;
        if record.kind == MatchKind::Public {
            let own = Totals::of_match(line, info.winner.team() == Some(line.team));
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
    }
    for (clan_id, totals) in &clans {
        add_totals(&transaction, "g26_clan_totals", "clan_id", *clan_id, totals)?;
    }
    transaction.commit()?;
    Ok(())
}

impl Totals {
    /// A player's line of one match: one match, a win if its team won.
    fn of_match(line: &PlayerLine, won: bool) -> Self {
        Self {
            matches: 1,
            wins: u64::from(won),
            score: line.score,
            kills: line.kills,
            deaths: line.deaths,
            counters: line.counters,
        }
    }

    /// A clan's one match with another member's line of it added: still
    /// one match, a win if any member's team won, the rest added up.
    fn with_member(&self, member: &Self) -> Self {
        let (own, more) = (&self.counters, &member.counters);
        Self {
            matches: 1,
            wins: self.wins.max(member.wins),
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

/// Adds a public match to `player`'s item totals: each item once, with
/// the counters of every kit it used that held it (a kit holding an item
/// twice counts once).
fn add_items(transaction: &Transaction, player: &PlayerRecord) -> Result<(), PersistError> {
    let mut items: BTreeMap<u16, (Gear, KitCounts)> = BTreeMap::new();
    for kit in &player.kits {
        let mut held: Vec<(u16, Gear)> = kit
            .items
            .iter()
            .map(|item| (item.dict_id, item.slot.gear()))
            .collect();
        held.sort_unstable_by_key(|&(dict_id, _)| dict_id);
        held.dedup_by_key(|&mut (dict_id, _)| dict_id);
        for (dict_id, gear) in held {
            items
                .entry(dict_id)
                .and_modify(|(_, counts)| *counts = counts.plus(kit.counts))
                .or_insert((gear, kit.counts));
        }
    }
    for (dict_id, (gear, counts)) in items {
        transaction.execute(
            "INSERT INTO g26_player_items (account_id, dict_id, gear, matches, kills, headshots,
                 shots, hits)
             VALUES (?1, ?2, ?3, 1, ?4, ?5, ?6, ?7)
             ON CONFLICT(account_id, dict_id) DO UPDATE SET
                 matches = matches + 1, kills = kills + excluded.kills,
                 headshots = headshots + excluded.headshots, shots = shots + excluded.shots,
                 hits = hits + excluded.hits",
            params![
                player.account_id,
                dict_id,
                gear.as_str(),
                counts.kills as i64,
                counts.headshots as i64,
                counts.shots as i64,
                counts.hits as i64,
            ],
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
            "INSERT INTO {table} ({key}, matches, wins, score, kills, deaths, assists, headshots,
                 grenade_kills, melee_kills, batteries, captures, artefacts, shots, hits)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
             ON CONFLICT({key}) DO UPDATE SET
                 matches = matches + excluded.matches, wins = wins + excluded.wins,
                 score = score + excluded.score, kills = kills + excluded.kills,
                 deaths = deaths + excluded.deaths, assists = assists + excluded.assists,
                 headshots = headshots + excluded.headshots,
                 grenade_kills = grenade_kills + excluded.grenade_kills,
                 melee_kills = melee_kills + excluded.melee_kills,
                 batteries = batteries + excluded.batteries,
                 captures = captures + excluded.captures,
                 artefacts = artefacts + excluded.artefacts,
                 shots = shots + excluded.shots, hits = hits + excluded.hits"
        ),
        params![
            id,
            totals.matches as i64,
            totals.wins as i64,
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
/// The columns of a [`MatchSummary`] of `m` (nine).
const SUMMARY: &str = "m.id, m.ended_at, m.duration_ms, m.map_id, m.game_mode, m.match_level,
     m.winner, m.team0_points, m.team1_points,
     (SELECT COUNT(*) FROM g26_match_players n WHERE n.match_id = m.id)";
/// The columns of a [`PlayerLine`] of `p` (sixteen).
const LINE: &str = "p.team, p.place, p.presence, p.rating, p.score, p.kills, p.deaths, p.assists,
     p.headshots, p.grenade_kills, p.melee_kills, p.batteries, p.captures, p.artefacts, p.shots,
     p.hits";
/// The columns of [`Totals`] of `t` (fourteen).
const TOTALS: &str = "t.matches, t.wins, t.score, t.kills, t.deaths, t.assists, t.headshots,
     t.grenade_kills, t.melee_kills, t.batteries, t.captures, t.artefacts, t.shots, t.hits";
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
const FAVOURITES: &str = "SELECT dict_id, matches, kills, headshots, shots, hits
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
    Ok(MatchSummary {
        id: u64_at(row, at)?,
        info: MatchInfo {
            ended_at: u64_at(row, at + 1)?,
            duration_ms: row.get(at + 2)?,
            map_id: row.get(at + 3)?,
            game_mode: row.get(at + 4)?,
            match_level: row.get(at + 5)?,
            winner: Winner::parse(&winner).ok_or_else(|| bad("winner", &winner))?,
            team_points: [row.get(at + 7)?, row.get(at + 8)?],
        },
        players: row.get(at + 9)?,
    })
}

fn line_at(row: &Row, at: usize) -> rusqlite::Result<PlayerLine> {
    let presence: String = row.get(at + 2)?;
    Ok(PlayerLine {
        team: row.get(at)?,
        place: row.get(at + 1)?,
        presence: Presence::parse(&presence).ok_or_else(|| bad("presence", &presence))?,
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
    })
}

/// [`Totals`] at `at`.
fn totals_at(row: &Row, at: usize) -> rusqlite::Result<Totals> {
    Ok(Totals {
        matches: u64_at(row, at)?,
        wins: u64_at(row, at + 1)?,
        score: u64_at(row, at + 2)?,
        kills: u64_at(row, at + 3)?,
        deaths: u64_at(row, at + 4)?,
        counters: Counters {
            assists: u64_at(row, at + 5)?,
            headshots: u64_at(row, at + 6)?,
            grenade_kills: u64_at(row, at + 7)?,
            melee_kills: u64_at(row, at + 8)?,
            batteries: u64_at(row, at + 9)?,
            capture_hundredths: u64_at(row, at + 10)?,
            artefacts: u64_at(row, at + 11)?,
            shots: u64_at(row, at + 12)?,
            hits: u64_at(row, at + 13)?,
        },
    })
}

fn level_of(experience: u32) -> u8 {
    level_at(experience).0
}

/// The history as the site reads it: public matches only, on a read-only
/// connection of its own, so a request never waits for the accounts'
/// writer (the file is in WAL mode). Every query is an indexed lookup of
/// at most [`MatchHistory::MAX_ROWS`] rows.
pub struct MatchHistory {
    connection: Mutex<Connection>,
}

impl MatchHistory {
    /// The most rows a list gives.
    pub const MAX_ROWS: usize = 50;

    /// Opens the file at `path`, which [`super::SqliteGameRepository`]
    /// made and brought to this schema.
    pub(super) fn open(path: &Path) -> Result<Self, PersistError> {
        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        connection.execute_batch("PRAGMA busy_timeout = 2000;")?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    /// The account with this in-game name (Latin letters in any case), of
    /// those with a public match: a name is most often still the login, so
    /// no other account is ever found.
    pub fn account_by_name(&self, name: &str) -> Result<Option<u32>, PersistError> {
        Ok(self
            .connection
            .locked()
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
            .connection
            .locked()
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
    /// `None`), newest first, at most `limit`.
    pub fn player_matches(
        &self,
        account_id: u32,
        before: Option<u64>,
        limit: usize,
    ) -> Result<Vec<PlayerMatch>, PersistError> {
        let connection = self.connection.locked();
        let mut statement = connection.prepare_cached(&format!(
            "SELECT {SUMMARY}, {LINE}
             FROM g26_match_players p JOIN g26_matches m ON m.id = p.match_id
             WHERE p.account_id = ?1 AND p.match_id < ?2 AND m.kind = 'public'
             ORDER BY p.match_id DESC LIMIT ?3"
        ))?;
        let rows = statement
            .query_map(params![account_id, upper(before), bounded(limit)], |row| {
                Ok((summary_at(row, 0)?, line_at(row, 10)?))
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
        let connection = self.connection.locked();
        let mut statement = connection.prepare_cached(FAVOURITES)?;
        let mut of = |gear: Gear| -> rusqlite::Result<Vec<ItemTotals>> {
            statement
                .query_map(params![account_id, gear.as_str(), bounded(limit)], |row| {
                    Ok(ItemTotals {
                        dict_id: row.get(0)?,
                        matches: u64_at(row, 1)?,
                        counts: counts_at(row, 2)?,
                    })
                })?
                .collect()
        };
        Ok(Favourites {
            weapons: of(Gear::Weapon)?,
            armour: of(Gear::Armour)?,
        })
    }

    /// The account's public matches that ended at `since` or later, added
    /// up by UTC day (`YYYY-MM-DD`), oldest first.
    pub fn player_days(
        &self,
        account_id: u32,
        since: u64,
    ) -> Result<Vec<(String, Totals)>, PersistError> {
        let connection = self.connection.locked();
        let mut statement = connection.prepare_cached(
            "SELECT date(m.ended_at, 'unixepoch') AS day, COUNT(*),
                 SUM(CASE WHEN (m.winner = 'team0' AND p.team = 0)
                           OR (m.winner = 'team1' AND p.team = 1) THEN 1 ELSE 0 END),
                 SUM(p.score), SUM(p.kills), SUM(p.deaths), SUM(p.assists), SUM(p.headshots),
                 SUM(p.grenade_kills), SUM(p.melee_kills), SUM(p.batteries), SUM(p.captures),
                 SUM(p.artefacts), SUM(p.shots), SUM(p.hits)
             FROM g26_match_players p JOIN g26_matches m ON m.id = p.match_id
             WHERE p.account_id = ?1 AND m.kind = 'public' AND m.ended_at >= ?2
               AND p.match_id >= IFNULL((SELECT id FROM g26_matches WHERE ended_at >= ?2
                   ORDER BY ended_at LIMIT 1), 9223372036854775807)
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
        let connection = self.connection.locked();
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

    /// A public match and its players, by team, then place.
    pub fn game_match(
        &self,
        id: u64,
    ) -> Result<Option<(MatchSummary, Vec<MatchPlayer>)>, PersistError> {
        let connection = self.connection.locked();
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

    /// The top `limit` players by `by`, of those with a public match and
    /// that `keep` keeps (it is asked of each in order until enough).
    pub fn top(
        &self,
        by: PlayerRanking,
        limit: usize,
        keep: &dyn Fn(u32) -> bool,
    ) -> Result<Vec<TopPlayer>, PersistError> {
        let connection = self.connection.locked();
        let mut statement = connection.prepare_cached(&top_query(by))?;
        // Room for the ones `keep` leaves out.
        let mut rows = statement.query([bounded(limit) + MatchHistory::MAX_ROWS as i64])?;
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

    /// The best `limit` single-match scores of public matches that ended
    /// at `since` or later, of the players that `keep` keeps (it is asked
    /// of each in order until enough).
    pub fn best(
        &self,
        since: u64,
        limit: usize,
        keep: &dyn Fn(u32) -> bool,
    ) -> Result<Vec<BestLine>, PersistError> {
        let connection = self.connection.locked();
        let mut statement = connection.prepare_cached(&best_query())?;
        // Room for the ones `keep` leaves out.
        let mut rows = statement.query(params![
            since.min(i64::MAX as u64) as i64,
            bounded(limit) + MatchHistory::MAX_ROWS as i64
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
                line: line_at(row, 14)?,
            });
        }
        Ok(best)
    }

    /// The top `limit` registered clans by `by`.
    pub fn clans(&self, by: TotalsKey, limit: usize) -> Result<Vec<ClanLine>, PersistError> {
        let connection = self.connection.locked();
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
        let connection = self.connection.locked();
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
                // c.leader: after the five of the clan and the fourteen of
                // TOTALS.
                |row| Ok((clan_line(row)?, row.get::<_, u32>(19)?)),
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

/// [`MatchHistory::top`]'s query by `by`. Accuracy reads the partial index
/// `g26_player_totals_accuracy`: its expression and its bound are the
/// migration's, word for word.
fn top_query(by: PlayerRanking) -> String {
    let (filter, order) = match by {
        PlayerRanking::Rating => (String::new(), "a.rating DESC, a.account_id".to_owned()),
        PlayerRanking::Totals(key) => (
            String::new(),
            format!("t.{} DESC, t.account_id", key.column()),
        ),
        PlayerRanking::Accuracy => (
            format!("WHERE t.shots >= {ACCURACY_MIN_SHOTS}"),
            "CAST(t.hits AS REAL) / t.shots DESC, t.account_id".to_owned(),
        ),
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
         WHERE m.ended_at >= ?1 AND m.kind = 'public'
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

    fn line(team: u8, place: u8, score: u64, kills: u64, deaths: u64) -> PlayerLine {
        PlayerLine {
            team,
            place,
            presence: Presence::Finished,
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

    /// Alice (10, of clan 7) beats Bob (11) on team 0's win.
    fn record(kind: MatchKind, ended_at: u64) -> MatchRecord {
        MatchRecord {
            game_match_id: 0xABCD,
            kind,
            info: MatchInfo {
                ended_at,
                duration_ms: 845_000,
                map_id: 3,
                game_mode: 2,
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
                },
                PlayerRecord {
                    account_id: 11,
                    clan_id: None,
                    line: line(1, 1, 100, 6, 14),
                    kits: vec![kit(1, 6, &[(7, AK)])],
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
            let mut repository = SqliteGameRepository::open(&dir.db()).unwrap();
            repository.save(10, &Account::new("Alice")).unwrap();
            let connection = Connection::open(dir.db()).unwrap();
            // Back to the step before: the history's tables gone.
            connection
                .execute_batch(
                    "DROP TABLE g26_match_kit_items; DROP TABLE g26_match_kits;
                     DROP TABLE g26_player_items; DROP TABLE g26_match_players;
                     DROP TABLE g26_matches;
                     DROP TABLE g26_player_totals; DROP TABLE g26_clan_totals;
                     DROP INDEX g26_accounts_by_name; DROP INDEX g26_accounts_by_rating;
                     DROP INDEX g26_clan_members_by_account;",
                )
                .unwrap();
            connection
                .execute(
                    "UPDATE g26_meta SET value = ?1 WHERE key = 'schema'",
                    [own as i64],
                )
                .unwrap();
        }
        let mut repository = SqliteGameRepository::open(&dir.db()).unwrap();
        assert_eq!(repository.version().unwrap(), MIGRATIONS.len() as i64);
        assert_eq!(repository.load_all().unwrap().len(), 1);
        repository
            .record_match(&record(MatchKind::Public, 1000))
            .unwrap();
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
    }

    #[test]
    fn the_kits_used_are_kept_with_what_they_held() {
        let dir = TempDir::new("kits");
        let (mut repository, history) = repository(&dir);
        repository
            .record_match(&record(MatchKind::Public, 100))
            .unwrap();
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
        let mut bare = record(MatchKind::Public, 200);
        bare.players[1].kits.clear();
        repository.record_match(&bare).unwrap();
        let (_, players) = history.game_match(2).unwrap().unwrap();
        assert!(players[1].kits.is_empty());
        assert_eq!(players[1].line.kills, 6);
    }

    #[test]
    fn favourites_count_matches_and_add_up_the_kits_that_held_them() {
        let dir = TempDir::new("favourites");
        let (mut repository, history) = repository(&dir);
        for (kind, at) in [
            (MatchKind::Public, 100),
            (MatchKind::Public, 200),
            // Not public: kept, not added up.
            (MatchKind::Private, 300),
        ] {
            repository.record_match(&record(kind, at)).unwrap();
        }
        let favourites = history.favourites(10, 10).unwrap();
        // The pistol is in both of Alice's kits: one match each time, the
        // two kits' counters.
        assert_eq!(
            favourites.weapons,
            [
                ItemTotals {
                    dict_id: AK,
                    matches: 2,
                    counts: KitCounts {
                        kills: 20,
                        headshots: 2,
                        shots: 200,
                        hits: 80,
                    },
                },
                ItemTotals {
                    dict_id: PISTOL,
                    matches: 2,
                    counts: KitCounts {
                        kills: 28,
                        headshots: 4,
                        shots: 400,
                        hits: 160,
                    },
                },
            ]
        );
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
    fn the_accuracy_list_needs_its_shots() {
        let dir = TempDir::new("accuracy");
        let (mut repository, history) = repository(&dir);
        // Alice: 100 shots, 40 hits a match; Bob 20 hits of the same.
        let mut played = record(MatchKind::Public, 100);
        played.players[1].line.counters.hits = 20;
        for _ in 0..9 {
            repository.record_match(&played).unwrap();
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
        assert_eq!(top(&history), ["Alice", "Bob"]);
        let accurate = history
            .top(PlayerRanking::Accuracy, 1, &|id| id != 10)
            .unwrap();
        assert_eq!(accurate[0].player.name, "Bob");
    }

    #[test]
    fn public_matches_are_served_and_added_up_the_others_only_kept() {
        let dir = TempDir::new("kinds");
        let (mut repository, history) = repository(&dir);
        repository
            .record_match(&record(MatchKind::Public, 1000))
            .unwrap();
        repository
            .record_match(&record(MatchKind::Private, 2000))
            .unwrap();
        repository
            .record_match(&record(MatchKind::VsBots, 3000))
            .unwrap();
        repository
            .record_match(&record(MatchKind::Public, 4000))
            .unwrap();

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
            }
        );
        let bob = history.player(11).unwrap().unwrap();
        assert_eq!((bob.totals.matches, bob.totals.wins), (2, 0));
        assert_eq!(bob.player.clan, None);

        let ids: Vec<u64> = history
            .matches(None, 50, None)
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
            repository
                .record_match(&record(MatchKind::Public, at * 100))
                .unwrap();
        }
        repository
            .record_match(&record(MatchKind::Private, 600))
            .unwrap();
        let page: Vec<u64> = history
            .player_matches(10, None, 2)
            .unwrap()
            .iter()
            .map(|played| played.summary.id)
            .collect();
        assert_eq!(page, [5, 4]);
        let next: Vec<u64> = history
            .player_matches(10, Some(4), 2)
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
            repository
                .record_match(&record(MatchKind::Public, at))
                .unwrap();
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
        assert_eq!(history.player_days(11, 0).unwrap()[0].1.wins, 0);
    }

    #[test]
    fn the_top_lists_skip_who_is_left_out() {
        let dir = TempDir::new("top");
        let (mut repository, history) = repository(&dir);
        repository
            .record_match(&record(MatchKind::Public, 100))
            .unwrap();
        let by_kills = history
            .top(PlayerRanking::Totals(TotalsKey::Kills), 10, &|_| true)
            .unwrap();
        let names: Vec<&str> = by_kills
            .iter()
            .map(|top| top.player.name.as_str())
            .collect();
        assert_eq!(names, ["Alice", "Bob"]);
        let by_rating = history
            .top(PlayerRanking::Rating, 10, &|id| id != 10)
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
        repository
            .record_match(&record(MatchKind::Public, 100))
            .unwrap();
        repository
            .record_match(&record(MatchKind::Private, 200))
            .unwrap();
        let clans = history.clans(TotalsKey::Score, 10).unwrap();
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
        };
        let with = |winner, players: Vec<PlayerRecord>| {
            let mut played = record(MatchKind::Public, 100);
            played.info.winner = winner;
            played.players = players;
            played
        };
        let bob = |team| PlayerRecord {
            account_id: 11,
            clan_id: None,
            line: line(team, 1, 100, 6, 14),
            kits: Vec::new(),
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
            // Members on both teams: one match, a win (one of them won).
            with(
                Winner::SecondTeam,
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
            with(Winner::Draw, vec![of_clan(12, line(0, 1, 10, 0, 0))]),
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
                wins: 2,
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
        assert_eq!((matches_of(10).matches, matches_of(10).wins), (3, 1));
        assert_eq!((matches_of(12).matches, matches_of(12).wins), (3, 2));
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
        repository
            .record_match(&record(MatchKind::Public, 200))
            .unwrap();
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

    /// The plan of `sql`, its parameters bound to `values`.
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
            plan.contains("SEARCH g26_player_items USING INDEX g26_player_items_by_matches"),
            "{plan}"
        );
        assert!(!plan.contains("TEMP B-TREE"), "{plan}");
    }

    #[test]
    fn the_accuracy_list_reads_its_partial_index() {
        let dir = TempDir::new("accuracy-plan");
        let (_repository, _history) = repository(&dir);
        let plan = plan_of(&dir, &top_query(PlayerRanking::Accuracy), [10]);
        assert!(
            plan.contains("SCAN t USING INDEX g26_player_totals_accuracy"),
            "{plan}"
        );
        // Only the ties are sorted further, by account.
        assert!(!plan.contains("USE TEMP B-TREE FOR ORDER BY"), "{plan}");
    }
}
