//! Client-test steps of the cover package (PR #705: bullet penetration, the pierce-aware cover; `an-sight.md` section 6, `fx-cover-client-tests.md`
//! T-PEN-1, T-PEN-3, T-GRASS, T-SPOT).
//!
//! The owner shoots, hides and walks in the open on School; the steps read the server's own view: the hits on a bot are its body pool falling
//! while he fired, the hits on him are his pool falling, and the **predicted share** of each is the map's bullet mesh (`Frame::damage_share`: the
//! client's penetration rule, the materials of the format 5 map data) from the shooter's eye to the target's chest, for the pierce of the
//! weapon the step names (AK-12 class 0.40, SR-2M class 0.275). The runner cannot place a dummy behind a named object, so the steps use the
//! enemy bots the match gives and the owner's own choice of cover (crates, a car, sandbags, a brick wall); the owner says which weapon he used
//! (ASK OWNER in the sheet). Where the map is not built a share is none and the hit is not classed.

use super::*;
use crate::v026g::bots::view::Place;
use crate::v026g::drive::key;
use mapdata::Vec3;

const SCHOOL: u8 = 2;
const EYE_STAND_M: f32 = 1.6;
const EYE_CROUCH_M: f32 = 1.1;
const CHEST_M: f32 = 1.2;
/// AK-12's bullet pierce (`bullet_pierce` times `k_arp`) and the lowest of the weapons bots meet (SR-2M).
const PIERCE_AK: f32 = 0.40;
const PIERCE_LOW: f32 = 0.275;
/// A hit is the owner's when he fired within this of it (the world is a moment behind his keys), ms, and from within this range, m.
const FIRED_WITHIN_MS: u32 = 1_500;
const HIT_RANGE_M: f32 = 80.0;
/// A share this high is a clear line, this low a stopped bullet.
const FULL_SHARE: f32 = 0.95;
const STOPPED_SHARE: f32 = 0.02;
/// The owner stands still under this speed, m/s, and a bot sees a clear line to him within this range, m.
const STILL_MPS: f32 = 0.3;
const SEE_M: f32 = 50.0;

fn eye_of(body: &Body) -> Vec3 {
    body.feet + Vec3::new(0.0, if body.crouched { EYE_CROUCH_M } else { EYE_STAND_M }, 0.0)
}

fn chest_of(body: &Body) -> Vec3 {
    body.feet + Vec3::new(0.0, CHEST_M, 0.0)
}

fn median(v: &mut [f64]) -> Option<f64> {
    v.sort_by(f64::total_cmp);
    (!v.is_empty()).then(|| v[v.len() / 2])
}

/// The hits the owner made on enemy bots through whatever stood between, by the predicted share of the line. Names: `hits`, `full_hits` and
/// `mean_drop_full` (share 0.95 or more: the body pool lost per hit), `partial_hits`, `mean_drop_partial`, `mean_share_partial`,
/// `blocked_hits` (a hit on a line the rule says stops the bullet: share 0.02 or less) and `mean_drop_blocked`, `partial_over_full` (the
/// mean drop through cover over the mean drop in the clear: to compare with the mean share of the partial ones; both are of different
/// ranges and parts, so only the order of magnitude is read).
#[derive(Debug)]
struct PenMeter {
    pierce: f32,
    health: Vec<(Place, f32)>,
    fired_at: Option<At>,
    full: Vec<f64>,
    partial: Vec<(f64, f64)>,
    blocked: Vec<f64>,
}

impl PenMeter {
    fn new(pierce: f32) -> Self {
        Self { pierce, health: Vec::new(), fired_at: None, full: Vec::new(), partial: Vec::new(), blocked: Vec::new() }
    }
}

impl CustomMeter for PenMeter {
    fn sample(&mut self, frame: &Frame) {
        let Some(owner) = frame.owner.as_ref() else {
            return;
        };
        if owner.body.keys & key::FIRE != 0 {
            self.fired_at = Some(frame.clock);
        }
        let fired = self.fired_at.is_some_and(|at| frame.clock.0.saturating_sub(at.0) <= FIRED_WITHIN_MS);
        for bot in frame.side(Side::Enemies) {
            let was = self.health.iter().find(|(p, _)| *p == bot.body.place).map(|(_, h)| *h);
            if let Some(was) = was
                && fired
                && bot.body.health < was - 1e-4
                && bot.body.feet.distance_xz(owner.body.feet) <= HIT_RANGE_M
                && let Some(share) = frame.damage_share(eye_of(&owner.body), chest_of(&bot.body), self.pierce)
            {
                let drop = f64::from(was - bot.body.health);
                if share >= FULL_SHARE {
                    self.full.push(drop);
                } else if share <= STOPPED_SHARE {
                    self.blocked.push(drop);
                } else {
                    self.partial.push((drop, f64::from(share)));
                }
            }
            match self.health.iter_mut().find(|(p, _)| *p == bot.body.place) {
                Some(entry) => entry.1 = bot.body.health,
                None => self.health.push((bot.body.place, bot.body.health)),
            }
        }
    }

