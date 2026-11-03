//! The rules section (snapshot_rules.md section 2): the rules in the order
//! of the mode's config (timelimit, respawn, the mode's rule, kd stats),
//! then the blackboard's two flags. Each rule reads its own bytes; the
//! section has no inner lengths. The server's own reader of it, for the
//! match's phase and standing, is `snapshot::Rules`; this one only walks
//! it and looks at what a game trusts.

use super::{Context, Section, battery_records};
use crate::reader::{Reader, Short};
use crate::{MAX_PLAYERS, Part, Reason, Refusal, finite, mode, timelimit};

/// `timelimit_rule_core`'s bytes (0x317F30): start, current time, players
/// mask, state.
const TIMELIMIT: usize = 13;
/// A roster place in `kd_stats_rule` (0x323870): kills, deaths, online.
const KILLS_DEATHS: usize = 5;
/// `tdm_rule_core`'s bytes (0x316340): the tickets, two masks.
const TDM: usize = 12;
/// `items_mask` is a u16.
const MAX_BATTERIES: usize = 16;
/// Every map has a container for each team.
const CONTAINERS: usize = 2;

/// The rules read to their exact length: the timelimit of the world's
/// time, the respawn rule with the map's points, the mode's own rule, the
/// kd stats of every place, the two flags; every place and battery they
/// name within the match.
pub fn rules(section: &Section, context: &Context) -> Result<(), Refusal> {
    let refuse = |reason| Refusal::new(Part::Rules, section.at, reason);
    let places = context.places;
    let Some(read) = decode(section.bytes, context) else {
        return Err(refuse(Reason::RulesUnread {
            len: section.bytes.len(),
            time: context.time,
            places,
        }));
    };
    let Some(respawn) = read.respawn else {
        return Err(refuse(Reason::NoRespawnAndModeRule));
    };
    if let Some(map) = context.respawn_points
        && respawn.points != map
    {
        return Err(refuse(Reason::RespawnPoints {
            found: respawn.points,
            map,
        }));
    }
    if let Some(place) = past(respawn.times_mask, places) {
        return Err(refuse(Reason::RespawnTimePast { place, places }));
    }
    match read.mode_rule {
        None if context.game_mode == mode::TEAM_DEATHMATCH => Err(refuse(Reason::NoTdmRule)),
        None if context.game_mode == mode::RESEARCH => Err(refuse(Reason::NoResearchRule)),
        None => Err(refuse(Reason::NoGatherRule)),
        Some(ModeRule::Tdm(tdm)) => {
            if past(tdm.alive_players_mask, places).is_some()
                || past(tdm.active_players_mask, places).is_some()
            {
                return Err(refuse(Reason::TdmMaskPast { places }));
            }
            Ok(())
        }
        Some(ModeRule::Research(research)) => match past(research.places_mask, places) {
            Some(_) => Err(refuse(Reason::ResearchMaskPast { places })),
            None => Ok(()),
        },
        Some(ModeRule::Gather(gather)) => match gather_fault(&gather, context) {
            Some(reason) => Err(refuse(reason)),
            None => Ok(()),
        },
    }
}

/// The lowest place `mask` names at `places` or past.
fn past(mask: u32, places: usize) -> Option<usize> {
    (places..32).find(|place| mask.checked_shr(*place as u32).unwrap_or(0) & 1 != 0)
}

/// What in a gather rule names a battery or a place the match does not
/// have, or holds a float that is no number.
fn gather_fault(gather: &Gather, context: &Context) -> Option<Reason> {
    let places = context.places;
    let batteries = context.batteries;
    let expected = match gather.available {
        Some(available) => (available as usize).min(batteries),
        None => batteries,
    };
    if gather.count != expected {
        return Some(Reason::Batteries {
            found: gather.count,
            map: expected,
        });
    }
    let batteries_read = gather.batteries.get(..gather.count).unwrap_or_default();
    if let Some(battery) = batteries_read
        .iter()
        .position(|battery| *battery == Battery::Free { finite: false })
    {
        return Some(Reason::BatteryNotNumber { battery });
    }
    for (container, held) in gather.containers.iter().enumerate() {
        if let Some(&user) = held.users.iter().find(|user| usize::from(**user) >= places) {
            return Some(Reason::ContainerUserPast {
                container,
                user,
                places,
            });
        }
        if let Some(&battery) = held
            .items
            .iter()
            .find(|item| usize::from(**item) >= batteries)
        {
            return Some(Reason::ContainerBatteryPast {
                container,
                battery,
                batteries,
            });
        }
    }
    None
}

