//! Smoke grenades (`rdg-m`): the clouds in the match, and what they hide from a bot's eyes.
//!
//! - **One owner.** The view's keeper ([`super::view::Tracker`]) sees every throw key, a human's as relayed
//!   and a bot's as made ([`super::live_grenades`]): a key that throws a smoke from a pocket becomes a
//!   [`SmokeThrow`], in the hand until its launch, then a [`Cloud`] where the arc model puts the grenade to
//!   rest (`arc.rs`, the frag's code with the smoke's own throw speed). The view carries the clouds
//!   ([`super::view::MatchView::smoke`]); nothing else makes or changes one.
//! - **A cloud** has a centre on the ground where the grenade rests, a time it begins to pour, and a shape over
//!   time from the game's data ([`CloudShape`]): it grows to its full radius, holds, then thins away. Times
//!   are match ms ([`MatchMs`]), parsed once from the tracker's clock.
//! - **Sight.** A sight line that runs through a live cloud for `SmokeTuning::thin_m` or more sees nothing past
//!   it ([`Veil::hides`]); a line that grazes the edge sees through. Bullets are not stopped: only the bots'
//!   eyes read the veil (noticing an enemy, `sense`; the aim's look at his parts, `aimpoint`), never the shot's
//!   wall test. A team whose tuning has `smoke.sight` off is given [`Veil::Clear`] and sees as before.
//! - **What a bot knows.** Every bot's eyes are blocked by every cloud: that is the cloud, not knowledge of it.
//!   Acting on a cloud (watching its edges, crossing under it) is for what a human sees: a cloud he could see
//!   form ([`Cloud::seen_forming_by`]).

use mapdata::Vec3;

use super::arc::Grenade;
use super::view::Place;

/// A match time, ms: the tracker's clock. Times of a cloud are of this kind, never a bare number.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct MatchMs(pub u32);

impl MatchMs {
    /// `ms` later (saturating).
    pub fn plus(self, ms: u32) -> Self {
        Self(self.0.saturating_add(ms))
    }

    /// How long after `earlier` this is, ms (0 when it is not after it).
    pub fn since(self, earlier: Self) -> u32 {
        self.0.saturating_sub(earlier.0)
    }
}

/// A cloud's shape over its life (`fx-smoke-data.md`, each number marked there). The client draws it from the
/// particle effect `fx/explosions/smoke_grenade`: 50 puffs of about 5 m (3.6-6.6) placed at once 0.2 s after the
/// blast, their centres in a cylinder of radius 8 m and height 2 m raised 1.5 m over the rest point, still, no
/// wind and no growth (PROVEN as read); each fades in over 1.9 s, holds to 17.5 s and fades out by 29 s (PROVEN,
/// the alpha keys; seconds LIKELY). The model: a vertical cylinder of the puffs' centres' radius, hiding once
/// the overlapping puffs are dense enough, its radius shrinking as they fade (the rim, with fewer puffs on a
/// line, clears first).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CloudShape {
    /// From the press to the blast, ms: the grenade's fuse (`ttl_ms` 4000, PROVEN; from the press LIKELY, as the
    /// frag's, which is PROVEN).
    pub fuse_ms: u32,
    /// From the blast until it hides, ms: the puffs appear at 0.2 s and fade in to 1.9 s; with four or more on a
    /// line through it, a quarter of their full opacity already hides (GUESS from the alpha keys: 0.7 s).
    pub forming_ms: u32,
    /// From the blast to the start of its fading, ms (17.5 s, the alpha keys).
    pub full_ms: u32,
    /// From the blast until it hides nothing, ms: the radius shrinks from full at `full_ms` to nothing here
    /// (GUESS: the puffs at a quarter of their opacity, 26 s of their 29).
    pub gone_ms: u32,
    /// The radius across the ground, metres: the puffs' centres' (8 m, PROVEN as read). The puffs reach about
    /// 2.5 m more, a thin rim seen through (`SmokeTuning::thin_m`).
    pub radius_m: f32,
    /// The cloud's top over the rest point, metres: the centres to 2.5-3.5 m and a puff's half size over them
    /// (LIKELY 5 m); it hides from half a metre under the rest point (a body on a slope) to there.
    pub height_m: f32,
}

