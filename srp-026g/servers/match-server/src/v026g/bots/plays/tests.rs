//! A scenario builder for the plays' tests (`player_bots_research.md` B8
//! point 2): a small map, places on it, the knowledge and signals a play
//! reads, and a [`Ctx`] over them. Each play's tests live in its own file
//! and build on this.
//!
//! ```ignore
//! let mut s = Scenario::field(Mode::Batteries);
//! s.bot(0, 0, Vec3::new(10.0, 0.0, 10.0));
//! s.enemy(1, 1, Vec3::new(60.0, 0.0, 10.0));
//! s.time_left(90.0).points(2, 3);
//! let cx = s.cx(0);
//! let mut out = Vec::new();
//! late_denial::candidates(&cx, &mut out);
//! ```

use std::sync::Arc;

use mapdata::Vec3;

use super::*;
use crate::v026g::bots::know::{Percept, Source, TeamKnowledge};
use crate::v026g::bots::signals::{MatchSignals, Mods};
use crate::v026g::bots::tactics::Tactics;
use crate::v026g::bots::tuning::Tuning;
use crate::v026g::bots::view::{BatteryState, ContainerView, MatchView, Mode, PlaceView};

/// A match in miniature for one play's test.
pub struct Scenario {
    pub view: MatchView,
    pub memory: Memory,
    pub team: TeamKnowledge,
    pub sig: MatchSignals,
    pub mods: Mods,
    pub tactics: Tactics,
    pub tuning: Tuning,
    /// `Ctx::memo`: a test that changes `tactics` after a question asked
    /// makes it anew.
    pub memo: super::memo::Memo,
    pub side: Option<i32>,
    pub current: Option<(PlayKind, TargetRef, u32)>,
    /// The slot `run` starts the play with (the assignment's).
    pub slot: Option<SlotKey>,
    pub last_ran: Vec<(PlayKind, u32)>,
    pub partner: Option<Place>,
    /// Whether the partner has it for partner too (`BotFacts::paired`).
    pub paired: bool,
    pub personality: usize,
    /// The kind of the bot's weapon 1; none: its personality's name as a role.
    pub weapon: Option<WeaponKind>,
    /// `BotFacts::weapons`: every place's weapon 1 as the match knows it.
    pub weapons: Vec<Option<WeaponKind>>,
    pub medkits_used: u8,
    pub pantry: Pantry,
    pub pools: Option<Pools>,
    pub zone_armour: std::sync::Arc<crate::v026g::bots::zone_cost::ZoneArmour>,
    pub no_way_to: Vec<(Vec3, u32)>,
    pub dry: bool,
    /// `BotFacts::ammo`.
    pub ammo: crate::v026g::bots::guns::Ammo,
    /// `BotFacts::failed_stands`.
    pub failed_stands: Vec<super::collect::FailedStand>,
    /// `BotFacts::crossing`.
    pub crossing: Option<Vec3>,
    pub held_choke: Option<(Place, Vec3, u32)>,
}

impl Scenario {
    /// The 80 x 40 m field of `tactics::tests::field` (side 0 spawns at
    /// x = 5, side 1 at x = 75, a wall across x = 40 up to z = 25), with
    /// a container of team 0 at (12, 0, 30) and of team 1 at (68, 0, 30),
    /// at 600 s left of 900. The batteries a test places are known to the
    /// bot (the battery cheat on); the fair battery knowledge has its own
    /// tests (`collect`, `know`, `sense`).
    pub fn field(mode: Mode) -> Self {
        // The two containers have their use boxes (a metre over each), as
        // the maps' do.
        let world = crate::v026g::bots::tactics::tests::field_with(|map| {
            for (id, team, x) in [(0, 0, 12.0), (1, 1, 68.0)] {
                let position = Vec3::new(x, 0.0, 30.0);
                map.containers.push(mapdata::Container {
                    position,
                    team,
                    id,
                    use_shapes: vec![mapdata::Shape {
                        kind: mapdata::ShapeKind::Box,
                        center: position + Vec3::new(0.0, 1.0, 0.0),
                        rotation: mapdata::Rotation::IDENTITY,
                        half: Vec3::new(1.0, 0.85, 0.65),
                    }],
                });
            }
        });
        let tactics = Tactics::new(world, 1.0);
        let view = MatchView {
            clock: 300_000,
            mode,
            time_left_s: 600.0,
            total_s: 900.0,
            points: [0, 0],
            places: Vec::new(),
            batteries: Vec::new(),
            marks: vec![],
            containers: vec![
                ContainerView {
                    id: 0,
                    team: 0,
                    at: Vec3::new(12.0, 0.0, 30.0),
                    batteries: 0,
                },
                ContainerView {
                    id: 1,
                    team: 1,
                    at: Vec3::new(68.0, 0.0, 30.0),
                    batteries: 0,
                },
            ],
            sounds: Vec::new(),
            deaths: Vec::new(),
            world_time: 299_000,
            can_respawn: [true, true],
            next_battery: None,
            grenades: Vec::new(),
            smoke: Default::default(),
        };
        // The bots as they were, an option on where a test says so (the owner's set is the defaults).
        let mut tuning = Tuning::all_off();
        tuning.cheats.batteries = true;
        let mut s = Self {
            view,
            memory: Memory::default(),
            team: TeamKnowledge::default(),
            sig: MatchSignals::default(),
            mods: Mods::default(),
            tactics,
            tuning,
            memo: Default::default(),
            side: Some(0),
            current: None,
            slot: None,
            last_ran: Vec::new(),
            partner: None,
            paired: true,
            personality: 0,
            weapon: None,
            weapons: Vec::new(),
            medkits_used: 0,
            pantry: Pantry::standard(),
            pools: None,
            zone_armour: Default::default(),
            no_way_to: Vec::new(),
            dry: false,
            ammo: Default::default(),
            failed_stands: Vec::new(),
            crossing: None,
            held_choke: None,
        };
        s.signals();
        s
    }

