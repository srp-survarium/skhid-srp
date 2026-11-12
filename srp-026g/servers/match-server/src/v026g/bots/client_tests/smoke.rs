//! Client-test steps of the smoke package (PR #713 the sight through smoke, PR #719 the throws and the watch of the edge; `fx-smoke-client-tests.md`
//! S1, S2, S4, S5b). All on Rudnya, team deathmatch, any size: one match load.
//!
//! The server's note of a cloud is `smoke cloud by player P (team T) at (x, y, z), pours at A ms, gone at B ms` (`view.rs`); the steps read the
//! clouds from those notes, the owner's body from the world and the bots' target and fire key. The cloud's model is a sphere of 8 m
//! (`bots/smoke.rs`) from its pour to its end. Not here: S3 (a bot smokes a crossing the owner holds) needs the K1 lane and two bots sent
//! across it, which a script cannot give; the owner's answers (when the cloud looked solid, when he could see through, whether he heard the
//! pop) are asked in the chat (ASK OWNER in the sheet). Every step starts 3 s after it is told, so the note of a cloud thrown after that is
//! seen by the meters (a note in the poll that starts a try is not).

use super::*;
use crate::v026g::drive::key;
use mapdata::Vec3;

const RUDNYA: u8 = 1;
/// The cloud's solid radius in the model, m.
const CLOUD_R_M: f32 = 8.0;
/// A smoked length of this or more on a line hides it, m.
const HIDING_M: f32 = 2.0;
/// An enemy bot counts against the owner within this range, m.
const NEAR_M: f32 = 60.0;
/// The owner stands still under this speed, m/s, for this long, ms.
const STILL_MPS: f32 = 0.2;
const STOP_MS: u32 = 3_000;

/// A cloud read from a note.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Cloud {
    centre: Vec3,
    pours: u32,
    gone: u32,
}

/// `smoke cloud by player P (team T) at (x, y, z), pours at A ms, gone at B ms`.
fn cloud_of(text: &str) -> Option<Cloud> {
    let rest = text.strip_prefix("smoke cloud by player ")?;
    if rest.contains("put on its grenade") {
        return None;
    }
    let at = rest.split("at (").nth(1)?;
    let (xyz, tail) = at.split_once(')')?;
    let mut parts = xyz.split(',').map(|p| p.trim().parse::<f32>().ok());
    let (x, y, z) = (parts.next()??, parts.next()??, parts.next()??);
    let pours = tail.split("pours at ").nth(1)?.split(" ms").next()?.trim().parse().ok()?;
    let gone = tail.split("gone at ").nth(1)?.split(" ms").next()?.trim().parse().ok()?;
    Some(Cloud { centre: Vec3::new(x, y, z), pours, gone })
}

/// The length of the segment `a` to `b` inside a sphere of radius `r` about `c`, m.
fn chord_m(a: Vec3, b: Vec3, c: Vec3, r: f32) -> f32 {
    let d = b - a;
    let len = d.length();
    if len < 1e-4 {
        return 0.0;
    }
    let u = d * (1.0 / len);
    let m = a - c;
    let bq = m.dot(u);
    let disc = bq * bq - (m.dot(m) - r * r);
    if disc <= 0.0 {
        return 0.0;
    }
    let (t0, t1) = (-bq - disc.sqrt(), -bq + disc.sqrt());
    (t1.min(len) - t0.max(0.0)).max(0.0)
}

/// The clouds the notes have told of so far.
#[derive(Debug, Default)]
struct Clouds(Vec<Cloud>);

impl Clouds {
    fn read(&mut self, frame: &Frame) {
        for heard in &frame.heard {
            if let Some(cloud) = cloud_of(&heard.text)
                && !self.0.contains(&cloud)
            {
                self.0.push(cloud);
            }
        }
    }

    fn active(&self, ms: u32) -> impl Iterator<Item = &Cloud> {
        self.0.iter().filter(move |c| (c.pours..c.gone).contains(&ms))
    }
}

