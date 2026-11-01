//! The world snapshot: 0xE8's state and the patched client's upload
//! (`docs/notes/match.md` §3, `snapshot_rules.md`).
//!
//! The state is `u32 time_ms` and then the **body**, which is what
//! `game_state_history_item::serialize` (0x324BE0) writes:
//!
//! ```text
//! u32 active_clients_mask
//! per set bit, ascending: a player block (no length; ends in a bit stream)
//! u16 len + bullet manager
//! u16 len + statistics            <- read here, for the results
//! u16 len + game objects
//! u16 len + physics world
//! u16 len + game rules            <- read here, for the match's phase
//! u16 len + game effect players
//! ```
//!
//! The player blocks are walked from the front ([`super::players`]): each
//! is a length-prefixed state and an animation bit stream whose length its
//! own bits give. Bytes that do not walk so are still tried from the end:
//! the six u16 prefixes chain to the end of the body, and the statistics
//! must parse to exactly their length.
//!
//! The layouts are PROVEN in the exe (the notes give the addresses), and so
//! is the order of the rules inside the rules section: the mode config's
//! `rules` (timelimit, respawn, the mode's rule, kd stats), then the two
//! respawn flags. Every world of real 0.26g matches the server saved
//! (33,106, 2026-10-12..14) walks into its six sections.

use std::collections::HashMap;

use super::super::maps::mode;
use super::{MAX_PLAYERS, players, rules};

/// Score events (`match_stats_events_dict_enum` grew from 24 to 28).
pub const EVENTS: usize = 28;
/// The kits ("presets") a player's counters are split by.
pub const KITS: usize = 3;

/// The events a snapshot carries as one byte, in wire order (table
/// 0x920588). A count over 255 wraps on the wire.
pub const BYTE_EVENTS: [usize; 22] = [
    0, 1, 2, 3, 4, 5, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 19, 20, 21, 24, 25, 27,
];
/// The events carried as u16, after a kit's one-byte ones (table 0x9205E0).
pub const WORD_EVENTS: [usize; 6] = [6, 17, 18, 22, 23, 26];
/// Events whose counters are hundredths: the client weighs them × 0.01.
pub const HUNDREDTHS: [usize; 3] = [6, 17, 18];

/// The event numbers. 0..=23 are 0.21d's; the places that count 0, 2, 3,
/// 4, 5, 7..=13, 15 and 17..=23 were read in the 0.26g exe
/// (snapshot_rules.md §3.1, results.md §9), the others are LIKELY kept.
/// 24..=27 are new; the game gives them no name, these are srp's.
pub mod event {
    pub const KILL: usize = 0;
    pub const ASSIST: usize = 1;
    pub const DEATH: usize = 2;
    pub const SUICIDE: usize = 3;
    pub const TEAM_KILL: usize = 4;
    pub const ARTEFACT: usize = 5;
    pub const BRING_VICTORY_ITEM: usize = 6;
    pub const TEAMMATE_CURE: usize = 7;
    pub const TEAMMATE_RESCUE: usize = 8;
    pub const PARTICIPATING: usize = 11;
    pub const WON: usize = 12;
    pub const MELEE_KILL: usize = 13;
    pub const SILENT_KILL: usize = 14;
    pub const HEADSHOT_KILL: usize = 15;
    pub const WINNING_TEAM_VI_BONUS: usize = 16;
    pub const STEAL_VICTORY_ITEM: usize = 17;
    pub const RECOVER_VICTORY_ITEM: usize = 18;
    pub const VICTORY_ITEM_FOUND: usize = 19;
    pub const PUT_VICTORY_ITEM_IN_CONTAINER: usize = 20;
    pub const BULLET_FIRE: usize = 22;
    pub const BULLET_HIT: usize = 23;
    /// A battery put into the container by a player who found it fresh (or
    /// took it from the enemy) and whom no teammate helped carry it
    /// (0x334C46, test 0x334D00).
    pub const SOLO_DELIVERY: usize = 24;
    /// A kill with a grenade (results.md §9).
    pub const GRENADE_KILL: usize = 25;
    /// A battery of the enemy's put into the container by the player who
    /// stole it (0x334C7F). A u16 on the wire, counted by ones.
    pub const STOLEN_AND_DELIVERED: usize = 26;
    /// The end of the match for a player whose protection value is 0
    /// (results.md §9).
    pub const NEW_PLAYER_MATCH: usize = 27;
    /// Research (mode 5, exe patch 53; `research_mode.md`) counts in three
    /// of these places: a unit captured, a player's share of it in
    /// hundredths (a battery's delivery share otherwise);
    pub const RESEARCH_CAPTURE: usize = BRING_VICTORY_ITEM;
    /// a unit taken from the other team, the same (a rescued battery's);
    pub const RESEARCH_NEUTRALISE: usize = RECOVER_VICTORY_ITEM;
    /// an enemy killed inside a unit his team does not own, by ones (a
    /// battery carrier killed).
    pub const KILL_INVADER: usize = 10;
}

/// The CRC a client reports in 0xDF for a world: boost's CRC-32 (the
/// common reflected 0x04C11DB7 one) over the **body**, without the time
/// (`game_state_history_item::hash` 0x325150).
pub fn crc(body: &[u8]) -> u32 {
    const TABLE: [u32; 256] = {
        let mut table = [0u32; 256];
        let mut index = 0;
        while index < 256 {
            let mut value = index as u32;
            let mut bit = 0;
            while bit < 8 {
                value = if value & 1 != 0 {
                    (value >> 1) ^ 0xEDB8_8320
                } else {
                    value >> 1
                };
                bit += 1;
            }
            table[index] = value;
            index += 1;
        }
        table
    };
    !body.iter().fold(!0u32, |crc, &byte| {
        (crc >> 8) ^ TABLE[usize::from((crc as u8) ^ byte)]
    })
}

/// A cursor over a blob. Every read is `None` past the end.
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }

    fn take(&mut self, len: usize) -> Option<&'a [u8]> {
        let slice = self.bytes.get(self.at..self.at.checked_add(len)?)?;
        self.at += len;
        Some(slice)
    }

    fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }

    fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes(self.take(2)?.try_into().ok()?))
    }

    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }

    fn f32(&mut self) -> Option<f32> {
        Some(f32::from_bits(self.u32()?))
    }

    fn rest(&self) -> usize {
        self.bytes.len() - self.at
    }
}

/// A body split into its parts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sections<'a> {
    pub active_clients_mask: u32,
    /// The player blocks, one after another.
    pub players: &'a [u8],
    pub bullets: &'a [u8],
    pub statistics: &'a [u8],
    pub objects: &'a [u8],
    pub physics: &'a [u8],
    pub rules: &'a [u8],
    pub effects: &'a [u8],
}

impl<'a> Sections<'a> {
    /// The body these parts make. `None`: a section over 65 535 bytes.
    pub fn to_body(&self) -> Option<Vec<u8>> {
        let mut body = Vec::new();
        body.extend(self.active_clients_mask.to_le_bytes());
        body.extend_from_slice(self.players);
        for section in [
            self.bullets,
            self.statistics,
            self.objects,
            self.physics,
            self.rules,
            self.effects,
        ] {
            body.extend(u16::try_from(section.len()).ok()?.to_le_bytes());
            body.extend_from_slice(section);
        }
        Some(body)
    }

    /// `body` read from the front: the mask, the player blocks walked one
    /// by one ([`players::walk`]), then six length-prefixed sections that
    /// end exactly at its end. `None` for bytes that are not that.
    pub fn walk(body: &'a [u8]) -> Option<Self> {
        let (_, players_end) = players::walk(body)?;
        let mut at = players_end;
        let mut sections = [&body[..0]; 6];
        for section in &mut sections {
            let len = usize::from(u16::from_le_bytes(body.get(at..at + 2)?.try_into().ok()?));
            *section = body.get(at + 2..at + 2 + len)?;
            at += 2 + len;
        }
        (at == body.len()).then_some(Self {
            active_clients_mask: u32::from_le_bytes([body[0], body[1], body[2], body[3]]),
            players: &body[4..players_end],
            bullets: sections[0],
            statistics: sections[1],
            objects: sections[2],
            physics: sections[3],
            rules: sections[4],
            effects: sections[5],
        })
    }

    /// Every way to read `body` as a mask, player blocks and six
    /// length-prefixed sections that end exactly at its end: the one the
    /// walk from the front gives, if it gives one; else those the prefixes
    /// chained from the end give. Bytes of a player block can look like a
    /// prefix, so there may be several of those; [`decode`] picks the one
    /// whose statistics and rules read.
    pub fn candidates(body: &'a [u8]) -> Vec<Self> {
        if let Some(walked) = Self::walk(body) {
            return vec![walked];
        }
        let Some(mask) = body.get(..4) else {
            return Vec::new();
        };
        let mask = u32::from_le_bytes([mask[0], mask[1], mask[2], mask[3]]);
        // Where each possible u16 prefix's section would end.
        let mut ending_at: HashMap<usize, Vec<usize>> = HashMap::new();
        for at in 4..body.len().saturating_sub(1) {
            let end = at + 2 + usize::from(u16::from_le_bytes([body[at], body[at + 1]]));
            if end <= body.len() {
                ending_at.entry(end).or_default().push(at);
            }
        }
        let mut chains = Vec::new();
        let mut chain = [0; 6];
        Self::chain(&ending_at, body.len(), 6, &mut chain, &mut chains);
        chains
            .into_iter()
            // No players: the bullets follow the mask at once. Players: a
            // block is never empty.
            .filter(|starts| (mask == 0) == (starts[0] == 4))
            .map(|starts| {
                let section = |index: usize| {
                    let end = starts.get(index + 1).copied().unwrap_or(body.len());
                    &body[starts[index] + 2..end]
                };
                Self {
                    active_clients_mask: mask,
                    players: &body[4..starts[0]],
                    bullets: section(0),
                    statistics: section(1),
                    objects: section(2),
                    physics: section(3),
                    rules: section(4),
                    effects: section(5),
                }
            })
            .collect()
    }

    /// The prefixes of the `left` sections that end at `end`, last first.
    fn chain(
        ending_at: &HashMap<usize, Vec<usize>>,
        end: usize,
        left: usize,
        chain: &mut [usize; 6],
        chains: &mut Vec<[usize; 6]>,
    ) {
        if left == 0 {
            chains.push(*chain);
            return;
        }
        for &at in ending_at.get(&end).into_iter().flatten() {
            chain[left - 1] = at;
            Self::chain(ending_at, at, left - 1, chain, chains);
        }
    }
}

/// One roster place's statistics (`player_shared_statistics::serialize`
/// 0x335750).
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerStatistics {
    /// `events[event][kit]`: the kit is the one the player was using.
    pub events: [[u16; KITS]; EVENTS],
    /// `received_damage[kit][inflictor]`.
    pub received_damage: [[f32; MAX_PLAYERS]; KITS],
    /// `last_hit_ms[receiver]`; 0 is none.
    pub last_hit_ms: [u32; MAX_PLAYERS],
    pub last_killed_enemy_ms: u32,
    pub last_hit_enemy_id: u8,
    pub killstreak: u8,
    pub newbee_bonus_player_action_coef: f32,
    pub longest_killstreak: u16,
}

impl Default for PlayerStatistics {
    fn default() -> Self {
        Self {
            events: [[0; KITS]; EVENTS],
            received_damage: [[0.0; MAX_PLAYERS]; KITS],
            last_hit_ms: [0; MAX_PLAYERS],
            last_killed_enemy_ms: 0,
            last_hit_enemy_id: 0,
            killstreak: 0,
            newbee_bonus_player_action_coef: 0.0,
            longest_killstreak: 0,
        }
    }
}

impl PlayerStatistics {
    /// An event's count over the three kits [LIKELY the player's total:
    /// the scoreboard code was not read].
    pub fn total(&self, event: usize) -> u32 {
        self.events[event]
            .iter()
            .map(|&count| u32::from(count))
            .sum()
    }

    /// Every event's total.
    pub fn totals(&self) -> [u32; EVENTS] {
        std::array::from_fn(|event| self.total(event))
    }

