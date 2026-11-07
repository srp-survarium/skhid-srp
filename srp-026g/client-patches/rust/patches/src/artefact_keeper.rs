//! Keeper of the Artifacts (patch 109, docs/notes/artefact-skills.md "Keeper of the
//! Artifacts"): a player with the skill gets an artefact of the match's kind, whatever the map's
//! anomalies hold, once a match.
//!
//! - The kind: srp draws it once a match and writes its draw into every kit (modifier 40 bits
//!   9..11; artefact_carry::keeper, the table srp draws from).
//! - The artefact: the place's own of the map's hidden Keeper pool (content edit
//!   artefact-keeper-1.json: an anomaly with no container, 20 of each kind, one a place: place p
//!   the p-th of the kind by id), moved into the first empty artefact container by the game's own
//!   take_artefact. The pool's anomaly cooks, ticks and saves it as any anomaly does its own, so
//!   late join, resync and replay load it natively.
//! - When (the owner, 2026-11-07), decided at each tick of the player (base_player::tick before
//!   tick_animations, after the glow's): due when the match itself begins (the time rule in
//!   process, core +18FE4h 3: the countdown's end, while everyone loads; no extra delay), and for
//!   a late joiner (srp marks its kits, modifier 40 bit 13) only once its place has a death
//!   counted. Due and alive with an empty container: taken at that tick. Dead, or every container
//!   held: pending, taken at the first later tick with room (a container freed in the same life,
//!   or a respawn with room).
//! - Once a match, with nothing kept outside the world: the mark is the pool artefact's activation
//!   time (artefact +2A0h): the constructor's u32::MAX until a give writes 0 (an activation, only
//!   ever after a give, writes its time). Nothing else writes it, it is read only in state 4, and
//!   every world saves it in every state: a place's artefact whose time is not u32::MAX was given,
//!   in every game, late joiner, resync and replay alike. (Not its count: an artefact is not a
//!   stack, so the count's bytes are its condition, a float a death's degrade does maths on.)
//! - Once given it is an artefact as any found one (the owner): a death keeps it in that kit's
//!   container (state 5 while its holder is dead: passive off), the respawn in that kit switches
//!   its passive on again (setup_from_profile's on_taken); a respawn in another kit leaves it in
//!   the first kit's container (each kit its own object). It goes dormant only when a take with
//!   every container held replaces it (take_artefact 0x2FC897); it stays given then.
//! - Every input is simulated state: the static info's kits, the time state, the statistics, the
//!   pool's states and counts, and the inventory, read at the player's simulation tick in every
//!   game, re-simulation and replay.
#![forbid(unsafe_code)]

use artefact_carry::ArtefactId;
use artefact_carry::keeper::Kind;

use crate::artefact_skills::Skills;
use crate::vostok::keeper::Recipient;
use crate::vostok::log::{Piece::{Dec, Text}, line};
use crate::vostok::{Context, Flow, NoExit};

/// Whether a pool artefact was given this match (its activation time, the mark).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum History {
    /// Never given: dormant in the pool, its activation time the constructor's u32::MAX.
    Unused,
    /// Given: in a container (its holder alive or dead), on the ground, or dormant again after
    /// a take with every container held replaced it.
    Given,
}

/// One artefact of the map's Keeper pool.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Registered {
    pub id: ArtefactId,
    pub kind: Kind,
    pub history: History,
}

/// Where the match is for a place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Standing {
    /// Before the start (waiting, the countdown).
    NotStarted,
    /// In process: the deaths and suicides the statistics count for the place.
    Started { deaths: u32 },
}

/// The player's artefact containers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Containers {
    /// No container, or dead.
    None,
    /// Every container holds one.
    Full,
    /// One at least is empty.
    Free,
}

/// What a tick does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gift {
    /// No skill, no draw of a kind, not due yet, already given, or the pool has none for the
    /// place (a map without it).
    Nothing,
    /// Due and not given, but the player is dead or has no empty container: the next ticks again.
    Pending,
    Take(ArtefactId, Kind),
}

/// The decision at one tick of `place`'s player, from its kit's skills, its standing, its containers
/// and the map's Keeper pool (in id order).
pub fn gift(skills: Skills, place: u8, standing: Standing, containers: Containers, pool: impl Iterator<Item = Registered>) -> Gift {
    let Standing::Started { deaths } = standing else { return Gift::Nothing };
    if !skills.keeper() || (skills.late() && deaths == 0) {
        return Gift::Nothing;
    }
    let Some(kind) = skills.draw().and_then(|draw| Kind::of_draw(draw.get())) else { return Gift::Nothing };
    // The place's own (the pool comes in id order): the same artefact in every game, and it says
    // whether it was given.
    let Some(mine) = pool.filter(|r| r.kind == kind).nth(usize::from(place)) else { return Gift::Nothing };
    match (mine.history, containers) {
        (History::Given, _) => Gift::Nothing,
        (History::Unused, Containers::Free) => Gift::Take(mine.id, kind),
        (History::Unused, Containers::None | Containers::Full) => Gift::Pending,
    }
}

