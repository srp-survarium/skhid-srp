//! A free-for-all's sides in the respawn rule and the resupply zones (patch
//! 142; docs/notes/ffa.md). In a free-for-all srp gives each player a team
//! of his own (4 + his place); the game's two-team code then needs three
//! changes, each at a site of sites_026g.tsv:
//!
//! - the respawn tick (`respawn.due_team`) puts each due player into his
//!   team's vector, and it has two: a player of his own side goes into the
//!   first (no player of team 0 is in such a match);
//! - the tick's selection (`respawn.select`) of that vector: [`RespawnSelect`],
//!   the team-2 group of points and the game's own clear test, one point at a
//!   time;
//! - a resupply zone serves its team (a zone of team 2 everyone): a player
//!   of his own side is served by none (`resupply.can_use`,
//!   `resupply.use_info`: [`Resupply`]), whatever team the map gave the zone.
//!
//! Every field is fields_026g.tsv's, every function engine_026g.tsv's.

use super::context::{Context, Reg};
use super::hook::Thread;
use super::world::{floats, read};
use crate::raw;

/// A player's side, from his profile's team: one of a match's two teams, or
/// a side of his own (any other value: srp's free-for-all teams).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Team(TwoTeams),
    Own(u32),
}

/// One of the two teams the game's team code knows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TwoTeams {
    Zero,
    One,
}

impl Side {
    pub fn of(team: u32) -> Side {
        match team {
            0 => Side::Team(TwoTeams::Zero),
            1 => Side::Team(TwoTeams::One),
            own => Side::Own(own),
        }
    }

    /// The team value the game keeps.
    fn team(self) -> u32 {
        match self {
            Side::Team(TwoTeams::Zero) => 0,
            Side::Team(TwoTeams::One) => 1,
            Side::Own(team) => team,
        }
    }
}

/// The side of a live player.
///
/// # Safety
/// `player` is alive on the hook's thread.
unsafe fn side_of(player: raw::Player) -> Option<Side> {
    // SAFETY: the caller's: a live player and its match profile.
    unsafe { Some(Side::of(read(player.profile()?.team()))) }
}

/// At `respawn.due_team`: the due player's side (EAX, his team).
pub fn due_side(ctx: &Context<'_>) -> Side {
    // SAFETY: the context is this site's: EAX the team just read.
    Side::of(unsafe { raw::site::respawn_due_team::team(ctx) })
}

/// At `respawn.due_team`: the player goes into the first of the tick's two
/// vectors (EAX 0, which the replayed `lea` multiplies into its offset).
pub fn into_first_vector(ctx: &mut Context<'_>) {
    ctx.set_reg(Reg::Eax, 0);
}

/// A player of the due vector: his place and side.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Due {
    pub place: u8,
    pub side: Side,
}

/// A point of the team-2 group: where it is and how far the other teams'
/// players must be.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub position: [f32; 3],
    pub enemy_radius: f32,
}

/// An index of the team-2 group.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PointIndex(usize);

impl PointIndex {
    pub fn get(self) -> usize {
        self.0
    }
}

/// select_respawn_points at `respawn.select`: one team vector's due players,
/// the living ones, the rule's team-2 group, the vector of chosen points.
pub struct RespawnSelect<'t> {
    rule: raw::PlayerRespawnRuleBase,
    frame: raw::RespawnSelectFrame,
    _thread: &'t Thread,
}

