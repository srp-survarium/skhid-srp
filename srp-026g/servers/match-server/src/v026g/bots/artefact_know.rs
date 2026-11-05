//! [`artefact_know`](super::tuning::HazardTuning) What the bots know of the artefact places,
//! knowledge only: no bot goes to one, takes one or uses one (the owner, 2026-10-27: "prepare
//! now, pickup later"; `grenades-record/live-zones-design.md` 7).
//!
//! Root cause of there being nothing to build a pickup on: the map file named a core's artefact
//! containers by a count alone, so a container could not be told from the core that owns it,
//! nor how long taking its artefact holds a body (6 s, 2 s on London, 5 s at Vostok's antenna),
//! nor which artefacts the core draws from. Format version 3 keeps the names, the hold and
//! cooldown times and the pool ([`mapdata::ArtefactCore`]); a version 2 file has none, and then
//! every core here has no spots and the counts below stay empty.
//!
//! - **The places** ([`Spot`]): each container a core names, linked once at load to its zone
//!   (`ZoneKind::ArtefactContainer`, the use box), with its hold and cooldown. A name the zones
//!   lack, or a pool id that is not a 0.26g artefact, is counted ([`ArtefactKnow::unlinked`]),
//!   not guessed.
//! - **What a core has** ([`CoreArtefacts`]): from the simulation the bots already run
//!   (`anomalies.rs`: the core's draws, corrected from every checked world by its random value;
//!   nothing here reads the world's bytes, so nothing is parsed twice, next to
//!   `field_source::v1`): an artefact due at a match time, or its containers full. The world
//!   does not say which container holds the artefact nor which kind (the objects' artefact records
//!   are not read), so it is the core's, not a spot's.
//! - **How well the simulation has it** ([`Counts`]): before each world is taken in, what the
//!   simulation said of every core it finds in it, against what the world says. The spawn time
//!   of the first artefact is exact from the seed (match 0x63755501: 47.710, 64.861 and
//!   73.058 s, PROVEN by `real_worlds_agree_with_the_simulation`); a pick-up is unseen and
//!   re-arms a core with a new draw, so the later ones can differ: that is what is counted.

use mapdata::{MapData, Vec3, ZoneKind};

use super::anomalies::{Anomalies, TICK_MS};

/// The artefacts of 0.26g, by the item's id in the game's dictionary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArtefactKind {
    Lifebone,
    Spring,
    Onyx,
}

impl ArtefactKind {
    /// The kind of dictionary id `id`: 54 lifebone, 74 spring, 76 onyx; none for another item.
    pub fn of_dict(id: u32) -> Option<Self> {
        match id {
            54 => Some(Self::Lifebone),
            74 => Some(Self::Spring),
            76 => Some(Self::Onyx),
            _ => None,
        }
    }
}

/// A place an artefact lies at and is taken from: one container of one core.
#[derive(Clone, Debug, PartialEq)]
pub struct Spot {
    /// The core's index in `MapData::anomalies` (and `Anomalies::cores()`).
    pub core: usize,
    /// The container's zone in `MapData::zones`.
    pub zone: usize,
    /// The centre of the container's use box (its zone's position if it has none).
    pub at: Vec3,
    /// How long a body holds USE to take the artefact, seconds.
    pub hold_s: f32,
    /// How long the container is shut after a take, seconds.
    pub cooldown_s: f32,
}

/// What one core names: the kinds its spawns draw from and its spots.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CoreSpots {
    /// `(kind, count)` of its pool, in config order.
    pub kinds: Vec<(ArtefactKind, u32)>,
    /// Its spots, as indices into [`ArtefactKnow::spots`].
    pub spots: Vec<usize>,
}

/// What a core has of artefacts, as the simulation (corrected from the worlds) has it. `Due` is a
/// spawn still to come; with more than one artefact allowed (`max_count`) some may already lie.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CoreArtefacts {
    /// The core has no artefacts.
    None,
    /// One is due at this match time (ms): the client's `pending`.
    Due { at: u32 },
    /// Its containers hold all they may: nothing is due (`pending` false after a spawn).
    Full,
}

impl CoreArtefacts {
    /// From [`super::anomalies::Core::artefact`].
    pub fn of(artefact: Option<(bool, u32)>) -> Self {
        match artefact {
            None => Self::None,
            Some((true, at)) => Self::Due { at },
            Some((false, _)) => Self::Full,
        }
    }
}

