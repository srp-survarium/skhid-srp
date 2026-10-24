//! The bench's count of how the bots fight (audit issues 4 and 8): where
//! their shots go, how soon they turn to an enemy close by, whether they
//! keep a far target with a near enemy in view, standoffs at arm's length,
//! and how often they churn their plays. Read from the stand-in game only
//! (bodies, keys, the target each input was made with, the shots), never
//! from the bots' own state, so before and after count alike.

use std::collections::BTreeMap;

use mapdata::Vec3;

use super::super::sense::in_view;
use super::super::view::Place;
use super::bench_spread::AimState;
use super::{Game, Note};
use crate::v026g::drive::key;

/// One shot as the stand-in fired it.
#[derive(Clone, Copy, Debug)]
pub(super) struct ShotFact {
    pub shooter: Place,
    pub tick: u32,
    /// The enemy it hit, if any.
    pub hit: Option<Place>,
    /// The shooter's target (as its input was made): its distance, metres,
    /// and how far the shot's line was off his chest, degrees.
    pub target: Option<(Place, f32, f32)>,
    /// Keys at the shot: a move key held (strafe included), down the
    /// sights, crouched.
    pub moving: bool,
    pub aimed: bool,
    pub crouched: bool,
    /// The aim's state and the shot's cone, degrees ([`super::bench_spread`]).
    pub aim_state: AimState,
    pub cone: f32,
}

/// Distance bands of the shot counts, metres (upper bounds).
const BANDS: [f32; 5] = [10.0, 20.0, 35.0, 60.0, f32::MAX];
/// The engage episodes' reach (`engage.py`: 40 m, a line, in the view).
const CONTACT_M: f32 = 40.0;
/// A close enemy: within this, any direction, a line.
const CLOSE_M: f32 = 8.0;
/// A standoff: within this with a line both ways for `STANDOFF_MS`, no hit
/// either way.
const STANDOFF_M: f32 = 5.0;
const STANDOFF_MS: u32 = 3_000;
/// Sampled this often, ms.
pub(super) const SAMPLE_MS: u32 = 200;
/// The view counts as on an enemy within this, degrees.
const ON_DEG: f32 = 15.0;
/// [PR C] The range measure's bins, metres: each this wide, the last one
/// open.
const RANGE_BIN_M: f32 = 10.0;
const RANGE_BINS: usize = 13;

fn range_bin(d: f32) -> usize {
    ((d / RANGE_BIN_M) as usize).min(RANGE_BINS - 1)
}

#[derive(Default, Clone, Copy)]
struct Count {
    shots: u32,
    hits: u32,
}

impl Count {
    fn add(&mut self, hit: bool) {
        self.shots += 1;
        self.hits += u32::from(hit);
    }

    fn text(&self) -> String {
        format!(
            "{}/{} {:.1}%",
            self.hits,
            self.shots,
            100.0 * f64::from(self.hits) / f64::from(self.shots.max(1))
        )
    }
}

#[derive(Default, Clone, Copy)]
struct Contact {
    /// Since when (ms), and whether the bot had him as its target / its
    /// view on him yet (ms after the start).
    since: u32,
    engaged: Option<u32>,
    /// Engage episode: aimed at an enemy at least 5 m farther meanwhile.
    farther: bool,
}