impl<'t> RespawnSelect<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<RespawnSelect<'t>> {
        use raw::site::respawn_select as site;
        // SAFETY: the context is this site's: ECX the rule, EBP the frame of
        // its select_respawn_points, alive while it runs on the game's thread.
        let (rule, frame) = unsafe { (site::rule(ctx)?, site::frame(ctx)?) };
        Some(RespawnSelect { rule, frame, _thread: ctx.thread() })
    }

    /// The tick's time, ms.
    pub fn time(&self) -> u32 {
        // SAFETY: the frame's argument, alive for the hook.
        unsafe { read(self.frame.time()) }
    }

    /// The due players, in the vector's order (the tick spawns the first as
    /// many as points are chosen, in that order).
    pub fn due(&self) -> impl Iterator<Item = Due> + '_ {
        // SAFETY: the frame's due vector, its begin..end players alive for
        // the hook (the tick holds a reference on each).
        let (first, count) = unsafe {
            match self.frame.due() {
                Some(due) => match (due.begin(), due.end()) {
                    (Some(first), Some(end)) => (Some(first), end.index_from(first).unwrap_or(0)),
                    _ => (None, 0),
                },
                None => (None, 0),
            }
        };
        first.into_iter().flat_map(move |first| {
            (0..count).filter_map(move |i| {
                // SAFETY: i is under the vector's count; the player is alive.
                unsafe {
                    let player = first.at(i).player()?;
                    Some(Due { place: read(player.place()), side: side_of(player)? })
                }
            })
        })
    }

    /// Where each living player stands, in the vector's order: the
    /// translation of his simulated transform (+43Ch), which the game's own
    /// clear test measures from (0x2F483A: the transform his collision
    /// object gives, 0x1F17C0, its +30h).
    pub fn living(&self) -> impl Iterator<Item = [f32; 3]> + '_ {
        // SAFETY: the frame's living vector, its begin..end players alive for
        // the hook (the tick holds a reference on each).
        let (first, count) = unsafe {
            match self.frame.living() {
                Some(living) => match (living.begin(), living.end()) {
                    (Some(first), Some(end)) => (Some(first), end.index_from(first).unwrap_or(0)),
                    _ => (None, 0),
                },
                None => (None, 0),
            }
        };
        first.into_iter().flat_map(move |first| {
            (0..count).filter_map(move |i| {
                // SAFETY: i is under the vector's count; the player is alive.
                unsafe { Some(floats::<3>(first.at(i).player()?.feet())) }
            })
        })
    }

    /// The team-2 group's size.
    pub fn point_count(&self) -> usize {
        // SAFETY: the rule's count, alive for the hook.
        let count = unsafe { read(self.rule.team_2_count()) } as usize;
        // SAFETY: as above, its array.
        if unsafe { self.rule.team_2_points() }.is_some() { count } else { 0 }
    }

    fn native(&self, index: PointIndex) -> Option<raw::RespawnPointCore> {
        (index.0 < self.point_count()).then_some(())?;
        // SAFETY: the rule's inline array of point_count points; the index is under it.
        unsafe { Some(self.rule.team_2_points()?.at(index.0)) }
    }

    /// The `index`th point of the team-2 group (None past the group).
    pub fn point(&self, index: usize) -> Option<(PointIndex, Point)> {
        let at = PointIndex(index);
        let point = self.native(at)?;
        // SAFETY: a point of the rule's group, alive for the hook.
        let (position, enemy_radius) = unsafe { (floats::<3>(point.position()), read(point.enemy_radius())) };
        Some((at, Point { position, enemy_radius }))
    }

    /// The game's own test (0x2F4800): no living player of `side` nearer than
    /// the point's ally radius and none of another side nearer than its enemy
    /// radius.
    pub fn is_clear(&self, index: PointIndex, side: Side) -> bool {
        let Some(point) = self.native(index) else { return false };
        // SAFETY: a point of the group and the frame's living players, alive
        // for the hook; the function only reads them.
        unsafe { raw::engine_respawn_point_clear(Some(point), side.team(), self.frame.living()) != 0 }
    }

    /// The next due player's point: pushed onto the chosen vector, which
    /// holds as many as the due vector (both the tick's, room for 20).
    pub fn choose(&mut self, index: PointIndex) {
        let Some(point) = self.native(index) else { return };
        let mut chosen = Some(point);
        // SAFETY: the frame's chosen vector, alive for the hook; push_back
        // copies the pointer from our local.
        unsafe { raw::engine_respawn_points_push(self.frame.chosen(), &mut chosen) }
    }
}

/// The two resupply sites: the use test and the use hint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResupplySite {
    CanUse,
    UseInfo,
}

/// What a resupply zone is to the player who asks it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resupply {
    /// One of a match's two teams: the game's own test of the zone's team
    /// decides, as it always did.
    ZonesTeamTest,
    /// A side of his own (a free-for-all): no zone serves him, whatever its
    /// team (the game's test alone would serve him at a zone of team 2, and
    /// at one whose team is his side's number).
    Refused,
}