    fn value(&self, n: usize) -> Option<f64> {
        let mean = |v: &[f64]| (!v.is_empty()).then(|| v.iter().sum::<f64>() / v.len() as f64);
        let partial_drops: Vec<f64> = self.partial.iter().map(|p| p.0).collect();
        let partial_shares: Vec<f64> = self.partial.iter().map(|p| p.1).collect();
        match n {
            0 => Some((self.full.len() + self.partial.len() + self.blocked.len()) as f64),
            1 => Some(self.full.len() as f64),
            2 => mean(&self.full),
            3 => Some(self.partial.len() as f64),
            4 => mean(&partial_drops),
            5 => mean(&partial_shares),
            6 => Some(self.blocked.len() as f64),
            7 => mean(&self.blocked),
            8 => Some(mean(&partial_drops)? / mean(&self.full)?),
            _ => None,
        }
    }
}

const PEN_NAMES: &[&str] = &[
    "hits",
    "full_hits",
    "mean_drop_full",
    "partial_hits",
    "mean_drop_partial",
    "mean_share_partial",
    "blocked_hits",
    "mean_drop_blocked",
    "partial_over_full",
];

fn pen_ak(_: &TryStart) -> Box<dyn CustomMeter> {
    Box::new(PenMeter::new(PIERCE_AK))
}

fn pen_low(_: &TryStart) -> Box<dyn CustomMeter> {
    Box::new(PenMeter::new(PIERCE_LOW))
}

/// The hits on the owner while he stands still (T-PEN-3): each by the predicted share from the bot that was aiming at him and firing, for
/// pierce 0.40. Names: `still_s`, `still_hits`, `hits_share_low` (the line is stopped or nearly: a hit there is a drift shot of the bot's
/// reckoning, an-sight cause 2), `hits_share_high`, `unattributed` (a drop with no firing bot aimed at him).
#[derive(Debug, Default)]
struct StillMeter {
    last: Option<(At, f32, bool)>,
    still_ms: u32,
    hits: u32,
    low: u32,
    high: u32,
    unattributed: u32,
}

impl StillMeter {
    fn make(_: &TryStart) -> Box<dyn CustomMeter> {
        Box::new(Self::default())
    }
}

impl CustomMeter for StillMeter {
    fn sample(&mut self, frame: &Frame) {
        let Some(owner) = frame.owner.as_ref() else {
            return;
        };
        let still = owner.body.speed < STILL_MPS;
        if let Some((at, health, was_still)) = self.last
            && was_still
            && still
        {
            self.still_ms += frame.clock.0.saturating_sub(at.0);
            if owner.body.health < health - 1e-4 {
                self.hits += 1;
                let best = frame
                    .side(Side::Enemies)
                    .filter(|b| b.body.alive && b.mind.target == Some(owner.body.place) && b.body.keys & key::FIRE != 0)
                    .filter_map(|b| frame.damage_share(eye_of(&b.body), chest_of(&owner.body), PIERCE_AK))
                    .reduce(f32::max);
                match best {
                    Some(share) if share <= 0.1 => self.low += 1,
                    Some(_) => self.high += 1,
                    None => self.unattributed += 1,
                }
            }
        }
        self.last = Some((frame.clock, owner.body.health, still));
    }

    fn value(&self, n: usize) -> Option<f64> {
        self.last?;
        match n {
            0 => Some(f64::from(self.still_ms) / 1_000.0),
            1 => Some(f64::from(self.hits)),
            2 => Some(f64::from(self.low)),
            3 => Some(f64::from(self.high)),
            4 => Some(f64::from(self.unattributed)),
            _ => None,
        }
    }
}

/// T-SPOT: the time from an enemy bot first having a clear line to the owner within 50 m to its first fire key. Names: `spots` (bots that
/// fired after a clear line), `median_near_ms` (the line first clear within 30 m), `median_far_ms` (farther), `fastest_ms`. The time
/// includes the bot's turn to him; it is not the notice time alone.
#[derive(Debug, Default)]
struct SpotMeter {
    clear_since: Vec<(Place, At, bool)>,
    near: Vec<f64>,
    far: Vec<f64>,
    done: Vec<Place>,
    seen: bool,
}