#[derive(Default)]
pub(super) struct FightTally {
    band: [Count; 5],
    /// [moving][aimed]
    stance: [[Count; 2]; 2],
    crouched: Count,
    /// By the aim's state: hip, aiming in, aimed in; and the cones' sum.
    aim: [Count; 3],
    cone_sum: f64,
    /// The first three shots at a target, and the rest.
    first: Count,
    later: Count,
    /// Shots with a target: the line within his body's half width, and
    /// the offs in degrees (for the median).
    on_body: Count,
    offs: Vec<f32>,
    last_target: BTreeMap<Place, (Place, u32)>,
    /// Engage episodes (`engage.py`): by (bot, enemy).
    contacts: BTreeMap<(Place, Place), Contact>,
    episodes: u32,
    farther: u32,
    never: u32,
    /// Close contacts: the times to engage (ms) and those never engaged.
    close: BTreeMap<(Place, Place), Contact>,
    close_times: Vec<u32>,
    close_never: u32,
    /// Standoffs: by the unordered pair, since when; the last hit between
    /// them.
    near: BTreeMap<(Place, Place), u32>,
    standoffs: u32,
    last_hit: BTreeMap<(Place, Place), u32>,
    /// Seen by an enemy within 30 m (his view, a line): samples, and those
    /// with the bot not facing him (over 45 degrees).
    exposed: u32,
    not_facing: u32,
    /// Plays: the last two takes of each place (what, when); ended within
    /// 1 s; A, B, A within 10 s.
    takes: BTreeMap<Place, Vec<(String, u32)>>,
    quick_ends: u32,
    flips: u32,
    next_sample: u32,
    /// [play 20] Grenades let go (the thrower, the place, the blast's
    /// time) not gone off yet; and the counts: thrown, held back for a
    /// teammate, and at the blasts the thrower's teammates within 3, 6 and
    /// 10 m of the place (the RGD-5's `splash_radius` is 10 m) and the
    /// thrower within 6 and 10 m. The stand-in has no grenades: the place
    /// thrown at stands for where it lands.
    blasts: Vec<(Place, Vec3, u32)>,
    grenades: [u32; 2],
    mates_near_blast: [u32; 3],
    thrower_near_blast: [u32; 2],
    /// [PR C] Each place's weapon class, team, feet and life as of the last
    /// step (a shot is counted before its step's sample).
    places: Vec<(&'static str, u8, Vec3, bool)>,
    /// Shots at the shooter's target by its weapon class, by the
    /// distance's [`range_bin`].
    by_class: BTreeMap<&'static str, [Count; RANGE_BINS]>,
    /// By team: the hits it took and its deaths, by the shooter's distance
    /// (a death at its last hit's).
    hits_taken: [[u32; RANGE_BINS]; 2],
    deaths_at: [[u32; RANGE_BINS]; 2],
    last_hit_m: BTreeMap<Place, f32>,
}

/// [play 20] "throws a grenade at (x, y, z), blast at <ms>": the place and
/// the time.
fn thrown_at(text: &str) -> Option<(Vec3, u32)> {
    let rest = text.strip_prefix("throws a grenade at (")?;
    let (xyz, rest) = rest.split_once("), blast at ")?;
    let mut v = xyz.split(", ").map(|n| n.parse::<f32>().ok());
    let at = Vec3::new(v.next()??, v.next()??, v.next()??);
    Some((at, rest.trim().parse().ok()?))
}

fn moving(keys: u32) -> bool {
    let fb = keys & (key::FORWARD | key::BACK);
    let lr = keys & (key::LEFT | key::RIGHT);
    (fb == key::FORWARD || fb == key::BACK) || (lr == key::LEFT || lr == key::RIGHT)
}

/// How far `body`'s view is off `point`, degrees (yaw and pitch, the
/// larger).
fn off_deg(eye: Vec3, yaw: f32, pitch: f32, point: Vec3) -> f32 {
    use super::super::kin::{wrap, yaw_of};
    let (dx, dz) = (point.x - eye.x, point.z - eye.z);
    let want_yaw = yaw_of([dx, dz]);
    let want_pitch = (point.y - eye.y).atan2(dx.hypot(dz));
    wrap(want_yaw - yaw)
        .abs()
        .max((want_pitch - pitch).abs())
        .to_degrees()
}

impl FightTally {
    pub(super) fn shot(&mut self, fact: ShotFact) {
        let hit = fact.hit.is_some();
        if let Some(victim) = fact.hit {
            self.last_hit
                .insert((fact.shooter.min(victim), fact.shooter.max(victim)), fact.tick);
        }
        self.stance[usize::from(fact.moving)][usize::from(fact.aimed)].add(hit);
        if fact.crouched {
            self.crouched.add(hit);
        }
        self.aim[fact.aim_state as usize].add(hit);
        self.cone_sum += f64::from(fact.cone);
        if let (Some(victim), Some(&(_, _, feet, _))) = (fact.hit, self.places.get(fact.shooter))
            && let Some(&(_, team, at, _)) = self.places.get(victim)
        {
            let d = feet.distance_xz(at);
            self.hits_taken[usize::from(team).min(1)][range_bin(d)] += 1;
            self.last_hit_m.insert(victim, d);
        }
        if let (Some((_, distance, _)), Some(&(class, ..))) = (fact.target, self.places.get(fact.shooter)) {
            self.by_class.entry(class).or_insert([Count::default(); RANGE_BINS])[range_bin(distance)].add(hit);
        }
        let Some((target, distance, off)) = fact.target else {
            return;
        };
        let band = BANDS.iter().position(|b| distance <= *b).unwrap_or(4);
        self.band[band].add(hit);
        let n = match self.last_target.get(&fact.shooter) {
            Some(&(t, n)) if t == target => n + 1,
            _ => 1,
        };
        self.last_target.insert(fact.shooter, (target, n));
        if n <= 3 {
            self.first.add(hit);
        } else {
            self.later.add(hit);
        }
        let half_deg = 0.25f32.atan2(distance.max(0.5)).to_degrees();
        self.on_body.add(off <= half_deg);
        self.offs.push(off);
    }

