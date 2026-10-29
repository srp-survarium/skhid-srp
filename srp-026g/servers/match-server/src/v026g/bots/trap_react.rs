//! A bot and a trap that is not its team's: the chance to notice it, and what
//! the bots' passes by one come to (`docs/notes/bear_traps.md`, C18).
//!
//! **A noticed trap is shot** (`traps.react.shot`, off): the bot stops, looks
//! at the trap's centre and takes single, aimed shots at it with its own
//! aim model (no aim boost: the cone, the kick and the hit chance are the
//! reflex's and the client's).
//!
//! **A noticed trap is defused by hand** (`traps.react.defuse`, off) by a bot
//! that does not shoot it: it walks to a spot `defuse_stand_m` from the trap
//! on its own side, crouches, looks at the trap's centre and holds USE for the
//! trap's defuse time (5 s), unless something calls it away or a mate is
//! already beside the trap. The same one-at-a-time rule as the shot.
//!
//! **A noticed trap is kept off, or jumped** (`traps.react.pass`, `.jump`,
//! off): the traps the bot noticed and does not shoot make the floors round
//! them a *soft* part of its route filter (a way round them where that is not
//! much longer, else over them), and a route that still runs over one is
//! jumped with the motor's own jump a little before it. Private: nothing is
//! shared with the team's field.
//!
//! - **Private.** A bot's notice is its own: [`Seen`] lives in the `Bot`,
//!   nothing goes to `TeamKnowledge` (the ledger: a rule, not a cheat). The
//!   team's list of its own traps ([`super::trap_know`]) stays shared, as the
//!   placer could say it.
//! - **A chance, not knowledge.** The server knows every trap (the world's
//!   objects, `match_msgs::traps::find`); a bot may use one only after a roll
//!   it wins. A roll happens when an armed trap that is not the team's lies
//!   within `far_m`, in front of the bot (`cone_deg` wide), with a clear line
//!   from its eye to the trap; its chance is [`chance`]: `p0` within `near_m`,
//!   falling to 0 at `far_m`, and divided by `fight_div` while the bot is in a
//!   fight. One roll a trap per `roll_s`. The recordings show no measurable
//!   reaction of the players to an armed trap (`wp-t-trap-notice-baseline`);
//!   `p0`, the distances and the cone are therefore **GUESSes tuned by the
//!   bench** against the players' numbers, not claims.
//! - **Counting a pass.** Between two thinks a bot walks a segment; the
//!   closest approach of the segment to a trap within 1.5 m is a pass, and one
//!   within 0.35 m (the sensor 0.25 m and a foot) is *through*. The same
//!   measure the recordings were read with (`trap_notice.py`), split by what
//!   the bot had rolled: noticed, missed, never rolled.

use std::sync::Arc;

use mapdata::Vec3;
use survarium::v026g::match_msgs::traps::{PlacedTrap, TrapState};

use super::aimpoint::sight_line;
use super::route_end::RouteEnd;
use super::sense::in_view;
use super::tactics::USE_REACH_M;
use super::think::Due;
use super::trap_know::TrapField;
use super::traps::{FIELD_M, TRIGGER_M};
use super::tuning::ReactTuning;
use super::view::{PlaceView, Sound, SoundKind};
use super::{Bot, Place, TeamState};

/// Rolls remembered at most.
const SEEN_KEEP: usize = 8;
/// A pass counts closer than this, metres.
pub const PASS_M: f32 = 1.5;
/// A pass is through the sensor closer than this, metres (0.25 m and a foot).
pub const THROUGH_M: f32 = 0.35;
/// The trap's hittable and visible centre above the floor point, metres.
const CENTRE_UP: f32 = 0.12;
/// A trap this far above or below the feet is not on the floor the bot is on.
const SAME_FLOOR_M: f32 = 1.5;
/// A noticed trap stays noticed this long, ms (while it stands).
const NOTICED_KEEP_MS: u32 = 600_000;
/// A trap the world shows disarmed this soon after the bot's last shot is
/// credited to the shot, ms (the worlds are up to 5 s apart).
pub const SHOT_CREDIT_MS: u32 = 6_000;

/// Which trap: the placing time and the place to a tenth of a metre (the
/// world's record; a recycled trap is another).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrapKey {
    placed_ms: u32,
    x: i32,
    z: i32,
}

impl TrapKey {
    pub fn of(trap: &PlacedTrap) -> Self {
        Self {
            placed_ms: trap.placed_ms,
            x: (trap.position[0] * 10.0).round() as i32,
            z: (trap.position[2] * 10.0).round() as i32,
        }
    }
}

/// What a roll for one trap came to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Not seen: no new roll until the time given in [`Seen`].
    Missed,
    /// Seen at this match time.
    Noticed { at: u32 },
}

/// One trap a bot has rolled for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Seen {
    pub key: TrapKey,
    pub verdict: Verdict,
    pub until: u32,
}

/// What a bot had rolled for a trap it passed, as the counts split.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Noticed,
    Missed,
    Unrolled,
}

impl Class {
    const ALL: [Class; 3] = [Class::Noticed, Class::Missed, Class::Unrolled];

    fn index(self) -> usize {
        Class::ALL.iter().position(|&c| c == self).expect("a class")
    }
}