    fn read(reader: &mut Reader) -> Option<Self> {
        let mut player = Self::default();
        for kit in 0..KITS {
            let mask = reader.u32()?;
            if mask >> EVENTS != 0 {
                return None;
            }
            for event in BYTE_EVENTS {
                if mask & (1 << event) != 0 {
                    player.events[event][kit] = reader.u8()?.into();
                }
            }
            for event in WORD_EVENTS {
                if mask & (1 << event) != 0 {
                    player.events[event][kit] = reader.u16()?;
                }
            }
        }
        for damage in &mut player.received_damage {
            read_masked_f32(reader, MAX_PLAYERS, damage)?;
        }
        let mask = reader.u32()?;
        if mask >> MAX_PLAYERS != 0 {
            return None;
        }
        for (receiver, time) in player.last_hit_ms.iter_mut().enumerate() {
            if mask & (1 << receiver) != 0 {
                *time = reader.u32()?;
            }
        }
        player.last_killed_enemy_ms = reader.u32()?;
        player.last_hit_enemy_id = reader.u8()?;
        player.killstreak = reader.u8()?;
        player.newbee_bonus_player_action_coef = reader.f32()?;
        player.longest_killstreak = reader.u16()?;
        Some(player)
    }

    /// As the game writes it. The events' mask is not started again for
    /// each kit: it is cleared once (0x33575D) and a kit's pass only sets
    /// bits in it (0x335778..0x3357A9), so a kit's mask has the bits of the
    /// kits before it too, and each bit is followed by that kit's count, a
    /// zero when it has none. The damage masks are each kit's own
    /// (0x3353E7).
    fn write(&self, out: &mut Vec<u8>) {
        let mut mask = 0u32;
        for kit in 0..KITS {
            mask = (0..EVENTS)
                .filter(|&event| self.events[event][kit] > 0)
                .fold(mask, |mask, event| mask | 1 << event);
            out.extend(mask.to_le_bytes());
            for event in BYTE_EVENTS {
                if mask & (1 << event) != 0 {
                    // `u8( events[...] )`: truncated.
                    out.push(self.events[event][kit] as u8);
                }
            }
            for event in WORD_EVENTS {
                if mask & (1 << event) != 0 {
                    out.extend(self.events[event][kit].to_le_bytes());
                }
            }
        }
        for damage in &self.received_damage {
            write_masked_f32(damage, out);
        }
        let mask = (0..MAX_PLAYERS)
            .filter(|&receiver| self.last_hit_ms[receiver] > 0)
            .fold(0u32, |mask, receiver| mask | 1 << receiver);
        out.extend(mask.to_le_bytes());
        for time in self.last_hit_ms.iter().filter(|&&time| time > 0) {
            out.extend(time.to_le_bytes());
        }
        out.extend(self.last_killed_enemy_ms.to_le_bytes());
        out.push(self.last_hit_enemy_id);
        out.push(self.killstreak);
        out.extend(self.newbee_bonus_player_action_coef.to_le_bytes());
        out.extend(self.longest_killstreak.to_le_bytes());
    }
}

/// A u32 mask over the first `count` values, then the value of each set
/// bit. `None`: a bit past `count`, or the bytes end.
fn read_masked_f32(
    reader: &mut Reader,
    count: usize,
    values: &mut [f32; MAX_PLAYERS],
) -> Option<()> {
    let mask = reader.u32()?;
    if u64::from(mask) >> count.min(MAX_PLAYERS) != 0 {
        return None;
    }
    for (index, value) in values.iter_mut().enumerate() {
        if mask & (1 << index) != 0 {
            *value = reader.f32()?;
        }
    }
    Some(())
}

/// A u32 mask of the values above 0, then those values.
fn write_masked_f32(values: &[f32; MAX_PLAYERS], out: &mut Vec<u8>) {
    let mask = (0..MAX_PLAYERS)
        .filter(|&index| values[index] > 0.0)
        .fold(0u32, |mask, index| mask | 1 << index);
    out.extend(mask.to_le_bytes());
    for value in values.iter().filter(|&&value| value > 0.0) {
        out.extend(value.to_le_bytes());
    }
}

/// One battery's status in the statistics (0x334DC0); modes 2 and 3 only.
#[derive(Clone, Debug, PartialEq)]
pub struct VictoryItemStatus {
    /// `carried_distances[kit][player]`.
    pub carried_distances: [[f32; MAX_PLAYERS]; KITS],
    /// `nearest_positions[team_1]`, `[team_2]`.
    pub nearest_positions: [f32; 2],
    pub owner_team: u8,
    pub first_finder: u8,
    /// Exe patch 92's (battery_progress): how near each team's container
    /// the battery has come in its life, in 10 m steps: 0 not carried yet in
    /// the life, else the steps + 1. After `first_finder`.
    pub progress: [u8; 2],
}

impl Default for VictoryItemStatus {
    fn default() -> Self {
        Self {
            carried_distances: [[0.0; MAX_PLAYERS]; KITS],
            nearest_positions: [0.0; 2],
            owner_team: 0,
            first_finder: 0,
            progress: [0; 2],
        }
    }
}

impl VictoryItemStatus {
    fn read(reader: &mut Reader, players: usize) -> Option<Self> {
        let mut status = Self::default();
        for distances in &mut status.carried_distances {
            read_masked_f32(reader, players, distances)?;
        }
        status.nearest_positions = [reader.f32()?, reader.f32()?];
        status.owner_team = reader.u8()?;
        status.first_finder = reader.u8()?;
        status.progress = [reader.u8()?, reader.u8()?];
        Some(status)
    }

    fn write(&self, out: &mut Vec<u8>) {
        for distances in &self.carried_distances {
            write_masked_f32(distances, out);
        }
        for position in self.nearest_positions {
            out.extend(position.to_le_bytes());
        }
        out.push(self.owner_team);
        out.push(self.first_finder);
        out.extend(self.progress);
    }
}

/// The statistics' last 11 bytes (0x2F5B08..0x2F5BCB): the standing. They
/// end the section whatever the roster and the mode are.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StatisticsTail {
    /// `m_team_points`: modes 2 and 3, the batteries in each team's
    /// container, 0 from the start and written with the rule's own pair
    /// (0x313AA5); mode 4, each team's tickets, written when play begins
    /// (0x3160D8) and 0 before.
    pub team_points: [u16; 2],
    /// `m_team_victory_items`: modes 2 and 3, the same count kept by the
    /// statistics' own handler (0x2F5570: +1 for the team of the player
    /// who puts a battery in, -1 for the other team when one is taken
    /// out); 0 in mode 4.
    pub team_victory_items: [u8; 2],
    /// `m_winner_team`: the current leader, 0, 1 or 2 for a draw; **3**
    /// until a rule's first event sets it (0x26D95A). Compute the winner
    /// from the points ([`Decoded::standing`]).
    pub winner_team: u8,
    /// `m_active_in_match_players_mask`: mode 4, the players in play.
    /// Nothing writes it in modes 2 and 3: not to be used there.
    pub active_players_mask: u32,
}

impl StatisticsTail {
    pub const LEN: usize = 11;

    /// The tail of a statistics section, without reading what is before
    /// it. `None`: the section is shorter (empty: the client had no
    /// statistics object).
    pub fn of(statistics: &[u8]) -> Option<Self> {
        let at = statistics.len().checked_sub(Self::LEN)?;
        Self::read(&mut Reader::new(&statistics[at..]))
    }

    fn read(reader: &mut Reader) -> Option<Self> {
        Some(Self {
            team_points: [reader.u16()?, reader.u16()?],
            team_victory_items: [reader.u8()?, reader.u8()?],
            winner_team: reader.u8()?,
            active_players_mask: reader.u32()?,
        })
    }

    fn write(&self, out: &mut Vec<u8>) {
        out.extend(self.team_points[0].to_le_bytes());
        out.extend(self.team_points[1].to_le_bytes());
        out.extend(self.team_victory_items);
        out.push(self.winner_team);
        out.extend(self.active_players_mask.to_le_bytes());
    }
}

/// The statistics section (`shared_statistics::serialize` 0x2F5AC0).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Statistics {
    /// One for each roster place.
    pub players: Vec<PlayerStatistics>,
    /// One for each battery; none in mode 4.
    pub victory_items: Vec<VictoryItemStatus>,
    pub tail: StatisticsTail,
}

impl Statistics {
    /// Reads a section of a match with `players` roster places and
    /// `batteries` batteries. `None` unless it fills `bytes` exactly.
    pub fn decode(bytes: &[u8], players: usize, batteries: usize) -> Option<Self> {
        if players > MAX_PLAYERS {
            return None;
        }
        let mut reader = Reader::new(bytes);
        let statistics = Self {
            players: (0..players)
                .map(|_| PlayerStatistics::read(&mut reader))
                .collect::<Option<_>>()?,
            victory_items: (0..batteries)
                .map(|_| VictoryItemStatus::read(&mut reader, players))
                .collect::<Option<_>>()?,
            tail: StatisticsTail::read(&mut reader)?,
        };
        (reader.rest() == 0).then_some(statistics)
    }

    /// The section's bytes, as the client writes them: a player's event
    /// masks too, which grow from kit to kit (`PlayerStatistics::write`).
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for player in &self.players {
            player.write(&mut out);
        }
        for status in &self.victory_items {
            status.write(&mut out);
        }
        self.tail.write(&mut out);
        out
    }
}

/// `timelimit_rule_core`'s 13 bytes (0x317F30). The times are absolute
/// match ms (0.21d wrote them relative to a base).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timelimit {
    /// `m_state_start_time_ms`; `None` (0xFFFFFFFF) until the match is
    /// ready.
    pub start: Option<u32>,
    /// `m_current_time_in_ms`: the rule's last tick, the snapshot's own
    /// time.
    pub current: u32,
    /// The players that entered and have not left.
    pub players_mask: u32,
    /// [`rules::state`].
    pub state: u8,
}

impl Timelimit {
    pub const LEN: usize = 13;

    fn read(bytes: &[u8]) -> Option<Self> {
        let mut reader = Reader::new(bytes);
        let start = reader.u32()?;
        Some(Self {
            start: (start != u32::MAX).then_some(start),
            current: reader.u32()?,
            players_mask: reader.u32()?,
            state: reader.u8()?,
        })
    }

    pub fn write(&self, out: &mut Vec<u8>) {
        out.extend(self.start.unwrap_or(u32::MAX).to_le_bytes());
        out.extend(self.current.to_le_bytes());
        out.extend(self.players_mask.to_le_bytes());
        out.push(self.state);
    }

    /// When the time limit ends the match, `match_ms` being the mode's
    /// match time: known once play has begun.
    pub fn time_limit_at(&self, match_ms: u32) -> Option<u32> {
        (self.state == rules::state::IN_PROCESS)
            .then_some(self.start)
            .flatten()
            .map(|start| start.saturating_add(match_ms))
    }
}

/// A bool as the client's writer puts it: 0x00 or 0xFF. PROVEN: the bool
/// writer 0x80FA0 stores `neg al; sbb al,al` (0x8103D), and it is what
/// `kd_stats_rule::serialize` calls for `online` (0x3238E7) and the
/// blackboard for its two flags (0x323D8A, 0x323DB2). The client's readers
/// take any byte but 0 as true (0x323952, 0x323DCA, 0x323E0B); this one is
/// stricter, because where the section ends is found by what reads. The
/// rules of 897 live dumps of five matches read with this (2026-10-12; no
/// byte but 0x00 and 0xFF is in these places in any of them); with 0 and 1
/// only, none did. 0x01 is taken as true too: it is what this file wrote
/// until then.
/// `None`: no bool.
fn wire_bool(byte: u8) -> Option<bool> {
    match byte {
        0x00 => Some(false),
        0x01 | WIRE_TRUE => Some(true),
        _ => None,
    }
}

/// `true` on the wire.
const WIRE_TRUE: u8 = 0xFF;

/// A roster place in `kd_stats_rule` (0x323870): 5 bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KillsDeaths {
    pub kills: u16,
    pub deaths: u16,
    pub online: bool,
}

impl KillsDeaths {
    pub const LEN: usize = 5;

    pub fn write(&self, out: &mut Vec<u8>) {
        out.extend(self.kills.to_le_bytes());
        out.extend(self.deaths.to_le_bytes());
        out.push(if self.online { WIRE_TRUE } else { 0 });
    }
}

/// `tdm_rule_core`'s 12 bytes (0x316340); mode 4.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tdm {
    pub tickets: [u16; 2],
    pub alive_players_mask: u32,
    pub active_players_mask: u32,
}

impl Tdm {
    pub const LEN: usize = 12;

    pub fn write(&self, out: &mut Vec<u8>) {
        out.extend(self.tickets[0].to_le_bytes());
        out.extend(self.tickets[1].to_le_bytes());
        out.extend(self.alive_players_mask.to_le_bytes());
        out.extend(self.active_players_mask.to_le_bytes());
    }
}