    pub(super) fn note(&mut self, note: &Note) {
        if let Some((at, blast)) = thrown_at(&note.text) {
            self.grenades[0] += 1;
            self.blasts.push((note.place, at, blast));
            return;
        }
        if note.text.starts_with("holds a grenade back") {
            self.grenades[1] += 1;
            return;
        }
        let Some(rest) = note.text.strip_prefix("takes ") else {
            return;
        };
        let what = rest.split(" (score").next().unwrap_or(rest).to_owned();
        let list = self.takes.entry(note.place).or_default();
        if let Some((_, at)) = list.last()
            && note.time < at + 1_000
        {
            self.quick_ends += 1;
        }
        if list.len() >= 2 {
            let (a, a_at) = &list[list.len() - 2];
            let (b, _) = &list[list.len() - 1];
            if *a == what && *b != what && note.time < a_at + 10_000 {
                self.flips += 1;
            }
        }
        list.push((what, note.time));
        if list.len() > 2 {
            list.remove(0);
        }
    }

    /// After the game's steps up to `clock`.
    pub(super) fn sample(&mut self, game: &Game, clock: u32) {
        // [PR C] Deaths by the last hit's distance; then the places.
        for (place, body) in game.bodies.iter().enumerate() {
            if let Some(&(_, team, _, true)) = self.places.get(place)
                && !body.alive
                && let Some(d) = self.last_hit_m.remove(&place)
            {
                self.deaths_at[usize::from(team).min(1)][range_bin(d)] += 1;
            }
        }
        self.places = game
            .bodies
            .iter()
            .map(|b| (b.gun.class(), b.team, b.feet, b.alive))
            .collect();
        // The grenades going off: who of the thrower's team is near.
        for (thrower, at, _) in self.blasts.iter().filter(|(_, _, blast)| clock >= *blast) {
            let Some(team) = game.bodies.get(*thrower).map(|b| b.team) else {
                continue;
            };
            for (place, body) in game.bodies.iter().enumerate() {
                if !body.alive || body.team != team {
                    continue;
                }
                let d = body.feet.distance_xz(*at);
                if place == *thrower {
                    self.thrower_near_blast[0] += u32::from(d <= 6.0);
                    self.thrower_near_blast[1] += u32::from(d <= 10.0);
                } else {
                    for (count, m) in self.mates_near_blast.iter_mut().zip([3.0, 6.0, 10.0]) {
                        *count += u32::from(d <= m);
                    }
                }
            }
        }
        self.blasts.retain(|(_, _, blast)| clock < *blast);
        if clock < self.next_sample {
            return;
        }
        self.next_sample = clock + SAMPLE_MS;
        let n = game.bodies.len();
        let sight = &game.world.sight;
        let mut line = vec![vec![None::<bool>; n]; n];
        let mut has_line = |a: Place, b: Place| -> bool {
            let (lo, hi) = (a.min(b), a.max(b));
            *line[lo][hi].get_or_insert_with(|| {
                let (x, y) = (&game.bodies[lo], &game.bodies[hi]);
                sight.visible(Game::eye(x), Game::chest(y)) || sight.visible(Game::eye(y), Game::chest(x))
            })
        };
        for b in 0..n {
            for e in 0..n {
                let (me, him) = (&game.bodies[b], &game.bodies[e]);
                if b == e || me.team == him.team {
                    continue;
                }
                let alive = me.alive && him.alive;
                let d = me.feet.distance_xz(him.feet);
                let pair = (b, e);
                // Engage episodes.
                let contact = alive
                    && d <= CONTACT_M
                    && in_view(me.feet, me.yaw, him.feet, 90.0)
                    && has_line(b, e);
                if contact {
                    if !self.contacts.contains_key(&pair) {
                        self.episodes += 1;
                    }
                    let c = self.contacts.entry(pair).or_insert(Contact {
                        since: clock,
                        ..Contact::default()
                    });
                    if me.target == Some(e) && c.engaged.is_none() {
                        c.engaged = Some(clock - c.since);
                    }
                    if let Some(o) = me.target.filter(|&o| o != e)
                        && game.bodies[o].alive
                        && game.bodies[o].team != me.team
                        && game.bodies[o].feet.distance_xz(me.feet) >= d + 5.0
                    {
                        c.farther = true;
                    }
                } else if let Some(c) = self.contacts.remove(&pair) {
                    self.farther += u32::from(c.farther);
                    self.never += u32::from(c.engaged.is_none());
                }
                // Close contacts, any direction.
                let close = alive && d <= CLOSE_M && has_line(b, e);
                if close {
                    let c = self.close.entry(pair).or_insert(Contact {
                        since: clock,
                        ..Contact::default()
                    });
                    let on = off_deg(Game::eye(me), me.yaw, me.recoil.view(me.pitch), Game::chest(him))
                        <= ON_DEG;
                    if (on || me.target == Some(e)) && c.engaged.is_none() {
                        c.engaged = Some(clock - c.since);
                        self.close_times.push(clock - c.since);
                    }
                } else if let Some(c) = self.close.remove(&pair) {
                    self.close_never += u32::from(c.engaged.is_none());
                }
                // Exposed: he has the bot in his view within 30 m.
                if alive && d <= 30.0 && in_view(him.feet, him.yaw, me.feet, 90.0) && has_line(b, e) {
                    self.exposed += 1;
                    if !in_view(me.feet, me.yaw, him.feet, 90.0) {
                        self.not_facing += 1;
                    }
                }
                // Standoffs: the unordered pair.
                if b < e {
                    let near = alive && d <= STANDOFF_M && has_line(b, e);
                    let key = (b, e);
                    if near {
                        let since = *self.near.entry(key).or_insert(clock);
                        let hit = self.last_hit.get(&key).is_some_and(|&t| t >= since);
                        if hit {
                            self.near.remove(&key);
                        } else if clock >= since + STANDOFF_MS {
                            self.standoffs += 1;
                            // Counted once: it starts again.
                            self.near.insert(key, clock + 1_000_000);
                        }
                    } else {
                        self.near.remove(&key);
                    }
                }
            }
        }
    }