impl CloudShape {
    /// The whole life from the blast, ms.
    pub fn life_ms(&self) -> u32 {
        self.gone_ms
    }

    /// The radius `ms` after the blast that hides: none while it forms and once it is gone, full, then
    /// shrinking.
    pub fn radius_after(&self, ms: u32) -> Option<f32> {
        let full = self.radius_m;
        if ms < self.forming_ms || ms >= self.gone_ms {
            None
        } else if ms < self.full_ms {
            Some(full)
        } else {
            let left = self.gone_ms - ms;
            Some(full * left as f32 / (self.gone_ms - self.full_ms).max(1) as f32)
        }
    }
}

/// The smoke grenade: how it flies (the arc model's numbers) and its cloud.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SmokeGrenade {
    /// For the arc: the throw speed, and the fuse as the time it may fly.
    pub flight: Grenade,
    pub cloud: CloudShape,
}

/// The game data's smoke grenade.
pub const SMOKE_ITEM: &str = "rdg-m";

impl SmokeGrenade {
    /// The 0.26g RDG-M (`fx-smoke-data.md`): the RGD-5's grenade class with no damage (fuse 4000 ms, throw
    /// power 17, PROVEN), and the cloud of its particle effect ([`CloudShape`]).
    pub const RDG_M: SmokeGrenade = SmokeGrenade {
        flight: Grenade {
            speed: 17.0,
            fuse_s: 4.0,
            splash_m: 0.0,
        },
        cloud: CloudShape {
            fuse_ms: 4_000,
            forming_ms: 700,
            full_ms: 17_500,
            gone_ms: 26_000,
            radius_m: 8.0,
            height_m: 5.0,
        },
    };

    /// The smoke grenade of the game data: its config's fuse and throw power (a patched item flies and goes off
    /// by its own), the cloud the RDG-M's (the client draws it from a particle effect the server's data does
    /// not hold). Read once.
    pub fn of_game_data() -> &'static SmokeGrenade {
        static SHARED: std::sync::OnceLock<SmokeGrenade> = std::sync::OnceLock::new();
        SHARED.get_or_init(|| match survarium::v026g::weapons::grenade_named(SMOKE_ITEM) {
            Some(params) => {
                let mut smoke = Self::RDG_M;
                smoke.flight.speed = params.throw_power;
                smoke.flight.fuse_s = params.ttl_ms as f32 / 1000.0;
                smoke.cloud.fuse_ms = params.ttl_ms;
                smoke
            }
            None => {
                log::error!("the game data has no smoke grenade {SMOKE_ITEM}: the 0.26g RDG-M's numbers stand in");
                Self::RDG_M
            }
        })
    }
}

/// A smoke cloud: where, whose, when it begins to pour, and its shape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cloud {
    pub by: Place,
    pub team: u8,
    /// Where the grenade rests: the cloud's centre on the ground.
    pub centre: Vec3,
    /// When the grenade was thrown (the key went down), and when the smoke begins.
    pub thrown: MatchMs,
    pub pours: MatchMs,
    pub shape: CloudShape,
}

impl Cloud {
    /// When it is gone.
    pub fn ends(&self) -> MatchMs {
        self.pours.plus(self.shape.life_ms())
    }

    /// Its radius that hides at `at`: none before it hides (in the air, forming) and once gone.
    pub fn radius_at(&self, at: MatchMs) -> Option<f32> {
        if at < self.pours {
            return None;
        }
        self.shape.radius_after(at.since(self.pours))
    }

    /// Whether it is (or will be) there at `at`: not yet ended.
    pub fn live_at(&self, at: MatchMs) -> bool {
        at < self.ends()
    }

