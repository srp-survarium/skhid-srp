//! Client-test steps of the fights package (PR #730: the fair read of a carrier, the finish of a hurt enemy, the range band; `an-fights.md`
//! section 6, `fx-fights-client-tests.md` F1-F3).
//!
//! F1 is run twice by the owner (COORD-BOARD 'Client tests'): the server with the shipped defaults, then with `SRP_BOT_TUNING` naming
//! `traces-20261028/fx-fights-parts-on.json` (the shelved worth, hurter-keep and push parts ON). The two result files tell the runs
//! apart by their `tuning` column; the bars below are the ON run's, and the OFF run is the baseline that is expected to miss them.
//!
//! What the steps read is the server's own view of the match: the enemy bots' reflex (`target`), their fire key, the map's line of sight
//! from each bot's eye to the owner's chest (until the map is built a line counts as clear), the owner's pools and the battery's state.
//! A step cannot place a bot, so F2's two holders and F3's "one bot at a time" are the bots the match gives; the numbers say how many
//! came.

use super::*;
use mapdata::Vec3;

use crate::v026g::bots::pools::Pools;
use crate::v026g::bots::reflex::LegsWhy;
use crate::v026g::bots::view::{BatteryState, Place};
use crate::v026g::drive::key;

const VECTOR_LABORATORY: u8 = 7;
const COLOGNE: u8 = 8;

/// Eye and chest heights over the feet, m: the line a shot takes from a bot to the owner.
const EYE_M: f32 = 1.6;
const CHEST_M: f32 = 1.2;
/// An enemy bot counts against the carrier within this range, m (`fx-fights-client-tests.md` F1), and "close" within this one.
const NEAR_M: f32 = 40.0;
const CLOSE_M: f32 = 25.0;
/// A pool this low is empty (the hand of an-fights 2a).
const POOL_ZERO: f32 = 0.005;
/// F2: the pain pool under which the owner is hurt, and how far an enemy still sees him in the open.
const HURT_PAIN: f32 = 0.3;
const SEES_M: f32 = 60.0;

/// The owner's place and chest, if he is in the match and alive.
fn owner_chest(frame: &Frame) -> Option<(Place, Vec3)> {
    let owner = frame.owner.as_ref().filter(|o| o.body.alive)?;
    Some((owner.body.place, owner.body.feet + Vec3::new(0.0, CHEST_M, 0.0)))
}

/// Whether an alive enemy bot has a clear line from its eye to the owner's chest within `range_m` (none: the map is not built, so clear).
fn sees_owner(frame: &Frame, bot: &BotView, chest: Vec3, range_m: f32) -> bool {
    bot.body.alive
        && bot.body.feet.distance_xz(chest) <= range_m
        && frame.clear_line(bot.body.feet + Vec3::new(0.0, EYE_M, 0.0), chest).unwrap_or(true)
}

fn carries(frame: &Frame) -> bool {
    let owner = frame.owner.as_ref().map(|o| o.body.place);
    frame.batteries.iter().any(|b| matches!(b, BatteryState::Carried(p) if *p == owner))
}

fn carrying(frame: &Frame, _since: Span) -> bool {
    carries(frame)
}

fn carry_over(frame: &Frame, _progress: &Progress) -> bool {
    !carries(frame)
}

/// F1: what the enemy bots did against the owner while he carried a battery. Seconds are bot-seconds (a bot counts for the time
/// between two frames while it holds). Names, by index: `carrier_clear_s` (enemy bots within 40 m with a clear line to him),
/// `carrier_targeted_s` (of those, the ones whose target is him), `carrier_clear_not_targeted_s`, `close_clear_s` and `close_share`
/// (within 25 m: the share of the clear bot-seconds that are on him), `first_aimed_s` (from the first frame to the first one in
/// which a clear enemy aims at him and fires), `hand_zero` (1: a hand pool reached zero) and `free_after_hand_zero` (1: after it the
/// battery was no longer in his hands and lay free; none when the hand never reached zero: settles an-fights 2a).
#[derive(Debug, Default)]
struct StealMeter {
    first: Option<At>,
    last: Option<At>,
    clear_ms: f64,
    targeted_ms: f64,
    close_clear_ms: f64,
    close_targeted_ms: f64,
    first_aimed: Option<At>,
    hand_zero: bool,
    free_after: bool,
}