    fn place(&mut self, place: Place, team: u8, feet: Vec3, bot: bool) -> &mut PlaceView {
        while self.view.places.len() <= place {
            let n = self.view.places.len();
            self.view.places.push(PlaceView {
                place: n,
                team: 0,
                bot: false,
                present: false,
                alive: false,
                feet: Vec3::ZERO,
                yaw: 0.0,
                pitch: 0.0,
                velocity: [0.0, 0.0],
                keys: 0,
                health: 1.0,
                spawned: 0,
                kills: 0,
                deaths: 0,
                streak: 0,
                carrying: None,
                seen_at: 0,
                late_ms: 0,
                known_at: 0,
            });
        }
        let p = &mut self.view.places[place];
        p.team = team;
        p.bot = bot;
        p.present = true;
        p.alive = true;
        p.feet = feet;
        p
    }

    /// A bot at `place` of `team` standing at `feet`.
    /// A number of a play's tuning (a switch is 1.0).
    pub fn number(&mut self, kind: PlayKind, name: &str, value: f32) {
        self.tuning
            .plays
            .entry(kind)
            .or_insert_with(|| super::default_tuning(kind))
            .numbers
            .insert(name.into(), value);
    }

    pub fn bot(&mut self, place: Place, team: u8, feet: Vec3) -> &mut PlaceView {
        self.place(place, team, feet, true)
    }

    /// A human at `place`.
    pub fn human(&mut self, place: Place, team: u8, feet: Vec3) -> &mut PlaceView {
        self.place(place, team, feet, false)
    }

    /// An enemy human at `place`, known to the bot as `source` says (or not
    /// at all with `None`).
    pub fn enemy(
        &mut self,
        place: Place,
        team: u8,
        feet: Vec3,
        source: Option<Source>,
    ) -> &mut Self {
        self.place(place, team, feet, false);
        if let Some(source) = source {
            let now = self.view.clock;
            self.memory.update(
                &[Percept {
                    who: place,
                    at: feet,
                    error_m: 0.0,
                    time: now - 400,
                    source,
                    busy: false,
                }],
                now - 400,
                |_| 200,
                3_000,
                20_000,
            );
            self.memory.update(
                &[Percept {
                    who: place,
                    at: feet,
                    error_m: 0.0,
                    time: now,
                    source,
                    busy: false,
                }],
                now,
                |_| 200,
                3_000,
                20_000,
            );
        }
        self
    }

    pub fn battery(&mut self, state: BatteryState) -> &mut Self {
        self.view.batteries.push(state);
        self
    }

    pub fn container_count(&mut self, id: u8, batteries: usize) -> &mut Self {
        if let Some(c) = self.view.containers.iter_mut().find(|c| c.id == id) {
            c.batteries = batteries;
        }
        self
    }

    pub fn time_left(&mut self, s: f32) -> &mut Self {
        self.view.time_left_s = s;
        self.signals();
        self
    }

    /// Team 0's points and team 1's.
    pub fn points(&mut self, ours: u32, theirs: u32) -> &mut Self {
        self.view.points = [ours, theirs];
        self.signals();
        self
    }

    /// The signals of team 0 as the view now has them, and the modifiers
    /// settled to them.
    pub fn signals(&mut self) {
        self.sig = MatchSignals::read(&self.view, 0, 1e6);
        for _ in 0..40 {
            self.mods.update(&self.sig, &self.tuning.mods);
        }
    }