    /// The length of the line from `a` to `b` inside it at `at`, metres: a vertical cylinder of its radius
    /// then, from half a metre under its centre to its top.
    pub fn inside_m(&self, a: Vec3, b: Vec3, at: MatchMs) -> f32 {
        let Some(r) = self.radius_at(at).filter(|r| *r > 0.0) else {
            return 0.0;
        };
        let d = b - a;
        let length = d.length();
        if !length.is_finite() || length <= 0.0 {
            return 0.0;
        }
        // The part of the line (0..1) within the cloud's height.
        let (bottom, top) = (self.centre.y - 0.5, self.centre.y + self.shape.height_m);
        let (mut t0, mut t1) = (0.0f32, 1.0f32);
        if d.y.abs() < 1e-6 {
            if a.y < bottom || a.y > top {
                return 0.0;
            }
        } else {
            let (ta, tb) = ((bottom - a.y) / d.y, (top - a.y) / d.y);
            t0 = t0.max(ta.min(tb));
            t1 = t1.min(ta.max(tb));
        }
        if t0 >= t1 {
            return 0.0;
        }
        // And within its radius across the ground: |(a - c) + t d| <= r on x, z.
        let (px, pz) = (a.x - self.centre.x, a.z - self.centre.z);
        let qa = d.x * d.x + d.z * d.z;
        let qb = 2.0 * (px * d.x + pz * d.z);
        let qc = px * px + pz * pz - r * r;
        if qa < 1e-9 {
            // A vertical line: inside wholly or not at all.
            return if qc <= 0.0 { (t1 - t0) * length } else { 0.0 };
        }
        let disc = qb * qb - 4.0 * qa * qc;
        if disc <= 0.0 {
            return 0.0;
        }
        let root = disc.sqrt();
        let (c0, c1) = ((-qb - root) / (2.0 * qa), (-qb + root) / (2.0 * qa));
        let (t0, t1) = (t0.max(c0), t1.min(c1));
        if t0 >= t1 { 0.0 } else { (t1 - t0) * length }
    }

    /// Whether a body with its eyes at `eye`, facing anywhere, could have seen it form: its centre in a line
    /// from the eye (the map's `sees`) within `see_m` while it poured, or the thrower one of its team. What a
    /// human knows of a smoke: he saw it go up, or his mate threw it.
    pub fn seen_forming_by(
        &self,
        team: u8,
        eye: Vec3,
        see_m: f32,
        sees: impl Fn(Vec3, Vec3) -> bool,
    ) -> bool {
        team == self.team
            || (eye.distance_xz(self.centre) <= see_m
                && sees(eye, self.centre + Vec3::new(0.0, 1.0, 0.0)))
    }
}

/// The clouds in the match at one moment (the view's): those not yet ended, in the order thrown.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Clouds(Vec<Cloud>);

impl Clouds {
    pub fn new(clouds: Vec<Cloud>) -> Self {
        Self(clouds)
    }

    pub fn all(&self) -> &[Cloud] {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// What a team's eyes go by at `clock` (match ms): the clouds if its tuning has `smoke.sight` on and one
    /// has smoke now, else [`Veil::Clear`] (no line is tested).
    pub fn veil(&self, clock: u32, tuning: &super::tuning::SmokeTuning) -> Veil<'_> {
        let at = MatchMs(clock);
        if !tuning.sight || !self.0.iter().any(|c| c.radius_at(at).is_some()) {
            return Veil::Clear;
        }
        Veil::Smoke {
            clouds: &self.0,
            at,
            thin_m: tuning.thin_m.get(),
        }
    }
}

/// What a bot's eyes go by: no smoke to read, or the clouds at a moment.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Veil<'a> {
    /// No cloud is read: the sight is the bullet mesh's alone (the switch off, or no smoke in the match now).
    #[default]
    Clear,
    Smoke {
        clouds: &'a [Cloud],
        at: MatchMs,
        /// A line inside a cloud for this long or more hides, metres (`SmokeTuning::thin_m`).
        thin_m: f32,
    },
}