/// What the rules section gives that a check looks at.
struct Rules<'a> {
    /// `None`: the bytes between the timelimit and the kd stats did not
    /// read as a respawn rule and the mode's rule.
    respawn: Option<Respawn>,
    /// The mode's rule, when it reads (Team Deathmatch's and Research's
    /// from the kd stats back, even without a respawn rule before them).
    mode_rule: Option<ModeRule<'a>>,
}

/// The mode's own rule, between the respawn rule and the kd stats.
enum ModeRule<'a> {
    /// Modes 2 and 3.
    Gather(Gather<'a>),
    /// Mode 4: the 12 bytes before the kd stats.
    Tdm(Tdm),
    /// Mode 5: 21 bytes and 96 a unit before the kd stats.
    Research(Research),
}

/// Research's rule (exe patch 53, `rr_write` / `rr_read`): two points, two
/// shown scores, the end, the entered places, the unit count, then each
/// unit: progress, taking, owner, the two teams' counts and place masks,
/// a mark for each of the 20 places.
struct Research {
    /// Every place the rule names: the entered and each unit's inside.
    places_mask: u32,
}

impl Research {
    const HEAD: usize = 21;
    const UNIT: usize = 16 + 4 * MAX_PLAYERS;
    /// `RR_MAX_ZONES`.
    const MAX_UNITS: usize = 3;
    /// `RR_FULL`: a unit's whole capture.
    const FULL: u32 = 1_000_000_000;
    /// `RR_NOBODY`: neither team.
    const NOBODY: u8 = 2;

    /// The rule that ends `bytes`: the length its unit count gives, the most
    /// units first. Where it starts, and the rule.
    fn read_back(bytes: &[u8]) -> Option<(usize, Self)> {
        (1..=Self::MAX_UNITS).rev().find_map(|units| {
            let len = Self::UNIT.checked_mul(units)?.checked_add(Self::HEAD)?;
            let at = bytes.len().checked_sub(len)?;
            Self::read(bytes.get(at..)?, units).map(|rule| (at, rule))
        })
    }

    /// A rule of `units` units that fills `bytes`, as `rr_read` takes it.
    fn read(bytes: &[u8], units: usize) -> Option<Self> {
        let mut reader = Reader::new(bytes, 0);
        reader.skip(16).ok()?;
        let entered = reader.u32().ok()?;
        if usize::from(reader.u8().ok()?) != units {
            return None;
        }
        let mut places_mask = entered;
        for _ in 0..units {
            let progress = reader.u32().ok()?;
            let [taking, owner, first, second] = reader.array::<4>().ok()?;
            let inside = [reader.u32().ok()?, reader.u32().ok()?];
            reader.skip(4 * MAX_PLAYERS).ok()?;
            if progress > Self::FULL
                || taking > Self::NOBODY
                || owner > Self::NOBODY
                || u32::from(first) != inside[0].count_ones()
                || u32::from(second) != inside[1].count_ones()
            {
                return None;
            }
            places_mask |= inside[0] | inside[1];
        }
        (reader.is_done() && places_mask.checked_shr(MAX_PLAYERS as u32).unwrap_or(0) == 0)
            .then_some(Self { places_mask })
    }
}

/// `player_respawn_rule_core` (0x316F10).
struct Respawn {
    /// The places that wait for a respawn.
    times_mask: u32,
    /// The respawn points: two bytes each.
    points: usize,
}

struct Tdm {
    alive_players_mask: u32,
    active_players_mask: u32,
}