/// The register_artefacts entry (0x31B8E0): the map's Keeper pool, as the core world numbers it.
pub fn on_register(ctx: &mut Context<'_>) -> Flow<NoExit> {
    crate::vostok::keeper::register(ctx);
    Flow::Continue
}

/// Each player's tick (base_player::tick before tick_animations, after the glow's).
pub fn on_tick(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(recipient) = Recipient::at_tick(ctx) else { return Flow::Continue };
    let skills = recipient.skills();
    // Only a Keeper's tick reads the match (time rule, statistics, containers, the pool).
    if !skills.keeper() {
        return Flow::Continue;
    }
    if let Gift::Take(id, kind) = gift(skills, recipient.place(), recipient.standing(), recipient.containers(), recipient.pool()) {
        let given = recipient.give(id);
        line(&[
            Text("keeper: place "), Dec(recipient.place().into()), Text(if given { " takes " } else { " REFUSED " }),
            Text(kind.name()), Text(" id "), Dec(id.get().into()),
        ]);
    }
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skills(value: u32) -> Skills {
        Skills::parse(value as f32).unwrap()
    }

    /// Two of each kind; place 1's Onyx and place 0's Spring given.
    fn pool() -> [Registered; 6] {
        let at = |id: u8, kind, history| Registered { id: ArtefactId::parse(id).unwrap(), kind, history };
        [
            at(30, Kind::Larkspur, History::Unused),
            at(31, Kind::Larkspur, History::Unused),
            at(32, Kind::Onyx, History::Unused),
            at(33, Kind::Onyx, History::Given),
            at(34, Kind::Spring, History::Given),
            at(35, Kind::Spring, History::Unused),
        ]
    }

    const KEEPER: u32 = 8;
    const LATE: u32 = 1 << 13;
    fn draw(n: u32) -> u32 {
        n << 9
    }

    #[test]
    fn each_place_takes_its_own_of_the_draws_kind_once() {
        let id = |n| ArtefactId::parse(n).unwrap();
        let at = |value, place| gift(skills(value), place, Standing::Started { deaths: 0 }, Containers::Free, pool().into_iter());
        assert_eq!(at(KEEPER + draw(1), 0), Gift::Take(id(30), Kind::Larkspur));
        assert_eq!(at(KEEPER + draw(1), 1), Gift::Take(id(31), Kind::Larkspur));
        assert_eq!(at(KEEPER + draw(2), 0), Gift::Take(id(32), Kind::Onyx));
        // Given already (wherever it is now).
        assert_eq!(at(KEEPER + draw(2), 1), Gift::Nothing);
        assert_eq!(at(KEEPER + draw(3), 0), Gift::Nothing);
        assert_eq!(at(KEEPER + draw(3), 1), Gift::Take(id(35), Kind::Spring));
        // None for a place past the pool, no draw of a kind, no skill.
        assert_eq!(at(KEEPER + draw(1), 2), Gift::Nothing);
        assert_eq!(at(KEEPER + draw(4), 0), Gift::Nothing);
        assert_eq!(at(KEEPER, 0), Gift::Nothing);
        assert_eq!(at(draw(1), 0), Gift::Nothing);
    }

    #[test]
    fn due_from_the_start_or_a_late_joiners_first_death_and_pending_without_room() {
        let id = |n| ArtefactId::parse(n).unwrap();
        let at = |value, standing, containers| gift(skills(value), 0, standing, containers, pool().into_iter());
        let started = |deaths| Standing::Started { deaths };
        let take = Gift::Take(id(30), Kind::Larkspur);
        assert_eq!(at(KEEPER + draw(1), Standing::NotStarted, Containers::Free), Gift::Nothing);
        assert_eq!(at(KEEPER + draw(1), started(0), Containers::Free), take);
        assert_eq!(at(KEEPER + draw(1), started(3), Containers::Free), take);
        assert_eq!(at(KEEPER + draw(1), started(0), Containers::Full), Gift::Pending);
        assert_eq!(at(KEEPER + draw(1), started(0), Containers::None), Gift::Pending);
        assert_eq!(at(KEEPER + draw(1) + LATE, started(0), Containers::Free), Gift::Nothing);
        assert_eq!(at(KEEPER + draw(1) + LATE, started(1), Containers::Free), take);
        assert_eq!(at(KEEPER + draw(1) + LATE, started(2), Containers::Full), Gift::Pending);
    }
}