impl Veil<'_> {
    /// Whether smoke hides `b` from `a`: the line runs through one cloud for `thin_m` or more, or through
    /// several for that much in all. No ray is cast.
    pub fn hides(&self, a: Vec3, b: Vec3) -> bool {
        match *self {
            Veil::Clear => false,
            Veil::Smoke { clouds, at, thin_m } => {
                let mut inside = 0.0;
                for cloud in clouds {
                    inside += cloud.inside_m(a, b, at);
                    if inside >= thin_m {
                        return true;
                    }
                }
                false
            }
        }
    }

    /// Whether any cloud is read.
    pub fn is_clear(&self) -> bool {
        matches!(self, Veil::Clear)
    }
}

/// A smoke thrown and not yet a cloud: the key is down, the grenade in the hand (the view may still turn).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SmokeThrow {
    pub by: Place,
    pub team: u8,
    pub press: MatchMs,
    pub grenade: SmokeGrenade,
}

impl SmokeThrow {
    /// The cloud of this throw once the arc put the grenade to rest at `rest`.
    pub fn cloud_at(&self, rest: Vec3) -> Cloud {
        Cloud {
            by: self.by,
            team: self.team,
            centre: rest,
            thrown: self.press,
            pours: self.press.plus(self.grenade.cloud.fuse_ms),
            shape: self.grenade.cloud,
        }
    }
}

/// What the tracker made of smoke throws (a counter, for the logs and the benches), by the thrower's team.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SmokeCounts {
    /// Smoke throw keys seen.
    pub thrown: [u32; 2],
    /// Clouds placed (the arc found a rest).
    pub clouds: [u32; 2],
    /// Dropped: the thrower died before the launch, or the arc found no ground.
    pub dropped: [u32; 2],
}

/// The bench's counts of what smoke did to the bots' eyes, by the bot's team, on this thread (a bench match
/// runs on one thread; the tally reads the change over the match).
#[cfg(test)]
pub mod tally {
    use std::cell::Cell;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Event {
        /// A think's look at an enemy it had in sight found smoke hiding him (`sense`).
        SightLost,
        /// A think's look at an enemy not in sight found him behind smoke, not the world (`sense`).
        NotSeen,
        /// The aim's look at its target found smoke hiding him (`aimpoint`).
        AimLost,
        /// A tick of fire sprayed at the last part seen of a target gone into smoke.
        Spray,
    }

    pub const EVENTS: usize = 4;

    thread_local! {
        static COUNTS: Cell<[[u64; EVENTS]; 2]> = const { Cell::new([[0; EVENTS]; 2]) };
    }

    pub fn add(team: u8, event: Event) {
        COUNTS.with(|c| {
            let mut counts = c.get();
            counts[usize::from(team & 1)][event as usize] += 1;
            c.set(counts);
        });
    }