/// A battery as the gather rule has it (`victory_item_core::serialize`
/// 0x301470, reader 0x301660).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Battery {
    /// A player has it: the rule writes nothing of it.
    Carried,
    /// In the container with this id.
    InContainer(u8),
    /// Lying in the world; whether its position is numbers.
    Free { finite: bool },
}

/// A team's container (`victory_items_container_core::serialize`
/// 0x31CE40, reader 0x31CEE0).
#[derive(Clone, Copy, Debug, Default)]
struct Container<'a> {
    /// The roster places using it.
    users: &'a [u8],
    /// The batteries in it, by their number.
    items: &'a [u8],
}

/// The gather rule of modes 2 (`gather_victory_items_rule_core` 0x313CA0)
/// and 3 (0x3149B0).
struct Gather<'a> {
    /// Mode 3: `m_available_victory_items_count`.
    available: Option<u32>,
    /// The batteries the rule holds; `batteries[..count]` are they.
    count: usize,
    batteries: [Battery; MAX_BATTERIES],
    containers: [Container<'a>; CONTAINERS],
}

/// A bool as the game's writer puts it (0x00 or 0xFF), or 0x01, which the
/// server's tests wrote until 2026-10-12 (`wire_bool` in the server). The
/// game takes any byte but 0 as true; this is stricter, because where the
/// section ends is found by what reads.
fn wire_bool(byte: u8) -> bool {
    matches!(byte, 0x00 | 0x01 | 0xFF)
}

/// Reads the rules of a world (`snapshot::Rules::decode` in the server).
/// `None`: the section does not start with a timelimit rule of the
/// world's time (a state it can have, no player outside the roster), or
/// does not end in kd stats and two flags.
///
/// The respawn rule has two bytes for each respawn point of the map, and
/// nothing says how many: in mode 4 the rest is the tdm rule's 12 bytes;
/// in mode 5 Research's 21 + 96 a unit (its unit count is its byte 20); in
/// modes 2 and 3 it is the number with which the gather rule after it
/// reads to the kd stats exactly, the smallest if several do.
fn decode<'a>(bytes: &'a [u8], context: &Context) -> Option<Rules<'a>> {
    let places = context.places;
    if places > MAX_PLAYERS {
        return None;
    }
    // The timelimit, the respawn rule's mask and seed, the kd stats, the
    // two flags.
    let kd_len = KILLS_DEATHS.checked_mul(places)?;
    let flags_at = bytes.len().checked_sub(2)?;
    let kd_at = flags_at.checked_sub(kd_len)?;
    if kd_at < TIMELIMIT + 8 {
        return None;
    }
    let flags = bytes.get(flags_at..)?;
    let kills_deaths = bytes.get(kd_at..flags_at)?;
    if !flags.iter().copied().all(wire_bool)
        || !kills_deaths
            .chunks_exact(KILLS_DEATHS)
            .all(|record| record.get(4).copied().is_some_and(wire_bool))
    {
        return None;
    }
    let mut timelimit = Reader::new(bytes.get(..TIMELIMIT)?, 0);
    let read = (|| {
        let _start = timelimit.u32()?;
        Ok::<_, Short>((timelimit.u32()?, timelimit.u32()?, timelimit.u8()?))
    })();
    let (current, players_mask, state) = read.ok()?;
    if current != context.time
        || state > timelimit::IN_PROCESS
        || u64::from(players_mask)
            .checked_shr(places as u32)
            .unwrap_or(0)
            != 0
    {
        return None;
    }
    let middle = bytes.get(TIMELIMIT..kd_at)?;
    let head = respawn_head(middle);
    // The respawn rule's points end where the mode's rule starts.
    let respawn_to = |end: usize| {
        let (times_mask, head_len) = head?;
        (end >= head_len && end.saturating_sub(head_len).is_multiple_of(2)).then_some(Respawn {
            times_mask,
            points: end.saturating_sub(head_len) / 2,
        })
    };
    // Team Deathmatch's and Research's rules say their own length: read
    // from the kd stats back. The gather rules do not: the respawn points
    // are the number with which the gather rule after them reads to the
    // kd stats exactly, the smallest if several do.
    let (respawn, mode_rule) = match context.game_mode {
        mode::TEAM_DEATHMATCH => match middle.len().checked_sub(TDM) {
            Some(at) => (
                respawn_to(at),
                read_tdm(middle.get(at..)?).map(ModeRule::Tdm),
            ),
            None => (None, None),
        },
        mode::RESEARCH => match Research::read_back(middle) {
            Some((at, research)) => (respawn_to(at), Some(ModeRule::Research(research))),
            None => (None, None),
        },
        _ => {
            let batteries = battery_records(context);
            // With the map's respawn points known, the gather rule starts
            // after them: one read, not one at every even offset (which is
            // quadratic in the section's length).
            let found = head.and_then(|(_, head_len)| {
                let read = |at: usize| {
                    read_gather(middle.get(at..)?, context.game_mode, batteries)
                        .map(|gather| (at, gather))
                };
                match context.respawn_points {
                    Some(points) => read(points.checked_mul(2)?.checked_add(head_len)?),
                    None => (head_len..=middle.len()).step_by(2).find_map(read),
                }
            });
            match found {
                Some((at, gather)) => (respawn_to(at), Some(ModeRule::Gather(gather))),
                None => (None, None),
            }
        }
    };
    Some(Rules { respawn, mode_rule })
}