/// The counts of the response (R1: rolls and passes).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReactCounts {
    /// Rolls made, and their results.
    pub rolls: u32,
    pub noticed: u32,
    pub missed: u32,
    /// Passes within [`PASS_M`] of an armed trap that is not the team's, by
    /// [`Class`], and those of them through the sensor.
    pub passes: [u32; 3],
    pub through: [u32; 3],
    /// The shot: begun, single shots fired, traps the world then showed
    /// disarmed within [`SHOT_CREDIT_MS`] of the bot's last shot, given up
    /// (shots or time spent, a fight, the weapon empty), held for a mate
    /// already shooting, and a second bot starting on a trap another was
    /// already shooting.
    pub shoot_started: u32,
    pub shots_fired: u32,
    pub disarmed_by_shot: u32,
    pub gave_up: u32,
    pub held_for_mate: u32,
    pub double_shooters: u32,
    /// The step and the jump: passes of a noticed trap that ended beyond the
    /// sensor with the trap still armed, jumps pressed for a noticed trap,
    /// and noticed traps that went off with the bot's pass within the foot's
    /// reach of them (a proxy: the world shows no name of who stood on it).
    pub trap_passed: u32,
    pub trap_jumped: u32,
    pub trap_caught_after_notice: u32,
    /// The hand defuse: begun (a bot set out for a trap), traps the world
    /// then showed gone with the bot's USE held on them for 3 s or more (the
    /// players' own measure; a proxy: a mate's shot or another's foot may
    /// have done it), and given up (walk too long, a hit, an enemy, out of
    /// reach, the 5 s not enough).
    pub defuse_started: u32,
    pub disarmed_by_hand: u32,
    pub defuse_gave_up: u32,
}

impl ReactCounts {
    /// The bench line, `trap react: ...` (the prefix is fixed; zero counts are printed).
    pub fn line(&self) -> String {
        format!(
            "trap react: rolls {} noticed {} missed {} passes {}/{}/{} through {}/{}/{} shoot_started {} shots_fired {} \
             disarmed_by_shot {} gave_up {} held_for_mate {} double_shooters {} trap_passed {} trap_jumped {} \
             trap_caught_after_notice {} defuse_started {} disarmed_by_hand {} defuse_gave_up {}",
            self.rolls,
            self.noticed,
            self.missed,
            self.passes[0],
            self.passes[1],
            self.passes[2],
            self.through[0],
            self.through[1],
            self.through[2],
            self.shoot_started,
            self.shots_fired,
            self.disarmed_by_shot,
            self.gave_up,
            self.held_for_mate,
            self.double_shooters,
            self.trap_passed,
            self.trap_jumped,
            self.trap_caught_after_notice,
            self.defuse_started,
            self.disarmed_by_hand,
            self.defuse_gave_up
        )
    }

    pub fn add(&mut self, other: &ReactCounts) {
        self.rolls += other.rolls;
        self.noticed += other.noticed;
        self.missed += other.missed;
        self.shoot_started += other.shoot_started;
        self.shots_fired += other.shots_fired;
        self.disarmed_by_shot += other.disarmed_by_shot;
        self.gave_up += other.gave_up;
        self.held_for_mate += other.held_for_mate;
        self.double_shooters += other.double_shooters;
        self.trap_passed += other.trap_passed;
        self.trap_jumped += other.trap_jumped;
        self.trap_caught_after_notice += other.trap_caught_after_notice;
        self.defuse_started += other.defuse_started;
        self.disarmed_by_hand += other.disarmed_by_hand;
        self.defuse_gave_up += other.defuse_gave_up;
        for k in 0..3 {
            self.passes[k] += other.passes[k];
            self.through[k] += other.through[k];
        }
    }

    pub fn passes_all(&self) -> u32 {
        self.passes.iter().sum()
    }

    pub fn through_all(&self) -> u32 {
        self.through.iter().sum()
    }
}

/// What the bot is, for the roll.
#[derive(Clone, Copy, Debug)]
pub struct Look {
    pub now: u32,
    pub feet: Vec3,
    pub eye: Vec3,
    pub yaw: f32,
    /// An enemy seen, or a hit taken lately: the bot's eyes are not on the
    /// floor.
    pub fighting: bool,
}

/// The chance of noticing a trap `d` metres off: `p0` within `near_m`,
/// falling linearly to 0 at `far_m`, divided by `fight_div` in a fight.
pub fn chance(d: f32, fighting: bool, t: &ReactTuning) -> f32 {
    if d >= t.far_m {
        return 0.0;
    }
    let f = if d <= t.near_m {
        1.0
    } else {
        (t.far_m - d) / (t.far_m - t.near_m)
    };
    let p = t.p0 * f;
    if fighting { p / t.fight_div } else { p }
}

/// The closest approach (horizontal metres) of the segment `a` to `b` to the
/// point `at`.
pub fn segment_closest(a: Vec3, b: Vec3, at: Vec3) -> f32 {
    let (dx, dz) = (b.x - a.x, b.z - a.z);
    let l2 = dx * dx + dz * dz;
    let f = if l2 < 1e-9 {
        0.0
    } else {
        (((at.x - a.x) * dx + (at.z - a.z) * dz) / l2).clamp(0.0, 1.0)
    };
    ((a.x + f * dx - at.x).powi(2) + (a.z + f * dz - at.z).powi(2)).sqrt()
}

/// A pass in the making: the closest approach so far to one trap.
#[derive(Clone, Copy, Debug)]
struct Passing {
    key: TrapKey,
    class: Class,
    closest: f32,
}