impl StealMeter {
    fn make(_start: &TryStart) -> Box<dyn CustomMeter> {
        Box::new(Self::default())
    }
}

impl CustomMeter for StealMeter {
    fn sample(&mut self, frame: &Frame) {
        let Some((owner, chest)) = owner_chest(frame) else {
            return;
        };
        let dt = f64::from(self.last.map_or(0, |at| frame.clock.0.saturating_sub(at.0)));
        self.first.get_or_insert(frame.clock);
        self.last = Some(frame.clock);
        for bot in frame.side(Side::Enemies) {
            if !sees_owner(frame, bot, chest, NEAR_M) {
                continue;
            }
            let on_him = bot.mind.target == Some(owner);
            let close = bot.body.feet.distance_xz(chest) <= CLOSE_M;
            self.clear_ms += dt;
            self.targeted_ms += if on_him { dt } else { 0.0 };
            if close {
                self.close_clear_ms += dt;
                self.close_targeted_ms += if on_him { dt } else { 0.0 };
            }
            if on_him && bot.body.keys & key::FIRE != 0 {
                self.first_aimed.get_or_insert(frame.clock);
            }
        }
        if let Some(pools) = frame.owner.as_ref().and_then(|o| o.pools)
            && pools.hands.iter().any(|&h| h <= POOL_ZERO)
        {
            self.hand_zero = true;
        }
        if self.hand_zero && !carries(frame) && frame.batteries.iter().any(|b| matches!(b, BatteryState::Free(_))) {
            self.free_after = true;
        }
    }

    fn value(&self, n: usize) -> Option<f64> {
        let first = self.first?;
        Some(match n {
            0 => self.clear_ms / 1_000.0,
            1 => self.targeted_ms / 1_000.0,
            2 => (self.clear_ms - self.targeted_ms) / 1_000.0,
            3 => self.close_clear_ms / 1_000.0,
            4 if self.close_clear_ms > 0.0 => self.close_targeted_ms / self.close_clear_ms,
            4 => return None,
            5 => f64::from(self.first_aimed?.0.saturating_sub(first.0)) / 1_000.0,
            6 => f64::from(u8::from(self.hand_zero)),
            7 if self.hand_zero => f64::from(u8::from(self.free_after)),
            7 => return None,
            _ => return None,
        })
    }
}

fn steal_measure() -> Measure {
    Measure::Custom {
        names: &[
            "carrier_clear_s",
            "carrier_targeted_s",
            "carrier_clear_not_targeted_s",
            "close_clear_s",
            "close_share",
            "first_aimed_s",
            "hand_zero",
            "free_after_hand_zero",
        ],
        make: StealMeter::make,
    }
}

/// The owner is hurt (pain under 30 %) and no enemy within 60 m sees him: he has broken sight (F2's t0).
fn hurt_and_hidden(frame: &Frame, _since: Span) -> bool {
    let Some((_, chest)) = owner_chest(frame) else {
        return false;
    };
    let hurt = frame.owner.as_ref().and_then(|o| o.pools).is_some_and(|p| p.pain < HURT_PAIN);
    hurt && !frame.side(Side::Enemies).any(|b| sees_owner(frame, b, chest, SEES_M))
}

/// F2: what the enemy bots did after the owner, hurt, broke sight (the try starts at that moment). Names: `first_push_s` (the first
/// note "pushes on hurt enemy" after the start, s; none: no push), `nearest_enemy_8s_m` and `reached_10m_by_8s` (the nearest an enemy
/// came to him in the first 8 s, m, and 1 when that is within 10 m), `hurt_alive_15s` (1: at the end he is alive and still hurt).
#[derive(Debug, Default)]
struct HurtMeter {
    first: Option<At>,
    first_push: Option<At>,
    nearest_8s: Option<f32>,
    last: Option<(bool, f32)>,
}

impl HurtMeter {
    fn make(_start: &TryStart) -> Box<dyn CustomMeter> {
        Box::new(Self::default())
    }
}