/// Team Deathmatch's 12 bytes: the tickets and two place masks.
fn read_tdm(bytes: &[u8]) -> Option<Tdm> {
    let mut reader = Reader::new(bytes, 0);
    let _tickets = reader.u32().ok()?;
    Some(Tdm {
        alive_players_mask: reader.u32().ok()?,
        active_players_mask: reader.u32().ok()?,
    })
}

/// The respawn rule's mask, times and seed: what comes before its points.
/// The mask and how many bytes they take.
fn respawn_head(bytes: &[u8]) -> Option<(u32, usize)> {
    let mut reader = Reader::new(bytes, 0);
    let mask = reader.u32().ok()?;
    if mask.checked_shr(MAX_PLAYERS as u32).unwrap_or(0) != 0 {
        return None;
    }
    reader
        .skip((mask.count_ones() as usize).saturating_mul(4))
        .ok()?;
    let _seed = reader.u32().ok()?;
    Some((mask, reader.at()))
}

/// Reads a gather rule that fills `bytes` exactly. `batteries` is how many
/// the mask covers at most (the mode's count).
fn read_gather(bytes: &[u8], game_mode: u8, batteries: usize) -> Option<Gather<'_>> {
    let mut reader = Reader::new(bytes, 0);
    let _seed = reader.u32().ok()?;
    let team_points = reader.array::<2>().ok()?;
    let available = if game_mode == mode::TIMED_BATTERIES {
        Some(reader.u32().ok()?)
    } else {
        None
    };
    let count = match available {
        Some(available) => (available as usize).min(batteries),
        None => batteries,
    };
    let mask = reader.u16().ok()?;
    if count > MAX_BATTERIES || u32::from(mask).checked_shr(count as u32).unwrap_or(0) != 0 {
        return None;
    }
    let mut read = [Battery::Carried; MAX_BATTERIES];
    for (number, battery) in read.iter_mut().take(count).enumerate() {
        if mask.checked_shr(number as u32).unwrap_or(0) & 1 != 0 {
            *battery = read_battery(&mut reader)?;
        }
    }
    let mut containers = [Container::default(); CONTAINERS];
    for container in &mut containers {
        *container = read_container(&mut reader)?;
    }
    if available.is_some() {
        // The match start, the next spawn, the spawn time and its
        // variation, `m_match_started`.
        reader.skip(4 * 4 + 1).ok()?;
    }
    // The teams have no more points than the containers hold, and the
    // batteries in a container say so of themselves. The containers can
    // hold one more than the points: live mode 3 worlds of five matches
    // that ended on time at 9 to 0 (2026-10-22) have all 10 batteries in
    // the winner's container and points of 9, in every game's world (two
    // vouched for them). Why the tenth is in and not counted was not traced
    // [LIKELY the rule's completion, match.md section 26: once `+0x15C` is
    // set the handler counts no put]. The points are not used to index
    // anything; the batteries and containers are, and they are still
    // checked one by one.
    let held = containers.iter().fold(0usize, |held, container| {
        held.saturating_add(container.items.len())
    });
    let [first, second] = team_points;
    let read_batteries = read.get(..count)?;
    let consistent = usize::from(first).saturating_add(usize::from(second)) <= held
        && containers.iter().enumerate().all(|(id, container)| {
            container.items.iter().all(|item| {
                read_batteries.get(usize::from(*item)) == Some(&Battery::InContainer(id as u8))
            })
        });
    (reader.is_done() && consistent).then_some(Gather {
        available,
        count,
        batteries: read,
        containers,
    })
}