/// A bot's response to the traps of others: what it has rolled, the pass in
/// the making and the counts. Idle (and free) with the switch off.
#[derive(Clone, Debug, Default)]
pub struct React {
    seen: Vec<Seen>,
    passing: Option<Passing>,
    prev: Option<(u32, Vec3)>,
    counts: ReactCounts,
    reaction: Reaction,
    /// Whether the trap of the reaction is still shown armed (set at each
    /// think from the world).
    target_armed: bool,
    /// Traps the bot has shot at and given up: not started on again.
    tried: Vec<TrapKey>,
    /// The trap of the shot the team's record has the bot at.
    registered: Option<TrapKey>,
    /// The traps the bot noticed, that stand and are not its target: kept off
    /// (`pass`) and jumped (`jump`).
    private: Vec<(TrapKey, Vec3)>,
    /// Their floors, for the route filter.
    field: Arc<TrapField>,
    /// Traps the bot has jumped.
    jumped: Vec<TrapKey>,
    /// Traps the bot has rolled for a defuse (taken or declined): once each.
    tried_defuse: Vec<TrapKey>,
}

impl React {
    pub fn counts(&self) -> ReactCounts {
        self.counts
    }

    pub fn reaction(&self) -> Reaction {
        self.reaction
    }

    /// What the bot has rolled for `key` and still stands.
    pub fn verdict(&self, key: TrapKey, now: u32) -> Option<Verdict> {
        self.seen
            .iter()
            .find(|s| s.key == key && now < s.until)
            .map(|s| s.verdict)
    }

    /// Whether the bot has noticed `key` (and the trap still stands).
    pub fn noticed(&self, key: TrapKey, now: u32) -> bool {
        matches!(self.verdict(key, now), Some(Verdict::Noticed { .. }))
    }

    fn class_of(&self, key: TrapKey, now: u32) -> Class {
        match self.verdict(key, now) {
            Some(Verdict::Noticed { .. }) => Class::Noticed,
            Some(Verdict::Missed) => Class::Missed,
            None => Class::Unrolled,
        }
    }

    /// The roll for `trap` at this think: none if the bot has a standing
    /// verdict, the trap is out of range, behind the bot or hidden. `unit` is
    /// a uniform draw in `[0, 1)`; `sees` is the ray from the eye to the
    /// trap's centre (asked only when everything cheaper has passed).
    pub fn roll(
        &mut self,
        look: &Look,
        trap: &PlacedTrap,
        t: &ReactTuning,
        mut unit: impl FnMut() -> f32,
        sees: impl FnOnce(Vec3, Vec3) -> bool,
    ) -> Option<Verdict> {
        let key = TrapKey::of(trap);
        if let Some(standing) = self.verdict(key, look.now) {
            return Some(standing);
        }
        let at = Vec3::new(trap.position[0], trap.position[1], trap.position[2]);
        let d = look.feet.distance_xz(at);
        if d >= t.far_m || (at.y - look.feet.y).abs() > SAME_FLOOR_M {
            return None;
        }
        let centre = at + Vec3::new(0.0, CENTRE_UP, 0.0);
        if !in_view(look.eye, look.yaw, at, t.cone_deg) || !sees(look.eye, centre) {
            return None;
        }
        self.counts.rolls += 1;
        let (verdict, until) = if unit() < chance(d, look.fighting, t) {
            self.counts.noticed += 1;
            (
                Verdict::Noticed { at: look.now },
                look.now.saturating_add(NOTICED_KEEP_MS),
            )
        } else {
            self.counts.missed += 1;
            (
                Verdict::Missed,
                look.now.saturating_add((t.roll_s * 1000.0) as u32),
            )
        };
        self.seen.retain(|s| look.now < s.until);
        if self.seen.len() >= SEEN_KEEP {
            self.seen.remove(0);
        }
        self.seen.push(Seen {
            key,
            verdict,
            until,
        });
        Some(verdict)
    }

    /// The segment the bot walked since the last think against the armed
    /// traps: a pass is counted when the bot leaves a trap's 1.5 m, by what
    /// it had rolled for it.
    pub fn walk(&mut self, now: u32, feet: Vec3, traps: &[(TrapKey, Vec3)]) {
        let prev = self.prev.replace((now, feet));
        let Some((_, from)) = prev else {
            return;
        };
        let nearest = traps
            .iter()
            .filter(|(_, at)| (at.y - feet.y).abs() <= SAME_FLOOR_M)
            .map(|&(key, at)| (key, segment_closest(from, feet, at)))
            .filter(|&(_, d)| d <= PASS_M)
            .min_by(|a, b| a.1.total_cmp(&b.1));
        match (self.passing, nearest) {
            (Some(p), Some((key, d))) if p.key == key => {
                self.passing = Some(Passing {
                    closest: p.closest.min(d),
                    ..p
                });
            }
            (current, near) => {
                if let Some(p) = current {
                    self.finish(p, false);
                }
                self.passing = near.map(|(key, d)| Passing {
                    key,
                    class: self.class_of(key, now),
                    closest: d,
                });
            }
        }
    }

    /// A pass ends: the bot left the trap's 1.5 m, or the trap is `gone`
    /// (fired, disarmed).
    fn finish(&mut self, p: Passing, gone: bool) {
        let k = p.class.index();
        self.counts.passes[k] += 1;
        self.counts.through[k] += u32::from(p.closest <= THROUGH_M);
        if p.class == Class::Noticed {
            self.counts.trap_passed += u32::from(!gone && p.closest > THROUGH_M);
            self.counts.trap_caught_after_notice += u32::from(gone && p.closest <= TRIGGER_M);
        }
    }

    /// The noticed traps the bot keeps off or jumps.
    pub fn private(&self) -> &[(TrapKey, Vec3)] {
        &self.private
    }

    /// Their floors.
    pub fn field(&self) -> &Arc<TrapField> {
        &self.field
    }

