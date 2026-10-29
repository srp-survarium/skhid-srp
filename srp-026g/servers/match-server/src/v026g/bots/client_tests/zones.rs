//! Client-test steps of the zones package (PR 704: crossings, hurt test, escort through zones; an-objective section 6).
//!
//! All on Vector Laboratory (level 7), batteries, any size: one match load. The questions they answer for the owner:
//! 1. Is a floor 0.27 m over a damage zone's top hurt by it (the client's capsule test, PROVEN exact in the exe)? `zones-top-*`.
//! 2. What does a walk on zone 13's floors cost the owner's pools, with his kit-15 and kit-4 characters (the calibration of
//!    `ZoneArmour` / the pool factors, an-objective section 6 test 2)? `zones-lower-ring-*` and `zones-walk-out`.
//! 3. Do the bots take Vector Laboratory's spawner 26 (the one-way trap)? Until the calibration they refuse it; `zones-bots-leave-26`
//!    says so, and flips to a pass bar for a take once the pools are in `zone_cost`.
//! 4. Does an escort stay with a carrier who walks over zone floors? `zones-escort-carry`.
//! 5. Does a zone's hit fall off towards its rim for the owner's own body (the hit curve, PROVEN in the exe and in bots' live losses)?
//!    `zones-rim-26` stands on the zone's floor near its rim, where the straight curve of the radiation zones gives under a tenth of
//!    the hit; `zones-lower-ring-*` is the same zone at a third of it.
//!
//! The runner cannot place a battery or a bot, so a step that needs a battery asks the owner to do the walk the match offers.
//! The owner's pools are as the last world showed them: **full is 1.0** (a loss is start minus now).

use super::*;
use crate::v026g::bots::pools::Pools;
use crate::v026g::bots::view::BatteryState;

const VECTOR_LABORATORY: u8 = 7;

/// Spawner 26's battery floor, 0.27 m over zone 13's top (y 16.90; the zone's top is 16.63).
const TOP_26: Spot = Spot::at(-1.8, 16.9, -11.3);
/// The survey's margin-only floors 3-4 m west of it (y 17.1-17.5).
const TOP_W1: Spot = Spot::at(-4.8, 17.3, -11.3);
const TOP_W2: Spot = Spot::at(-5.8, 17.2, -10.6);
const TOP_W3: Spot = Spot::at(-6.8, 17.1, -11.0);
/// The lower ring's floor inside zone 13 (y 13.8), where the owner crossed in match 0x717ab70c. Normalised distance 0.66 from the
/// zone's centre (the hit curve gives 0.34 of the hit there; `hazards::tests::rim_floors_of_the_zone_over_vector_laboratorys_lower_ring`).
const LOWER_RING: Spot = Spot::at(3.8, 13.8, -8.6);
/// A floor of the same zone near its rim, 3.9 m from the ring: normalised distance 0.92, the hit curve gives 0.08 of the hit
/// (the same measurement; the capsule still overlaps the zone's shape there, so the client hurts).
const RIM_26: Spot = Spot::at(5.5, 13.23, -12.0);

fn group() -> Group {
    Group::new(VECTOR_LABORATORY, BATTERIES)
}

/// What the owner's pools did over a try: loss from the first world that showed them (full is 1.0), the seconds they were falling,
/// and the dearest second. Names: `infection_loss`, `infection_per_s` (loss over the try's seconds), `body_loss`,
/// `seconds_hurt` (frames-to-seconds with any pool falling between two worlds), `worst_infection_per_s` (the dearest second).
#[derive(Debug)]
struct PoolMeter {
    first: Option<(At, Pools)>,
    last: Option<(At, Pools)>,
    hurt_ms: u32,
    /// Infection lost in the current second and the dearest second so far.
    window: (At, f32),
    worst: f32,
}

impl PoolMeter {
    fn make(_start: &TryStart) -> Box<dyn CustomMeter> {
        Box::new(Self { first: None, last: None, hurt_ms: 0, window: (At(0), 0.0), worst: 0.0 })
    }
}