/// How a world's core compared with the simulation's, for [`Counts`].
#[derive(Clone, Copy, Debug, PartialEq)]
enum Verdict {
    /// Alike: `error_ms` is the difference of the due times, when both are due.
    Agree { error_ms: Option<u32> },
    /// Both due, at times more than a tick apart.
    Off { error_ms: u32 },
    /// The simulation had it full, the world has one due after `now`: a pick-up re-armed it.
    Rearmed,
    /// The simulation had one due, the world has it full: a spawn the simulation did not make.
    Spawned,
}

/// `before` (the simulation just before the world is taken in, at match time `now`) against
/// `world` (the core as the world sets it). A world is a few seconds older than `now`, so a
/// spawn the simulation made in between ([`CoreArtefacts::Full`] against a due time not after
/// `now`) is not a difference.
fn judge(before: CoreArtefacts, world: CoreArtefacts, now: u32) -> Verdict {
    use CoreArtefacts::{Due, Full};
    match (before, world) {
        (Due { at: sim }, Due { at }) => {
            let error_ms = sim.abs_diff(at);
            if error_ms <= TICK_MS {
                Verdict::Agree {
                    error_ms: Some(error_ms),
                }
            } else {
                Verdict::Off { error_ms }
            }
        }
        (Full, Due { at }) if at > now => Verdict::Rearmed,
        (Due { .. }, Full) => Verdict::Spawned,
        _ => Verdict::Agree { error_ms: None },
    }
}

/// What the simulation got right, since the match began.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    /// Worlds taken in.
    pub worlds: u32,
    /// Cores with artefacts that a world was found for.
    pub cores: u32,
    /// Of those, alike (see [`judge`]).
    pub agree: u32,
    /// Both due, the due times more than a tick apart.
    pub off: u32,
    /// The simulation had the artefact lying, the world has one due: re-armed by a pick-up.
    pub rearmed: u32,
    /// The simulation had one due, the world has it lying: a spawn it missed.
    pub spawned: u32,
    /// The difference of the due times of the cores where both were due: at most a tick, at
    /// most 1 s, at most 5 s, more.
    pub due_error: [u32; 4],
    /// Cores with an artefact due / with their containers full, after the last world.
    pub due: u32,
    pub full: u32,
}

impl Counts {
    fn note(&mut self, verdict: Verdict) {
        self.cores += 1;
        let error_ms = match verdict {
            Verdict::Agree { error_ms } => {
                self.agree += 1;
                error_ms
            }
            Verdict::Off { error_ms } => {
                self.off += 1;
                Some(error_ms)
            }
            Verdict::Rearmed => {
                self.rearmed += 1;
                None
            }
            Verdict::Spawned => {
                self.spawned += 1;
                None
            }
        };
        if let Some(ms) = error_ms {
            let bin = [TICK_MS, 1_000, 5_000]
                .iter()
                .position(|&limit| ms <= limit)
                .unwrap_or(3);
            self.due_error[bin] += 1;
        }
    }

    /// The share of the cores read that the simulation had right; 1 with none read.
    pub fn agree_share(&self) -> f32 {
        if self.cores == 0 {
            1.0
        } else {
            self.agree as f32 / self.cores as f32
        }
    }
}

/// The artefact places of a map and how well the cores' artefacts are known, per match.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ArtefactKnow {
    spots: Vec<Spot>,
    /// By core (the index in `MapData::anomalies`).
    cores: Vec<CoreSpots>,
    unlinked: u32,
    counts: Counts,
}

impl ArtefactKnow {
    /// The places `map` names, linked once: each core's containers to their zones by name, its
    /// pool to the kinds. Without artefact cores (a version 2 file) there are none.
    pub fn of(map: &MapData) -> Self {
        let mut know = Self {
            cores: vec![CoreSpots::default(); map.anomalies.len()],
            ..Self::default()
        };
        for named in &map.artefact_cores {
            let Some(core) = map.anomalies.iter().position(|a| a.name == named.core) else {
                know.unlinked += 1;
                continue;
            };
            for &(id, count) in &named.pool {
                match ArtefactKind::of_dict(id) {
                    Some(kind) => know.cores[core].kinds.push((kind, count)),
                    None => know.unlinked += 1,
                }
            }
            for container in &named.containers {
                let Some(zone) = map
                    .zones
                    .iter()
                    .position(|z| z.kind == ZoneKind::ArtefactContainer && z.name == container.name)
                else {
                    know.unlinked += 1;
                    continue;
                };
                let place = &map.zones[zone];
                let spot = Spot {
                    core,
                    zone,
                    at: place.shapes.first().map_or(place.position, |s| s.center),
                    hold_s: container.search_s,
                    cooldown_s: container.cooldown_s,
                };
                know.cores[core].spots.push(know.spots.len());
                know.spots.push(spot);
            }
        }
        know
    }