    /// A new life: a pass in the making ends with it, the rolls are forgotten.
    pub fn new_life(&mut self) {
        self.reaction = Reaction::Idle;
        self.private.clear();
        self.field = Arc::default();
        self.jumped.clear();
        self.tried.clear();
        self.tried_defuse.clear();
        self.seen.clear();
        self.passing = None;
        self.prev = None;
    }

    /// Forget the rolls of traps the world no longer shows, and close a
    /// pass at a trap that is gone (fired by this bot or another, defused).
    pub fn prune(&mut self, armed: &[(TrapKey, Vec3)]) {
        self.seen
            .retain(|s| armed.iter().any(|(key, _)| *key == s.key));
        if let Some(p) = self.passing
            && !armed.iter().any(|(key, _)| *key == p.key)
        {
            self.finish(p, true);
            self.passing = None;
        }
    }
}

/// What a bot does about a trap it has noticed.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Reaction {
    #[default]
    Idle,
    /// Turning to the trap, the legs still, the aim coming on.
    Aim { key: TrapKey, at: Vec3, since: u32 },
    /// The aimed shots: `shots` fired, `last` the time of the last, `down`
    /// the fire key is down this tick (it goes up the next).
    Shoot {
        key: TrapKey,
        at: Vec3,
        since: u32,
        shots: u8,
        last: u32,
        down: bool,
    },
    /// Walking to the stand `spot` beside the trap, to defuse it.
    Walk {
        key: TrapKey,
        at: Vec3,
        spot: Vec3,
        since: u32,
    },
    /// At the stand, crouched, the view on the trap, USE held (`since`: the
    /// arrival).
    Defuse { key: TrapKey, at: Vec3, since: u32 },
}

impl Reaction {
    pub fn target(&self) -> Option<(TrapKey, Vec3)> {
        match *self {
            Reaction::Idle => None,
            Reaction::Aim { key, at, .. }
            | Reaction::Shoot { key, at, .. }
            | Reaction::Walk { key, at, .. }
            | Reaction::Defuse { key, at, .. } => Some((key, at)),
        }
    }

    /// When the reaction began, if it is going.
    pub fn since(&self) -> Option<u32> {
        match *self {
            Reaction::Idle => None,
            Reaction::Aim { since, .. }
            | Reaction::Shoot { since, .. }
            | Reaction::Walk { since, .. }
            | Reaction::Defuse { since, .. } => Some(since),
        }
    }

    /// Whether it is a hand defuse (walking to it or at it).
    pub fn by_hand(&self) -> bool {
        matches!(self, Reaction::Walk { .. } | Reaction::Defuse { .. })
    }
}

/// What a tick of the body tells the reaction.
#[derive(Clone, Copy, Debug)]
pub struct Tick {
    pub now: u32,
    /// The angle between the view and the trap's centre, radians.
    pub err_rad: f32,
    /// Nothing calls the bot away: no target, no shot, no use.
    pub calm: bool,
    /// The weapon in hand has a round in it and is not being reloaded.
    pub ready: bool,
    /// A mate is seen shooting at the trap ([`mate_firing`]), or standing
    /// beside it ([`mate_beside`]).
    pub mate_firing: bool,
    /// How far the bot is from its stand, metres (a walk).
    pub to_spot_m: f32,
    /// The eye's distance to the trap's centre, metres (the use ray's reach).
    pub eye_m: f32,
}

/// What the reaction asks of the body this tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Out {
    /// Look at this point (the trap's centre), hold aim, keep the legs still.
    pub look: Option<Vec3>,
    /// The fire key is down this tick.
    pub fire: bool,
    /// How the reaction ended this tick, if it did.
    pub ended: Option<End>,
    /// A shot was fired this tick.
    pub shot: bool,
    /// Walk to this stand (a hand defuse's approach).
    pub go: Option<Vec3>,
    /// Crouch, and press USE this tick (a hand defuse at the stand).
    pub crouch: bool,
    pub use_key: bool,
}

/// How a reaction ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    /// The trap is no longer shown armed (disarmed by anyone, or fired).
    Gone { shots: u8, last: u32 },
    /// Given up: too many shots or too long, a fight, the weapon empty.
    GaveUp,
    /// A mate was shooting at it: held for them.
    HeldForMate,
    /// The trap went (shown disarmed or gone) while USE had been held on it
    /// for `held_ms`.
    Defused { held_ms: u32 },
}

/// The bot is at its stand within this of it, metres.
const ARRIVE_M: f32 = 0.4;
/// The hold is given up this long after the defuse time, ms.
const DEFUSE_SLACK_MS: u32 = 1_500;
/// The crouch takes this long to bring the eye down to the use ray's reach,
/// ms; a bot still out of reach after it gives up.
const REACH_GRACE_MS: u32 = 1_200;
/// A hold of this long counts as the bot's defuse, ms (the players' measure:
/// a USE held 3 s or more).
const DEFUSE_CREDIT_MS: u32 = 3_000;
/// A mate this near a trap, standing and looking at it, is defusing it,
/// metres and m/s.
const BESIDE_M: f32 = 1.4;
const BESIDE_SPEED: f32 = 0.6;