    /// The context of the bot at `place`.
    pub fn cx(&self, place: Place) -> Ctx<'_> {
        Ctx {
            now: self.view.clock,
            view: &self.view,
            me: &self.view.places[place],
            bot: BotFacts {
                personality: &self.tuning.personalities[self.personality],
                weapon: self.weapon.or_else(|| {
                    WeaponKind::of_role(&self.tuning.personalities[self.personality].name)
                }),
                weapons: &self.weapons,
                side: self.side,
                current: self.current,
                last_ran: &self.last_ran,
                partner: self.partner,
                paired: self.paired,
                medkits_used: self.medkits_used,
                pantry: &self.pantry,
                pools: self.pools,
                zone_armour: &self.zone_armour,
                no_way_to: &self.no_way_to,
                set_aside: &[],
                failed_stands: &self.failed_stands,
                dry: self.dry,
                ammo: self.ammo,
                crossing: self.crossing,
                held_choke: self.held_choke,
            },
            memory: &self.memory,
            team: &self.team,
            sig: &self.sig,
            mods: &self.mods,
            tactics: &self.tactics,
            tuning: &self.tuning,
            memo: &self.memo,
        }
    }

    /// Every play's candidates for the bot at `place`, best first.
    pub fn candidates(&self, place: Place) -> Vec<Candidate> {
        let mut out = Vec::new();
        candidates(&self.cx(place), &mut out);
        out.sort_by(|a, b| {
            b.rank
                .cmp(&a.rank)
                .then(b.score.get().total_cmp(&a.score.get()))
        });
        out
    }

    /// Runs a play from its start for `steps` steps of `ms` each, the bot
    /// moving straight to where each intent goes at `speed` m/s; returns
    /// the steps.
    pub fn run(
        &mut self,
        place: Place,
        kind: PlayKind,
        target: TargetRef,
        steps: usize,
        ms: u32,
        speed: f32,
    ) -> Vec<Step> {
        let mut rng = crate::v026g::bots::rng::Rng::new(11);
        self.current = Some((kind, target, self.view.clock));
        let mut running = start(kind, target, self.slot, &self.cx(place), &mut rng);
        let mut out = Vec::new();
        for _ in 0..steps {
            let s = step(&mut running, target, &self.cx(place), &mut rng);
            let done = !matches!(s, Step::Continue(_));
            if let Step::Continue(Intent {
                go: Go::To { at, .. },
                ..
            }) = &s
            {
                let me = self.view.places[place].feet;
                let (dx, dz) = (at.x - me.x, at.z - me.z);
                let d = dx.hypot(dz);
                let reach = speed * ms as f32 / 1000.0;
                let t = if d <= reach { 1.0 } else { reach / d };
                self.view.places[place].feet = Vec3::new(me.x + dx * t, me.y, me.z + dz * t);
            }
            out.push(s);
            self.view.clock += ms;
            if done {
                break;
            }
        }
        out
    }
}

/// The Arc type the field map comes in, for tests that need it.
pub type Shared<T> = Arc<T>;

#[test]
fn late_denial_is_taken_behind_late_and_never_two_ahead() {
    let mut s = Scenario::field(Mode::Batteries);
    s.bot(0, 0, Vec3::new(30.0, 0.0, 30.0));
    s.container_count(1, 3).time_left(90.0).points(2, 3);
    let best = s.candidates(0);
    assert!(
        best.iter().any(|c| c.kind == PlayKind::LateDenial),
        "{best:#?}"
    );
    s.points(5, 3);
    assert!(
        s.candidates(0)
            .iter()
            .all(|c| c.kind != PlayKind::LateDenial)
    );
    s.points(2, 3).time_left(600.0);
    assert!(
        s.candidates(0)
            .iter()
            .all(|c| c.kind != PlayKind::LateDenial)
    );
}

#[test]
fn late_denial_goes_there_steals_and_drops() {
    let mut s = Scenario::field(Mode::Batteries);
    s.bot(0, 0, Vec3::new(60.0, 0.0, 30.0));
    s.container_count(1, 2).time_left(90.0).points(2, 3);
    let steps = s.run(
        0,
        PlayKind::LateDenial,
        TargetRef::Container(1),
        40,
        300,
        3.5,
    );
    let pressed: u32 = steps
        .iter()
        .filter_map(|s| match s {
            Step::Continue(intent) => Some(intent.press),
            _ => None,
        })
        .fold(0, |a, b| a | b);
    assert_ne!(pressed & crate::v026g::drive::key::USE, 0, "{steps:?}");
    // It never takes more than its tries of use when nothing comes of it.
    let uses = steps
        .iter()
        .filter(|s| matches!(s, Step::Continue(i) if i.press & crate::v026g::drive::key::USE != 0))
        .count();
    assert!(uses <= 4, "{uses}");
    assert!(
        matches!(steps.last(), Some(Step::Failed(_))),
        "{:?}",
        steps.last()
    );
}