    /// Every spot of the map.
    pub fn spots(&self) -> &[Spot] {
        &self.spots
    }

    /// The spots of core `core` (an index of `MapData::anomalies`).
    pub fn spots_of(&self, core: usize) -> impl Iterator<Item = &Spot> + '_ {
        self.cores
            .get(core)
            .into_iter()
            .flat_map(|c| c.spots.iter().map(|&i| &self.spots[i]))
    }

    /// The kinds core `core` draws from.
    pub fn kinds_of(&self, core: usize) -> &[(ArtefactKind, u32)] {
        self.cores.get(core).map_or(&[], |c| c.kinds.as_slice())
    }

    /// Containers or pool ids the map file names that it has no zone or kind for, and cores it
    /// names that are not anomalies.
    pub fn unlinked(&self) -> u32 {
        self.unlinked
    }

    pub fn counts(&self) -> &Counts {
        &self.counts
    }

    /// What every core of `anomalies` has of artefacts now (before a world is taken in: the
    /// argument of [`Self::observe`]).
    pub fn predicted(anomalies: &Anomalies) -> Vec<CoreArtefacts> {
        anomalies
            .cores()
            .iter()
            .map(|core| CoreArtefacts::of(core.artefact()))
            .collect()
    }

    /// A world taken in: `predicted` is [`Self::predicted`] from just before, `synced` the cores
    /// (indices) the world was found for, `now` the match time. Counts, and nothing else.
    pub fn observe(&mut self, predicted: &[CoreArtefacts], anomalies: &Anomalies, synced: &[usize], now: u32) {
        self.counts.worlds += 1;
        for &core in synced {
            let (Some(&before), Some(read)) = (predicted.get(core), anomalies.cores().get(core)) else {
                continue;
            };
            let world = CoreArtefacts::of(read.artefact());
            if world != CoreArtefacts::None {
                self.counts.note(judge(before, world, now));
            }
        }
        let (mut due, mut full) = (0, 0);
        for core in anomalies.cores() {
            match CoreArtefacts::of(core.artefact()) {
                CoreArtefacts::Due { .. } => due += 1,
                CoreArtefacts::Full => full += 1,
                CoreArtefacts::None => {}
            }
        }
        (self.counts.due, self.counts.full) = (due, full);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mapdata::{ArtefactContainer, ArtefactCore, Anomaly, Zone};

    use crate::v026g::bots::anomalies::tests::{cylinder, electric, zone};

    fn container(name: &str, at: Vec3) -> Zone {
        Zone {
            kind: ZoneKind::ArtefactContainer,
            name: name.into(),
            team: -1,
            position: at,
            damage_type: String::new(),
            damage: None,
            shapes: Vec::new(),
        }
    }

    fn named(core: &str, containers: &[(&str, f32)], pool: &[(u32, u32)]) -> ArtefactCore {
        ArtefactCore {
            core: core.into(),
            pool: pool.to_vec(),
            containers: containers
                .iter()
                .map(|&(name, search_s)| ArtefactContainer {
                    name: name.into(),
                    search_s,
                    cooldown_s: 80.0,
                })
                .collect(),
        }
    }

    #[test]
    fn the_containers_a_core_names_are_linked_to_their_zones_and_what_is_missing_is_counted() {
        let map = MapData {
            zones: vec![
                container("af/c1", Vec3::new(1.0, 0.0, 2.0)),
                container("af/c2", Vec3::new(5.0, 0.0, 6.0)),
            ],
            anomalies: vec![
                Anomaly {
                    name: "other".into(),
                    ..Anomaly::default()
                },
                Anomaly {
                    name: "core".into(),
                    ..Anomaly::default()
                },
            ],
            artefact_cores: vec![
                // c1 and c2, one container no zone has, an onyx and an item that is no artefact.
                named("core", &[("af/c1", 6.0), ("af/gone", 6.0), ("af/c2", 2.0)], &[(76, 5), (999, 1)]),
                // A core the map has no anomaly of.
                named("ghost", &[("af/c1", 6.0)], &[]),
            ],
            ..MapData::default()
        };
        let know = ArtefactKnow::of(&map);
        assert_eq!(know.spots().len(), 2);
        let spots: Vec<&Spot> = know.spots_of(1).collect();
        assert_eq!(
            spots.iter().map(|s| (s.zone, s.hold_s, s.cooldown_s)).collect::<Vec<_>>(),
            vec![(0, 6.0, 80.0), (1, 2.0, 80.0)]
        );
        assert_eq!(spots[0].at, Vec3::new(1.0, 0.0, 2.0), "no use box: the zone's position");
        assert_eq!(know.spots_of(0).count(), 0, "the other core names none");
        assert_eq!(know.kinds_of(1), &[(ArtefactKind::Onyx, 5)]);
        assert_eq!(know.unlinked(), 3, "a container, an item, a core");
        // A version 2 file: no artefact cores, no spots.
        let old = ArtefactKnow::of(&MapData {
            artefact_cores: Vec::new(),
            ..map
        });
        assert!(old.spots().is_empty() && old.unlinked() == 0);
    }

    #[test]
    fn a_world_is_judged_against_the_simulation_a_few_seconds_later() {
        use CoreArtefacts::{Due, Full, None as Nothing};
        let now = 100_000;
        let agree = |error_ms| Verdict::Agree { error_ms };
        assert_eq!(judge(Due { at: 80_000 }, Due { at: 80_000 }, now), agree(Some(0)));
        assert_eq!(judge(Due { at: 80_000 }, Due { at: 80_050 }, now), agree(Some(50)));
        assert_eq!(judge(Due { at: 80_000 }, Due { at: 83_000 }, now), Verdict::Off { error_ms: 3_000 });
        assert_eq!(judge(Full, Full, now), agree(None));
        // The simulation spawned after the world's time: not a difference.
        assert_eq!(judge(Full, Due { at: 98_000 }, now), agree(None));
        // A pick-up the simulation did not see re-armed the core after `now`.
        assert_eq!(judge(Full, Due { at: 130_000 }, now), Verdict::Rearmed);
        assert_eq!(judge(Due { at: 120_000 }, Full, now), Verdict::Spawned);
        assert_eq!(judge(Nothing, Nothing, now), agree(None));
    }

    #[test]
    fn the_counts_follow_the_verdicts_and_bin_the_errors() {
        let mut counts = Counts::default();
        assert_eq!(counts.agree_share(), 1.0);
        for verdict in [
            Verdict::Agree { error_ms: Some(0) },
            Verdict::Agree { error_ms: None },
            Verdict::Off { error_ms: 900 },
            Verdict::Off { error_ms: 4_000 },
            Verdict::Off { error_ms: 40_000 },
            Verdict::Rearmed,
            Verdict::Spawned,
        ] {
            counts.note(verdict);
        }
        assert_eq!(
            (counts.cores, counts.agree, counts.off, counts.rearmed, counts.spawned),
            (7, 2, 3, 1, 1)
        );
        assert_eq!(counts.due_error, [1, 1, 1, 1]);
        assert!((counts.agree_share() - 2.0 / 7.0).abs() < 1e-6);
    }

    /// The simulation's own cores: a core with artefacts is due at its first spawn time
    /// (60 s +- 30 s), then full (one allowed, chance 1).
    #[test]
    fn a_core_is_due_and_then_full_as_the_simulation_runs() {
        let map = MapData {
            zones: vec![
                zone("a", cylinder(10.0, 20.0, 2.0), 0.6),
                zone("b", cylinder(30.0, 20.0, 2.0), 0.6),
            ],
            anomalies: vec![electric("core", ["a", "b"], true)],
            ..MapData::default()
        };
        let mut anomalies = Anomalies::new(&map, 0x8000_0001);
        let CoreArtefacts::Due { at } = ArtefactKnow::predicted(&anomalies)[0] else {
            panic!("an artefact is due before the first spawn");
        };
        assert!((30_000..=90_000).contains(&at), "{at}");
        anomalies.advance(at + 1_000);
        assert_eq!(ArtefactKnow::predicted(&anomalies), vec![CoreArtefacts::Full]);
        let know = ArtefactKnow::of(&map);
        assert!(know.spots().is_empty(), "this map names no containers");
    }
}