impl CustomMeter for PoolMeter {
    fn sample(&mut self, frame: &Frame) {
        let Some(pools) = frame.owner.as_ref().and_then(|o| o.pools) else {
            return;
        };
        if let Some((at, before)) = self.last {
            let fell = before.infection - pools.infection;
            let body = before.body - pools.body;
            if fell > 1e-4 || body > 1e-4 {
                self.hurt_ms += frame.clock.0.saturating_sub(at.0);
            }
            if frame.clock.0.saturating_sub(self.window.0.0) >= 1_000 {
                self.window = (frame.clock, 0.0);
            }
            self.window.1 += fell.max(0.0);
            self.worst = self.worst.max(self.window.1);
        } else {
            self.window = (frame.clock, 0.0);
        }
        self.first.get_or_insert((frame.clock, pools));
        self.last = Some((frame.clock, pools));
    }

    fn value(&self, n: usize) -> Option<f64> {
        let ((t0, first), (t1, last)) = (self.first?, self.last?);
        let seconds = f64::from(t1.0.saturating_sub(t0.0)) / 1_000.0;
        Some(match n {
            0 => f64::from(first.infection - last.infection),
            1 if seconds > 0.0 => f64::from(first.infection - last.infection) / seconds,
            1 => return None,
            2 => f64::from(first.body - last.body),
            3 => f64::from(self.hurt_ms) / 1_000.0,
            4 => f64::from(self.worst),
            _ => return None,
        })
    }
}

fn pool_measure() -> Measure {
    Measure::Custom {
        names: &["infection_loss", "infection_per_s", "body_loss", "seconds_hurt", "worst_infection_per_s"],
        make: PoolMeter::make,
    }
}

/// The owner stands within 0.6 m of `y` (the top floors are told apart by their height).
fn at_height(frame: &Frame, y: f32) -> bool {
    frame.owner.as_ref().is_some_and(|o| (o.body.feet.y - y).abs() < 0.6)
}

/// The top floors are 16.9-17.5 m high.
fn on_top(frame: &Frame, _since: Span) -> bool {
    at_height(frame, 17.1)
}

fn on_lower_ring(frame: &Frame, _since: Span) -> bool {
    at_height(frame, 13.8)
}