/// A research unit as Research's rule writes it (exe patch 53, `rr_write`):
/// 96 bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResearchUnit {
    /// The capture of `taking`, 0 to [`Research::FULL`].
    pub progress: u32,
    /// Whose progress it is: 0, 1, or 2 for nobody.
    pub taking: u8,
    /// Whose unit it is: 0, 1, or 2 for nobody.
    pub owner: u8,
    /// The players of each team inside.
    pub counts: [u8; 2],
    /// Those players, by roster place: as many bits as `counts`.
    pub inside: [u32; 2],
    /// Each roster place's mark: its share of the capture when it went in,
    /// [`Research::FULL`] for all of it.
    pub marks: [u32; MAX_PLAYERS],
}

impl Default for ResearchUnit {
    fn default() -> Self {
        Self {
            progress: 0,
            taking: rules::DRAW,
            owner: rules::DRAW,
            counts: [0; 2],
            inside: [0; 2],
            marks: [0; MAX_PLAYERS],
        }
    }
}

impl ResearchUnit {
    pub const LEN: usize = 16 + 4 * MAX_PLAYERS;

    fn read(reader: &mut Reader) -> Option<Self> {
        let progress = reader.u32()?;
        let [taking, owner, first, second] =
            [reader.u8()?, reader.u8()?, reader.u8()?, reader.u8()?];
        let inside = [reader.u32()?, reader.u32()?];
        let mut marks = [0; MAX_PLAYERS];
        for mark in &mut marks {
            *mark = reader.u32()?;
        }
        let unit = Self {
            progress,
            taking,
            owner,
            counts: [first, second],
            inside,
            marks,
        };
        // What `rr_read` refuses.
        let sound = progress <= Research::FULL
            && taking <= rules::DRAW
            && owner <= rules::DRAW
            && (0..2).all(|team| u32::from(unit.counts[team]) == inside[team].count_ones())
            && (inside[0] | inside[1]) >> MAX_PLAYERS == 0;
        sound.then_some(unit)
    }

    pub fn write(&self, out: &mut Vec<u8>) {
        out.extend(self.progress.to_le_bytes());
        out.extend([self.taking, self.owner, self.counts[0], self.counts[1]]);
        out.extend(self.inside[0].to_le_bytes());
        out.extend(self.inside[1].to_le_bytes());
        for mark in self.marks {
            out.extend(mark.to_le_bytes());
        }
    }
}

/// Research's rule (class 480, exe patch 53; `research_mode.md`, "Wire"):
/// 21 bytes and 96 a unit; mode 5. Its own bytes say how many units it
/// has, so, as Team Deathmatch's, it is read from the kd stats back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Research {
    /// Each team's points, millionths: 0 to `resources_count` × 10^6.
    pub points: [u32; 2],
    /// The points shown, whole (rounded): what the statistics' tail has.
    pub scores: [u16; 2],
    /// When the match ends, match ms: 3 s after a team reached the goal.
    /// `None` (0xFFFFFFFF) before.
    pub end: Option<u32>,
    /// The roster places that entered and have not left.
    pub entered: u32,
    /// The units, by `cp_id` (three on every map).
    pub units: Vec<ResearchUnit>,
}

impl Research {
    /// A unit's whole capture.
    pub const FULL: u32 = 1_000_000_000;
    /// The bytes before the units.
    pub const HEAD_LEN: usize = 21;
    /// The units a map may have (`RR_MAX_ZONES`).
    pub const MAX_UNITS: usize = 3;

    /// The rule's length with `units` units.
    pub const fn len_of(units: usize) -> usize {
        Self::HEAD_LEN + ResearchUnit::LEN * units
    }

    /// Reads a rule that fills `bytes` exactly.
    fn read(bytes: &[u8]) -> Option<Self> {
        let mut reader = Reader::new(bytes);
        let points = [reader.u32()?, reader.u32()?];
        let scores = [reader.u16()?, reader.u16()?];
        let end = reader.u32()?;
        let entered = reader.u32()?;
        let units = usize::from(reader.u8()?);
        if units == 0 || units > Self::MAX_UNITS || entered >> MAX_PLAYERS != 0 {
            return None;
        }
        let units = (0..units)
            .map(|_| ResearchUnit::read(&mut reader))
            .collect::<Option<Vec<_>>>()?;
        (reader.rest() == 0).then_some(Self {
            points,
            scores,
            end: (end != u32::MAX).then_some(end),
            entered,
            units,
        })
    }

    /// Reads the rule at the end of `bytes`: the length its unit count
    /// gives, the most units first. Where it starts, and the rule.
    fn read_back(bytes: &[u8]) -> Option<(usize, Self)> {
        (1..=Self::MAX_UNITS).rev().find_map(|units| {
            let at = bytes.len().checked_sub(Self::len_of(units))?;
            Self::read(&bytes[at..]).map(|rule| (at, rule))
        })
    }

    pub fn write(&self, out: &mut Vec<u8>) {
        out.extend(self.points[0].to_le_bytes());
        out.extend(self.points[1].to_le_bytes());
        out.extend(self.scores[0].to_le_bytes());
        out.extend(self.scores[1].to_le_bytes());
        out.extend(self.end.unwrap_or(u32::MAX).to_le_bytes());
        out.extend(self.entered.to_le_bytes());
        out.push(self.units.len() as u8);
        for unit in &self.units {
            unit.write(out);
        }
    }
}

/// `player_respawn_rule_core` (0x316F10, base 0x316C90). PROVEN.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Respawn {
    /// `m_player_respawn_times[i]`, absolute match ms, for the roster
    /// places that wait for a respawn.
    pub times: Vec<(u8, u32)>,
    /// `m_respawn_random.seed()`.
    pub seed: u32,
    /// One for each respawn point of the map (`server_objects.
    /// respawn_points` of the mode's project: team 0's in config order,
    /// then team 1's, then team 2's), `m_inside_objects_count` of its two
    /// checkers [LIKELY allies, then enemies, in the point's zones]. Their
    /// number is the map's
    /// [`MapMode::respawn_points`](super::super::maps::MapMode::respawn_points).
    pub points: Vec<[u8; 2]>,
}

impl Respawn {
    /// The mask, the times and the seed: what comes before the points.
    /// The bytes read, and how many.
    fn read_head(bytes: &[u8]) -> Option<(Self, usize)> {
        let mut reader = Reader::new(bytes);
        let mask = reader.u32()?;
        if mask >> MAX_PLAYERS != 0 {
            return None;
        }
        let mut times = Vec::new();
        for place in 0..MAX_PLAYERS as u8 {
            if mask >> place & 1 != 0 {
                times.push((place, reader.u32()?));
            }
        }
        let seed = reader.u32()?;
        Some((
            Self {
                times,
                seed,
                points: Vec::new(),
            },
            reader.at,
        ))
    }

    pub fn write(&self, out: &mut Vec<u8>) {
        let mask = self
            .times
            .iter()
            .fold(0u32, |mask, (place, _)| mask | 1 << place);
        out.extend(mask.to_le_bytes());
        let mut times = self.times.clone();
        times.sort_unstable();
        for (_, time) in times {
            out.extend(time.to_le_bytes());
        }
        out.extend(self.seed.to_le_bytes());
        for point in &self.points {
            out.extend(point);
        }
    }
}

/// A battery as the gather rule has it (`victory_item_core::serialize`
/// 0x301470, reader 0x301660). PROVEN, but the place of the position
/// inside the transform [LIKELY].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Battery {
    /// A player has it: the rule writes nothing of it (the carrier is in
    /// that player's block, `m_carried_item`).
    Carried,
    /// In the container with this id (`victory_items_containers[..].id` of
    /// the map; whose it is:
    /// [`MapMode::container_teams`](super::super::maps::MapMode::container_teams)).
    InContainer(u8),
    /// Lying in the world.
    Free {
        /// `m_spotted_mask`.
        spotted_mask: u32,
        /// The translation of `m_transform` [LIKELY: its floats 12..15].
        position: [f32; 3],
    },
}

impl Battery {
    /// A free battery's bytes after its first six: two 4x4 transforms,
    /// two velocities, `deactivation_time`, `activation_state`; then a
    /// `u8` count of users and their ids.
    const FREE_FIXED: usize = 64 + 64 + 12 + 12 + 4 + 4;

    fn read(reader: &mut Reader, containers: usize) -> Option<Self> {
        let spotted_mask = reader.u32()?;
        // No carrier, or the rule would not have written it (0x313D84).
        if reader.u8()? != 0xFF {
            return None;
        }
        let container = reader.u8()?;
        if container != 0xFF {
            return (usize::from(container) < containers).then_some(Self::InContainer(container));
        }
        let fixed = reader.take(Self::FREE_FIXED)?;
        let float = |at: usize| {
            f32::from_le_bytes([fixed[at], fixed[at + 1], fixed[at + 2], fixed[at + 3]])
        };
        let users = usize::from(reader.u8()?);
        if users > MAX_PLAYERS
            || reader
                .take(users)?
                .iter()
                .any(|id| usize::from(*id) >= MAX_PLAYERS)
        {
            return None;
        }
        Some(Self::Free {
            spotted_mask,
            position: [float(64 + 48), float(64 + 52), float(64 + 56)],
        })
    }

    /// As the client writes it, a free one at rest with identity
    /// rotations and no users (for tests and tools).
    fn write(&self, out: &mut Vec<u8>) {
        match *self {
            Self::Carried => {}
            Self::InContainer(id) => {
                out.extend(0u32.to_le_bytes());
                out.extend([0xFF, id]);
            }
            Self::Free {
                spotted_mask,
                position,
            } => {
                out.extend(spotted_mask.to_le_bytes());
                out.extend([0xFF, 0xFF]);
                // `m_last_safe_transform`, `m_transform`: three rows of
                // rotation, then the position.
                for _ in 0..2 {
                    let [x, y, z] = position;
                    for value in [
                        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, x, y, z, 1.0,
                    ] {
                        out.extend(f32::to_le_bytes(value));
                    }
                }
                out.extend([0; 12 + 12 + 4 + 4]);
                out.push(0);
            }
        }
    }
}

/// A team's container (`victory_items_container_core::serialize`
/// 0x31CE40, reader 0x31CEE0). PROVEN.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Container {
    /// The roster places using it.
    pub users: Vec<u8>,
    /// The batteries in it, by their number.
    pub items: Vec<u8>,
}

impl Container {
    fn read(reader: &mut Reader) -> Option<Self> {
        let users = usize::from(reader.u8()?);
        let users = reader.take(users)?.to_vec();
        let items = reader.u32()? as usize;
        let items = reader.take(items)?.to_vec();
        (users.len() <= MAX_PLAYERS
            && users.iter().all(|id| usize::from(*id) < MAX_PLAYERS)
            && items.len() <= Gather::MAX_BATTERIES
            && items
                .iter()
                .all(|id| usize::from(*id) < Gather::MAX_BATTERIES))
        .then_some(Self { users, items })
    }

    fn write(&self, out: &mut Vec<u8>) {
        out.push(self.users.len() as u8);
        out.extend(&self.users);
        out.extend((self.items.len() as u32).to_le_bytes());
        out.extend(&self.items);
    }
}

/// Mode 3's own fields (`match_time_vi_spawn_rule_core::serialize`
/// 0x3149B0). PROVEN.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Timed {
    /// `m_available_victory_items_count`: the batteries placed so far.
    pub available: u32,
    /// `m_match_start_time_ms`, `m_next_spawn_time_ms`: absolute.
    pub match_start: u32,
    pub next_spawn: u32,
    /// `m_victory_items_spawn_time_ms`, `..._variation_ms`.
    pub spawn_time: u32,
    pub spawn_variation: u32,
    /// `m_match_started`.
    pub started: bool,
}

/// The gather rule of modes 2 (`gather_victory_items_rule_core` 0x313CA0)
/// and 3 (0x3149B0). PROVEN.
#[derive(Clone, Debug, PartialEq)]
pub struct Gather {
    /// `m_spawner_random.seed()`.
    pub seed: u32,
    /// `m_team_points`: the batteries in each team's container.
    pub team_points: [u8; 2],
    /// By battery number: the mode's `victory_items_count` of them, in
    /// mode 3 the ones placed so far.
    pub batteries: Vec<Battery>,
    /// By container id; the maps have two.
    pub containers: Vec<Container>,
    /// Mode 3 only.
    pub timed: Option<Timed>,
}

impl Gather {
    /// `items_mask` is a u16.
    pub const MAX_BATTERIES: usize = 16;
    /// Every map has a container for each team.
    pub const CONTAINERS: usize = 2;