/// S1: where he stands against the cloud. Names: `clouds`, `pour_to_gone_s` (of the first), `stops` (stands of 3 s after he has walked 3 m
/// from his place at the pour), `first_stop_m` and `last_stop_m` (their distance from the first cloud's centre: the first is the
/// visible edge the model puts at 9-11.5 m).
#[derive(Debug, Default)]
struct CloudMeter {
    clouds: Clouds,
    at_pour: Option<Vec3>,
    moved: bool,
    still_since: Option<At>,
    counted: bool,
    stops: Vec<f32>,
}

impl CloudMeter {
    fn make(_: &TryStart) -> Box<dyn CustomMeter> {
        Box::new(Self::default())
    }
}

impl CustomMeter for CloudMeter {
    fn sample(&mut self, frame: &Frame) {
        let Some(owner) = frame.owner.as_ref() else {
            return;
        };
        self.clouds.read(frame);
        let Some(first) = self.clouds.0.first().copied() else {
            return;
        };
        if frame.clock.0 < first.pours {
            return;
        }
        let here = owner.body.feet;
        let origin = *self.at_pour.get_or_insert(here);
        if here.distance_xz(origin) >= 3.0 {
            self.moved = true;
        }
        if self.moved && owner.body.speed < STILL_MPS {
            let since = *self.still_since.get_or_insert(frame.clock);
            if frame.clock.0.saturating_sub(since.0) >= STOP_MS && !self.counted {
                self.counted = true;
                self.stops.push(here.distance_xz(first.centre));
            }
        } else {
            self.still_since = None;
            self.counted = false;
        }
    }

    fn value(&self, n: usize) -> Option<f64> {
        match n {
            0 => Some(self.clouds.0.len() as f64),
            1 => self.clouds.0.first().map(|c| f64::from(c.gone.saturating_sub(c.pours)) / 1_000.0),
            2 => Some(self.stops.len() as f64),
            3 => self.stops.first().copied().map(f64::from),
            4 => self.stops.last().copied().map(f64::from),
            _ => None,
        }
    }
}

/// S2: do the enemy bots see and shoot through his smoke? Names: `smoked_line_s` (seconds an enemy bot within 60 m has 2 m or more of an
/// active cloud on the line from its eye to his chest), `target_s_while_smoked` (of them, the ones it has him as its target) and
/// `fire_s_while_smoked` (and holds its fire key: the bar; a spray within 0.75 s of a sight before it is allowed).
#[derive(Debug, Default)]
struct HidesMeter {
    clouds: Clouds,
    last: Option<At>,
    smoked_ms: f64,
    target_ms: f64,
    fire_ms: f64,
}

impl HidesMeter {
    fn make(_: &TryStart) -> Box<dyn CustomMeter> {
        Box::new(Self::default())
    }
}

impl CustomMeter for HidesMeter {
    fn sample(&mut self, frame: &Frame) {
        let Some(owner) = frame.owner.as_ref().filter(|o| o.body.alive) else {
            return;
        };
        self.clouds.read(frame);
        let dt = f64::from(self.last.map_or(0, |at| frame.clock.0.saturating_sub(at.0)));
        self.last = Some(frame.clock);
        let chest = owner.body.feet + Vec3::new(0.0, 1.2, 0.0);
        for bot in frame.side(Side::Enemies).filter(|b| b.body.alive && b.body.feet.distance_xz(owner.body.feet) <= NEAR_M) {
            let eye = bot.body.feet + Vec3::new(0.0, 1.6, 0.0);
            let smoked: f32 = self.clouds.active(frame.clock.0).map(|c| chord_m(eye, chest, c.centre + Vec3::new(0.0, 1.5, 0.0), CLOUD_R_M)).sum();
            if smoked >= HIDING_M {
                self.smoked_ms += dt;
                if bot.mind.target == Some(owner.body.place) {
                    self.target_ms += dt;
                    if bot.body.keys & key::FIRE != 0 {
                        self.fire_ms += dt;
                    }
                }
            }
        }
    }