/// The reaction's one transition: total over the ticks.
pub fn step(state: Reaction, tick: Tick, armed: bool, t: &ReactTuning) -> (Reaction, Out) {
    let idle = |ended| {
        (
            Reaction::Idle,
            Out {
                ended,
                ..Out::default()
            },
        )
    };
    let looking = |at: Vec3| Some(at + Vec3::new(0.0, CENTRE_UP, 0.0));
    match state {
        Reaction::Idle => idle(None),
        Reaction::Walk {
            key,
            at,
            spot,
            since,
        } => {
            if !armed {
                return idle(Some(End::Gone { shots: 0, last: 0 }));
            }
            if !tick.calm {
                return idle(Some(End::GaveUp));
            }
            if tick.mate_firing {
                return idle(Some(End::HeldForMate));
            }
            if tick.now.saturating_sub(since) as f32 / 1000.0 > t.defuse_walk_s {
                return idle(Some(End::GaveUp));
            }
            if tick.to_spot_m <= ARRIVE_M {
                return (
                    Reaction::Defuse {
                        key,
                        at,
                        since: tick.now,
                    },
                    Out {
                        look: looking(at),
                        crouch: true,
                        ..Out::default()
                    },
                );
            }
            (
                state,
                Out {
                    go: Some(spot),
                    ..Out::default()
                },
            )
        }
        Reaction::Defuse { at, since, .. } => {
            let held = tick.now.saturating_sub(since);
            if !armed {
                return idle(Some(End::Defused { held_ms: held }));
            }
            // Holding already: a mate that comes does not move this bot.
            if !tick.calm
                || held as f32 / 1000.0 > t.defuse_s + DEFUSE_SLACK_MS as f32 / 1000.0
                || (tick.eye_m > USE_REACH_M && held > REACH_GRACE_MS)
            {
                return idle(Some(End::GaveUp));
            }
            (
                state,
                Out {
                    look: looking(at),
                    crouch: true,
                    use_key: held >= t.use_settle_ms && tick.err_rad <= t.use_ok_rad,
                    ..Out::default()
                },
            )
        }
        Reaction::Aim { key, at, since } => {
            if !armed {
                return idle(Some(End::Gone { shots: 0, last: 0 }));
            }
            if !tick.calm || !tick.ready {
                return idle(Some(End::GaveUp));
            }
            let waited = tick.now.saturating_sub(since);
            if tick.mate_firing {
                // Hold for the mate for a while, then leave it to them.
                return if waited > t.hold_ms {
                    idle(Some(End::HeldForMate))
                } else {
                    (
                        state,
                        Out {
                            look: looking(at),
                            ..Out::default()
                        },
                    )
                };
            }
            if waited > t.aim_timeout_ms {
                return idle(Some(End::GaveUp));
            }
            if waited >= t.aim_settle_ms && tick.err_rad <= t.aim_ok_rad {
                // On it: the first shot goes this tick.
                return (
                    Reaction::Shoot {
                        key,
                        at,
                        since: tick.now,
                        shots: 1,
                        last: tick.now,
                        down: true,
                    },
                    Out {
                        look: looking(at),
                        fire: true,
                        shot: true,
                        ..Out::default()
                    },
                );
            }
            (
                state,
                Out {
                    look: looking(at),
                    ..Out::default()
                },
            )
        }
        Reaction::Shoot {
            key,
            at,
            since,
            shots,
            last,
            down,
        } => {
            if !armed {
                return idle(Some(End::Gone { shots, last }));
            }
            if !tick.calm || !tick.ready {
                return idle(Some(End::GaveUp));
            }
            if tick.mate_firing {
                return idle(Some(End::HeldForMate));
            }
            let spent = tick.now.saturating_sub(since) as f32 / 1000.0;
            if (shots >= t.max_shots && !down) || spent > t.max_shoot_s {
                return idle(Some(End::GaveUp));
            }
            let look = looking(at);
            if down {
                // The key goes up the tick after it went down.
                return (
                    Reaction::Shoot {
                        key,
                        at,
                        since,
                        shots,
                        last,
                        down: false,
                    },
                    Out {
                        look,
                        ..Out::default()
                    },
                );
            }
            if tick.now.saturating_sub(last) >= t.shot_gap_ms && tick.err_rad <= t.aim_ok_rad * 2.0
            {
                return (
                    Reaction::Shoot {
                        key,
                        at,
                        since,
                        shots: shots + 1,
                        last: tick.now,
                        down: true,
                    },
                    Out {
                        look,
                        fire: true,
                        shot: true,
                        ..Out::default()
                    },
                );
            }
            (
                state,
                Out {
                    look,
                    ..Out::default()
                },
            )
        }
    }
}

/// Whether a mate of the bot is seen shooting at the trap: a shot sound of
/// a mate within `claim_ms`, the mate within `claim_m` of the bot or of the
/// trap, and its view within `claim_yaw_deg` of the trap's bearing. Only
/// what a player sees and hears.
pub fn mate_firing(
    sounds: &[Sound],
    mates: &[&PlaceView],
    me: Place,
    trap: Vec3,
    me_at: Vec3,
    now: u32,
    t: &ReactTuning,
) -> bool {
    sounds.iter().any(|sound| {
        sound.kind == SoundKind::Shot
            && sound.from != me
            && now.saturating_sub(sound.time) <= t.claim_ms
            && mates.iter().any(|mate| {
                mate.place == sound.from
                    && (mate.feet.distance_xz(me_at) <= t.claim_m
                        || mate.feet.distance_xz(trap) <= t.claim_m)
                    && in_view(mate.eye(), mate.yaw, trap, t.claim_yaw_deg)
            })
    })
}

/// Whether a mate stands beside the trap looking at it: a hand defuse in
/// hand, as a player sees it. Within [`BESIDE_M`] of the trap, slower than
/// [`BESIDE_SPEED`], the view within `claim_yaw_deg` of the trap.
pub fn mate_beside(mates: &[&PlaceView], trap: Vec3, t: &ReactTuning) -> bool {
    mates.iter().any(|mate| {
        mate.feet.distance_xz(trap) <= BESIDE_M
            && mate.speed() <= BESIDE_SPEED
            && in_view(mate.eye(), mate.yaw, trap, t.claim_yaw_deg)
    })
}