impl SpotMeter {
    fn make(_: &TryStart) -> Box<dyn CustomMeter> {
        Box::new(Self::default())
    }
}

impl CustomMeter for SpotMeter {
    fn sample(&mut self, frame: &Frame) {
        let Some(owner) = frame.owner.as_ref().filter(|o| o.body.alive) else {
            return;
        };
        self.seen = true;
        for bot in frame.side(Side::Enemies).filter(|b| b.body.alive) {
            let dist = bot.body.feet.distance_xz(owner.body.feet);
            let clear = dist <= SEE_M && frame.clear_line(eye_of(&bot.body), chest_of(&owner.body)).unwrap_or(true);
            let place = bot.body.place;
            let index = self.clear_since.iter().position(|(p, _, _)| *p == place);
            match (clear, index) {
                (true, None) if !self.done.contains(&place) => self.clear_since.push((place, frame.clock, dist <= 30.0)),
                (true, Some(i)) if bot.body.keys & key::FIRE != 0 => {
                    let (_, since, near) = self.clear_since.remove(i);
                    let ms = f64::from(frame.clock.0.saturating_sub(since.0));
                    if near { self.near.push(ms) } else { self.far.push(ms) }
                    self.done.push(place);
                }
                (false, Some(i)) => {
                    self.clear_since.remove(i);
                }
                _ => {}
            }
            if !clear {
                self.done.retain(|p| *p != place);
            }
        }
    }

    fn value(&self, n: usize) -> Option<f64> {
        if !self.seen {
            return None;
        }
        match n {
            0 => Some((self.near.len() + self.far.len()) as f64),
            1 => median(&mut self.near.clone()),
            2 => median(&mut self.far.clone()),
            3 => self.near.iter().chain(&self.far).copied().reduce(f64::min),
            _ => None,
        }
    }
}

/// T-GRASS: how long he lasts hidden and still. Names: `first_shot_s` (from the start to the first frame an enemy bot with a clear line
/// to him within 50 m is aimed at him and fires; none: nobody did), `exposed_bot_s` (bot-seconds of enemy bots with a clear line within 50 m
/// before that; the grass file's metres are not in the frame, so the sheet marks which spot is grass).
#[derive(Debug, Default)]
struct GrassMeter {
    first: Option<At>,
    last: Option<At>,
    shot: Option<At>,
    exposed_ms: f64,
}

impl GrassMeter {
    fn make(_: &TryStart) -> Box<dyn CustomMeter> {
        Box::new(Self::default())
    }
}

impl CustomMeter for GrassMeter {
    fn sample(&mut self, frame: &Frame) {
        let Some(owner) = frame.owner.as_ref().filter(|o| o.body.alive) else {
            return;
        };
        let dt = f64::from(self.last.map_or(0, |at| frame.clock.0.saturating_sub(at.0)));
        self.first.get_or_insert(frame.clock);
        self.last = Some(frame.clock);
        for bot in frame.side(Side::Enemies).filter(|b| b.body.alive) {
            let clear = bot.body.feet.distance_xz(owner.body.feet) <= SEE_M && frame.clear_line(eye_of(&bot.body), chest_of(&owner.body)).unwrap_or(true);
            if clear && self.shot.is_none() {
                self.exposed_ms += dt;
                if bot.mind.target == Some(owner.body.place) && bot.body.keys & key::FIRE != 0 {
                    self.shot = Some(frame.clock);
                }
            }
        }
    }

    fn value(&self, n: usize) -> Option<f64> {
        let first = self.first?;
        match n {
            0 => self.shot.map(|at| f64::from(at.0.saturating_sub(first.0)) / 1_000.0),
            1 => Some(self.exposed_ms / 1_000.0),
            _ => None,
        }
    }
}

fn owner_still(frame: &Frame, _since: Span) -> bool {
    frame.owner.as_ref().is_some_and(|o| o.body.alive && o.body.speed < STILL_MPS)
}

fn owner_still_crouched(frame: &Frame, since: Span) -> bool {
    owner_still(frame, since) && frame.owner.as_ref().is_some_and(|o| o.body.crouched)
}

