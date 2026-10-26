//! A bot and a trap that is not its team's: the chance to notice it, and what
//! the bots' passes by one come to (`docs/notes/bear_traps.md`, C18).
//!
//! **This change only notices and counts; no bot acts on a noticed trap yet**
//! (the shot, the step and the jump follow, each behind its own switch).
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

use mapdata::Vec3;
use survarium::v026g::match_msgs::traps::{PlacedTrap, TrapState};

use super::aimpoint::sight_line;
use super::sense::in_view;
use super::think::Due;
use super::tuning::ReactTuning;
use super::{Bot, TeamState};

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
}

impl ReactCounts {
    pub fn add(&mut self, other: &ReactCounts) {
        self.rolls += other.rolls;
        self.noticed += other.noticed;
        self.missed += other.missed;
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
}

impl React {
    pub fn counts(&self) -> ReactCounts {
        self.counts
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
                    self.finish(p);
                }
                self.passing = near.map(|(key, d)| Passing {
                    key,
                    class: self.class_of(key, now),
                    closest: d,
                });
            }
        }
    }

    fn finish(&mut self, p: Passing) {
        let k = p.class.index();
        self.counts.passes[k] += 1;
        self.counts.through[k] += u32::from(p.closest <= THROUGH_M);
    }

    /// A new life: a pass in the making ends with it, the rolls are forgotten.
    pub fn new_life(&mut self) {
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
            self.finish(p);
            self.passing = None;
        }
    }
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

/// A think of a bot: its pass against the armed traps of others, and the
/// roll for the nearest one in range. Nothing is done with the result yet.
pub(super) fn think(bot: &mut Bot, due: &Due, state: &TeamState) {
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
    let nearest = others
        .iter()
        .map(|(trap, _, at)| (trap, me.feet.distance_xz(*at)))
        .filter(|&(_, d)| d < t.far_m)
        .min_by(|a, b| a.1.total_cmp(&b.1));
    let Some((trap, _)) = nearest else {
        return;
    };
    let fighting = !bot.memory.all_seen(due.now).is_empty()
        || bot
            .hurt
            .last_hit_ms
            .is_some_and(|hit| due.now < hit.saturating_add(FIGHT_HIT_MS));
    let look = Look {
        now: due.now,
        feet: me.feet,
        eye: me.eye(),
        yaw: me.yaw,
        fighting,
    };
    let rng = &mut bot.rng;
    bot.features.traps.react.roll(
        &look,
        trap,
        t,
        || rng.unit(),
        |from, to| sight_line(&due.tactics.world.sight, from, to),
    );
}