/// The angle between the view (`yaw`, `pitch`) and the point seen from `eye`.
pub fn view_error(eye: Vec3, yaw: f32, pitch: f32, point: Vec3) -> f32 {
    let (dx, dz) = (point.x - eye.x, point.z - eye.z);
    let want_yaw = super::kin::yaw_of([dx, dz]);
    let want_pitch = (point.y - eye.y).atan2(dx.hypot(dz));
    let wrap = |a: f32| {
        let tau = std::f32::consts::TAU;
        (a + std::f32::consts::PI).rem_euclid(tau) - std::f32::consts::PI
    };
    wrap(want_yaw - yaw).hypot(want_pitch - pitch)
}

/// The armed traps of a world (those not fired or disarmed).
pub fn armed_of(found: &[PlacedTrap]) -> Vec<PlacedTrap> {
    found
        .iter()
        .filter(|trap| trap.state == TrapState::Armed)
        .copied()
        .collect()
}

/// A trap this near a standing trap of the bot's team is that trap, metres.
const OWN_M: f32 = 1.0;
/// Hit this lately, ms, or an enemy seen: the bot is in a fight.
const FIGHT_HIT_MS: u32 = 3_000;
/// A bot hurt below this health leaves the trap alone.
const SHOOT_HEALTH: f32 = 0.4;
/// A shooter is remembered this long, ms (a second one starting on the same
/// trap within it is a double shooter).
const SHOOTER_KEEP_MS: u32 = 1_500;

/// A think of a bot: its pass against the armed traps of others, the roll
/// for the nearest one in range, and the start of a shot at a noticed one.
pub(super) fn think(bot: &mut Bot, due: &Due, state: &mut TeamState) {
    let t = &due.tuning.traps.react;
    let me = due.me;
    if !t.on || !me.alive {
        return;
    }
    let own: Vec<Vec3> = state.knowledge.traps.standing().map(|k| k.at).collect();
    let others: Vec<(PlacedTrap, TrapKey, Vec3)> = state
        .features.traps.world_traps
        .iter()
        .map(|trap| {
            (
                *trap,
                TrapKey::of(trap),
                Vec3::new(trap.position[0], trap.position[1], trap.position[2]),
            )
        })
        .filter(|(_, _, at)| !own.iter().any(|mine| mine.distance_xz(*at) < OWN_M))
        .collect();
    let keyed: Vec<(TrapKey, Vec3)> = others.iter().map(|&(_, key, at)| (key, at)).collect();
    bot.features.traps.react.prune(&keyed);
    bot.features.traps.react.walk(due.now, me.feet, &keyed);
    if let Some((key, _)) = bot.features.traps.react.reaction.target() {
        bot.features.traps.react.target_armed = keyed.iter().any(|(k, _)| *k == key);
        bot.features.traps.react.register(key, bot.place, due.now, state);
    }
    let fighting = !bot.memory.all_seen(due.now, due.view.clock).is_empty()
        || bot
            .hurt
            .last_hit_ms
            .is_some_and(|hit| due.now < hit.saturating_add(FIGHT_HIT_MS));
    let nearest = others
        .iter()
        .map(|(trap, _, at)| (trap, me.feet.distance_xz(*at)))
        .filter(|&(_, d)| d < t.far_m)
        .min_by(|a, b| a.1.total_cmp(&b.1));
    if let Some((trap, _)) = nearest {
        let look = Look {
            now: due.now,
            feet: me.feet,
            eye: me.eye(),
            yaw: me.yaw,
            fighting,
        };
        let rng = &mut bot.rng;
        let veil = due.view.smoke.veil(due.view.clock, &due.tuning.smoke);
        bot.features.traps.react.roll(
            &look,
            trap,
            t,
            || rng.unit(),
            // Smoke hides a trap as it hides a body (`smoke.rs`).
            |from, to| !veil.hides(from, to) && sight_line(&due.tactics.world.sight, from, to),
        );
    }
    if t.shot && bot.features.traps.react.reaction == Reaction::Idle && !fighting {
        start_shot(bot, due, &others);
    }
    if t.defuse && bot.features.traps.react.reaction == Reaction::Idle && !fighting {
        start_defuse(bot, due, &others);
    }
    refresh_private(bot, due, &others);
}

/// Whether to set out to defuse the nearest noticed trap by hand: not shot
/// at (the shot is off, or was not started above), within reach of a walk,
/// the bot unhurt and carrying nothing, no mate shooting at it or standing
/// beside it, a stand on its own side with a floor, and a roll won (once a
/// trap and a life).
fn start_defuse(bot: &mut Bot, due: &Due, others: &[(PlacedTrap, TrapKey, Vec3)]) {
    let t = &due.tuning.traps.react;
    let me = due.me;
    if me.carrying.is_some() || me.health < SHOOT_HEALTH {
        return;
    }
    let react = &bot.features.traps.react;
    let target = others
        .iter()
        .filter(|(_, key, _)| react.noticed(*key, due.now) && !react.tried_defuse.contains(key))
        .map(|&(_, key, at)| (key, at, me.feet.distance_xz(at)))
        .filter(|&(_, at, d)| d <= t.defuse_walk_m && (at.y - me.feet.y).abs() <= SAME_FLOOR_M)
        .min_by(|a, b| a.2.total_cmp(&b.2));
    let Some((key, at, d)) = target else {
        return;
    };
    bot.features.traps.react.tried_defuse.push(key);
    if bot.rng.unit() >= t.defuse_p {
        return;
    }
    let mates: Vec<&PlaceView> = due
        .view
        .team(me.team)
        .filter(|p| p.place != me.place)
        .collect();
    if mate_firing(&due.view.sounds, &mates, me.place, at, me.feet, due.now, t)
        || mate_beside(&mates, at, t)
    {
        return;
    }
    // The stand: on the bot's own side of the trap, so the approach does not
    // cross it.
    if d < 1e-3 {
        return;
    }
    let (dx, dz) = (me.feet.x - at.x, me.feet.z - at.z);
    let spot = Vec3::new(
        at.x + dx / d * t.defuse_stand_m,
        at.y,
        at.z + dz / d * t.defuse_stand_m,
    );
    if due.tactics.world.grid.locate(spot, 1.0).is_none() {
        return;
    }
    bot.features.traps.react.reaction = Reaction::Walk {
        key,
        at,
        spot,
        since: due.now,
    };
    bot.features.traps.react.target_armed = true;
    bot.features.traps.react.registered = None;
    bot.features.traps.react.counts.defuse_started += 1;
}

