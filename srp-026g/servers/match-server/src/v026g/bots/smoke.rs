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

use survarium::v026g::match_msgs::grenades::GrenadeId;

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
///
/// These are code constants read off the content (`particles/fx/explosions/smoke_grenade.particle`, which the server's
/// game data does not hold): a content patch that changes that effect must change them with it.
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
    /// The cloud's top over the floor under the rest point, metres: the centres to 2.5-3.5 m and a puff's half
    /// size over them (LIKELY 5 m).
    pub height_m: f32,
    /// How far under that floor it reaches, metres: the lowest centres at +0.5 m less a puff's half size (1.8-3.3
    /// m) reach 1.3-2.8 m under it; the least, 1.3 m (LIKELY: the cylinder's centring is a GUESS, `fx-smoke-data.md`
    /// section 2). A body on a slope below the cloud is still in it, as the client draws it.
    pub below_m: f32,
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
            below_m: 1.3,
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

/// Where a cloud's centre comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placed {
    /// The arc model's rest from the thrower's view at the launch (`arc.rs`: median 2 m off, P80 8 m).
    Arc,
    /// The grenade's body at rest as a trusted world showed it (`match_msgs::grenades`): the world's time, the body.
    Seen { world_ms: u32, id: GrenadeId },
}

/// A world's grenade is a cloud's when its blast is the cloud's within this, ms (the world's blast time is the press
/// plus the fuse on the 50 ms grid; the server's press is the relayed input's time).
pub const BLAST_MATCH_MS: u32 = 100;
/// A later sighting that moves a cloud this far or more is noted, metres.
pub const BODY_NOTE_M: f32 = 1.0;

/// The floor under `at`: the first surface of the bullet mesh straight down from just over it, within
/// [`FLOOR_BELOW_M`].
pub fn floor_under(mesh: &mapdata::RayTree, at: Vec3) -> Option<f32> {
    let from = at + Vec3::new(0.0, 0.2, 0.0);
    mesh.ray_cast(from, Vec3::new(0.0, -1.0, 0.0), FLOOR_BELOW_M)
        .map(|t| from.y - t)
}