/// A battery the rule's mask names: not carried, so its carrier byte is
/// 0xFF; in a container, or lying with its transforms and users.
fn read_battery(reader: &mut Reader) -> Option<Battery> {
    /// A free battery's bytes after its first six: two 4x4 transforms, two
    /// velocities, `deactivation_time`, `activation_state`.
    const FREE_FIXED: usize = 64 + 64 + 12 + 12 + 4 + 4;
    /// The translation of `m_transform` [LIKELY: its floats 12..15].
    const POSITION: usize = 64 + 48;
    let _spotted_mask = reader.u32().ok()?;
    // No carrier, or the rule would not have written it (0x313D84).
    if reader.u8().ok()? != 0xFF {
        return None;
    }
    let container = reader.u8().ok()?;
    if container != 0xFF {
        return (usize::from(container) < CONTAINERS).then_some(Battery::InContainer(container));
    }
    let fixed = reader.take(FREE_FIXED).ok()?;
    let mut position = Reader::new(fixed.get(POSITION..)?, 0);
    let mut finite_position = true;
    for _ in 0..3 {
        finite_position &= finite(position.f32_bits().ok()?);
    }
    let users = usize::from(reader.u8().ok()?);
    if users > MAX_PLAYERS
        || reader
            .take(users)
            .ok()?
            .iter()
            .any(|id| usize::from(*id) >= MAX_PLAYERS)
    {
        return None;
    }
    Some(Battery::Free {
        finite: finite_position,
    })
}