pub fn steps() -> Vec<Step> {
    let school = || Group::new(SCHOOL, BATTERIES).of_size(10);
    vec![
        // an-sight 6.1: the penetration rule against the real client. Each hit is classed by the predicted share of its line.
        Step::new(
            "cover-pen-ak",
            school(),
            "With your AK-12 class rifle shoot enemy bots through cover: crates, a car, sandbags, a brick wall. About 10 hits.",
        )
        .start(&[Cond::After(Span::s(3))])
        .end(&[End::OwnerDied])
        .measure(Measure::Custom { names: PEN_NAMES, make: pen_ak })
        .measure(Measure::OwnerKills)
        // The rule says these lines stop the bullet: a hit on one is a wrong rule (or a wrong material), whichever it is.
        .bar("blocked_hits", Test::AtMost(0.0))
        .budget_s(180)
        .arm_s(180)
        .tries(1),
        Step::new(
            "cover-pen-low",
            school(),
            "Same with your lowest-penetration weapon (an SR-2M, a VSK or an SMG): shoot bots through the same kinds of cover. About 10 hits.",
        )
        .start(&[Cond::After(Span::s(3))])
        .end(&[End::OwnerDied])
        .measure(Measure::Custom { names: PEN_NAMES, make: pen_low })
        .measure(Measure::OwnerKills)
        .bar("blocked_hits", Test::AtMost(0.0))
        .budget_s(180)
        .arm_s(180)
        .tries(1),
        // an-sight 6.2: still behind cover. A hit on a still owner behind a stopped line is the bot's reckoning drifting.
        Step::new(
            "cover-still",
            school(),
            "Stand still, fully behind the crate stack in the corridor, with a bot lane 30-40 m off, for a minute. Do not move or shoot.",
        )
        .start(&[Cond::Custom(WaitFor { describe: "you stand still", holds: owner_still })])
        .end(&[End::OwnerDied])
        .measure(Measure::Custom { names: &["still_s", "still_hits", "hits_share_low", "hits_share_high", "unattributed"], make: StillMeter::make })
        .bar("hits_share_low", Test::AtMost(0.0))
        .budget_s(60)
        .arm_s(180)
        .tries(1),
        // an-sight 6.4: the spotting time in the open.
        Step::new(
            "cover-spotting",
            school(),
            "Walk from cover into the open in a bot's view, without firing: at about 20 m, then at about 40 m, three times each.",
        )
        .start(&[Cond::After(Span::s(3))])
        .end(&[End::OwnerDied])
        .measure(Measure::Custom { names: &["spots", "median_near_ms", "median_far_ms", "fastest_ms"], make: SpotMeter::make })
        .budget_s(150)
        .arm_s(180)
        .tries(1),
        // an-sight 6.3: grass, then the open. The same meter twice.
        Step::new(
            "cover-grass-in",
            school(),
            "Crouch still in tall grass 30-40 m from a bot lane, without firing, for 90 s; stop when you are shot.",
        )
        .start(&[Cond::Custom(WaitFor { describe: "you are crouched and still", holds: owner_still_crouched })])
        .end(&[End::OwnerDied])
        .measure(Measure::Custom { names: &["first_shot_s", "exposed_bot_s"], make: GrassMeter::make })
        .budget_s(90)
        .arm_s(180)
        .tries(1),
        Step::new(
            "cover-grass-open",
            school(),
            "Now crouch still in the open, the same distance from the same lane, without firing, for 90 s; stop when you are shot.",
        )
        .start(&[Cond::Custom(WaitFor { describe: "you are crouched and still", holds: owner_still_crouched })])
        .end(&[End::OwnerDied])
        .measure(Measure::Custom { names: &["first_shot_s", "exposed_bot_s"], make: GrassMeter::make })
        .budget_s(90)
        .arm_s(180)
        .tries(1),
    ]
}

#[cfg(test)]
mod tests {
    use super::super::dry;
    use super::*;
    use crate::v026g::bots::reflex::Gates;

    fn bot(place: Place, at: Vec3, health: f32, target: Option<Place>, keys: u32) -> BotView {
        let mut body = dry::body(place, 1, at);
        body.health = health;
        body.keys = keys;
        BotView { body, mind: Mind { target, gates: Gates::default(), legs: None }, role: None }
    }

    fn frame(ms: u32, owner_keys: u32, owner_health: f32, speed: f32, bots: Vec<BotView>) -> Frame {
        let mut body = dry::body(0, 0, Vec3::ZERO);
        body.keys = owner_keys;
        body.health = owner_health;
        body.speed = speed;
        let mut f = dry::frame_at(ms, body);
        f.bots = bots;
        f
    }

    fn start() -> TryStart {
        TryStart { started: At(0), kills: 0, deaths: 0 }
    }