    pub(super) fn line(&mut self, game: &Game) -> String {
        let (mut kills, mut deaths) = ([0u32; 2], [0u32; 2]);
        for (body, stats) in game.bodies.iter().zip(&game.stats) {
            let team = usize::from(body.team).min(1);
            kills[team] += stats.0;
            deaths[team] += stats.1;
        }
        for (_, c) in std::mem::take(&mut self.contacts) {
            self.farther += u32::from(c.farther);
            self.never += u32::from(c.engaged.is_none());
        }
        for (_, c) in std::mem::take(&mut self.close) {
            self.close_never += u32::from(c.engaged.is_none());
        }
        let mut times = self.close_times.clone();
        times.sort_unstable();
        let median = |v: &[u32]| v.get(v.len() / 2).copied().unwrap_or(0);
        let within = |ms: u32| times.iter().filter(|&&t| t <= ms).count();
        let close_n = times.len() as u32 + self.close_never;
        let mut offs = self.offs.clone();
        offs.sort_by(f32::total_cmp);
        let off_median = offs.get(offs.len() / 2).copied().unwrap_or(0.0);
        let bands: Vec<String> = self.band.iter().map(Count::text).collect();
        format!(
            "fight: episodes {} farther {} never {}; close {} engaged<=1s {} median_ms {} never {}; standoffs {}; exposed {} not_facing {}; quick_ends {} flips {}; team kills {:?} deaths {:?}\n  \
             shots: bands(<10,<20,<35,<60,60+) {}; still hip {} still aim {} moving hip {} moving aim {}; crouched {}; first3 {} later {}; line on body {} off median {:.1} deg; aim hip {} aiming-in {} aimed-in {} cone mean {:.2} deg\n  \
             grenades: thrown {} held back {}; at the blasts teammates within 3/6/10 m {} {} {}; the thrower within 6/10 m {} {}
               ranges ({} m bins): {}; hits taken by team {:?} {:?}; deaths by team {:?} {:?}; at the target {} shots, {} into a wall, {} with nothing of him showing",
            self.episodes,
            self.farther,
            self.never,
            close_n,
            within(1_000),
            median(&times),
            self.close_never,
            self.standoffs,
            self.exposed,
            self.not_facing,
            self.quick_ends,
            self.flips,
            kills,
            deaths,
            bands.join(" | "),
            self.stance[0][0].text(),
            self.stance[0][1].text(),
            self.stance[1][0].text(),
            self.stance[1][1].text(),
            self.crouched.text(),
            self.first.text(),
            self.later.text(),
            self.on_body.text(),
            off_median,
            self.aim[0].text(),
            self.aim[1].text(),
            self.aim[2].text(),
            self.cone_sum / f64::from(self.aim.iter().map(|c| c.shots).sum::<u32>().max(1)),
            self.grenades[0],
            self.grenades[1],
            self.mates_near_blast[0],
            self.mates_near_blast[1],
            self.mates_near_blast[2],
            self.thrower_near_blast[0],
            self.thrower_near_blast[1],
            RANGE_BIN_M,
            self.by_class
                .iter()
                .map(|(class, bins)| format!(
                    "{class} {}",
                    bins.iter().map(|c| format!("{}/{}", c.hits, c.shots)).collect::<Vec<_>>().join(" ")
                ))
                .collect::<Vec<_>>()
                .join(" | "),
            self.hits_taken[0],
            self.hits_taken[1],
            self.deaths_at[0],
            self.deaths_at[1],
            self.band.iter().map(|c| c.shots).sum::<u32>(),
            game.walled,
            game.hidden,
        )
    }
}

/// [`ShotFact`]'s keys part.
pub(super) fn stance(keys: u32) -> (bool, bool, bool) {
    (moving(keys), keys & key::AIM != 0, keys & key::CROUCH != 0)
}

/// [`ShotFact`]'s target part: his distance and the shot line's off from
/// his chest, degrees.
pub(super) fn aim_off(eye: Vec3, dir: Vec3, chest: Vec3) -> (f32, f32) {
    let to = chest - eye;
    let d = to.length().max(1e-3);
    let cos = (to.dot(dir) / d).clamp(-1.0, 1.0);
    (Vec3::new(to.x, 0.0, to.z).length(), cos.acos().to_degrees())
}

/// [measure] The gate tally: the ticks a bot had its view on an enemy
/// (the on-view episodes, `tests.rs` `OnView`) and fired no shot, by the
/// first fire gate that held it and by every gate that did
/// ([`super::super::reflex::gate`]). The one part of the bench read from the
/// bots' own state (the reflex's record of its tick); nothing decides by it.
#[derive(Default)]
pub(super) struct GateTally {
    pub ticks: u32,
    /// By [`GATES`]' index.
    first: Vec<u32>,
    every: Vec<u32>,
}

/// The tally's gates: the reflex's ([`super::super::reflex::gate::NAMES`]),
/// with three of the bench's own in their place in the order: the bot's
/// record is older than a tick (it did not run), it aimed at another
/// enemy than the one in its view, and no gate failed.
const STALE: &str = "not_run";
const OTHER: &str = "other_target";
const NONE: &str = "none";

fn gates() -> Vec<(u32, &'static str)> {
    use super::super::reflex::gate;
    let mut all = vec![(0, STALE)];
    for &(bit, name) in &gate::NAMES {
        all.push((bit, name));
        // The reflex's own gates are about its target: another enemy than
        // the one in view comes before them.
        if bit == gate::LOST {
            all.push((0, OTHER));
        }
    }
    all.push((0, NONE));
    all
}

impl GateTally {
    /// A tick on view of `enemy` with no shot, the bot's gates `gates` as of
    /// `now`.
    pub(super) fn tick(&mut self, gates: super::super::reflex::Gates, enemy: Place, now: u32) {
        use super::super::reflex::gate;
        let names = gates_names();
        self.first.resize(names.len(), 0);
        self.every.resize(names.len(), 0);
        self.ticks += 1;
        let stale = now > gates.at + 2 * super::TICK_MS;
        let other = !stale && gates.target.is_some_and(|t| t != enemy);
        // Lost a moment: only the enemy in view; another lost is none.
        let lost = gates.bits & gate::LOST != 0;
        let failed = |bit: u32, name: &str| match name {
            STALE => stale,
            OTHER => other && gates.bits & (gate::NO_TARGET | gate::LOST) == 0,
            NONE => !stale && !other && gates.bits == 0,
            _ if bit == gate::LOST => !stale && lost && !other,
            _ if bit == gate::NO_TARGET => !stale && (gates.bits & bit != 0 || (lost && other)),
            _ => !stale && gates.bits & bit != 0,
        };
        let mut first = true;
        for (i, &(bit, name)) in names.iter().enumerate() {
            if failed(bit, name) {
                self.every[i] += 1;
                if first {
                    self.first[i] += 1;
                    first = false;
                }
            }
        }
    }

    /// One line: the ticks, then each gate's share as the first and as any.
    pub(super) fn line(&self) -> String {
        let pct = |n: u32| 100.0 * f64::from(n) / f64::from(self.ticks.max(1));
        let names = gates_names();
        let part = |counts: &[u32]| {
            names
                .iter()
                .zip(counts)
                .filter(|(_, n)| **n > 0)
                .map(|((_, name), n)| format!("{name} {n} {:.1}%", pct(*n)))
                .collect::<Vec<_>>()
                .join(", ")
        };
        format!(
            "gates on view, no shot: {} ticks; first: {}; every: {}",
            self.ticks,
            part(&self.first),
            part(&self.every)
        )
    }
}

fn gates_names() -> &'static [(u32, &'static str)] {
    static NAMES: std::sync::OnceLock<Vec<(u32, &'static str)>> = std::sync::OnceLock::new();
    NAMES.get_or_init(gates)
}