/// A smoke cloud: where, whose, when it begins to pour, and its shape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cloud {
    pub by: Place,
    pub team: u8,
    /// Where its centre comes from: the arc's prediction until a world shows the grenade.
    pub placed: Placed,
    /// The floor under where the grenade rests (`SmokeThrow::cloud_at`): the cloud's centre on the ground.
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

    /// Whether `point` is inside it at `at`.
    pub fn holds(&self, point: Vec3, at: MatchMs) -> bool {
        let (bottom, top) = self.span();
        self.radius_at(at)
            .is_some_and(|r| point.distance_xz(self.centre) <= r && point.y >= bottom && point.y <= top)
    }

    /// Its bottom and top, metres.
    fn span(&self) -> (f32, f32) {
        (self.centre.y - self.shape.below_m, self.centre.y + self.shape.height_m)
    }

    /// The length of the line from `a` to `b` inside it at `at`, metres: a vertical cylinder of its radius
    /// then, from `below_m` under its centre to its top.
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
        let (bottom, top) = self.span();
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

    /// Whether smoke covers `point` from `eye` at `clock` for a team whose tuning is `tuning`: the one home of
    /// "does smoke hide this" for every query that gives a bot knowledge by looking (noticing, the aim, a spot
    /// or lane checked by looking, a threat spot cleared as seen empty). Planning queries that predict cover from
    /// the map alone do not call it.
    pub fn covers(&self, clock: u32, tuning: &super::tuning::SmokeTuning, eye: Vec3, point: Vec3) -> bool {
        self.veil(clock, tuning).hides(eye, point)
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
                    // Strictly more, as Valve's `IsLineBlockedBySmoke`.
                    if inside > thin_m {
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

/// How far under a rest point the floor is looked for, metres: a grenade that stopped on a wall (20 of 45 recorded
/// first contacts, `arc.rs`) comes down to the floor under it.
pub const FLOOR_BELOW_M: f32 = 30.0;

impl SmokeThrow {
    /// The cloud of this throw once the arc put the grenade to rest at `rest`: centred on the floor under it
    /// (`floor`: the first surface of the bullet mesh straight down from just over the rest, none for no map or
    /// none within [`FLOOR_BELOW_M`]: the rest itself). The arc's rest keeps its first contact's height, a wall's
    /// or a slope's; the grenade falls to the floor (the client's physics body), and its particles are placed there.
    pub fn cloud_at(&self, rest: Vec3, floor: impl FnOnce(Vec3) -> Option<f32>) -> Cloud {
        let centre = floor(rest).map_or(rest, |y| Vec3::new(rest.x, y, rest.z));
        Cloud {
            by: self.by,
            team: self.team,
            placed: Placed::Arc,
            centre,
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
    /// Clouds whose grenade a world showed, and how far the first sighting moved them from the arc's rest, decimetres
    /// summed (the arc's error, measured live).
    pub bodies_seen: [u32; 2],
    pub moved_dm: [u32; 2],
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

    thread_local! {
        static SPRAYED: std::cell::RefCell<Vec<(usize, u32)>> = const { std::cell::RefCell::new(Vec::new()) };
    }

    /// The bot at `place` fired a spray at `now`: the fairness count leaves that shot out.
    pub fn sprayed(place: usize, now: u32) {
        SPRAYED.with(|s| {
            let mut s = s.borrow_mut();
            s.retain(|&(p, _)| p != place);
            s.push((place, now));
        });
    }

    /// When the bot at `place` last fired a spray.
    pub fn last_spray(place: usize) -> Option<u32> {
        SPRAYED.with(|s| s.borrow().iter().find(|&&(p, _)| p == place).map(|&(_, t)| t))
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
        .cloud_at(at, |_| None)
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

/// The sight paths through a cloud, as a bot meets one: noticing an enemy (`sense`) and the aim's look at its target
/// (`aimpoint::Sight`), over the cloud's life, at its edge and over its top; and the cost of the test.
#[cfg(test)]
mod scenarios {
    use std::sync::Arc;

    use mapdata::{RayTree, World};

    use super::*;
    use crate::v026g::bots::aimpoint::{self, PartsLook, Sight};
    use crate::v026g::bots::know::{LookOutcome, Miss};
    use crate::v026g::bots::rng::Rng;
    use crate::v026g::bots::sense::{self, RayBudget};
    use crate::v026g::bots::tuning::{SenseTuning, SmokeTuning, Tuning};
    use crate::v026g::bots::view::{MatchView, Mode, PlaceView};

    fn body(place: Place, team: u8, feet: Vec3) -> PlaceView {
        PlaceView {
            place,
            team,
            bot: place == 0,
            present: true,
            alive: true,
            feet,
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
        }
    }

    /// The bot at `me` facing +z, an enemy at `him`, the clouds centred at `clouds` (all poured at 0), at `clock`.
    fn view(me: Vec3, him: Vec3, clouds: &[Vec3], clock: u32) -> MatchView {
        MatchView {
            clock,
            mode: Mode::TeamDeathmatch,
            time_left_s: 600.0,
            total_s: 900.0,
            points: [0, 0],
            places: vec![body(0, 0, me), body(1, 1, him)],
            batteries: vec![],
            marks: vec![],
            containers: vec![],
            sounds: vec![],
            deaths: vec![],
            world_time: 0,
            can_respawn: [true, true],
            next_battery: None,
            grenades: Vec::new(),
            smoke: Clouds::new(
                clouds
                    .iter()
                    .map(|&centre| Cloud {
                        by: 1,
                        team: 1,
                        placed: Placed::Arc,
                        centre,
                        thrown: MatchMs(0),
                        pours: MatchMs(0),
                        shape: SmokeGrenade::RDG_M.cloud,
                    })
                    .collect(),
            ),
        }
    }

    /// What a think's look at the enemy came to (the sight refresh on), and whether the aim's look found a part.
    fn looks(view: &MatchView) -> (LookOutcome, bool) {
        let smoke = SmokeTuning::default();
        let veil = view.smoke.veil(view.clock, &smoke);
        let sense = SenseTuning {
            sight_refresh: true,
            ..SenseTuning::default()
        };
        let me = &view.places[0];
        let (_, looks) = sense::sense_looks_through(
            me,
            view,
            &RayTree::build(&[]),
            veil,
            &sense,
            &Tuning::all_off().cheats,
            0,
            &[1],
            &mut RayBudget::new(10),
            &mut Rng::new(1),
        );
        let world = Arc::new(World {
            map: mapdata::MapData::default(),
            grid: Arc::new(mapdata::WalkGrid::build(&[], [0.0, 0.0, 1.0, 1.0], 0.25, mapdata::Body::default())),
            sight: Arc::new(RayTree::build(&[])),
        });
        let look = Sight::default().update(me, Some(1), view, &world, &smoke, view.clock).unwrap();
        assert_ne!(look.look, PartsLook::Hidden, "{look:?}");
        (looks[0].1, look.look == PartsLook::Shows)
    }

    const SEEN: (LookOutcome, bool) = (LookOutcome::Seen, true);
    const SMOKED: (LookOutcome, bool) = (LookOutcome::Failed(Miss::Smoke), false);

    #[test]
    fn over_its_life_a_cloud_between_hides_him_from_both_looks_and_he_is_seen_again_as_it_thins() {
        let (me, him) = (Vec3::ZERO, Vec3::new(0.0, 0.0, 30.0));
        // Across the line, 4 m to its side: the line runs 13.9 m through it at the full 8 m radius.
        let cloud = [Vec3::new(4.0, 0.0, 15.0)];
        assert_eq!(looks(&view(me, him, &cloud, 500)), SEEN, "forming: not dense yet");
        assert_eq!(looks(&view(me, him, &cloud, 700)), SMOKED);
        assert_eq!(looks(&view(me, him, &cloud, 17_000)), SMOKED);
        // Thinning: the radius is 5.6 m at 20 s, still over the line; 3.8 m at 22 s, off it.
        assert_eq!(looks(&view(me, him, &cloud, 20_000)), SMOKED);
        assert_eq!(looks(&view(me, him, &cloud, 22_000)), SEEN);
        assert_eq!(looks(&view(me, him, &cloud, 26_000)), SEEN, "gone");
    }

    #[test]
    fn its_edge_and_its_top_are_where_the_hiding_ends() {
        let (me, him) = (Vec3::ZERO, Vec3::new(0.0, 0.0, 30.0));
        // Every part's line passes within 7.6 m of a cloud 7.5 m aside (2.5 m or more inside it): hidden. One
        // 8.5 m aside is outside the 8 m radius (its 2.5 m rim is thin and seen through): seen.
        assert_eq!(looks(&view(me, him, &[Vec3::new(7.5, 0.0, 15.0)], 5_000)), SMOKED);
        assert_eq!(looks(&view(me, him, &[Vec3::new(8.5, 0.0, 15.0)], 5_000)), SEEN);
        // From one roof 8 m up to another over a cloud on the ground between: the line is over its 5 m top.
        let (high_me, high_him) = (Vec3::new(0.0, 8.0, 0.0), Vec3::new(0.0, 8.0, 30.0));
        assert_eq!(looks(&view(high_me, high_him, &[Vec3::new(0.0, 0.0, 15.0)], 5_000)), SEEN);
        // A cloud on the same roof between them hides him again.
        assert_eq!(looks(&view(high_me, high_him, &[Vec3::new(0.0, 8.0, 15.0)], 5_000)), SMOKED);
        // Two clouds whose rims each hold under 2 m of the line add up past it.
        let tuning = SmokeTuning::default();
        let chest = Vec3::new(0.0, 1.4, 30.0);
        let rims = [Vec3::new(7.95, 0.0, 10.0), Vec3::new(-7.95, 0.0, 20.0)];
        let one = view(me, him, &rims[..1], 5_000);
        assert!(!one.smoke.covers(5_000, &tuning, one.places[0].eye(), chest));
        let both = view(me, him, &rims, 5_000);
        assert!(both.smoke.covers(5_000, &tuning, both.places[0].eye(), chest));
    }

    #[test]
    fn from_inside_a_cloud_the_near_are_seen_and_the_far_are_not() {
        // The thrower in his own cloud (40 % of human smokes rest within 8 m of the thrower).
        let me = Vec3::new(0.0, 0.0, 0.0);
        let cloud = [Vec3::new(0.0, 0.0, 0.0)];
        // 1.5 m off, inside it too: the line runs 1.5 m through smoke, under the 2 m it takes to hide.
        assert_eq!(looks(&view(me, Vec3::new(0.0, 0.0, 1.5), &cloud, 5_000)), SEEN);
        // 20 m off, outside: 8 m of smoke from the eye to the edge.
        assert_eq!(looks(&view(me, Vec3::new(0.0, 0.0, 20.0), &cloud, 5_000)), SMOKED);
    }

    #[test]
    fn a_cloud_hides_bodies_on_a_slope_below_its_floor_and_not_a_floor_under_it() {
        let cloud = [Vec3::new(0.0, 0.0, 15.0)];
        // Both 1.2 m down a slope from the rest point's floor: legs and all within the cloud's 1.3 m under it.
        let (low_me, low_him) = (Vec3::new(0.0, -1.2, 0.0), Vec3::new(0.0, -1.2, 30.0));
        assert_eq!(looks(&view(low_me, low_him, &cloud, 5_000)), SMOKED);
        // A floor 4 m under it (a cloud on a balcony): the lines pass under its base.
        let (under_me, under_him) = (Vec3::new(0.0, -4.0, 0.0), Vec3::new(0.0, -4.0, 30.0));
        assert_eq!(looks(&view(under_me, under_him, &cloud, 5_000)), SEEN);
    }

    #[test]
    fn a_grenade_that_stopped_on_a_wall_makes_its_cloud_on_the_floor_under_it() {
        // A floor at 0 and a wall at z = 12; the arc's rest is on the wall 2.5 m up.
        let (a, b) = (Vec3::new(-50.0, 0.0, -50.0), Vec3::new(50.0, 0.0, -50.0));
        let (c, d) = (Vec3::new(50.0, 0.0, 50.0), Vec3::new(-50.0, 0.0, 50.0));
        let (w0, w1) = (Vec3::new(-10.0, 0.0, 12.0), Vec3::new(10.0, 0.0, 12.0));
        let (w2, w3) = (Vec3::new(10.0, 4.0, 12.0), Vec3::new(-10.0, 4.0, 12.0));
        let mesh = RayTree::from_corners([[a, b, c], [a, c, d], [w0, w1, w2], [w0, w2, w3]]);
        let throw = SmokeThrow {
            by: 1,
            team: 1,
            press: MatchMs(0),
            grenade: SmokeGrenade::RDG_M,
        };
        let floor = |rest: Vec3| {
            let from = rest + Vec3::new(0.0, 0.2, 0.0);
            mesh.ray_cast(from, Vec3::new(0.0, -1.0, 0.0), FLOOR_BELOW_M).map(|t| from.y - t)
        };
        let cloud = throw.cloud_at(Vec3::new(0.0, 2.5, 11.95), floor);
        assert!(cloud.centre.y.abs() < 1e-3, "{:?}", cloud.centre);
        // Its base on the floor hides a man's shins behind it from a crouched eye (1.0 m): from the rest's height
        // (its base 1.2 m up) the line ran under it.
        let at = MatchMs(cloud.pours.0 + 5_000);
        let clouds = Clouds::new(vec![cloud]);
        let tuning = SmokeTuning::default();
        let eye = Vec3::new(0.0, 1.0, 0.0);
        assert!(clouds.covers(at.0, &tuning, eye, Vec3::new(0.0, 0.3, 20.0)), "shins");
        let up = Clouds::new(vec![throw.cloud_at(Vec3::new(0.0, 2.5, 11.95), |_| None)]);
        assert!(!up.covers(at.0, &tuning, eye, Vec3::new(0.0, 0.3, 20.0)), "the rest's height leaves the shins");
        // No floor under it within reach: the rest itself.
        assert_eq!(throw.cloud_at(Vec3::new(0.0, 2.5, 11.95), |_| None).centre.y, 2.5);
    }

    /// The cost of a sight query (`aimpoint::look_parts` through the veil) with 0, 1 and 4 live clouds off the
    /// line (the usual case: smoke elsewhere on the map), and one across it (no ray cast for a hidden part).
    /// `cargo test -p match-server --release smoke_sight_cost -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn smoke_sight_cost() {
        let (me, him) = (Vec3::ZERO, Vec3::new(0.0, 0.0, 30.0));
        let sight = RayTree::build(&[]);
        let tuning = SmokeTuning::default();
        let elsewhere = [
            Vec3::new(40.0, 0.0, 15.0),
            Vec3::new(-40.0, 0.0, 15.0),
            Vec3::new(40.0, 0.0, 60.0),
            Vec3::new(-40.0, 0.0, -30.0),
        ];
        let cases: [(&str, Vec<Vec3>); 4] = [
            ("0 clouds", vec![]),
            ("1 cloud off the line", elsewhere[..1].to_vec()),
            ("4 clouds off the line", elsewhere.to_vec()),
            ("1 cloud across the line", vec![Vec3::new(0.0, 0.0, 15.0)]),
        ];
        const N: u32 = 200_000;
        for (name, clouds) in cases {
            let v = view(me, him, &clouds, 5_000);
            let veil = v.smoke.veil(v.clock, &tuning);
            let eye = v.places[0].eye();
            let start = std::time::Instant::now();
            let mut shown = 0u32;
            for _ in 0..N {
                let look = aimpoint::look_parts(eye, &v.places[1], &sight, veil, || true);
                shown += u32::from(look == PartsLook::Shows);
            }
            let ns = start.elapsed().as_nanos() as f64 / f64::from(N);
            println!("smoke_sight_cost: {name}: {ns:.0} ns a look ({shown} of {N} shown)");
        }
    }
}