    fn value(&self, n: usize) -> Option<f64> {
        self.last?;
        match n {
            0 => Some(self.smoked_ms / 1_000.0),
            1 => Some(self.target_ms / 1_000.0),
            2 => Some(self.fire_ms / 1_000.0),
            _ => None,
        }
    }
}

/// S5b: how far the pop was from him. Names: `clouds` and `dist_1_m` .. `dist_4_m`, his flat distance to each of the first four clouds' centres at
/// its pour; the owner says in the chat which he heard.
#[derive(Debug, Default)]
struct HeardMeter {
    clouds: Clouds,
    dists: Vec<f32>,
    seen: bool,
}

impl HeardMeter {
    fn make(_: &TryStart) -> Box<dyn CustomMeter> {
        Box::new(Self::default())
    }
}

impl CustomMeter for HeardMeter {
    fn sample(&mut self, frame: &Frame) {
        let Some(owner) = frame.owner.as_ref() else {
            return;
        };
        self.seen = true;
        self.clouds.read(frame);
        while self.dists.len() < self.clouds.0.len() {
            let cloud = self.clouds.0[self.dists.len()];
            if frame.clock.0 < cloud.pours {
                break;
            }
            self.dists.push(owner.body.feet.distance_xz(cloud.centre));
        }
    }

    fn value(&self, n: usize) -> Option<f64> {
        if !self.seen {
            return None;
        }
        match n {
            0 => Some(self.clouds.0.len() as f64),
            1..=4 => self.dists.get(n - 1).copied().map(f64::from),
            _ => None,
        }
    }
}

pub fn steps() -> Vec<Step> {
    let rudnya = || Group::new(RUDNYA, TEAM_DEATHMATCH);
    vec![
        // S1: the cloud's size and duration against the model (calibration).
        Step::new(
            "smoke-cloud",
            rudnya(),
            "Throw a smoke about 10 m ahead on open ground, walk straight away from its middle and stop where you see clear through its edge for 3 s.",
        )
        .start(&[Cond::After(Span::s(3))])
        .end(&[End::OwnerDied])
        .measure(Measure::Custom { names: &["clouds", "pour_to_gone_s", "stops", "first_stop_m", "last_stop_m"], make: CloudMeter::make })
        .budget_s(60)
        .arm_s(180)
        .tries(1),
        // S2: a bot does not see (or shoot) the owner through his smoke.
        Step::new(
            "smoke-hides",
            rudnya(),
            "In an enemy bot's view throw a smoke between you and it, wait 2 s, then walk across the line behind the cloud, slowly, and stand 3 s behind it.",
        )
        .start(&[Cond::After(Span::s(3))])
        .end(&[End::OwnerDied])
        .measure(Measure::Custom { names: &["smoked_line_s", "target_s_while_smoked", "fire_s_while_smoked"], make: HidesMeter::make })
        .measure(Measure::OwnerDeaths)
        .bar("fire_s_while_smoked", Test::AtMost(0.75))
        .budget_s(30)
        .arm_s(180)
        .tries(2),
        // S4: the bot watches the near edge of the owner's smoke (PR #719, `smoke.react_watch`).
        Step::new(
            "smoke-edge-watch",
            rudnya(),
            "Stand 20-30 m in front of a bot that has not seen you, in its view; throw a smoke halfway, wait 2 s, walk into the cloud, step out at the bot's side.",
        )
        .start(&[Cond::After(Span::s(3))])
        .end(&[End::OwnerDied])
        .measure(Measure::Notes { name: "watch_notes", contains: "watches the edge of player" })
        .measure(Measure::Notes { name: "clouds", contains: "smoke cloud by player" })
        .bar("watch_notes", Test::AtLeast(1.0))
        .budget_s(25)
        .arm_s(180)
        .tries(2),
        // S5b: how far the pop is heard (calibration, no bar).
        Step::new(
            "smoke-heard",
            rudnya(),
            "Throw a smoke over your shoulder at about 10, 15, 20 and 30 m behind you, one after the other; say which pops you heard.",
        )
        .start(&[Cond::After(Span::s(3))])
        .end(&[End::OwnerDied])
        .measure(Measure::Custom { names: &["clouds", "dist_1_m", "dist_2_m", "dist_3_m", "dist_4_m"], make: HeardMeter::make })
        .budget_s(120)
        .arm_s(180)
        .tries(1),
    ]
}