impl CustomMeter for HurtMeter {
    fn sample(&mut self, frame: &Frame) {
        let Some(owner) = frame.owner.as_ref() else {
            return;
        };
        let start = *self.first.get_or_insert(frame.clock);
        if self.first_push.is_none() && frame.heard.iter().any(|h| h.text.contains("pushes on hurt enemy")) {
            self.first_push = Some(frame.clock);
        }
        if frame.clock.0.saturating_sub(start.0) <= 8_000 {
            let near = frame
                .side(Side::Enemies)
                .filter(|b| b.body.alive)
                .map(|b| b.body.feet.distance_xz(owner.body.feet))
                .fold(None, |m: Option<f32>, d| Some(m.map_or(d, |m| m.min(d))));
            if let Some(d) = near {
                self.nearest_8s = Some(self.nearest_8s.map_or(d, |m| m.min(d)));
            }
        }
        self.last = Some((owner.body.alive, owner.pools.map_or(1.0, |p| p.pain)));
    }

    fn value(&self, n: usize) -> Option<f64> {
        let start = self.first?;
        Some(match n {
            0 => f64::from(self.first_push?.0.saturating_sub(start.0)) / 1_000.0,
            1 => f64::from(self.nearest_8s?),
            2 => f64::from(u8::from(self.nearest_8s? <= 10.0)),
            3 => {
                let (alive, pain) = self.last?;
                f64::from(u8::from(alive && pain < HURT_PAIN))
            }
            _ => return None,
        })
    }
}

fn hurt_measure() -> Measure {
    Measure::Custom { names: &["first_push_s", "nearest_enemy_8s_m", "reached_10m_by_8s", "hurt_alive_15s"], make: HurtMeter::make }
}

/// F3: the owner's fights by the distance to the enemy bot, from the frames. A kill of his is the enemy bot that went from alive to
/// dead, a death of his the bot that was nearest the frame before; the distance is to that bot at the frame before. Names:
/// `close_kills`, `close_deaths` (under 10 m), `mid_kills`, `mid_deaths` (10 to 20 m), `far_kills`, `far_deaths` (20 to 30 m),
/// `exchanges` (all that fell in 0 to 30 m), `range_step_backs` (times an enemy bot within 20 m of him took its legs for the weapon's
/// band, `LegsWhy::Range`).
#[derive(Debug, Default)]
struct RangeMeter {
    /// The enemy bots of the frame before (place, alive, feet, legs for the band) and the owner's feet and aliveness then.
    before: Vec<(Place, bool, Vec3, bool)>,
    owner_before: Option<(bool, Vec3)>,
    kills: [u32; 3],
    deaths: [u32; 3],
    step_backs: u32,
}

fn band(distance_m: f32) -> Option<usize> {
    match distance_m {
        d if d < 10.0 => Some(0),
        d if d < 20.0 => Some(1),
        d if d < 30.0 => Some(2),
        _ => None,
    }
}

impl RangeMeter {
    fn make(_start: &TryStart) -> Box<dyn CustomMeter> {
        Box::new(Self::default())
    }
}

impl CustomMeter for RangeMeter {
    fn sample(&mut self, frame: &Frame) {
        let Some(owner) = frame.owner.as_ref() else {
            return;
        };
        let now: Vec<(Place, bool, Vec3, bool)> = frame
            .side(Side::Enemies)
            .map(|b| (b.body.place, b.body.alive, b.body.feet, b.mind.legs == Some(LegsWhy::Range)))
            .collect();
        if let Some((owner_alive, owner_feet)) = self.owner_before {
            for &(place, alive, feet, on_band) in &now {
                let Some(&(_, was_alive, was_feet, was_on_band)) = self.before.iter().find(|b| b.0 == place) else {
                    continue;
                };
                if was_alive
                    && !alive
                    && let Some(i) = band(was_feet.distance_xz(owner_feet))
                {
                    self.kills[i] += 1;
                }
                if on_band && !was_on_band && alive && feet.distance_xz(owner.body.feet) < 20.0 {
                    self.step_backs += 1;
                }
            }
            if owner_alive && !owner.body.alive {
                let nearest = self
                    .before
                    .iter()
                    .filter(|b| b.1)
                    .map(|b| b.2.distance_xz(owner_feet))
                    .fold(None, |m: Option<f32>, d| Some(m.map_or(d, |m| m.min(d))));
                if let Some(i) = nearest.and_then(band) {
                    self.deaths[i] += 1;
                }
            }
        }
        self.before = now;
        self.owner_before = Some((owner.body.alive, owner.body.feet));
    }