    /// By team, by [`Event`] in order.
    pub fn counts() -> [[u64; EVENTS]; 2] {
        COUNTS.with(Cell::get)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::bots::tuning::SmokeTuning;

    fn cloud(at: Vec3) -> Cloud {
        SmokeThrow {
            by: 0,
            team: 0,
            press: MatchMs(1_000),
            grenade: SmokeGrenade::RDG_M,
        }
        .cloud_at(at)
    }

    #[test]
    fn a_cloud_forms_holds_and_thins_away_from_its_blast() {
        let c = cloud(Vec3::ZERO);
        let s = c.shape;
        assert_eq!(c.pours, MatchMs(1_000 + 4_000), "the fuse from the press");
        assert_eq!(c.radius_at(MatchMs(1_000)), None, "in the air");
        let pours = c.pours.0;
        assert_eq!(c.radius_at(MatchMs(pours + 500)), None, "forming");
        assert_eq!(c.radius_at(MatchMs(pours + 700)), Some(8.0));
        assert_eq!(c.radius_at(MatchMs(pours + 17_499)), Some(8.0));
        let fading = c.radius_at(MatchMs(pours + (s.full_ms + s.gone_ms) / 2)).unwrap();
        assert!((fading - 4.0).abs() < 0.01, "{fading}");
        assert_eq!(c.radius_at(c.ends()), None);
        assert_eq!(c.ends(), MatchMs(1_000 + 4_000 + 26_000));
        assert!(c.live_at(MatchMs(1_000)) && !c.live_at(c.ends()));
    }

    #[test]
    fn a_line_through_a_full_cloud_is_inside_it_for_its_width_and_a_graze_is_thin() {
        let c = cloud(Vec3::ZERO);
        let full = MatchMs(c.pours.0 + c.shape.forming_ms + 100);
        let r = c.shape.radius_m;
        // Through the middle at eye height: the whole diameter.
        let m = c.inside_m(Vec3::new(-20.0, 1.6, 0.0), Vec3::new(20.0, 1.6, 0.0), full);
        assert!((m - 2.0 * r).abs() < 1e-3, "{m}");
        // Past it, and over its top: nothing.
        assert_eq!(c.inside_m(Vec3::new(-20.0, 1.6, r + 0.1), Vec3::new(20.0, 1.6, r + 0.1), full), 0.0);
        let over = c.shape.height_m + 0.5;
        assert_eq!(c.inside_m(Vec3::new(-20.0, over, 0.0), Vec3::new(20.0, over, 0.0), full), 0.0);
        // A line that ends inside: from the edge to the end.
        let m = c.inside_m(Vec3::new(-20.0, 1.6, 0.0), Vec3::new(0.0, 1.6, 0.0), full);
        assert!((m - r).abs() < 1e-3, "{m}");
        // A graze 0.01 m inside the edge is a 0.8 m chord: under thin_m, seen through.
        let tuning = SmokeTuning::default();
        let clouds = Clouds::new(vec![c]);
        let veil = clouds.veil(full.0, &tuning);
        let z = r - 0.01;
        assert!(!veil.hides(Vec3::new(-20.0, 1.6, z), Vec3::new(20.0, 1.6, z)));
        assert!(veil.hides(Vec3::new(-20.0, 1.6, 0.0), Vec3::new(20.0, 1.6, 0.0)));
    }

    #[test]
    fn the_veil_is_clear_with_the_switch_off_before_the_smoke_and_after_it() {
        let c = cloud(Vec3::ZERO);
        let clouds = Clouds::new(vec![c]);
        let on = SmokeTuning::default();
        let off = SmokeTuning {
            sight: false,
            ..SmokeTuning::default()
        };
        let full = c.pours.0 + c.shape.forming_ms;
        assert!(!clouds.veil(full, &on).is_clear());
        assert!(clouds.veil(full, &off).is_clear());
        assert!(clouds.veil(c.thrown.0, &on).is_clear(), "still in the air");
        assert!(clouds.veil(c.ends().0, &on).is_clear(), "gone");
        assert!(Clouds::default().veil(full, &on).is_clear());
    }

    #[test]
    fn a_cloud_is_known_to_the_throwers_team_and_to_an_enemy_who_saw_it_form() {
        let c = cloud(Vec3::new(30.0, 0.0, 0.0));
        let open = |_: Vec3, _: Vec3| true;
        let walled = |_: Vec3, _: Vec3| false;
        let eye = Vec3::new(0.0, 1.6, 0.0);
        assert!(c.seen_forming_by(0, eye, 10.0, walled), "his mate threw it");
        assert!(c.seen_forming_by(1, eye, 40.0, open));
        assert!(!c.seen_forming_by(1, eye, 40.0, walled), "behind a wall");
        assert!(!c.seen_forming_by(1, eye, 20.0, open), "too far");
    }
}