    /// Reads a gather rule that fills `bytes` exactly. `batteries` is how
    /// many the mask covers at most (the mode's count).
    fn read(bytes: &[u8], game_mode: u8, batteries: usize) -> Option<Self> {
        let timed = game_mode == mode::TIMED_BATTERIES;
        let mut reader = Reader::new(bytes);
        let seed = reader.u32()?;
        let team_points = [reader.u8()?, reader.u8()?];
        let available = if timed { Some(reader.u32()?) } else { None };
        let count = match available {
            Some(available) => (available as usize).min(batteries),
            None => batteries,
        };
        let mask = reader.u16()?;
        if count > Self::MAX_BATTERIES || u32::from(mask) >> count != 0 {
            return None;
        }
        let mut read = Vec::with_capacity(count);
        for battery in 0..count {
            read.push(if mask >> battery & 1 != 0 {
                Battery::read(&mut reader, Self::CONTAINERS)?
            } else {
                Battery::Carried
            });
        }
        let containers = (0..Self::CONTAINERS)
            .map(|_| Container::read(&mut reader))
            .collect::<Option<Vec<_>>>()?;
        let timed = match available {
            Some(available) => {
                let timed = Timed {
                    available,
                    match_start: reader.u32()?,
                    next_spawn: reader.u32()?,
                    spawn_time: reader.u32()?,
                    spawn_variation: reader.u32()?,
                    started: reader.u8()? != 0,
                };
                Some(timed)
            }
            None => None,
        };
        // What the containers hold is what the teams have, and what the
        // batteries say of themselves.
        let held: usize = containers
            .iter()
            .map(|container| container.items.len())
            .sum();
        let consistent = held == usize::from(team_points[0]) + usize::from(team_points[1])
            && containers.iter().enumerate().all(|(id, container)| {
                container.items.iter().all(|item| {
                    read.get(usize::from(*item)) == Some(&Battery::InContainer(id as u8))
                })
            });
        (reader.rest() == 0 && consistent).then_some(Self {
            seed,
            team_points,
            batteries: read,
            containers,
            timed,
        })
    }

    /// The rule's bytes (free batteries as [`Battery::write`] has them).
    pub fn write(&self, out: &mut Vec<u8>) {
        out.extend(self.seed.to_le_bytes());
        out.extend(self.team_points);
        if let Some(timed) = &self.timed {
            out.extend(timed.available.to_le_bytes());
        }
        let mask = self
            .batteries
            .iter()
            .enumerate()
            .filter(|(_, battery)| **battery != Battery::Carried)
            .fold(0u16, |mask, (index, _)| mask | 1 << index);
        out.extend(mask.to_le_bytes());
        for battery in &self.batteries {
            battery.write(out);
        }
        for container in &self.containers {
            container.write(out);
        }
        if let Some(timed) = &self.timed {
            out.extend(timed.match_start.to_le_bytes());
            out.extend(timed.next_spawn.to_le_bytes());
            out.extend(timed.spawn_time.to_le_bytes());
            out.extend(timed.spawn_variation.to_le_bytes());
            out.push(timed.started.into());
        }
    }
}

/// What the rules section gives. The rules are in the mode config's
/// order (PROVEN: `pvp_match_core_cook::on_resources_loaded` 0x2CBC60
/// copies them as requested, and nothing sorts them): the timelimit, the
/// respawn rule, the mode's rule, the kd stats; then the blackboard's two
/// flags.
#[derive(Clone, Debug, PartialEq)]
pub struct Rules {
    pub timelimit: Timelimit,
    /// `None`: the bytes between the timelimit and the kd stats did not
    /// read as a respawn rule and the mode's rule.
    pub respawn: Option<Respawn>,
    /// Modes 2 and 3.
    pub gather: Option<Gather>,
    /// Mode 4: the 12 bytes before the kd stats.
    pub tdm: Option<Tdm>,
    /// Mode 5: the bytes before the kd stats, as many as its units make.
    pub research: Option<Research>,
    /// One for each roster place.
    pub kills_deaths: Vec<KillsDeaths>,
    /// The blackboard's `team_1_respawn`, `team_2_respawn`: whether each
    /// team may still respawn. Team Deathmatch clears one when the team's
    /// tickets reach 0.
    pub team_respawn: [bool; 2],
}

impl Rules {
    /// The least a rules section holds: the timelimit, the respawn rule's
    /// mask and seed, the kd stats, the two flags.
    fn min_len(players: usize) -> usize {
        Timelimit::LEN + 8 + KillsDeaths::LEN * players + 2
    }

    /// Reads the rules of the snapshot at `time` of a match of `game_mode`
    /// with `players` roster places and, in modes 2 and 3, `batteries`
    /// batteries. `None`: the section does not start with a timelimit
    /// rule of that time (a state it can have, no player outside the
    /// roster), or does not end in kd stats and two flags.
    ///
    /// The respawn rule has two bytes for each respawn point of the map,
    /// and nothing says how many: in mode 4 the rest is the tdm rule's 12
    /// bytes; in mode 5 Research's 21 + 96 a unit (309 with its three
    /// units); in modes 2 and 3 it is the number with which the gather
    /// rule after it reads to the kd stats exactly, the smallest if
    /// several do.
    pub fn decode(
        bytes: &[u8],
        time: u32,
        players: usize,
        game_mode: u8,
        batteries: usize,
    ) -> Option<Self> {
        if players > MAX_PLAYERS || bytes.len() < Self::min_len(players) {
            return None;
        }
        let flags_at = bytes.len() - 2;
        let kd_at = flags_at - KillsDeaths::LEN * players;
        // The bools of the section are 0x00 or 0xFF. Until 2026-10-12
        // only 0 and 1 were taken, and the rules of no live world read:
        // not one of the 37 matches that ended by themselves until then
        // had its rules looked at (the standing is in the statistics, so
        // battery matches ended all the same; a Team Deathmatch could not
        // have ended by its rule).
        let team_respawn = [wire_bool(bytes[flags_at])?, wire_bool(bytes[flags_at + 1])?];
        let kills_deaths = bytes[kd_at..flags_at]
            .as_chunks::<{ KillsDeaths::LEN }>()
            .0
            .iter()
            .map(|record| {
                Some(KillsDeaths {
                    kills: u16::from_le_bytes([record[0], record[1]]),
                    deaths: u16::from_le_bytes([record[2], record[3]]),
                    online: wire_bool(record[4])?,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        let timelimit = Timelimit::read(&bytes[..Timelimit::LEN]).filter(|rule| {
            rule.current == time
                && rule.state <= rules::state::IN_PROCESS
                && u64::from(rule.players_mask) >> players == 0
        })?;
        let middle = &bytes[Timelimit::LEN..kd_at];
        let tdm = (game_mode == mode::TEAM_DEATHMATCH)
            .then(|| middle.len().checked_sub(Tdm::LEN))
            .flatten()
            .and_then(|at| {
                let mut reader = Reader::new(&middle[at..]);
                Some(Tdm {
                    tickets: [reader.u16()?, reader.u16()?],
                    alive_players_mask: reader.u32()?,
                    active_players_mask: reader.u32()?,
                })
            });
        let (research_at, research) = match (game_mode == mode::RESEARCH)
            .then(|| Research::read_back(middle))
            .flatten()
        {
            Some((at, rule)) => (Some(at), Some(rule)),
            None => (None, None),
        };
        let mut respawn = None;
        let mut gather = None;
        if let Some((mut head, head_len)) = Respawn::read_head(middle) {
            let points = |bytes: &[u8]| -> Vec<[u8; 2]> { bytes.as_chunks::<2>().0.to_vec() };
            // Where a mode rule of a size known from the end starts.
            let fixed_at = match game_mode {
                mode::TEAM_DEATHMATCH => Some(middle.len().checked_sub(Tdm::LEN)),
                mode::RESEARCH => Some(research_at),
                _ => None,
            };
            if let Some(end) = fixed_at {
                if let Some(end) = end
                    && end >= head_len
                    && (end - head_len).is_multiple_of(2)
                {
                    head.points = points(&middle[head_len..end]);
                    respawn = Some(head);
                }
            } else {
                let found = (head_len..=middle.len()).step_by(2).find_map(|at| {
                    Gather::read(&middle[at..], game_mode, batteries).map(|gather| (at, gather))
                });
                if let Some((at, read)) = found {
                    head.points = points(&middle[head_len..at]);
                    respawn = Some(head);
                    gather = Some(read);
                }
            }
        }
        Some(Self {
            timelimit,
            respawn,
            gather,
            tdm,
            research,
            kills_deaths,
            team_respawn,
        })
    }

    /// The section's bytes, as the client writes them (free batteries as
    /// [`Battery::write`] has them; a missing respawn rule as one with no
    /// points).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut respawn = Vec::new();
        self.respawn.clone().unwrap_or_default().write(&mut respawn);
        let mut mode_rule = Vec::new();
        if let Some(gather) = &self.gather {
            gather.write(&mut mode_rule);
        }
        if let Some(tdm) = &self.tdm {
            tdm.write(&mut mode_rule);
        }
        if let Some(research) = &self.research {
            research.write(&mut mode_rule);
        }
        self.encode(&respawn, &mode_rule)
    }

    /// A rules section from parts, for tests and tools: the timelimit,
    /// `respawn` (the respawn rule's bytes), `mode_rule` (the gather,
    /// timed, tdm or research rule's), the kd stats, the two flags.
    pub fn encode(&self, respawn: &[u8], mode_rule: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        self.timelimit.write(&mut out);
        out.extend_from_slice(respawn);
        out.extend_from_slice(mode_rule);
        for record in &self.kills_deaths {
            record.write(&mut out);
        }
        out.extend(
            self.team_respawn
                .map(|flag| if flag { WIRE_TRUE } else { 0 }),
        );
        out
    }
}

/// The match's standing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Standing {
    /// Batteries (modes 2, 3), tickets left (mode 4) or the points shown
    /// (mode 5).
    pub points: [u16; 2],
    /// 0, 1, or 2 for a draw.
    pub winner: u8,
}

/// A snapshot body, read.
#[derive(Clone, Debug, PartialEq)]
pub struct Decoded<'a> {
    pub sections: Sections<'a>,
    /// `None` when the section is empty: the client had no statistics
    /// object.
    pub statistics: Option<Statistics>,
    /// `None` when the rules did not read ([`Rules::decode`]); the split
    /// then rests on the statistics alone.
    pub rules: Option<Rules>,
}

impl Decoded<'_> {
    /// Who leads, by the rules of `modes_maps.md` §11. `teams[i]` is roster
    /// place i's team. `None`: the snapshot does not say (no statistics,
    /// and in mode 4 no tdm rule either).
    ///
    /// Mode 4 takes the tdm rule's tickets when it was read: the
    /// statistics' copy is written when play begins and is 0 before.
    /// Modes 2 and 3 take the statistics' points, and with no statistics
    /// the gather rule's own, which are the same count. Mode 5 takes
    /// Research's shown points when its rule was read (the rule copies
    /// them into the statistics in play only), else the statistics'.
    pub fn standing(&self, game_mode: u8, teams: &[u8]) -> Option<Standing> {
        let tail = self.statistics.as_ref().map(|statistics| statistics.tail);
        if game_mode == mode::RESEARCH {
            let research = self.rules.as_ref().and_then(|rules| rules.research.as_ref());
            if let Some(research) = research {
                return Some(Standing {
                    points: research.scores,
                    winner: rules::research_winner(
                        research.scores,
                        research.points,
                        research.end.is_some(),
                    ),
                });
            }
            // Without the rule: the tail's leader, which patch 53's publish()
            // writes from rr_leader every tick (the millionths at the goal),
            // when it is one (3 until a rule's first event); else the points.
            let tail = tail?;
            let winner = if tail.winner_team <= rules::DRAW {
                tail.winner_team
            } else {
                rules::winner_by_points(tail.team_points)
            };
            return Some(Standing {
                points: tail.team_points,
                winner,
            });
        }
        if game_mode != mode::TEAM_DEATHMATCH {
            let gather = self
                .rules
                .as_ref()
                .and_then(|rules| rules.gather.as_ref())
                .map(|gather| gather.team_points.map(u16::from));
            let points = tail.map(|tail| tail.team_points).or(gather)?;
            return Some(Standing {
                points,
                winner: rules::winner_by_points(points),
            });
        }
        let tdm = self.rules.as_ref().and_then(|rules| rules.tdm);
        let points = tdm
            .map(|tdm| tdm.tickets)
            .or(tail.map(|tail| tail.team_points))?;
        let alive = tdm.map_or([0; 2], |tdm| {
            rules::alive_by_team(tdm.alive_players_mask, teams)
        });
        Some(Standing {
            points,
            winner: rules::winner_tdm(points, alive),
        })
    }