#[cfg(test)]
mod tests {
    use super::super::dry;
    use super::*;
    use crate::v026g::bots::reflex::Gates;

    fn note(ms: u32, text: &str) -> Heard {
        Heard { place: usize::MAX, time: At(ms), text: text.to_string() }
    }

    const CLOUD_NOTE: &str = "smoke cloud by player 0 (team 0) at (10.0, 0.0, 0.0), pours at 5000 ms, gone at 31000 ms";

    /// The note is read; the line that says a cloud was put on its grenade is not a new cloud.
    #[test]
    fn a_cloud_note_is_read_and_the_correction_is_not() {
        let cloud = cloud_of(CLOUD_NOTE).unwrap();
        assert_eq!((cloud.centre, cloud.pours, cloud.gone), (Vec3::new(10.0, 0.0, 0.0), 5_000, 31_000));
        assert!(cloud_of("smoke cloud by player 0 put on its grenade 7 at rest at (1.0, 0.0, 0.0) in the world at 4000 ms: 2.0 m from x").is_none());
        assert!(cloud_of("takes Collect Battery(3)").is_none());
    }

    #[test]
    fn the_chord_is_the_length_inside_the_sphere() {
        let c = Vec3::new(10.0, 0.0, 0.0);
        assert!((chord_m(Vec3::ZERO, Vec3::new(20.0, 0.0, 0.0), c, 8.0) - 16.0).abs() < 1e-4);
        assert!((chord_m(Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0), c, 8.0) - 8.0).abs() < 1e-4, "ends at the middle");
        assert_eq!(chord_m(Vec3::ZERO, Vec3::new(20.0, 30.0, 0.0), c, 8.0), 0.0, "passes beside it");
        assert_eq!(chord_m(Vec3::ZERO, Vec3::ZERO, c, 8.0), 0.0);
    }

    fn frame(ms: u32, at: Vec3, speed: f32, bots: Vec<BotView>, heard: Vec<Heard>) -> Frame {
        let mut body = dry::body(0, 0, at);
        body.speed = speed;
        let mut f = dry::frame_at(ms, body);
        f.bots = bots;
        f.heard = heard;
        f
    }

    fn enemy(at: Vec3, target: Option<usize>, keys: u32) -> BotView {
        let mut body = dry::body(10, 1, at);
        body.keys = keys;
        BotView { body, mind: Mind { target, gates: Gates::default(), legs: None }, role: None }
    }

    fn start() -> TryStart {
        TryStart { started: At(0), kills: 0, deaths: 0 }
    }

    /// He walks away from the cloud and stops 3 s at 10 m: the first stop is read at its distance from the middle.
    #[test]
    fn the_cloud_meter_reads_the_first_stop() {
        let mut meter = CloudMeter::make(&start());
        meter.sample(&frame(4_000, Vec3::new(0.0, 0.0, 0.0), 0.0, Vec::new(), vec![note(4_000, CLOUD_NOTE)]));
        meter.sample(&frame(5_500, Vec3::new(0.0, 0.0, 0.0), 0.0, Vec::new(), Vec::new()));
        meter.sample(&frame(6_000, Vec3::new(-2.0, 0.0, 0.0), 3.0, Vec::new(), Vec::new()));
        meter.sample(&frame(7_000, Vec3::new(-5.0, 0.0, 0.0), 3.0, Vec::new(), Vec::new()));
        meter.sample(&frame(8_000, Vec3::new(-6.0, 0.0, 0.0), 0.0, Vec::new(), Vec::new()));
        meter.sample(&frame(11_500, Vec3::new(-6.0, 0.0, 0.0), 0.0, Vec::new(), Vec::new()));
        let v = |n| meter.value(n).unwrap();
        assert_eq!((v(0), v(1), v(2)), (1.0, 26.0, 1.0));
        assert!((v(3) - 16.0).abs() < 1e-6, "from the middle at x = 10 to x = -6: {}", v(3));
    }

    /// A bot at 30 m on the far side of the cloud, aimed at him and firing, is firing through 16 m of cloud: counted; before the pour it is not.
    #[test]
    fn the_hides_meter_counts_fire_through_a_cloud() {
        let mut meter = HidesMeter::make(&start());
        let shooter = |keys| enemy(Vec3::new(30.0, 0.0, 0.0), Some(0), keys);
        meter.sample(&frame(0, Vec3::ZERO, 0.0, vec![shooter(key::FIRE)], vec![note(0, CLOUD_NOTE)]));
        meter.sample(&frame(1_000, Vec3::ZERO, 0.0, vec![shooter(key::FIRE)], Vec::new()));
        meter.sample(&frame(7_000, Vec3::ZERO, 0.0, vec![shooter(key::FIRE)], Vec::new()));
        meter.sample(&frame(8_000, Vec3::ZERO, 0.0, vec![shooter(0)], Vec::new()));
        let v = |n| meter.value(n).unwrap();
        // The frame at 7 s is the first inside the cloud (it pours at 5 s) and counts the 6 s since the one before; the one at 8 s adds a second
        // of a bot aimed at him that holds its fire.
        assert_eq!((v(0), v(1), v(2)), (7.0, 7.0, 6.0));
    }

    #[test]
    fn the_heard_meter_reads_the_distance_at_each_pour() {
        let mut meter = HeardMeter::make(&start());
        meter.sample(&frame(1_000, Vec3::new(-10.0, 0.0, 0.0), 0.0, Vec::new(), vec![note(1_000, CLOUD_NOTE)]));
        meter.sample(&frame(5_500, Vec3::new(-15.0, 0.0, 0.0), 0.0, Vec::new(), Vec::new()));
        assert_eq!((meter.value(0), meter.value(1), meter.value(2)), (Some(1.0), Some(25.0), None));
    }

    /// A cloud note starts the cloud step; it ends at its budget.
    #[test]
    fn dry_run_the_cloud_and_hides_steps() {
        let named = |id: &str| steps().into_iter().find(|s| s.id == id).unwrap().tries(1);
        let (results, _) = dry::run(named("smoke-cloud"), 90, |ms| {
            let mut notes = Vec::new();
            if ms == 5_000 {
                notes.push(note(ms, CLOUD_NOTE));
            }
            frame(ms, Vec3::new(0.0, 0.0, 0.0), 0.0, Vec::new(), notes)
        });
        assert!(results.contains("smoke-cloud") && results.contains("\tINFO\t") && results.contains("clouds=1"), "{results}");
        let (results, _) = dry::run(named("smoke-hides"), 60, |ms| {
            let notes = if ms == 5_000 { vec![note(ms, "smoke cloud by player 0 (team 0) at (10.0, 0.0, 0.0), pours at 5000 ms, gone at 50000 ms")] } else { Vec::new() };
            frame(ms, Vec3::ZERO, 0.0, vec![enemy(Vec3::new(30.0, 0.0, 0.0), Some(0), 0)], notes)
        });
        assert!(results.contains("smoke-hides") && results.contains("\tPASS\t") && results.contains("fire_s_while_smoked=0"), "{results}");
    }
}