    /// Without a built map a hit has no predicted share and is not classed (the meter counts nothing it cannot judge).
    #[test]
    fn a_hit_with_no_map_is_not_classed() {
        let mut meter = PenMeter::new(PIERCE_AK);
        meter.sample(&frame(0, key::FIRE, 1.0, 0.0, vec![bot(10, Vec3::new(20.0, 0.0, 0.0), 1.0, None, 0)]));
        meter.sample(&frame(500, 0, 1.0, 0.0, vec![bot(10, Vec3::new(20.0, 0.0, 0.0), 0.8, None, 0)]));
        assert_eq!(meter.value(0), Some(0.0));
        assert!(meter.value(2).is_none() && meter.value(8).is_none());
        assert_eq!(PEN_NAMES.len(), 9);
        assert!(damage_share_is_none(&frame(0, 0, 1.0, 0.0, Vec::new())));
    }

    fn damage_share_is_none(frame: &Frame) -> bool {
        frame.damage_share(Vec3::ZERO, Vec3::new(5.0, 0.0, 0.0), PIERCE_AK).is_none() && frame.damage_share(Vec3::ZERO, Vec3::new(5.0, 0.0, 0.0), 0.0).is_none()
    }

    /// A still owner hit with no firing bot aimed at him is unattributed; the still seconds count only while he stays still.
    #[test]
    fn the_still_meter_counts_hits_while_still() {
        let mut meter = StillMeter::make(&start());
        meter.sample(&frame(0, 0, 1.0, 0.0, Vec::new()));
        meter.sample(&frame(1_000, 0, 0.9, 0.0, Vec::new()));
        meter.sample(&frame(2_000, 0, 0.9, 2.0, Vec::new()));
        meter.sample(&frame(3_000, 0, 0.5, 2.0, Vec::new()));
        let v = |n| meter.value(n).unwrap();
        assert_eq!((v(0), v(1), v(4)), (1.0, 1.0, 1.0), "one still second, one hit while still (the one while moving does not count)");
        assert_eq!((v(2), v(3)), (0.0, 0.0));
    }

    /// The spot meter times a bot from the first clear line to its first fire key, near and far apart.
    #[test]
    fn the_spot_meter_times_the_first_shot_after_a_clear_line() {
        let mut meter = SpotMeter::make(&start());
        let near = |keys| bot(10, Vec3::new(20.0, 0.0, 0.0), 1.0, None, keys);
        let far = |keys| bot(11, Vec3::new(40.0, 0.0, 0.0), 1.0, None, keys);
        meter.sample(&frame(0, 0, 1.0, 2.0, vec![near(0), far(0)]));
        meter.sample(&frame(500, 0, 1.0, 2.0, vec![near(key::FIRE), far(0)]));
        meter.sample(&frame(1_200, 0, 1.0, 2.0, vec![near(key::FIRE), far(key::FIRE)]));
        let v = |n| meter.value(n).unwrap();
        assert_eq!((v(0), v(1), v(2), v(3)), (2.0, 500.0, 1_200.0, 500.0));
    }

    /// The grass meter: the first shot at him with a clear line, and the exposure before it.
    #[test]
    fn the_grass_meter_reads_the_first_shot() {
        let mut meter = GrassMeter::make(&start());
        let b = |keys, target| bot(10, Vec3::new(30.0, 0.0, 0.0), 1.0, target, keys);
        meter.sample(&frame(0, 0, 1.0, 0.0, vec![b(0, None)]));
        meter.sample(&frame(4_000, 0, 1.0, 0.0, vec![b(0, Some(0))]));
        meter.sample(&frame(6_000, 0, 1.0, 0.0, vec![b(key::FIRE, Some(0))]));
        assert_eq!((meter.value(0), meter.value(1)), (Some(6.0), Some(6.0)));
        assert!(GrassMeter::make(&start()).value(0).is_none());
    }

    /// The steps run through the real runner: the still step starts when he stands still and ends at its budget; an INFO row for the spot step.
    #[test]
    fn dry_run_the_still_and_spotting_steps() {
        let named = |id: &str| steps().into_iter().find(|s| s.id == id).unwrap().tries(1);
        let (results, said) = dry::run(named("cover-still"), 90, |ms| frame(ms, 0, 1.0, 0.0, Vec::new()));
        assert!(results.contains("cover-still") && results.contains("\tPASS\t") && results.contains("still_s="), "{results}");
        assert!(said.iter().any(|l| l.contains("crate stack")), "{said:?}");
        let (results, _) = dry::run(named("cover-spotting"), 200, |ms| {
            frame(ms, 0, 1.0, 2.0, vec![bot(10, Vec3::new(20.0, 0.0, 0.0), 1.0, None, if ms >= 5_000 { key::FIRE } else { 0 })])
        });
        assert!(results.contains("cover-spotting") && results.contains("\tINFO\t") && results.contains("spots=1"), "{results}");
    }
}