    /// Whether a game that takes this as its **first** world ends up with
    /// other statistics than these, and so out of step with every game
    /// that has the world already. It does in a Team Deathmatch
    /// (`game_mode` 4) that is in play, unless the standing is still the
    /// one of the start (`tickets` each, the config's `tickets_count`, and
    /// a draw). PROVEN in the 0.26g exe (match.md §21):
    /// - a world is applied statistics first (0x2D2B18), then the rules in
    ///   their order, the timelimit before the tdm rule (0x2D2B42);
    /// - `timelimit_rule_core::deserialize` 0x317FD0 sends the rules event
    ///   0, "the match started", when the state it reads is "in process"
    ///   and the one it had was not (0x31802B..0x31803E): true of a game's
    ///   first world and of none after it;
    /// - the tdm rule's answer to event 0 (0x316090 → 0x3161F0) writes its
    ///   tickets and the winner they give into the statistics; it still
    ///   has the constructor's, its own `deserialize` (0x3163D0) coming
    ///   after and writing the statistics no more.
    ///
    /// Seen live on 2026-10-12: 4 of 4 players back in a Team Deathmatch
    /// had 100, 100 and a draw there, against 88, 78 and team 0.
    /// `false` when the rules or the statistics did not read: not known.
    pub fn first_load_loses_the_standing(&self, game_mode: u8, tickets: u16) -> bool {
        if game_mode != mode::TEAM_DEATHMATCH {
            return false;
        }
        let (Some(rules), Some(statistics)) = (&self.rules, &self.statistics) else {
            return false;
        };
        let tail = statistics.tail;
        rules.timelimit.state == rules::state::IN_PROCESS
            && (tail.team_points != [tickets; 2] || tail.winner_team != rules::DRAW)
    }
}

/// Reads the body of the snapshot at `time` (0xE8's state without its
/// time; an upload's body) of a match of `game_mode` with `players` roster
/// places and, in modes 2 and 3, `batteries` batteries (the mode config's
/// `victory_items_count`).
///
/// Of the ways to split the body, the one whose statistics fill their
/// section exactly and whose rules read; failing that one whose statistics
/// do; failing that one with empty statistics whose rules read.
pub fn decode(
    time: u32,
    body: &[u8],
    players: usize,
    game_mode: u8,
    batteries: usize,
) -> Option<Decoded<'_>> {
    let batteries = if mode::WITH_BATTERIES.contains(&game_mode) {
        batteries
    } else {
        0
    };
    let mut best: Option<(u8, Decoded)> = None;
    for sections in Sections::candidates(body) {
        let rules = Rules::decode(sections.rules, time, players, game_mode, batteries);
        let statistics = if sections.statistics.is_empty() {
            None
        } else {
            // With no gather rule object the client writes no battery
            // records at all.
            let Some(statistics) = Statistics::decode(sections.statistics, players, batteries)
                .or_else(|| Statistics::decode(sections.statistics, players, 0))
            else {
                continue;
            };
            Some(statistics)
        };
        let rank = match (&statistics, &rules) {
            (Some(_), Some(_)) => 3,
            (Some(_), None) => 2,
            (None, Some(_)) => 1,
            (None, None) => continue,
        };
        let decoded = Decoded {
            sections,
            statistics,
            rules,
        };
        if rank == 3 {
            return Some(decoded);
        }
        if best.as_ref().is_none_or(|(held, _)| *held < rank) {
            best = Some((rank, decoded));
        }
    }
    best.map(|(_, decoded)| decoded)
}