fn on_rim_floor(frame: &Frame, _since: Span) -> bool {
    at_height(frame, 13.3)
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

/// A try of "stand on the floor over zone 13's top for `seconds`": no loss of infection or body (the capsule rule, not the 1 m margin,
/// is where the client hurts). A rise of 0.01 a second or more says the floor hurts: then `hurt_by_shape` needs a vertical margin.
fn top_step(id: &'static str, instruction: &'static str, spot: Spot, seconds: u32) -> Step {
    Step::new(id, group(), instruction)
        .start(&[Cond::OwnerNear { at: spot, r_m: 1.0 }, Cond::Custom(WaitFor { describe: "you stand on the marked floor", holds: on_top })])
        .end(&[End::OwnerDied, End::OwnerLeft { at: spot, r_m: 3.0 }])
        .measure(pool_measure())
        .bar("infection_per_s", Test::AtMost(0.002))
        .bar("body_loss", Test::AtMost(0.01))
        .budget_s(seconds)
        .tries(2)
}

pub fn steps() -> Vec<Step> {
    vec![
        // Z1: spawner 26's battery floor.
        top_step(
            "zones-top-26",
            "Drop onto the floor under the upper ring west of the middle (spawner 26's battery) and stand on it for 20 s.",
            TOP_26,
            20,
        ),
        // Z2: the margin-only floors west of it, 10 s each.
        top_step("zones-top-w1", "Walk west 3 m along that floor to the marker and stand there for 10 s.", TOP_W1, 10),
        top_step("zones-top-w2", "Walk on to the next marker (1 m west) and stand there for 10 s.", TOP_W2, 10),
        top_step("zones-top-w3", "Walk on to the last marker (1 m west) and stand there for 10 s.", TOP_W3, 10),
        // Z3: what zone 13 costs on its own floor (the calibration of the pool factors); INFO, no bar but the control that it hurts at all.
        Step::new(
            "zones-lower-ring-kit15",
            group(),
            "With your kit-15 character: stand on the lower ring floor at the marker, inside zone 13, for 10 s.",
        )
        .start(&[Cond::OwnerNear { at: LOWER_RING, r_m: 1.0 }, Cond::Custom(WaitFor { describe: "you stand on the lower ring", holds: on_lower_ring })])
        .end(&[End::OwnerDied, End::OwnerLeft { at: LOWER_RING, r_m: 3.0 }])
        .measure(pool_measure())
        // Control: the zone hurts there (the bots price 0.32 a second raw over 4 parts; the pools-routed cost is what this calibrates).
        .bar("infection_per_s", Test::AtLeast(0.01))
        .budget_s(10)
        .tries(2),
        Step::new(
            "zones-lower-ring-kit4",
            group(),
            "Now with your kit-4 character: stand on the same lower ring marker for 10 s.",
        )
        .start(&[Cond::OwnerNear { at: LOWER_RING, r_m: 1.0 }, Cond::Custom(WaitFor { describe: "you stand on the lower ring", holds: on_lower_ring })])
        .end(&[End::OwnerDied, End::OwnerLeft { at: LOWER_RING, r_m: 3.0 }])
        .measure(pool_measure())
        .bar("infection_per_s", Test::AtLeast(0.01))
        .budget_s(10)
        .tries(2),
        // Z4: the hit curve for the owner's own body. At the rim marker (normalised distance 0.92) the zone takes 0.08 of its hit
        // (`mapdata::Zone::intensity_at`): the bots price the full hit (0.32 a second raw, 0.13-0.24 through a kit's armour) unless
        // `hazard.zone_intensity` says otherwise; with the curve it is 0.01-0.02 a second. A flat zone would show 0.1 or more.
        Step::new(
            "zones-rim-26",
            group(),
            "Stand on the floor at the marker near the zone's edge (about 4 m from the lower ring marker, outwards) for 10 s.",
        )
        .start(&[Cond::OwnerNear { at: RIM_26, r_m: 1.0 }, Cond::Custom(WaitFor { describe: "you stand on the rim floor", holds: on_rim_floor })])
        .end(&[End::OwnerDied, End::OwnerLeft { at: RIM_26, r_m: 3.0 }])
        .measure(pool_measure())
        // The zone hurts there (the capsule is in its shape) but far less than on the lower ring: under 0.06, where the full hit through
        // the same kit would give at least 0.1 (kit armour factors of the live bots: 0.4 to 0.76 of 0.32).
        .bar("infection_per_s", Test::AtMost(0.06))
        .budget_s(10)
        .tries(2),
        // Z6: the walk the owner made in match 0x717ab70c: from the battery floor down to the lower ring, over the zone's floor, out to the
        // nearest dry floor. The pool loss of the whole walk is the number `leg_out` (price_way_out) is checked against; INFO.
        Step::new(
            "zones-walk-out",
            group(),
            "From spawner 26's battery floor drop to the lower ring, walk the zone's floor to the nearest dry floor, and stop there.",
        )
        .start(&[Cond::OwnerNear { at: TOP_26, r_m: 1.5 }, Cond::Custom(WaitFor { describe: "you stand on the battery floor", holds: on_top })])
        .end(&[End::OwnerDied, End::Custom(EndWhen { reason: "dry", describe: "you are 30 m from the battery floor", holds: far_from_26 })])
        .measure(pool_measure())
        .budget_s(90)
        .tries(2),
        // F1: the bots and spawner 26. Today they refuse it ("its zone hurts too much", the way out priced); the owner stays out of the
        // way. After the calibration (the pools in `zone_cost`) the bars become: taken (`takes_26` at least 1) within 120 s.
        Step::new(
            "zones-bots-leave-26",
            group(),
            "Stay in your spawn and watch; do not move for 2 minutes.",
        )
        .start(&[Cond::After(Span::s(2))])
        .end(&[End::OwnerDied])
        .measure(Measure::Notes { name: "refused_26", contains: "battery 26 is no Collect candidate" })
        .measure(Measure::Notes { name: "takes_26", contains: "takes Collect Battery(26)" })
        .measure(Measure::Notes { name: "no_way_ends", contains: "ends Collect: no way" })
        .bar("takes_26", Test::AtMost(0.0))
        .budget_s(120)
        .tries(1),
        // Z4: escort through a zone carry. The owner takes any battery lying in a zone (spawner 26's, as in the 15:17Z match) and carries it home.
        Step::new(
            "zones-escort-carry",
            group(),
            "Take a battery that lies in a zone (spawner 26's: drop from the upper ring) and carry it home across the lower ring.",
        )
        .start(&[Cond::Custom(WaitFor { describe: "you carry a battery", holds: carrying })])
        .end(&[End::OwnerDied, End::Custom(EndWhen { reason: "home", describe: "you put the battery in", holds: carry_over })])
        .measure(Measure::Notes { name: "escorts_begun", contains: "takes Escort" })
        .measure(Measure::Notes { name: "escort_no_way", contains: "ends Escort: no way" })
        .measure(Measure::Notes { name: "escort_no_dry_floor", contains: "no dry floor sees him" })
        .measure(Measure::OwnerDeaths)
        // Before the fix 18 such ends came in 25 s of one carry.
        .bar("escort_no_way", Test::AtMost(3.0))
        .budget_s(180)
        .tries(2),
    ]
}

fn far_from_26(frame: &Frame, progress: &Progress) -> bool {
    // After 10 s on the way (the drop and the first metres), 30 m from the battery floor: out of the zone's reach.
    progress.since_start >= Span::s(10)
        && frame.owner.as_ref().is_some_and(|o| o.body.feet.distance_xz(TOP_26.vec()) > 30.0)
}

#[cfg(test)]
mod tests {
    use mapdata::Vec3;

    use super::*;

    fn frame(clock: u32, y: f32, infection: f32, carried: bool) -> Frame {
        Frame {
            clock: At(clock),
            playing: true,
            owner: Some(Owner {
                body: Body {
                    place: 0,
                    team: 0,
                    alive: true,
                    feet: Vec3::new(-1.8, y, -11.3),
                    health: 1.0,
                    kills: 0,
                    deaths: 0,
                    crouched: false,
                    keys: 0,
                    yaw: 0.0,
                    pitch: 0.0,
                    speed: 0.0,
                },
                pools: Some(Pools { infection, ..Pools::FULL }),
            }),
            bots: Vec::new(),
            batteries: vec![BatteryState::Carried(carried.then_some(0))],
            heard: Vec::new(),
            sight: None,
        }
    }

    /// Infection falling from 1.0 to 0.8 over 2 s reads as a loss of 0.2, 0.1 a second, hurt for 2 s, the dearest second 0.1; a steady pool reads 0.
    #[test]
    fn the_pool_meter_reads_the_loss_from_the_first_world() {
        let start = TryStart { started: At(0), kills: 0, deaths: 0 };
        let mut hurt = PoolMeter::make(&start);
        let mut calm = PoolMeter::make(&start);
        for (i, ms) in (0..=2_000u32).step_by(500).enumerate() {
            hurt.sample(&frame(ms, 13.8, 1.0 - 0.0125 * i as f32 * 2.0, false));
            calm.sample(&frame(ms, 17.1, 1.0, false));
        }
        let value = |meter: &dyn CustomMeter, n| meter.value(n).unwrap();
        assert!((value(&*hurt, 0) - 0.1).abs() < 1e-5, "{hurt:?}");
        assert!((value(&*hurt, 1) - 0.05).abs() < 1e-5);
        assert!(value(&*hurt, 3) >= 1.9);
        assert_eq!((value(&*calm, 0), value(&*calm, 2), value(&*calm, 3)), (0.0, 0.0, 0.0));
    }

    /// The top floors are told apart from the lower ring by height, and a carry is the owner's battery.
    #[test]
    fn the_height_and_carry_conditions_read_the_frame() {
        assert!(on_top(&frame(0, 16.9, 1.0, false), Span(0)));
        assert!(on_top(&frame(0, 17.5, 1.0, false), Span(0)));
        assert!(!on_top(&frame(0, 13.8, 1.0, false), Span(0)));
        assert!(on_lower_ring(&frame(0, 13.8, 1.0, false), Span(0)));
        assert!(!carries(&frame(0, 16.9, 1.0, false)));
        assert!(carries(&frame(0, 16.9, 1.0, true)));
        let progress = Progress { since_start: Span::s(1), owner_kills: 0, owner_deaths: 0 };
        assert!(carry_over(&frame(0, 16.9, 1.0, false), &progress));
        assert!(!far_from_26(&frame(0, 13.8, 1.0, false), &progress));
    }
}