fn read_container<'a>(reader: &mut Reader<'a>) -> Option<Container<'a>> {
    let users = usize::from(reader.u8().ok()?);
    let users = reader.take(users).ok()?;
    let items = reader.u32().ok()? as usize;
    let items = reader.take(items).ok()?;
    (users.len() <= MAX_PLAYERS
        && users.iter().all(|id| usize::from(*id) < MAX_PLAYERS)
        && items.len() <= MAX_BATTERIES
        && items.iter().all(|id| usize::from(*id) < MAX_BATTERIES))
    .then_some(Container { users, items })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec::Vec;

    /// A timelimit at 539 650 ms with places 0 and 1 in the match.
    const TIMELIMIT: [u8; 13] = [
        0x08, 0x52, 0x00, 0x00, 0x02, 0x3C, 0x08, 0x00, 0x03, 0x00, 0x00, 0x00, 0x03,
    ];
    const TIME: u32 = 539_650;
    /// No one waiting, a seed, three respawn points.
    const RESPAWN: [u8; 14] = [0, 0, 0, 0, 0x44, 0x33, 0x22, 0x11, 0, 0, 0, 0, 0, 0];
    /// Kills, deaths, online for two places, then the two flags.
    const KD_AND_FLAGS: [u8; 12] = [3, 0, 0, 0, 0xFF, 0, 0, 3, 0, 0xFF, 0xFF, 0xFF];

    fn context(game_mode: u8, respawn_points: Option<usize>) -> Context<'static> {
        Context {
            time: TIME,
            places: 2,
            game_mode,
            batteries: 0,
            respawn_points,
            profiles: None,
            artefacts: None,
            objects: None,
            inventory: None,
        }
    }

    /// Research's rule as `rr_write` puts it: team 0 owns unit A, team 1
    /// takes unit B with the places of `inside_b`, unit C is nobody's.
    fn research(units: usize, inside_b: u32) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend(64_400_000u32.to_le_bytes());
        out.extend(37_500_000u32.to_le_bytes());
        out.extend([64, 0, 38, 0]);
        out.extend(u32::MAX.to_le_bytes());
        out.extend(0b11u32.to_le_bytes());
        out.push(units as u8);
        for unit in 0..units {
            let (progress, taking, owner, inside) = match unit {
                0 => (Research::FULL, 0, 0, [0, 0]),
                1 => (500_000_000, 1, 2, [0, inside_b]),
                _ => (0, 2, 2, [0, 0]),
            };
            out.extend(progress.to_le_bytes());
            out.extend([
                taking,
                owner,
                inside[0].count_ones() as u8,
                inside[1].count_ones() as u8,
            ]);
            out.extend(inside[0].to_le_bytes());
            out.extend(inside[1].to_le_bytes());
            out.extend([0; 4 * MAX_PLAYERS]);
        }
        out
    }

    fn check(rule: &[u8], game_mode: u8, respawn_points: Option<usize>) -> Result<(), Refusal> {
        let bytes = [&TIMELIMIT[..], &RESPAWN, rule, &KD_AND_FLAGS].concat();
        rules(
            &Section {
                at: 0,
                bytes: &bytes,
            },
            &context(game_mode, respawn_points),
        )
    }

    fn reason(result: Result<(), Refusal>) -> Option<Reason> {
        result.err().map(|refusal| refusal.reason)
    }

    #[test]
    fn a_research_rule_reads_with_its_units_and_the_respawn_points() {
        for units in 1..=3 {
            let rule = research(units, 0b10);
            assert_eq!(rule.len(), 21 + 96 * units);
            assert_eq!(check(&rule, mode::RESEARCH, Some(3)), Ok(()), "{units}");
        }
        assert_eq!(
            reason(check(&research(3, 0b10), mode::RESEARCH, Some(4))),
            Some(Reason::RespawnPoints { found: 3, map: 4 })
        );
    }

    #[test]
    fn a_wrong_research_rule_is_refused() {
        // A place past the two of the match inside a unit.
        assert_eq!(
            reason(check(&research(3, 0b100), mode::RESEARCH, None)),
            Some(Reason::ResearchMaskPast { places: 2 })
        );
        // A place past the 20: no rule `rr_read` takes.
        let past = research(3, 1 << 20);
        assert_eq!(
            reason(check(&past, mode::RESEARCH, None)),
            Some(Reason::NoRespawnAndModeRule)
        );
        // A count that is not the places inside, an owner 3, a progress over
        // a whole capture.
        for (at, value) in [(21 + 96 + 7, 2u8), (21 + 5, 3), (21 + 3, 0x40)] {
            let mut wrong = research(3, 0b10);
            wrong[at] = value;
            assert_eq!(
                reason(check(&wrong, mode::RESEARCH, None)),
                Some(Reason::NoRespawnAndModeRule),
                "{at}"
            );
        }
        // A byte short of its unit count.
        let mut short = research(3, 0b10);
        short.pop();
        assert!(check(&short, mode::RESEARCH, None).is_err());
    }

    #[test]
    fn a_research_rule_is_no_other_modes_rule() {
        let rule = research(3, 0b10);
        assert_eq!(
            reason(check(&rule, mode::BATTERIES, None)),
            Some(Reason::NoRespawnAndModeRule)
        );
        let tdm = [97, 0, 100, 0, 3, 0, 0, 0, 3, 0, 0, 0];
        assert_eq!(check(&tdm, mode::TEAM_DEATHMATCH, Some(3)), Ok(()));
        assert!(check(&tdm, mode::RESEARCH, None).is_err());
    }
}