/// The scoreboard's score of a player, as the client computes it
/// (0x26DC30, PROVEN): for each event in order, the three kits' counts
/// added as u16 (wrapping), as f32, times the weight, times 0.01 for the
/// hundredths events; all added up in f32; 0.5 added and rounded down,
/// once, at the end.
pub fn score(player: &PlayerStatistics, weights: &[f32; EVENTS]) -> u32 {
    let mut total = 0.0f32;
    for event in 0..EVENTS {
        let factor: f32 = if HUNDREDTHS.contains(&event) {
            0.01
        } else {
            1.0
        };
        let count = player.events[event]
            .iter()
            .fold(0u16, |sum, kit| sum.wrapping_add(*kit));
        total += f32::from(count) * weights[event] * factor;
    }
    (total + 0.5).floor().clamp(0.0, u32::MAX as f32) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_crc_is_the_common_crc_32() {
        // The check value of CRC-32/ISO-HDLC, which boost's crc_32_type is.
        assert_eq!(crc(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc(b""), 0);
        assert_eq!(crc(&[0]), 0xD202_EF8D);
    }

    /// A player's statistics, byte for byte as snapshot_rules.md §3.1 lays
    /// them out: 3 kills and a death with kit 0, 150 hundredths of a
    /// battery brought and 300 bullets fired with kit 1.
    fn player_bytes() -> Vec<u8> {
        let mut bytes = Vec::new();
        // Kit 0: events 0 (kill) and 2 (death), one byte each.
        bytes.extend([0x05, 0, 0, 0, 3, 1]);
        // Kit 1: kit 0's bits stay, a zero each; then events 6 and 22, u16
        // each, in the table's order.
        bytes.extend([0x45, 0x00, 0x40, 0x00, 0, 0, 0x96, 0x00, 0x2C, 0x01]);
        // Kit 2: nothing of its own, so the same mask and zeros.
        bytes.extend([0x45, 0x00, 0x40, 0x00, 0, 0, 0, 0, 0, 0]);
        // Damage received with kit 0 from player 1: 12.5.
        bytes.extend([0x02, 0, 0, 0, 0x00, 0x00, 0x48, 0x41]);
        bytes.extend([0, 0, 0, 0]);
        bytes.extend([0, 0, 0, 0]);
        // Last hit on player 19 at 70 000 ms.
        bytes.extend([0x00, 0x00, 0x08, 0x00, 0x70, 0x11, 0x01, 0x00]);
        // Last kill at 65 536 ms, last hit enemy 19, streak 2.
        bytes.extend([0x00, 0x00, 0x01, 0x00, 19, 2]);
        // The action coefficient 1.0, the longest streak 3.
        bytes.extend([0x00, 0x00, 0x80, 0x3F, 3, 0]);
        bytes
    }

    fn player() -> PlayerStatistics {
        let mut player = PlayerStatistics::default();
        player.events[event::KILL][0] = 3;
        player.events[event::DEATH][0] = 1;
        player.events[event::BRING_VICTORY_ITEM][1] = 150;
        player.events[event::BULLET_FIRE][1] = 300;
        player.received_damage[0][1] = 12.5;
        player.last_hit_ms[19] = 70_000;
        player.last_killed_enemy_ms = 65_536;
        player.last_hit_enemy_id = 19;
        player.killstreak = 2;
        player.newbee_bonus_player_action_coef = 1.0;
        player.longest_killstreak = 3;
        player
    }

    /// The 11-byte tail: 4 and 7 points, victory items 1 and 2, team 1
    /// leads, players 0 and 1 active.
    const TAIL: [u8; 11] = [4, 0, 7, 0, 1, 2, 1, 0x03, 0, 0, 0];

    fn tail() -> StatisticsTail {
        StatisticsTail {
            team_points: [4, 7],
            team_victory_items: [1, 2],
            winner_team: 1,
            active_players_mask: 0b11,
        }
    }

    /// An untouched player: three empty event masks, three damage masks,
    /// the hit mask, then 12 bytes.
    const EMPTY_PLAYER: [u8; 40] = [0; 40];

    #[test]
    fn a_players_statistics_are_read_and_written_byte_for_byte() {
        let mut bytes = player_bytes();
        bytes.extend(EMPTY_PLAYER);
        bytes.extend(TAIL);
        let statistics = Statistics::decode(&bytes, 2, 0).unwrap();
        assert_eq!(statistics.players[0], player());
        assert_eq!(statistics.players[1], PlayerStatistics::default());
        assert_eq!(statistics.tail, tail());
        assert_eq!(statistics.encode(), bytes);
        assert_eq!(statistics.players[0].total(event::KILL), 3);
        // One byte over or under is not the section.
        assert_eq!(Statistics::decode(&bytes[..bytes.len() - 1], 2, 0), None);
        bytes.push(0);
        assert_eq!(Statistics::decode(&bytes, 2, 0), None);
    }

    #[test]
    fn one_byte_events_come_before_a_kits_wide_ones() {
        // Events 5, 6, 7 with one kit: 5 and 7 as bytes, then 6 as u16.
        let mut bytes = vec![0xE0, 0, 0, 0, 55, 77, 0x9A, 0x02];
        // The other two kits: the same mask, and zeros in the same order.
        bytes.extend([0xE0, 0, 0, 0, 0, 0, 0, 0]);
        bytes.extend([0xE0, 0, 0, 0, 0, 0, 0, 0]);
        bytes.extend([0; 28]);
        bytes.extend(TAIL);
        let statistics = Statistics::decode(&bytes, 1, 0).unwrap();
        let events = &statistics.players[0].events;
        assert_eq!((events[5][0], events[6][0], events[7][0]), (55, 666, 77));
        assert_eq!(statistics.encode(), bytes);
        // Every event is in one table or the other, once.
        let mut all: Vec<usize> = BYTE_EVENTS.into_iter().chain(WORD_EVENTS).collect();
        all.sort_unstable();
        assert_eq!(all, (0..EVENTS).collect::<Vec<_>>());
    }

    #[test]
    fn an_event_over_255_wraps_in_its_byte() {
        let mut player = PlayerStatistics::default();
        player.events[event::KILL][0] = 0x0105;
        let mut out = Vec::new();
        player.write(&mut out);
        assert_eq!(out[..5], [0x01, 0, 0, 0, 0x05]);
    }

    /// The events of a made-up player's record, as `write` gives them: the
    /// bytes before the three damage masks.
    fn event_bytes(player: &PlayerStatistics) -> Vec<u8> {
        let mut out = Vec::new();
        player.write(&mut out);
        // Three damage masks, the hit mask and 12 bytes follow.
        out.truncate(out.len() - 28);
        out
    }

    #[test]
    fn a_kits_event_mask_keeps_the_bits_of_the_kits_before_it() {
        // Kit 0 alone: 4 kills, a death and 300 bullets fired.
        let mut first_kit = PlayerStatistics::default();
        first_kit.events[event::KILL][0] = 4;
        first_kit.events[event::DEATH][0] = 1;
        first_kit.events[event::BULLET_FIRE][0] = 300;
        let mask = [0x05, 0x00, 0x40, 0x00];
        let mut bytes = Vec::new();
        bytes.extend(mask);
        bytes.extend([4, 1, 0x2C, 0x01]);
        // The same mask twice more, a zero for each of its events.
        bytes.extend(mask);
        bytes.extend([0, 0, 0, 0]);
        bytes.extend(mask);
        bytes.extend([0, 0, 0, 0]);
        assert_eq!(event_bytes(&first_kit), bytes);

        // Kit 1 alone: kit 0's mask has none of its bits, kit 2's has all.
        let mut second_kit = PlayerStatistics::default();
        second_kit.events[event::KILL][1] = 4;
        second_kit.events[event::DEATH][1] = 1;
        second_kit.events[event::BULLET_FIRE][1] = 300;
        let mut bytes = vec![0, 0, 0, 0];
        bytes.extend(mask);
        bytes.extend([4, 1, 0x2C, 0x01]);
        bytes.extend(mask);
        bytes.extend([0, 0, 0, 0]);
        assert_eq!(event_bytes(&second_kit), bytes);

        // Each kit an event of its own: a bit more with every kit, and a
        // kit's count under a bit an earlier kit set.
        let mut every_kit = PlayerStatistics::default();
        every_kit.events[event::KILL] = [2, 0, 7];
        every_kit.events[event::BRING_VICTORY_ITEM][1] = 150;
        every_kit.events[event::DEATH][2] = 1;
        let mut bytes = vec![0x01, 0, 0, 0, 2];
        bytes.extend([0x41, 0, 0, 0, 0, 0x96, 0x00]);
        bytes.extend([0x45, 0, 0, 0, 7, 1, 0, 0]);
        assert_eq!(event_bytes(&every_kit), bytes);

        // What is written reads back as it was.
        let statistics = Statistics {
            players: vec![first_kit, second_kit, every_kit],
            victory_items: Vec::new(),
            tail: tail(),
        };
        let bytes = statistics.encode();
        assert_eq!(Statistics::decode(&bytes, 3, 0), Some(statistics));
    }

    #[test]
    fn a_kits_counts_add_up_to_the_players() {
        let mut player = PlayerStatistics::default();
        player.events[event::KILL] = [2, 3, 4];
        player.events[event::BRING_VICTORY_ITEM] = [100, 0, 50];
        assert_eq!(player.total(event::KILL), 9);
        assert_eq!(player.totals()[event::BRING_VICTORY_ITEM], 150);
        let mut weights = [0.0; EVENTS];
        weights[event::KILL] = 100.0;
        weights[event::BRING_VICTORY_ITEM] = 100.0;
        assert_eq!(score(&player, &weights), 900 + 150);
    }

    /// A battery's record (snapshot_rules.md §3.2): carried 2.0 by player
    /// 1 with kit 0, nearest 1.0 and 3.0, team 1 owns it, player 0 found it,
    /// 3 steps from team 0's container and untouched for team 1's.
    fn battery_bytes() -> Vec<u8> {
        let mut bytes = vec![0x02, 0, 0, 0, 0x00, 0x00, 0x00, 0x40];
        bytes.extend([0; 8]);
        bytes.extend([0x00, 0x00, 0x80, 0x3F, 0x00, 0x00, 0x40, 0x40, 1, 0, 4, 0]);
        bytes
    }

    #[test]
    fn batteries_follow_the_players_in_modes_two_and_three() {
        let mut bytes = EMPTY_PLAYER.to_vec();
        bytes.extend(EMPTY_PLAYER);
        bytes.extend(battery_bytes());
        // An untouched battery: three masks, two positions, four bytes.
        bytes.extend([0; 24]);
        bytes.extend(TAIL);
        let statistics = Statistics::decode(&bytes, 2, 2).unwrap();
        let first = &statistics.victory_items[0];
        assert_eq!(first.carried_distances[0][1], 2.0);
        assert_eq!(first.nearest_positions, [1.0, 3.0]);
        assert_eq!((first.owner_team, first.first_finder), (1, 0));
        assert_eq!(first.progress, [4, 0]);
        assert_eq!(statistics.victory_items[1], VictoryItemStatus::default());
        assert_eq!(statistics.encode(), bytes);
        // Read with another battery count it does not fit.
        assert_eq!(Statistics::decode(&bytes, 2, 1), None);
        assert_eq!(Statistics::decode(&bytes, 2, 3), None);
        // A carrier outside the roster is no battery record.
        bytes[80] = 0x04;
        assert_eq!(Statistics::decode(&bytes, 2, 2), None);
    }

    #[test]
    fn the_tail_is_read_without_the_rest() {
        let mut bytes = vec![0xEE; 100];
        bytes.extend(TAIL);
        assert_eq!(StatisticsTail::of(&bytes), Some(tail()));
        assert_eq!(StatisticsTail::of(&TAIL), Some(tail()));
        assert_eq!(StatisticsTail::of(&TAIL[1..]), None);
        assert_eq!(StatisticsTail::of(&[]), None);
    }

    /// The timelimit rule at 539 650 ms (snapshot_rules.md §2.2): in
    /// process since 21 000 ms, players 0 and 1 in.
    const TIMELIMIT: [u8; 13] = [
        0x08, 0x52, 0x00, 0x00, // start 21 000
        0x02, 0x3C, 0x08, 0x00, // current 539 650
        0x03, 0x00, 0x00, 0x00, // players
        0x03, // in process
    ];
    const TIME: u32 = 539_650;

    /// The respawn rule with no one waiting, a seed and three points.
    const RESPAWN: [u8; 14] = [0, 0, 0, 0, 0x44, 0x33, 0x22, 0x11, 0, 0, 0, 0, 0, 0];
    /// The tdm rule (§2.6): tickets 97 and 100, players 0 and 1 alive and
    /// active.
    const TDM: [u8; 12] = [97, 0, 100, 0, 3, 0, 0, 0, 3, 0, 0, 0];
    /// Kills, deaths, online for two players (§2.7), then the two flags.
    /// `true` is 0xFF, as the client writes it.
    const KD_AND_FLAGS: [u8; 12] = [3, 0, 0, 0, 0xFF, 0, 0, 3, 0, 0xFF, 0xFF, 0xFF];

    fn tdm_rules() -> Vec<u8> {
        [&TIMELIMIT[..], &RESPAWN, &TDM, &KD_AND_FLAGS].concat()
    }

    #[test]
    fn the_rules_of_a_team_deathmatch() {
        let bytes = tdm_rules();
        let rules = Rules::decode(&bytes, TIME, 2, mode::TEAM_DEATHMATCH, 0).unwrap();
        assert_eq!(
            rules.timelimit,
            Timelimit {
                start: Some(21_000),
                current: TIME,
                players_mask: 0b11,
                state: 3,
            }
        );
        assert_eq!(rules.timelimit.time_limit_at(900_000), Some(921_000));
        // Nobody waiting, a seed, three respawn points.
        assert_eq!(
            rules.respawn,
            Some(Respawn {
                times: vec![],
                seed: 0x1122_3344,
                points: vec![[0, 0]; 3],
            })
        );
        assert_eq!(rules.gather, None);
        assert_eq!(
            rules.kills_deaths,
            [
                KillsDeaths {
                    kills: 3,
                    deaths: 0,
                    online: true
                },
                KillsDeaths {
                    kills: 0,
                    deaths: 3,
                    online: true
                },
            ]
        );
        assert_eq!(rules.team_respawn, [true, true]);
        assert_eq!(
            rules.tdm,
            Some(Tdm {
                tickets: [97, 100],
                alive_players_mask: 3,
                active_players_mask: 3,
            })
        );
        assert_eq!(rules.encode(&RESPAWN, &TDM), bytes);
        assert_eq!(rules.to_bytes(), bytes);
        // The same bytes as a battery match: no tdm rule is looked for,
        // and 12 bytes are no gather rule.
        let rules = Rules::decode(&bytes, TIME, 2, mode::BATTERIES, 10).unwrap();
        assert_eq!((rules.tdm, rules.gather, rules.respawn), (None, None, None));
    }

    /// Research's rule (snapshot_rules.md §2.8), made up: team 0 owns unit
    /// A and has 64.4 points, team 1 has 37.5; player 1 is taking unit B
    /// for team 1, half done; unit C is nobody's.
    fn research() -> Research {
        let mut marks = [0; MAX_PLAYERS];
        marks[1] = 250_000_000;
        Research {
            points: [64_400_000, 37_500_000],
            scores: [64, 38],
            end: None,
            entered: 0b11,
            units: vec![
                ResearchUnit {
                    progress: Research::FULL,
                    taking: 0,
                    owner: 0,
                    ..ResearchUnit::default()
                },
                ResearchUnit {
                    progress: 500_000_000,
                    taking: 1,
                    counts: [0, 1],
                    inside: [0, 0b10],
                    marks,
                    ..ResearchUnit::default()
                },
                ResearchUnit::default(),
            ],
        }
    }

    fn research_bytes(rule: &Research) -> Vec<u8> {
        let mut bytes = Vec::new();
        rule.write(&mut bytes);
        bytes
    }

    fn research_rules(rule: &Research) -> Vec<u8> {
        [
            &TIMELIMIT[..],
            &RESPAWN,
            &research_bytes(rule),
            &KD_AND_FLAGS,
        ]
        .concat()
    }

    #[test]
    fn the_rules_of_a_research_match() {
        let rule = research();
        let own = research_bytes(&rule);
        // 21 bytes and 96 a unit, little-endian.
        assert_eq!(own.len(), 309);
        assert_eq!(Research::len_of(3), 309);
        assert_eq!(own[..8], [0x80, 0xAA, 0xD6, 0x03, 0x60, 0x34, 0x3C, 0x02]);
        assert_eq!(
            own[8..21],
            [64, 0, 38, 0, 0xFF, 0xFF, 0xFF, 0xFF, 3, 0, 0, 0, 3]
        );
        // Unit B: progress, taking, owner, counts, inside, place 1's mark.
        let b = &own[21 + 96..21 + 2 * 96];
        assert_eq!(
            b[..16],
            [0, 0x65, 0xCD, 0x1D, 1, 2, 0, 1, 0, 0, 0, 0, 2, 0, 0, 0]
        );
        assert_eq!(b[20..24], [0x80, 0xB2, 0xE6, 0x0E]);

        let bytes = research_rules(&rule);
        let rules = Rules::decode(&bytes, TIME, 2, mode::RESEARCH, 0).unwrap();
        assert_eq!(rules.research.as_ref(), Some(&rule));
        assert_eq!((rules.tdm, rules.gather.as_ref()), (None, None));
        // The rest before it is the respawn rule's three points.
        assert_eq!(rules.respawn.as_ref().unwrap().points, vec![[0, 0]; 3]);
        assert_eq!(rules.kills_deaths[0].kills, 3);
        assert_eq!(rules.to_bytes(), bytes);
        // The same bytes as another mode's: no research rule.
        for game_mode in [mode::BATTERIES, mode::TEAM_DEATHMATCH] {
            let rules = Rules::decode(&bytes, TIME, 2, game_mode, 10).unwrap();
            assert_eq!(rules.research, None, "{game_mode}");
        }
        // Its end, once a team has the goal.
        let over = Research {
            points: [100_000_000, 41_000_000],
            scores: [100, 41],
            end: Some(600_000),
            ..rule
        };
        let rules = Rules::decode(&research_rules(&over), TIME, 2, mode::RESEARCH, 0).unwrap();
        assert_eq!(rules.research.unwrap().end, Some(600_000));
    }

    #[test]
    fn the_research_rules_length_gives_the_respawn_points() {
        let rule = research();
        // 20 respawn points (London's) and one unit less: each read from
        // the rule's own count.
        let respawn = Respawn {
            times: vec![(1, 70_000)],
            seed: 9,
            points: vec![[1, 0]; 20],
        };
        let mut respawn_bytes = Vec::new();
        respawn.write(&mut respawn_bytes);
        let two = Research {
            units: rule.units[..2].to_vec(),
            ..rule.clone()
        };
        for each in [&rule, &two] {
            let bytes = [
                &TIMELIMIT[..],
                &respawn_bytes,
                &research_bytes(each),
                &KD_AND_FLAGS,
            ]
            .concat();
            let rules = Rules::decode(&bytes, TIME, 2, mode::RESEARCH, 0).unwrap();
            assert_eq!(rules.research.as_ref(), Some(each));
            assert_eq!(rules.respawn.as_ref(), Some(&respawn));
        }
        // An odd byte before the rule: no respawn rule, the research rule
        // still read from the end.
        let bytes = [
            &TIMELIMIT[..],
            &RESPAWN,
            &[0],
            &research_bytes(&rule),
            &KD_AND_FLAGS,
        ]
        .concat();
        let rules = Rules::decode(&bytes, TIME, 2, mode::RESEARCH, 0).unwrap();
        assert_eq!((rules.respawn, rules.research), (None, Some(rule)));
    }

    #[test]
    fn a_wrong_research_rule_is_refused() {
        let unread = |rule: &Research| {
            Rules::decode(&research_rules(rule), TIME, 2, mode::RESEARCH, 0)
                .unwrap()
                .research
        };
        let good = research();
        assert!(unread(&good).is_some());
        let mut wrong = good.clone();
        // Inside counts that are not the places inside.
        wrong.units[1].counts = [0, 2];
        assert_eq!(unread(&wrong), None);
        // A team 3, a progress over a whole capture, a place over 20.
        let mut wrong = good.clone();
        wrong.units[0].owner = 3;
        assert_eq!(unread(&wrong), None);
        let mut wrong = good.clone();
        wrong.units[2].progress = Research::FULL + 1;
        assert_eq!(unread(&wrong), None);
        let mut wrong = good.clone();
        wrong.entered = 1 << 20;
        assert_eq!(unread(&wrong), None);
        // Four units, or none.
        let mut wrong = good.clone();
        wrong.units.push(ResearchUnit::default());
        assert_eq!(unread(&wrong), None);
        wrong.units.clear();
        assert_eq!(unread(&wrong), None);
        // A byte short (its unit count): no count where the length says.
        let mut bytes = research_rules(&good);
        bytes.remove(Timelimit::LEN + RESPAWN.len() + 20);
        let rules = Rules::decode(&bytes, TIME, 2, mode::RESEARCH, 0).unwrap();
        assert_eq!(rules.research, None);
    }

    #[test]
    fn a_research_matchs_standing_is_its_rules_shown_points() {
        let mut statistics = player_bytes();
        statistics.extend(EMPTY_PLAYER);
        // The tail as the rule writes it in play: the shown points, the
        // leader; no batteries.
        statistics.extend([64, 0, 38, 0, 0, 0, 0, 3, 0, 0, 0]);
        let rules = research_rules(&research());
        let whole = body(&statistics, &rules);
        // A battery count is not used in mode 5.
        let decoded = decode(TIME, &whole, 2, mode::RESEARCH, 10).unwrap();
        let read = decoded.statistics.as_ref().unwrap();
        assert!(read.victory_items.is_empty());
        assert_eq!(decoded.rules.as_ref().unwrap().research, Some(research()));
        assert_eq!(
            decoded.standing(mode::RESEARCH, &[0, 1]),
            Some(Standing {
                points: [64, 38],
                winner: 0
            })
        );
        // The rule's points before the statistics' (written in play only).
        let mut early = statistics.clone();
        let at = early.len() - StatisticsTail::LEN;
        early[at..at + 4].fill(0);
        let early = body(&early, &rules);
        let decoded = decode(TIME, &early, 2, mode::RESEARCH, 0).unwrap();
        let standing = decoded.standing(mode::RESEARCH, &[0, 1]).unwrap();
        assert_eq!(standing.points, [64, 38]);
        // Without the rule, the statistics': equal points, the leader the
        // rule wrote a draw.
        let mut even = statistics.clone();
        even[at..at + 4].copy_from_slice(&[40, 0, 40, 0]);
        even[at + 6] = rules::DRAW;
        let even = body(&even, &[]);
        let decoded = decode(TIME, &even, 2, mode::RESEARCH, 0).unwrap();
        assert_eq!(decoded.rules, None);
        assert_eq!(
            decoded.standing(mode::RESEARCH, &[0, 1]),
            Some(Standing {
                points: [40, 40],
                winner: rules::DRAW
            })
        );
    }

    #[test]
    fn a_research_match_without_its_rule_takes_the_tails_leader() {
        // Both show 100 at the goal; the tail's leader (rr_leader, by the
        // millionths) is team 1.
        let mut statistics = player_bytes();
        statistics.extend(EMPTY_PLAYER);
        statistics.extend([100, 0, 100, 0, 0, 0, 1, 3, 0, 0, 0]);
        let whole = body(&statistics, &[]);
        let decoded = decode(TIME, &whole, 2, mode::RESEARCH, 0).unwrap();
        assert_eq!(decoded.rules, None);
        assert_eq!(
            decoded.standing(mode::RESEARCH, &[0, 1]),
            Some(Standing {
                points: [100, 100],
                winner: 1
            })
        );
        // No leader written yet (3): the points, equal a draw.
        let at = statistics.len() - StatisticsTail::LEN + 6;
        statistics[at] = 3;
        let whole = body(&statistics, &[]);
        let decoded = decode(TIME, &whole, 2, mode::RESEARCH, 0).unwrap();
        assert_eq!(
            decoded.standing(mode::RESEARCH, &[0, 1]).map(|standing| standing.winner),
            Some(rules::DRAW)
        );
    }

    #[test]
    fn a_research_match_at_the_goal_is_won_by_the_millionths() {
        // Both show 100: team 1 reached the goal, team 0 has 99.6.
        let mut rule = research();
        rule.points = [99_600_000, 100_000_000];
        rule.scores = [100, 100];
        rule.end = Some(TIME + 3_000);
        let mut statistics = player_bytes();
        statistics.extend(EMPTY_PLAYER);
        statistics.extend([100, 0, 100, 0, 0, 0, 1, 3, 0, 0, 0]);
        let whole = body(&statistics, &research_rules(&rule));
        let decoded = decode(TIME, &whole, 2, mode::RESEARCH, 0).unwrap();
        assert_eq!(decoded.rules.as_ref().unwrap().research, Some(rule.clone()));
        assert_eq!(
            decoded.standing(mode::RESEARCH, &[0, 1]),
            Some(Standing {
                points: [100, 100],
                winner: 1
            })
        );
    }

    /// The rules section of a live world (match 0x25ee80b6 of 2026-10-12,
    /// level_07, mode 2, seven players, at 914 900 ms): the bools are 0xFF.
    /// Team 0 has nine batteries in its container (id 0 on level_07) and
    /// the tenth lies in the world.
    const LIVE_BATTERY_RULES: &[&str] = &[
        "885e0000d4f50d007f000000030000000053fd5f92000000000000000000000000000000000000000000000000000000",
        "00000000000000000000000000000000000000000000000000000000000000000000001b7f10470900ff0300000000ff",
        "0000000000ff0000000000ff0001000000ffff867e6fbf00000000d3deb43e00000000000000000000803f0000000000",
        "000000d3deb4be00000000867e6fbf000000009cec4e42d138a641e8db02400000803f869240bfb0c4c23d1fe9263f00",
        "0000000899a6bbed187d3f30ad19be00000000ffab28bfa1fdedbdde433ebf00000000b34c5142f0a3a6415d893d4000",
        "00803f0000000000000000000000000000000000000000000000000c110140020000000000000000ff0000000000ff00",
        "00000000ff0000000000ff0000000000ff0000000000ff00000900000006000501020907040800000000000f001400ff",
        "0c000b00ff11000900ff0d000f00ff0c000800ff0e001100ff05000a00ffffff",
    ];

    #[test]
    fn the_rules_of_a_live_battery_match_read() {
        let bytes: Vec<u8> = LIVE_BATTERY_RULES
            .concat()
            .as_bytes()
            .chunks(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect();
        assert_eq!(bytes.len(), 368);
        let rules = Rules::decode(&bytes, 914_900, 7, mode::BATTERIES, 10).expect("the rules");
        assert_eq!(rules.timelimit.start, Some(24_200));
        assert_eq!(rules.timelimit.state, rules::state::IN_PROCESS);
        assert_eq!(rules.team_respawn, [true, true]);
        assert!(rules.kills_deaths.iter().all(|record| record.online));
        assert_eq!(
            rules
                .kills_deaths
                .iter()
                .map(|record| (record.kills, record.deaths))
                .collect::<Vec<_>>(),
            [
                (15, 20),
                (12, 11),
                (17, 9),
                (13, 15),
                (12, 8),
                (14, 17),
                (5, 10)
            ]
        );
        // The map's respawn points (`MapMode::respawn_points`): level_07's
        // in batteries.
        let map = super::super::super::maps::playable_in(mode::BATTERIES)
            .find(|map| map.level == 7)
            .unwrap();
        assert_eq!(
            rules.respawn.as_ref().map(|respawn| respawn.points.len()),
            Some(map.respawn_points)
        );
        let gather = rules.gather.expect("the gather rule");
        assert_eq!(gather.team_points, [9, 0]);
        assert_eq!(gather.containers[0].items, [6, 0, 5, 1, 2, 9, 7, 4, 8]);
        assert!(gather.containers[1].items.is_empty());
        for (number, battery) in gather.batteries.iter().enumerate() {
            if number == 3 {
                assert!(matches!(
                    battery,
                    Battery::Free {
                        spotted_mask: 1,
                        ..
                    }
                ));
            } else {
                assert_eq!(*battery, Battery::InContainer(0));
            }
        }
        // A byte that is no bool is still refused.
        let mut wrong = bytes.clone();
        *wrong.last_mut().unwrap() = 7;
        assert_eq!(Rules::decode(&wrong, 914_900, 7, mode::BATTERIES, 10), None);
    }

    #[test]
    fn the_timelimit_rule_is_the_first_and_of_the_snapshots_time() {
        // 0.21d's order (the mode rule and the respawn rule first) is not
        // 0.26g's: not read.
        let bytes = [&[0xEE; 6][..], &RESPAWN, &TIMELIMIT, &KD_AND_FLAGS].concat();
        assert_eq!(Rules::decode(&bytes, TIME, 2, mode::BATTERIES, 10), None);
        // Read for another time the rules are not this snapshot's.
        let bytes = tdm_rules();
        assert_eq!(
            Rules::decode(&bytes, TIME + 50, 2, mode::TEAM_DEATHMATCH, 0),
            None
        );
    }

    #[test]
    fn respawn_points_are_what_is_left_before_the_tdm_rule() {
        // Player 1 waits to respawn at 70 000 ms; three points.
        let respawn = Respawn {
            times: vec![(1, 70_000)],
            seed: 0x1234_5678,
            points: vec![[0, 0], [1, 0], [0, 2]],
        };
        let mut respawn_bytes = Vec::new();
        respawn.write(&mut respawn_bytes);
        assert_eq!(
            respawn_bytes,
            [
                2, 0, 0, 0, // the mask: player 1
                0x70, 0x11, 0x01, 0x00, // its time
                0x78, 0x56, 0x34, 0x12, // the seed
                0, 0, 1, 0, 0, 2, // the points' two counts each
            ]
        );
        let bytes = [&TIMELIMIT[..], &respawn_bytes, &TDM, &KD_AND_FLAGS].concat();
        let rules = Rules::decode(&bytes, TIME, 2, mode::TEAM_DEATHMATCH, 0).unwrap();
        assert_eq!(rules.respawn, Some(respawn));
        assert_eq!(rules.tdm.unwrap().tickets, [97, 100]);
        assert_eq!(rules.to_bytes(), bytes);
        // An odd number of bytes for the points: no respawn rule read, the
        // tdm rule still is.
        let bytes = [&TIMELIMIT[..], &respawn_bytes, &[9], &TDM, &KD_AND_FLAGS].concat();
        let rules = Rules::decode(&bytes, TIME, 2, mode::TEAM_DEATHMATCH, 0).unwrap();
        assert_eq!(rules.respawn, None);
        assert_eq!(rules.tdm.unwrap().tickets, [97, 100]);
    }

    /// A gather rule of four batteries: 0 in container 1, 1 carried, 2 free
    /// at (1, 2, 3), 3 in container 0; player 0 at container 0.
    fn gather() -> Gather {
        Gather {
            seed: 0xAABB_CCDD,
            team_points: [1, 1],
            batteries: vec![
                Battery::InContainer(1),
                Battery::Carried,
                Battery::Free {
                    spotted_mask: 0b10,
                    position: [1.0, 2.0, 3.0],
                },
                Battery::InContainer(0),
            ],
            containers: vec![
                Container {
                    users: vec![0],
                    items: vec![3],
                },
                Container {
                    users: vec![],
                    items: vec![0],
                },
            ],
            timed: None,
        }
    }

    #[test]
    fn the_gather_rule_is_read_battery_by_battery() {
        let mut bytes = Vec::new();
        gather().write(&mut bytes);
        // The head, as snapshot_rules.md §2.4 has it.
        assert_eq!(bytes[..8], [0xDD, 0xCC, 0xBB, 0xAA, 1, 1, 0b1101, 0]);
        // Battery 0: no spotters, no user, container 1.
        assert_eq!(bytes[8..14], [0, 0, 0, 0, 0xFF, 1]);
        // Battery 2: spotted by player 1, no user, no container; then 160
        // bytes and a count of users.
        assert_eq!(bytes[14..20], [2, 0, 0, 0, 0xFF, 0xFF]);
        let free = &bytes[20..20 + 161];
        assert_eq!(
            free[64 + 48..64 + 60],
            [0, 0, 0x80, 0x3F, 0, 0, 0, 0x40, 0, 0, 0x40, 0x40]
        );
        assert_eq!(free[160], 0);
        // Battery 3, then the containers: users, their ids, a u32 count,
        // the batteries.
        assert_eq!(
            bytes[181..],
            [0, 0, 0, 0, 0xFF, 0, 1, 0, 1, 0, 0, 0, 3, 0, 1, 0, 0, 0, 0]
        );
        assert_eq!(Gather::read(&bytes, mode::BATTERIES, 4), Some(gather()));
        // More batteries than the mask's four were written for; a byte
        // short; a byte over.
        assert_eq!(Gather::read(&bytes, mode::BATTERIES, 3), None);
        assert_eq!(
            Gather::read(&bytes[..bytes.len() - 1], mode::BATTERIES, 4),
            None
        );
        let mut longer = bytes.clone();
        longer.push(0);
        assert_eq!(Gather::read(&longer, mode::BATTERIES, 4), None);
        // Team points that are not what the containers hold.
        let mut wrong = bytes.clone();
        wrong[4] = 2;
        assert_eq!(Gather::read(&wrong, mode::BATTERIES, 4), None);

        // Mode 3: the count of batteries placed before the mask, five
        // fields after the containers.
        let timed = Gather {
            timed: Some(Timed {
                available: 4,
                match_start: 21_000,
                next_spawn: 81_000,
                spawn_time: 60_000,
                spawn_variation: 10_000,
                started: true,
            }),
            ..gather()
        };
        let mut bytes = Vec::new();
        timed.write(&mut bytes);
        assert_eq!(bytes[6..12], [4, 0, 0, 0, 0b1101, 0]);
        assert_eq!(
            bytes[bytes.len() - 17..bytes.len() - 13],
            21_000u32.to_le_bytes()
        );
        assert_eq!(bytes[bytes.len() - 1], 1);
        assert_eq!(Gather::read(&bytes, mode::TIMED_BATTERIES, 10), Some(timed));
    }

    #[test]
    fn the_gather_rule_is_found_after_the_respawn_points() {
        // 30 respawn points (level_02 in modes 2 and 3), counts that look
        // like anything.
        let respawn = Respawn {
            times: vec![],
            seed: 7,
            points: (0..30).map(|point| [point as u8 % 3, 0xFF]).collect(),
        };
        let rules = Rules {
            timelimit: Timelimit::read(&TIMELIMIT).unwrap(),
            respawn: Some(respawn),
            gather: Some(gather()),
            tdm: None,
            research: None,
            kills_deaths: vec![KillsDeaths::default(); 2],
            team_respawn: [true, true],
        };
        let bytes = rules.to_bytes();
        assert_eq!(bytes.len(), 13 + 8 + 60 + 200 + 10 + 2);
        assert_eq!(
            Rules::decode(&bytes, TIME, 2, mode::BATTERIES, 4),
            Some(rules)
        );
    }

    #[test]
    fn a_waiting_match_has_no_start_time_before_it_is_ready() {
        let mut bytes = Vec::new();
        Timelimit {
            start: None,
            current: 50,
            players_mask: 0,
            state: 0,
        }
        .write(&mut bytes);
        assert_eq!(bytes[..4], [0xFF; 4]);
        bytes.extend(RESPAWN);
        bytes.extend(KD_AND_FLAGS);
        let rules = Rules::decode(&bytes, 50, 2, mode::BATTERIES, 10).unwrap();
        assert_eq!(rules.timelimit.start, None);
        assert_eq!(rules.timelimit.time_limit_at(900_000), None);
    }

    #[test]
    fn rules_that_do_not_end_in_kd_stats_and_flags_are_refused() {
        let good = tdm_rules();
        let last = good.len() - 1;
        let mut bytes = good.clone();
        bytes[last] = 2; // a flag that is no bool
        assert_eq!(
            Rules::decode(&bytes, TIME, 2, mode::TEAM_DEATHMATCH, 0),
            None
        );
        let mut bytes = good.clone();
        bytes[last - 2] = 7; // `online` that is no bool
        assert_eq!(
            Rules::decode(&bytes, TIME, 2, mode::TEAM_DEATHMATCH, 0),
            None
        );
        let mut bytes = good.clone();
        bytes[8] = 0x07; // a player outside the roster in the mask
        assert_eq!(
            Rules::decode(&bytes, TIME, 2, mode::TEAM_DEATHMATCH, 0),
            None
        );
        assert_eq!(
            Rules::decode(&good[..20], TIME, 2, mode::TEAM_DEATHMATCH, 0),
            None
        );
    }

    /// A body: the mask, two "player blocks", the six sections.
    fn body(statistics: &[u8], rules: &[u8]) -> Vec<u8> {
        Sections {
            active_clients_mask: 0b11,
            // Bytes that look like prefixes, as a bit stream may.
            players: &[0x02, 0x00, 0xAA, 0xBB, 0x00, 0x00, 0x01, 0x00, 0xCC],
            bullets: &[1, 2, 3],
            statistics,
            objects: &[4; 300],
            physics: &[5; 7],
            rules,
            effects: &[],
        }
        .to_body()
        .unwrap()
    }

    fn tdm_statistics() -> Vec<u8> {
        let mut bytes = player_bytes();
        bytes.extend(EMPTY_PLAYER);
        // Tickets 97 and 100, team 1 leads, both players active.
        bytes.extend([97, 0, 100, 0, 0, 0, 1, 3, 0, 0, 0]);
        bytes
    }

    #[test]
    fn a_body_is_the_mask_the_players_and_six_prefixed_sections() {
        let sections = Sections {
            active_clients_mask: 0x0000_0101,
            players: &[0xAA, 0xBB],
            bullets: &[1],
            statistics: &[],
            objects: &[2, 3],
            physics: &[4],
            rules: &[5, 6, 7],
            effects: &[8],
        };
        let expected = [
            0x01, 0x01, 0x00, 0x00, // mask
            0xAA, 0xBB, // player blocks
            1, 0, 1, // bullets
            0, 0, // statistics, empty
            2, 0, 2, 3, // objects
            1, 0, 4, // physics
            3, 0, 5, 6, 7, // rules
            1, 0, 8, // effects
        ];
        let body = sections.to_body().unwrap();
        assert_eq!(body, expected);
        assert!(Sections::candidates(&body).contains(&sections));
    }

    #[test]
    fn a_running_team_deathmatch_is_read_from_its_snapshot() {
        let statistics = tdm_statistics();
        let rules = tdm_rules();
        let body = body(&statistics, &rules);
        let decoded = decode(TIME, &body, 2, mode::TEAM_DEATHMATCH, 10).unwrap();
        assert_eq!(decoded.sections.statistics, statistics);
        assert_eq!(decoded.sections.rules, rules);
        assert_eq!(decoded.sections.bullets, [1, 2, 3]);
        assert_eq!(decoded.sections.players.len(), 9);
        let read = decoded.statistics.as_ref().unwrap();
        assert_eq!(read.players[0], player());
        assert_eq!(read.tail.team_points, [97, 100]);
        assert_eq!(decoded.rules.as_ref().unwrap().timelimit.state, 3);
        assert_eq!(
            decoded.standing(mode::TEAM_DEATHMATCH, &[0, 1]),
            Some(Standing {
                points: [97, 100],
                winner: 1
            })
        );
        // The body round-trips, and its CRC is what a client reports.
        assert_eq!(decoded.sections.to_body().unwrap(), body);
        assert_eq!(crc(&body), crc(&decoded.sections.to_body().unwrap()));
    }

    #[test]
    fn a_battery_match_is_read_with_its_batteries() {
        let mut statistics = EMPTY_PLAYER.to_vec();
        statistics.extend(EMPTY_PLAYER);
        statistics.extend(battery_bytes());
        statistics.extend(TAIL);
        let rules = [&TIMELIMIT[..], &RESPAWN, &[0xEE; 30], &KD_AND_FLAGS].concat();
        let body = body(&statistics, &rules);
        let decoded = decode(TIME, &body, 2, mode::BATTERIES, 1).unwrap();
        assert_eq!(decoded.statistics.as_ref().unwrap().victory_items.len(), 1);
        assert_eq!(
            decoded.standing(mode::BATTERIES, &[0, 1]),
            Some(Standing {
                points: [4, 7],
                winner: 1
            })
        );
        // The wrong battery count: nothing reads.
        assert_eq!(decode(TIME, &body, 2, mode::BATTERIES, 2), None);
    }

    #[test]
    fn tickets_come_from_the_rule_before_the_first_death() {
        // Nobody died: the statistics' points are still 0, the rule has
        // 100 tickets each. Not "both at zero".
        let mut statistics = EMPTY_PLAYER.to_vec();
        statistics.extend(EMPTY_PLAYER);
        statistics.extend([0; 11]);
        let rules = [
            &TIMELIMIT[..],
            &RESPAWN,
            &[100, 0, 100, 0, 1, 0, 0, 0, 3, 0, 0, 0],
            &KD_AND_FLAGS,
        ]
        .concat();
        let body = body(&statistics, &rules);
        let decoded = decode(TIME, &body, 2, mode::TEAM_DEATHMATCH, 0).unwrap();
        assert_eq!(
            decoded.standing(mode::TEAM_DEATHMATCH, &[0, 1]),
            Some(Standing {
                points: [100, 100],
                winner: rules::DRAW
            })
        );
    }

    #[test]
    fn empty_statistics_leave_the_rules() {
        let rules = tdm_rules();
        let body = body(&[], &rules);
        let decoded = decode(TIME, &body, 2, mode::TEAM_DEATHMATCH, 0).unwrap();
        assert_eq!(decoded.statistics, None);
        assert_eq!(decoded.rules.as_ref().unwrap().kills_deaths[0].kills, 3);
        assert_eq!(
            decoded
                .standing(mode::TEAM_DEATHMATCH, &[0, 1])
                .unwrap()
                .points,
            [97, 100]
        );
        // A battery match whose gather rule did not read says nothing of
        // its points without statistics.
        let decoded = decode(TIME, &body, 2, mode::BATTERIES, 10).unwrap();
        assert_eq!(decoded.standing(mode::BATTERIES, &[0, 1]), None);
    }

    #[test]
    fn with_no_statistics_a_battery_matchs_points_are_the_gather_rules() {
        let mut won = gather();
        won.team_points = [0, 2];
        won.containers[0].items.clear();
        won.containers[1].items = vec![0, 3];
        won.batteries[3] = Battery::InContainer(1);
        let rules = Rules {
            timelimit: Timelimit::read(&TIMELIMIT).unwrap(),
            respawn: Some(Respawn::default()),
            gather: Some(won),
            tdm: None,
            research: None,
            kills_deaths: vec![KillsDeaths::default(); 2],
            team_respawn: [true, true],
        }
        .to_bytes();
        let body = body(&[], &rules);
        let decoded = decode(TIME, &body, 2, mode::BATTERIES, 4).unwrap();
        assert_eq!(decoded.statistics, None);
        assert_eq!(
            decoded.standing(mode::BATTERIES, &[0, 1]),
            Some(Standing {
                points: [0, 2],
                winner: 1
            })
        );
    }

    #[test]
    fn a_body_whose_player_blocks_walk_is_split_from_the_front() {
        // A state whose bytes look like a chain of prefixes from the end
        // would mislead a search; the walk does not look at them.
        let animation = players::animation_bytes(&[true, true], &[1, 2, 3], &[0.5]);
        let mut blocks = players::block_bytes(&[0x04, 0x00, 0x02, 0x00, 0, 0], Some(&animation));
        blocks.extend(players::block_bytes(&[0xAB; 33], None));
        let statistics = tdm_statistics();
        let rules = tdm_rules();
        let sections = Sections {
            active_clients_mask: 0b11,
            players: &blocks,
            bullets: &[1, 2, 3],
            statistics: &statistics,
            objects: &[4; 300],
            physics: &[5; 7],
            rules: &rules,
            effects: &[],
        };
        let body = sections.to_body().unwrap();
        assert_eq!(Sections::walk(&body), Some(sections));
        assert_eq!(Sections::candidates(&body), [sections]);
        let walked = players::blocks(&body).unwrap();
        assert_eq!(walked.len(), 2);
        assert_eq!((walked[1].place, walked[1].state.len()), (1, 33));
        let decoded = decode(TIME, &body, 2, mode::TEAM_DEATHMATCH, 0).unwrap();
        assert_eq!(decoded.sections, sections);
        // The fake blocks of the other tests do not walk: found from the
        // end as before.
        let fake = super::tests::body(&statistics, &rules);
        assert_eq!(Sections::walk(&fake), None);
        assert_eq!(Sections::candidates(&fake).len(), 1);
    }

    #[test]
    fn statistics_that_read_are_kept_when_the_rules_do_not() {
        let statistics = tdm_statistics();
        let body = body(&statistics, &[0xEE; 40]);
        let decoded = decode(TIME, &body, 2, mode::TEAM_DEATHMATCH, 0).unwrap();
        assert_eq!(decoded.rules, None);
        assert_eq!(
            decoded.standing(mode::TEAM_DEATHMATCH, &[0, 1]),
            Some(Standing {
                points: [97, 100],
                winner: 1
            })
        );
    }

    #[test]
    fn garbage_is_not_a_snapshot() {
        assert_eq!(decode(50, &[], 2, mode::BATTERIES, 10), None);
        assert_eq!(decode(50, &[0; 3], 2, mode::BATTERIES, 10), None);
        assert_eq!(decode(50, &[0; 40], 2, mode::BATTERIES, 10), None);
        let body = body(&tdm_statistics(), &tdm_rules());
        assert_eq!(
            decode(TIME, &body[..body.len() - 1], 2, mode::TEAM_DEATHMATCH, 0),
            None
        );
        // Another roster size: neither the statistics nor the rules read.
        assert_eq!(decode(TIME, &body, 3, mode::TEAM_DEATHMATCH, 0), None);
    }
}