/// The traps the bot keeps off or jumps: the noticed, armed ones it is not
/// shooting. A route that runs over one that is new to the set is let go of,
/// to be searched again round it.
fn refresh_private(bot: &mut Bot, due: &Due, others: &[(PlacedTrap, TrapKey, Vec3)]) {
    let t = &due.tuning.traps.react;
    let target = bot.features.traps.react.reaction.target().map(|(key, _)| key);
    let want: Vec<(TrapKey, Vec3)> = if t.pass || t.jump {
        others
            .iter()
            .filter(|(_, key, _)| bot.features.traps.react.noticed(*key, due.now) && Some(*key) != target)
            .map(|&(_, key, at)| (key, at))
            .collect()
    } else {
        Vec::new()
    };
    if want == bot.features.traps.react.private {
        return;
    }
    bot.features.traps.react
        .jumped
        .retain(|key| want.iter().any(|(k, _)| k == key));
    let discs = want.iter().map(|&(_, at)| at).collect();
    bot.features.traps.react.private = want;
    bot.features.traps.react.field = Arc::new(TrapField::round(discs, &due.tactics.world.grid));
    if t.pass && !bot.features.traps.react.field.is_empty() {
        let field = bot.features.traps.react.field.clone();
        if super::traps::route_hits(bot, &field, due.me.feet) {
            bot.end_route(RouteEnd::TrapAhead, due.now);
        }
    }
}

/// A bot slower than this does not jump a trap, m/s (the jump carries it
/// over by its run).
const JUMP_MIN_SPEED: f32 = 1.5;
/// The jump is not pressed closer than this to the trap, metres along the
/// route.
const JUMP_LATEST_M: f32 = 0.3;

/// Whether to press the jump this tick for a noticed trap on the route: the
/// trap's closest point on the route lies `jump_before_m` to
/// [`JUMP_LATEST_M`] ahead and within [`FIELD_M`] of it. Once a trap.
pub(super) fn jump_due(bot: &mut Bot, me: &PlaceView, now: u32, t: &ReactTuning) -> bool {
    if !t.on
        || !t.jump
        || bot.features.traps.react.private.is_empty()
        || bot.jump_hold > 0
        || me.speed() < JUMP_MIN_SPEED
        || !bot.hurt.may_jump(now)
    {
        return false;
    }
    let reach = t.jump_before_m + 1.0;
    let near: Vec<(TrapKey, Vec3)> = bot
        .features
        .traps
        .react
        .private
        .iter()
        .filter(|(key, at)| {
            !bot.features.traps.react.jumped.contains(key)
                && at.distance_xz(me.feet) < reach
                && (at.y - me.feet.y).abs() < SAME_FLOOR_M
        })
        .copied()
        .collect();
    if near.is_empty() {
        return false;
    }
    let Some((route, _)) = bot.route.as_ref() else {
        return false;
    };
    // A copy: the motor's own progress along the route is not touched.
    let mut route = route.clone();
    let (s, _) = route.progress(me.feet);
    for (key, at) in near {
        let mut best = (f32::INFINITY, s);
        let mut along = s;
        while along <= (s + reach).min(route.length()) {
            let d = route.point_at(along).distance_xz(at);
            if d < best.0 {
                best = (d, along);
            }
            along += 0.1;
        }
        let ahead = best.1 - s;
        if best.0 < FIELD_M && (JUMP_LATEST_M..=t.jump_before_m).contains(&ahead) {
            bot.features.traps.react.jumped.push(key);
            bot.features.traps.react.counts.trap_jumped += 1;
            return true;
        }
    }
    false
}

impl React {
    /// A bot that has begun shooting is put in the team's record; a second
    /// one starting on the same trap soon after is counted.
    fn register(&mut self, key: TrapKey, place: Place, now: u32, state: &mut TeamState) {
        if !matches!(
            self.reaction,
            Reaction::Shoot { .. } | Reaction::Defuse { .. }
        ) || self.registered == Some(key)
        {
            return;
        }
        self.registered = Some(key);
        state
            .features
            .traps
            .shooters
            .retain(|&(_, _, at)| now.saturating_sub(at) <= SHOOTER_KEEP_MS);
        if state
            .features
            .traps
            .shooters
            .iter()
            .any(|&(other, who, _)| other == key && who != place)
        {
            self.counts.double_shooters += 1;
        }
        state.features.traps.shooters.push((key, place, now));
    }
}