    fn value(&self, n: usize) -> Option<f64> {
        self.owner_before?;
        Some(match n {
            0 => f64::from(self.kills[0]),
            1 => f64::from(self.deaths[0]),
            2 => f64::from(self.kills[1]),
            3 => f64::from(self.deaths[1]),
            4 => f64::from(self.kills[2]),
            5 => f64::from(self.deaths[2]),
            6 => f64::from(self.kills.iter().chain(&self.deaths).sum::<u32>()),
            7 => f64::from(self.step_backs),
            _ => return None,
        })
    }
}

fn range_measure() -> Measure {
    Measure::Custom {
        names: &[
            "close_kills",
            "close_deaths",
            "mid_kills",
            "mid_deaths",
            "far_kills",
            "far_deaths",
            "exchanges",
            "range_step_backs",
        ],
        make: RangeMeter::make,
    }
}

/// Ten exchanges (his kills and deaths) since the start.
fn ten_exchanges(_frame: &Frame, progress: &Progress) -> bool {
    progress.owner_kills + progress.owner_deaths >= 10
}

pub fn steps() -> Vec<Step> {
    vec![
        // F1: an-fights 6.1 and 6.4. Cologne, batteries, 10 a side, run twice (shipped defaults, then the parts ON).
        Step::new(
            "fights-steal",
            Group::new(COLOGNE, BATTERIES).of_size(10),
            "Take a battery from the enemy container and carry it home through the middle. Do not shoot; run, do not stop.",
        )
        .start(&[Cond::Custom(WaitFor { describe: "you carry a battery", holds: carrying })])
        .end(&[End::OwnerDied, End::Custom(EndWhen { reason: "home_or_dropped", describe: "the battery left your hands", holds: carry_over })])
        .measure(steal_measure())
        .measure(Measure::OwnerDeaths)
        .measure(Measure::MinHealth)
        // The bars of the ON run. No clear bot-second at all is a failed read, not a pass (the step finds nothing to compare).
        .bar("carrier_clear_s", Test::AtLeast(1.0))
        .bar("first_aimed_s", Test::AtMost(4.0))
        .bar("close_share", Test::AtLeast(0.6))
        .budget_s(150)
        .arm_s(180)
        .tries(2),
        // F2: an-fights 6.2. Vector Laboratory, team deathmatch. The try starts when he is hurt and out of sight.
        Step::new(
            "fights-hurt-hide",
            Group::new(VECTOR_LABORATORY, TEAM_DEATHMATCH),
            "Fight in the open until your legs or arms are red, then break sight behind cover 15-20 m off. Wait 10 s without shooting.",
        )
        .start(&[Cond::Custom(WaitFor { describe: "you are hurt and out of sight", holds: hurt_and_hidden })])
        .end(&[End::OwnerDied])
        .measure(hurt_measure())
        .measure(Measure::Notes { name: "pushers", contains: "pushes on hurt enemy" })
        .measure(Measure::NearestBotM(Side::Enemies))
        // The ON run's bar (the OFF run is the baseline and shows none): one or two push, the first within 3 s, one within 10 m by 8 s.
        .bar("pushers", Test::Within(1.0, 2.0))
        .bar("first_push_s", Test::AtMost(3.0))
        .bar("reached_10m_by_8s", Test::AtLeast(1.0))
        .budget_s(15)
        .arm_s(180)
        .tries(3),
        // F3: an-fights 6.3. Two steps on the same meter, calibration only (no bar): who wins by the distance.
        Step::new(
            "fights-range-close",
            Group::new(VECTOR_LABORATORY, TEAM_DEATHMATCH),
            "Fight single bots from close: get under 10 m of each bot you meet and fight it. Ten fights.",
        )
        .end(&[End::Custom(EndWhen { reason: "ten_exchanges", describe: "ten fights are over", holds: ten_exchanges })])
        .measure(range_measure())
        .budget_s(180)
        .tries(1),
        Step::new(
            "fights-range-far",
            Group::new(VECTOR_LABORATORY, TEAM_DEATHMATCH),
            "Fight single bots from afar: hold 20-30 m from each bot you meet and fight it. Ten fights.",
        )
        .end(&[End::Custom(EndWhen { reason: "ten_exchanges", describe: "ten fights are over", holds: ten_exchanges })])
        .measure(range_measure())
        .budget_s(180)
        .tries(1),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::bots::reflex::Gates;

    fn body(place: Place, team: u8, alive: bool, at: Vec3, keys: u32) -> Body {
        Body { place, team, alive, feet: at, health: 1.0, kills: 0, deaths: 0, crouched: false, keys, yaw: 0.0, pitch: 0.0, speed: 0.0 }
    }

    fn bot(place: Place, alive: bool, at: Vec3, target: Option<Place>, keys: u32, legs: Option<LegsWhy>) -> BotView {
        BotView {
            body: body(place, 1, alive, at, keys),
            mind: Mind { target, gates: Gates { at: 0, bits: 0, target: None }, legs },
            role: None,
        }
    }

    fn frame(clock: u32, owner: Body, pools: Pools, bots: Vec<BotView>, batteries: Vec<BatteryState>) -> Frame {
        Frame {
            clock: At(clock),
            playing: true,
            owner: Some(Owner { body: owner, pools: Some(pools) }),
            bots,
            batteries,
            heard: Vec::new(),
            sight: None,
            exits: Default::default(),
            counters: Default::default(),
        }
    }

    fn start() -> TryStart {
        TryStart { started: At(0), kills: 0, deaths: 0 }
    }

    /// Two enemy bots clear of him within 40 m for 2 s (one on him, one on a mate), one of them within 25 m; the near one fires at him
    /// from the second frame: the clear seconds, the share on him, the time to the first aimed shot.
    #[test]
    fn the_steal_meter_counts_bot_seconds_and_the_first_aimed_shot() {
        let owner = || body(0, 0, true, Vec3::new(0.0, 0.0, 0.0), 0);
        let mut meter = StealMeter::make(&start());
        for (i, ms) in [0u32, 1_000, 2_000].into_iter().enumerate() {
            let near = bot(10, true, Vec3::new(10.0, 0.0, 0.0), Some(0), if i >= 1 { key::FIRE } else { 0 }, None);
            let far = bot(11, true, Vec3::new(35.0, 0.0, 0.0), Some(5), 0, None);
            let dead_far = bot(12, false, Vec3::new(30.0, 0.0, 0.0), Some(0), key::FIRE, None);
            let out_of_range = bot(13, true, Vec3::new(80.0, 0.0, 0.0), Some(0), key::FIRE, None);
            meter.sample(&frame(ms, owner(), Pools::FULL, vec![near, far, dead_far, out_of_range], vec![BatteryState::Carried(Some(0))]));
        }
        let v = |n| meter.value(n).unwrap();
        assert!((v(0) - 4.0).abs() < 1e-9, "two bots, 2 s each: {}", v(0));
        assert!((v(1) - 2.0).abs() < 1e-9 && (v(2) - 2.0).abs() < 1e-9);
        assert!((v(3) - 2.0).abs() < 1e-9 && (v(4) - 1.0).abs() < 1e-9, "the close one is all on him");
        assert!((v(5) - 1.0).abs() < 1e-9, "first aimed shot at the second frame: {}", v(5));
        assert_eq!(v(6), 0.0);
        assert!(meter.value(7).is_none(), "the hand never reached zero");
    }

    /// A hand at zero while he carries, then the battery lying free: the answer of an-fights 2a; a battery still in his hands says no.
    #[test]
    fn the_steal_meter_reads_whether_a_broken_hand_drops_the_battery() {
        let owner = || body(0, 0, true, Vec3::ZERO, 0);
        let broken = Pools { hands: [0.0, 0.7], ..Pools::FULL };
        let mut drops = StealMeter::make(&start());
        drops.sample(&frame(0, owner(), Pools::FULL, Vec::new(), vec![BatteryState::Carried(Some(0))]));
        drops.sample(&frame(500, owner(), broken, Vec::new(), vec![BatteryState::Carried(Some(0))]));
        assert_eq!(drops.value(7), Some(0.0), "zero, the battery still in his hands");
        drops.sample(&frame(900, owner(), broken, Vec::new(), vec![BatteryState::Free(Vec3::ZERO)]));
        assert_eq!((drops.value(6), drops.value(7)), (Some(1.0), Some(1.0)));
    }

    /// The try starts when he is hurt and no enemy sees him; the carry conditions read the batteries.
    #[test]
    fn the_conditions_read_pain_sight_and_the_carry() {
        let owner = || body(0, 0, true, Vec3::ZERO, 0);
        let hurt = Pools { pain: 0.2, ..Pools::FULL };
        let near = bot(10, true, Vec3::new(30.0, 0.0, 0.0), Some(0), 0, None);
        let far = bot(10, true, Vec3::new(100.0, 0.0, 0.0), None, 0, None);
        assert!(!hurt_and_hidden(&frame(0, owner(), Pools::FULL, vec![far.clone()], Vec::new()), Span(0)), "not hurt");
        assert!(!hurt_and_hidden(&frame(0, owner(), hurt, vec![near], Vec::new()), Span(0)), "seen");
        assert!(hurt_and_hidden(&frame(0, owner(), hurt, vec![far], Vec::new()), Span(0)), "hurt and out of sight");
        let carried = frame(0, owner(), Pools::FULL, Vec::new(), vec![BatteryState::Carried(Some(0))]);
        let elsewhere = frame(0, owner(), Pools::FULL, Vec::new(), vec![BatteryState::Carried(Some(4))]);
        assert!(carries(&carried) && !carries(&elsewhere));
    }

    /// The hurt meter: the first push note, the nearest enemy in the first 8 s, and whether he is still hurt and alive at the end.
    #[test]
    fn the_hurt_meter_reads_the_push_the_nearest_enemy_and_the_end() {
        let owner = |alive| body(0, 0, alive, Vec3::ZERO, 0);
        let hurt = Pools { pain: 0.1, ..Pools::FULL };
        let mut meter = HurtMeter::make(&start());
        meter.sample(&frame(0, owner(true), hurt, vec![bot(10, true, Vec3::new(40.0, 0.0, 0.0), None, 0, None)], Vec::new()));
        let mut second = frame(2_000, owner(true), hurt, vec![bot(10, true, Vec3::new(9.0, 0.0, 0.0), None, 0, None)], Vec::new());
        second.heard.push(Heard { place: 10, time: At(2_000), text: "pushes on hurt enemy 0".to_string() });
        meter.sample(&second);
        meter.sample(&frame(12_000, owner(true), hurt, vec![bot(10, true, Vec3::new(2.0, 0.0, 0.0), None, 0, None)], Vec::new()));
        let v = |n| meter.value(n).unwrap();
        assert!((v(0) - 2.0).abs() < 1e-9, "first push at 2 s: {}", v(0));
        assert!((v(1) - 9.0).abs() < 1e-6 && v(2) == 1.0, "9 m in the first 8 s (the 12 s frame is outside)");
        assert_eq!(v(3), 1.0);
        meter.sample(&frame(14_000, owner(false), hurt, Vec::new(), Vec::new()));
        assert_eq!(meter.value(3), Some(0.0), "dead at the end");
        let quiet = HurtMeter::make(&start());
        assert!(quiet.value(0).is_none());
    }

    /// The range meter puts a kill of his in the band of the bot that fell, a death of his in the band of the nearest bot, and counts a
    /// bot taking the band legs once.
    #[test]
    fn the_range_meter_bands_his_fights_by_distance() {
        let owner = |alive| body(0, 0, alive, Vec3::ZERO, 0);
        let mut meter = RangeMeter::make(&start());
        let a = |alive, legs| bot(10, alive, Vec3::new(6.0, 0.0, 0.0), None, 0, legs);
        let b = |alive| bot(11, alive, Vec3::new(25.0, 0.0, 0.0), None, 0, None);
        meter.sample(&frame(0, owner(true), Pools::FULL, vec![a(true, None), b(true)], Vec::new()));
        meter.sample(&frame(500, owner(true), Pools::FULL, vec![a(true, Some(LegsWhy::Range)), b(true)], Vec::new()));
        meter.sample(&frame(900, owner(true), Pools::FULL, vec![a(true, Some(LegsWhy::Range)), b(true)], Vec::new()));
        // The 6 m bot falls: a close kill. Then the owner dies with the 25 m bot nearest: a far death.
        meter.sample(&frame(1_500, owner(true), Pools::FULL, vec![a(false, None), b(true)], Vec::new()));
        meter.sample(&frame(2_500, owner(false), Pools::FULL, vec![a(false, None), b(true)], Vec::new()));
        let v = |n| meter.value(n).unwrap();
        assert_eq!((v(0), v(1), v(2), v(3), v(4), v(5)), (1.0, 0.0, 0.0, 0.0, 0.0, 1.0));
        assert_eq!(v(6), 2.0);
        assert_eq!(v(7), 1.0, "the band legs counted once");
        let progress = |k, d| Progress { since_start: Span::s(30), owner_kills: k, owner_deaths: d };
        assert!(!ten_exchanges(&frame(0, owner(true), Pools::FULL, Vec::new(), Vec::new()), &progress(6, 3)));
        assert!(ten_exchanges(&frame(0, owner(true), Pools::FULL, Vec::new(), Vec::new()), &progress(6, 4)));
        assert_eq!(band(9.9), Some(0));
        assert_eq!(band(10.0), Some(1));
        assert_eq!(band(30.0), None);
    }

    // ---- dry runs: the runner given a made-up match (dry.rs) ----

    fn named<'a>(steps: &'a [Step], id: &str) -> &'a Step {
        steps.iter().find(|s| s.id == id).unwrap_or_else(|| panic!("no step {id}"))
    }

    fn mind(target: Option<Place>) -> Mind {
        Mind { target, gates: Gates::default(), legs: None }
    }

    /// The steal: he picks a battery up at 5 s, an enemy 15 m off aims at him and fires from 8 s, another at 30 m looks elsewhere; at 40 s the
    /// battery is home. The try reads about three seconds to the first aimed shot and a share of 1.0 within 25 m: PASS, ended by the carry.
    #[test]
    fn dry_run_the_steal_passes_when_the_enemy_turns_to_the_carrier() {
        let step = named(&steps(), "fights-steal").clone().tries(1);
        let (results, said) = super::super::dry::run(step, 60, |ms| {
            let mut f = super::super::dry::frame_at(ms, super::super::dry::body(0, 0, Vec3::ZERO));
            let carried = (5_000..40_000).contains(&ms);
            f.batteries = vec![if carried { BatteryState::Carried(Some(0)) } else { BatteryState::Home(0) }];
            let near = BotView { body: body(10, 1, true, Vec3::new(15.0, 0.0, 0.0), if ms >= 8_000 { key::FIRE } else { 0 }), mind: mind(Some(0)), role: None };
            let far = BotView { body: body(11, 1, true, Vec3::new(30.0, 0.0, 0.0), 0), mind: mind(None), role: None };
            f.bots = vec![near, far];
            f
        });
        assert!(results.contains("fights-steal"), "{results}");
        assert!(results.contains("\tPASS\thome_or_dropped\t"), "{results}");
        assert!(results.contains("first_aimed_s=2.9") && results.contains("close_share=1"), "{results}");
        assert!(said.iter().any(|l| l.contains("Take a battery")), "{said:?}");
    }

    /// Hurt and hide: his pain is 0.2 and nobody sees him from the start; a note says an enemy pushes at 3 s and it stands 8 m off from 4 s.
    #[test]
    fn dry_run_hurt_and_hide_reads_the_push() {
        let step = named(&steps(), "fights-hurt-hide").clone().tries(1);
        let (results, _) = super::super::dry::run(step, 40, |ms| {
            let mut f = super::super::dry::frame_at(ms, super::super::dry::body(0, 0, Vec3::ZERO));
            f.owner.as_mut().unwrap().pools = Some(Pools { pain: 0.2, ..Pools::FULL });
            let at = if ms >= 4_000 { Vec3::new(8.0, 0.0, 0.0) } else { Vec3::new(100.0, 0.0, 0.0) };
            f.bots = vec![BotView { body: body(10, 1, true, at, 0), mind: mind(None), role: None }];
            if ms == 3_000 {
                f.heard.push(Heard { place: 10, time: At(ms), text: "pushes on hurt enemy 0".to_string() });
            }
            f
        });
        assert!(results.contains("fights-hurt-hide") && results.contains("\tPASS\t"), "{results}");
        assert!(results.contains("pushers=1") && results.contains("reached_10m_by_8s=1"), "{results}");
    }

    /// The range steps end on the tenth exchange: his kills and deaths count, ten of them end the try as an INFO row.
    #[test]
    fn dry_run_the_range_step_ends_on_ten_exchanges() {
        let step = named(&steps(), "fights-range-close").clone();
        let (results, _) = super::super::dry::run(step, 80, |ms| {
            let mut owner = super::super::dry::body(0, 0, Vec3::ZERO);
            owner.kills = (ms / 4_000).saturating_sub(1);
            super::super::dry::frame_at(ms, owner)
        });
        assert!(results.contains("\tINFO\tten_exchanges\t"), "{results}");
    }
}