impl Resupply {
    /// The one rule: a free-for-all has no ammunition refill.
    pub fn of(side: Side) -> Resupply {
        match side {
            Side::Team(_) => Resupply::ZonesTeamTest,
            Side::Own(_) => Resupply::Refused,
        }
    }
}

/// At `resupply.can_use` or `resupply.use_info` (`site`), before the zone's
/// test of the team: what the zone is to the player asking (EAX). His side is
/// his profile's team, the match options' own: the same for every game of the
/// match, a late joiner's and a replay's. A player without a profile is the
/// game's to refuse.
pub fn resupply_at(ctx: &Context<'_>, site: ResupplySite) -> Resupply {
    // SAFETY: the context is `site`'s: EAX the player using the zone, alive
    // on the game's thread.
    let side = unsafe {
        let player = match site {
            ResupplySite::CanUse => raw::site::resupply_can_use::player(ctx),
            ResupplySite::UseInfo => raw::site::resupply_use_info::player(ctx),
        };
        player.and_then(|player| side_of(player))
    };
    side.map_or(Resupply::ZonesTeamTest, Resupply::of)
}

#[cfg(test)]
mod resupply_tests {
    use super::*;

    #[test]
    fn a_free_for_all_side_is_refused_and_a_team_is_the_games() {
        assert_eq!(Resupply::of(Side::of(0)), Resupply::ZonesTeamTest);
        assert_eq!(Resupply::of(Side::of(1)), Resupply::ZonesTeamTest);
        // Every side of a free-for-all (4 + the place), and the two values
        // the game reads as "neutral" and "none".
        for team in (2..=23).chain([u32::MAX]) {
            assert_eq!(Resupply::of(Side::of(team)), Resupply::Refused, "{team}");
        }
    }
}

/// A non-negative, finite duration from the signed integer config format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ProtectionMillis(u32);

impl ProtectionMillis {
    fn parse(value: super::config::ConfigValue<'_>) -> Option<Self> {
        let value = value.integer().ok()?;
        (value <= i32::MAX as u32).then_some(Self(value))
    }
}

pub enum ProtectionLoad { Legacy, Overridden }

/// Apply a point's optional millisecond setting instead of its legacy seconds load.
/// Missing or malformed optional data leaves the native duration intact.
pub fn apply_protection_override(ctx: &Context<'_>) -> ProtectionLoad {
    use super::config::ConfigValue;
    use raw::site::respawn_protection_read as site;
    // SAFETY: at this site's load, EBX owns a live relocated config and EBP
    // owns the point being initialized. Neither escapes this synchronous hook.
    let Some((point, config)) = (unsafe { site::point(ctx).zip(site::config(ctx)) }) else { return ProtectionLoad::Legacy };
    // SAFETY: the loaded resource remains alive throughout this hook.
    let Ok(table) = (unsafe { ConfigValue::read(config) }).and_then(ConfigValue::table) else { return ProtectionLoad::Legacy };
    let Ok(Some(value)) = table.get(c"invulnerability_time_ms") else { return ProtectionLoad::Legacy };
    let Some(duration) = ProtectionMillis::parse(value) else { return ProtectionLoad::Legacy };
    // SAFETY: this hook is the point's initialization owner; it replaces the
    // duration during the load before the point can be used by a respawn rule.
    unsafe { point.protection_ms().write(duration.0) };
    ProtectionLoad::Overridden
}

#[cfg(test)]
mod protection_tests {
    use super::*;
    use super::super::config::ConfigValue;
    #[test]
    fn millisecond_precision_and_zero_are_preserved() {
        assert_eq!(ProtectionMillis::parse(ConfigValue::Integer(500)), Some(ProtectionMillis(500)));
        assert_eq!(ProtectionMillis::parse(ConfigValue::Integer(0)), Some(ProtectionMillis(0)));
        assert_eq!(ProtectionMillis::parse(ConfigValue::Integer(2000)), Some(ProtectionMillis(2000)));
    }
    #[test]
    fn negative_or_wrong_type_cannot_become_a_duration() {
        assert_eq!(ProtectionMillis::parse(ConfigValue::Integer(u32::MAX)), None);
        assert_eq!(ProtectionMillis::parse(ConfigValue::Number(0.5)), None);
        assert_eq!(ProtectionMillis::parse(ConfigValue::Unsupported), None);
    }
}