/// A dry bot (nothing to fire) takes no play that seeks a fight: no hunt
/// of the last enemy; it still collects a battery and roams.
#[test]
fn a_dry_bot_takes_no_fight_play_and_still_collects() {
    let mut s = Scenario::field(Mode::TeamDeathmatch);
    s.bot(0, 0, Vec3::new(10.0, 0.0, 10.0));
    s.enemy(1, 1, Vec3::new(60.0, 0.0, 30.0), Some(Source::Heard));
    s.view.can_respawn = [true, false];
    assert_eq!(s.candidates(0)[0].kind, PlayKind::Hunt);
    s.dry = true;
    let best = s.candidates(0);
    assert!(best.iter().all(|c| !c.kind.seeks_fight()), "{best:#?}");
    let mut b = Scenario::field(Mode::Batteries);
    b.bot(0, 0, Vec3::new(10.0, 0.0, 10.0));
    b.battery(BatteryState::Free(Vec3::new(30.0, 0.0, 35.0)));
    b.dry = true;
    let best = b.candidates(0);
    assert_eq!(best[0].kind, PlayKind::Collect, "{best:#?}");
    assert!(best.iter().any(|c| c.kind == PlayKind::Roam));
}

#[test]
fn collect_fetches_a_known_battery_and_roam_heads_for_an_exit() {
    let mut s = Scenario::field(Mode::Batteries);
    s.bot(0, 0, Vec3::new(10.0, 0.0, 10.0));
    s.battery(BatteryState::Free(Vec3::new(30.0, 0.0, 35.0)));
    let best = s.candidates(0);
    assert_eq!(best[0].kind, PlayKind::Collect, "{best:#?}");
    assert!(best.iter().any(|c| c.kind == PlayKind::Roam));
    let steps = s.run(0, PlayKind::Collect, TargetRef::Battery(0), 60, 300, 3.5);
    assert!(
        steps.iter().any(
            |s| matches!(s, Step::Continue(i) if i.press & crate::v026g::drive::key::USE != 0)
        ),
        "{steps:?}"
    );
}


/// One teammate covers one threat: two threats he alone sees leave one
/// free; a second teammate for the other covers both; a teammate who
/// sees both and one who sees one are matched so both are covered.
#[test]
fn each_threat_needs_a_teammate_of_his_own() {
    assert_eq!(cover_each(&[]), None);
    assert_eq!(cover_each(&[vec![]]), Some(0));
    assert_eq!(cover_each(&[vec![true]]), None);
    assert_eq!(cover_each(&[vec![true], vec![true]]), Some(1));
    assert_eq!(cover_each(&[vec![true, false], vec![false, true]]), None);
    assert_eq!(cover_each(&[vec![true, true], vec![true, false]]), None);
    assert_eq!(cover_each(&[vec![false, false], vec![true, true]]), Some(0));
}

/// What `FriendsNearTarget` counts for a slot's holder and for a teammate
/// who would take it (a Cut at one point): living teammates within 25 m of
/// the target, the bot itself not counted, the dead, the absent and the
/// enemies not counted. So each of the two counts the other when the other
/// is near the target: the holder standing at it makes the claimant's
/// count one higher, and a claimant already near it makes the holder's one
/// higher. Who is near the target, not how crowded it will be once the
/// slot has changed hands, decides that difference (`team::takes_held`
/// compares the two without it).
#[test]
fn friends_near_a_held_target_count_the_other_contender() {
    let at = Vec3::new(20.0, 0.0, 10.0);
    let target = TargetRef::Point(at);
    let friends = |s: &Scenario, place| {
        s.cx(place)
            .input(Input::FriendsNearTarget, PlayKind::Cut, target)
    };
    let mut s = Scenario::field(Mode::Batteries);
    // A third teammate near the target; a dead one and an absent one at it;
    // an enemy at it.
    s.bot(2, 0, Vec3::new(25.0, 0.0, 10.0));
    s.bot(3, 0, at).alive = false;
    s.bot(4, 0, at).present = false;
    s.enemy(5, 1, at, None);
    // The holder at the target, the claimant 40 m off.
    s.bot(0, 0, at);
    s.bot(1, 0, Vec3::new(60.0, 0.0, 10.0));
    assert_eq!(friends(&s, 0), 1.0, "the holder: the third teammate");
    assert_eq!(friends(&s, 1), 2.0, "the claimant: the holder, the third");
    // The holder still on its way, the claimant near the target.
    s.view.places[0].feet = Vec3::new(60.0, 0.0, 10.0);
    s.view.places[1].feet = Vec3::new(22.0, 0.0, 10.0);
    assert_eq!(friends(&s, 0), 2.0, "the holder: the claimant, the third");
    assert_eq!(friends(&s, 1), 1.0, "the claimant: the third");
    // Both near it: each counts the other.
    s.view.places[0].feet = at;
    assert_eq!(friends(&s, 0), 2.0);
    assert_eq!(friends(&s, 1), 2.0);
}