/// Whether to begin a shot at the nearest noticed trap in reach: a clear
/// line, the weapon ready, the bot unhurt and carrying nothing, and no mate
/// already at it.
fn start_shot(bot: &mut Bot, due: &Due, others: &[(PlacedTrap, TrapKey, Vec3)]) {
    let t = &due.tuning.traps.react;
    let me = due.me;
    if me.carrying.is_some() || me.health < SHOOT_HEALTH || !weapon_ready(bot, due.now) {
        return;
    }
    let react = &bot.features.traps.react;
    let target = others
        .iter()
        .filter(|(_, key, _)| react.noticed(*key, due.now) && !react.tried.contains(key))
        .map(|&(_, key, at)| (key, at, me.feet.distance_xz(at)))
        .filter(|&(_, _, d)| d <= t.shoot_m)
        .min_by(|a, b| a.2.total_cmp(&b.2));
    let Some((key, at, _)) = target else {
        return;
    };
    let centre = at + Vec3::new(0.0, CENTRE_UP, 0.0);
    let mates: Vec<&PlaceView> = due
        .view
        .team(me.team)
        .filter(|p| p.place != me.place)
        .collect();
    if mate_firing(&due.view.sounds, &mates, me.place, at, me.feet, due.now, t)
        || due.view.smoke.covers(due.view.clock, &due.tuning.smoke, me.eye(), centre)
        || !sight_line(&due.tactics.world.sight, me.eye(), centre)
    {
        return;
    }
    bot.features.traps.react.reaction = Reaction::Aim {
        key,
        at,
        since: due.now,
    };
    bot.features.traps.react.target_armed = true;
    bot.features.traps.react.registered = None;
    bot.features.traps.react.counts.shoot_started += 1;
}

/// The weapon in hand has a round in it and is not being reloaded.
fn weapon_ready(bot: &Bot, now: u32) -> bool {
    !bot.reflex.dry()
        && !bot
            .reflex
            .reload_note()
            .is_some_and(|reload| now < reload.until)
}

/// What the reaction asks of a drive tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct ReactDrive {
    /// Look at this point.
    pub look: Point,
    /// The keys to hold this tick (aim, and fire on the tick of a shot; USE
    /// at a defuse).
    pub keys: u32,
    /// Walk to this stand instead of standing still.
    pub go: Point,
    /// Crouch.
    pub crouch: bool,
}

type Point = Option<Vec3>;

/// A tick of the body: the reaction's step. `calm` is false when an enemy
/// is seen or the reflex aims at one.
pub(super) fn drive(
    bot: &mut Bot,
    me: &PlaceView,
    view: &super::view::MatchView,
    now: u32,
    calm: bool,
    t: &ReactTuning,
) -> Option<ReactDrive> {
    let (key, at) = bot.features.traps.react.reaction.target()?;
    let by_hand = bot.features.traps.react.reaction.by_hand();
    let centre = at + Vec3::new(0.0, CENTRE_UP, 0.0);
    let mates: Vec<&PlaceView> = view.team(me.team).filter(|p| p.place != me.place).collect();
    // A hit since the hand defuse began calls the bot away; the shot is not
    // so ended (its own rule above).
    let hit = by_hand
        && bot
            .features
            .traps
            .react
            .reaction
            .since()
            .is_some_and(|since| bot.hurt.last_hit_ms.is_some_and(|hit| hit >= since));
    let spot = match bot.features.traps.react.reaction {
        Reaction::Walk { spot, .. } => me.feet.distance_xz(spot),
        _ => 0.0,
    };
    let tick = Tick {
        now,
        err_rad: view_error(me.eye(), me.yaw, me.pitch, centre),
        calm: calm && me.carrying.is_none() && !hit,
        ready: weapon_ready(bot, now),
        mate_firing: mate_firing(&view.sounds, &mates, me.place, at, me.feet, now, t)
            || (by_hand && mate_beside(&mates, at, t)),
        to_spot_m: spot,
        eye_m: (centre - me.eye()).length(),
    };
    let (state, out) = step(bot.features.traps.react.reaction, tick, bot.features.traps.react.target_armed, t);
    bot.features.traps.react.reaction = state;
    if out.shot {
        bot.features.traps.react.counts.shots_fired += 1;
    }
    match out.ended {
        Some(End::GaveUp) if by_hand => bot.features.traps.react.counts.defuse_gave_up += 1,
        Some(End::HeldForMate) if by_hand => bot.features.traps.react.counts.held_for_mate += 1,
        Some(End::GaveUp) => {
            bot.features.traps.react.counts.gave_up += 1;
            bot.features.traps.react.tried.push(key);
        }
        Some(End::HeldForMate) => {
            bot.features.traps.react.counts.held_for_mate += 1;
            bot.features.traps.react.tried.push(key);
        }
        Some(End::Defused { held_ms }) if held_ms >= DEFUSE_CREDIT_MS => {
            bot.features.traps.react.counts.disarmed_by_hand += 1;
        }
        Some(End::Gone { shots, last })
            if shots > 0 && now.saturating_sub(last) <= SHOT_CREDIT_MS =>
        {
            bot.features.traps.react.counts.disarmed_by_shot += 1;
        }
        Some(End::Gone { .. } | End::Defused { .. }) | None => {}
    }
    if out.look.is_none() && out.go.is_none() {
        return None;
    }
    let mut keys = 0;
    if matches!(state, Reaction::Aim { .. } | Reaction::Shoot { .. }) {
        keys |= crate::v026g::drive::key::AIM;
    }
    if out.fire {
        keys |= crate::v026g::drive::key::FIRE;
    }
    if out.use_key {
        keys |= crate::v026g::drive::key::USE;
    }
    Some(ReactDrive {
        look: out.look,
        keys,
        go: out.go,
        crouch: out.crouch,
    })
}
