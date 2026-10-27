//! The flight of a thrown grenade as the 0.26g client launches it, where it
//! first touches the world and where it comes to rest: what a bot needs to
//! throw one where it means to and to keep its own side out of the blast
//! (play 20, `reflex`), and what an escape from a grenade needs to know
//! where one will go off. Pure maths and a ground to ask, nothing is cast
//! here but through [`Ground`].
//!
//! Read in the 0.26g client's code (the private `fact-grenade-arc.md` and
//! `fact-grenade-rest.md` of the fighting research; tags as there):
//! - **The launch is an event of the throw animation** (0x310680 calls
//!   0x33A770, which calls 0x30A250 `launch_grenade`): the grenade leaves
//!   **along the view** (the head transform's forward row, player+0x44C) at
//!   `throw_power × max(0, 1 + Σ modifier 27)` = 17 m/s for the bots (the
//!   grenade's `throw_power`, [`Grenade::speed`]), with no lift angle and none of the thrower's own speed (0x30A2D5..0x30A312).
//!   PROVEN. It leaves about [`LAUNCH_DELAY_S`] after the key goes down (the
//!   animation's event, not in the code) and the view that counts is the
//!   one **at the launch**, not at the press. LIKELY (a fit, ±0.2 s). The
//!   start is the right hand's bone at that frame, not the eye (0x309F00);
//!   where it is depends on the animation: [`HAND_DOWN_M`] is a GUESS.
//! - **The fuse runs from the press** (0x30A120 from 0x3103D7, 80 of 82
//!   recorded blasts at press + 4000 ms): [`Grenade::fuse_s`], so the flight
//!   has [`Grenade::flight_s`] left. PROVEN.
//! - **The body** (`grenade_core::load` 0x303890 → 0x357100): a capsule of
//!   radius [`RADIUS_M`], 0.3 kg, no damping, restitution 0, friction 0.5,
//!   never asleep, in Bullet 2.81 with gravity `(0, -10, 0)` (0x74E3D) and
//!   `stepSimulation(dt, 3, 1/60)` each 50 ms tick (0x75610, constant
//!   0xA2DBB4): three fixed steps of [`STEP_S`], gravity into the velocity
//!   and then the move. PROVEN.
//! - **After the first contact** the body keeps its tangential speed and
//!   loses the normal one (restitution 0), with the two frictions' product
//!   [`FRICTION`] (0x8E870, nothing sets either afterwards; the contact
//!   callback 0x1F1D3B4 is never written). PROVEN. Nothing stops it before
//!   the fuse, but on the real maps it stops soon: [`roll_m`] is what the
//!   recorded blasts say. LIKELY (26 kill cases, a median 2 m beyond the
//!   first contact, 80 % within 8 m; only grenades that killed are in it,
//!   so the tail is a lower bound).
//! - **No explosion on contact** (`explode_on_contact` false, `safe_time`
//!   0, 0x248A33); the blast is [`Grenade::splash_m`] around the grenade.
//!   The three numbers are the grenade's config (`weapons::grenade_named`),
//!   read from the game data: a grenade patch reaches the bots at the
//!   restart (the RGD-5 of the bots' kit is [`Grenade::bots`]).
//!
//! The fall is the sum Bullet makes, not the closed form: the two differ by
//! up to 0.15 m at the end of a level throw (9.32 against 9.47 m).
//!
//! Frames: the map's (x, z on the ground, y up); a yaw `a` faces
//! `(-sin a, cos a)` ([`kin::forward`]), a positive pitch looks up (the
//! game clamps it at ±1, [`PITCH_LIMIT`]).

use mapdata::{RayTree, Vec3};

use super::kin;

/// Gravity, m/s² (`setGravity(0, -10, 0)`, 0x74E3D..0x74E62). PROVEN.
pub const GRAVITY: f32 = 10.0;
/// One physics step, seconds: Bullet's fixed step (0xA2DBB4), three to the
/// game's 50 ms tick. PROVEN.
pub const STEP_S: f32 = 1.0 / 60.0;
/// The capsule's radius, metres: the grenade's centre is this far from the
/// surface it lies on (`create_bt_primitive` type 3, 0x303900). PROVEN.
pub const RADIUS_M: f32 = 0.05;
/// The two bodies' combined friction on the level, 0.5 × 0.5 (0x8E870;
/// Bullet multiplies them in `addContactPoint`, 0xAE310). PROVEN.
pub const FRICTION: f32 = 0.25;
/// From the key going down to the grenade leaving the hand, seconds (the
/// throw animation's event). LIKELY, ±0.2 s.
pub const LAUNCH_DELAY_S: f32 = 1.1;
/// How far under the eye the grenade starts, metres: the hand at the
/// launch frame. GUESS (the data cannot tell it from the eye; 0.3 m
/// shortens a level throw from 9.3 to 8.4 m).
pub const HAND_DOWN_M: f32 = 0.3;
/// The most the game lets the view tilt, either way, radians. PROVEN
/// ([`kin`]).
pub const PITCH_LIMIT: f32 = 1.0;

/// A surface whose normal leans less than this from level (its `y` under
/// it) is a wall: the grenade falls at its foot. LIKELY (20 of the 45
/// recorded first contacts were walls, with a median roll of 1.2 m).
pub const WALL_NORMAL_Y: f32 = 0.5;
/// How far past the first contact the grenade lies, metres, on a floor or a
/// slope: the median of the recorded blasts (+1.6 to +3.8 m by the three
/// ways of finding the contact). LIKELY.
pub const ROLL_M: f32 = 2.0;
/// The 80th percentile of the roll, metres: the reach of the mates' check
/// (recorded +7 to +8.5 m). LIKELY.
pub const ROLL_P80_M: f32 = 8.0;
/// The worst credible roll, metres (recorded up to +16 on a flat model,
/// +24 on the mesh). LIKELY.
pub const ROLL_WORST_M: f32 = 16.0;
/// The time a downhill roll runs, seconds (2.2 s are left between the
/// landing and the blast). LIKELY.
pub const ROLL_S: f32 = 2.0;

/// How far past a step's end the ground is asked: the capsule touches when
/// its centre is [`RADIUS_M`] from the surface, which a ray of the centre's
/// path meets up to this much later (a surface met at a 14° graze or more).
const SKIN_M: f32 = 4.0 * RADIUS_M;
/// A hit of a ray on the ground.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    /// The distance along the ray, metres.
    pub t: f32,
    /// The surface's unit normal, on the side the ray came from.
    pub normal: Vec3,
}

/// What a grenade can hit: the first surface a ray meets. The bullet mesh
/// ([`Mesh`]) for the game's own ground; [`Floor`] for a level plane; a
/// closure `(from, dir, max_t) -> Option<Hit>` for anything else.
pub trait Ground {
    /// The first hit of the ray from `from` along the unit `dir` within
    /// `max_t` metres.
    fn cast(&self, from: Vec3, dir: Vec3, max_t: f32) -> Option<Hit>;
}

impl<F: Fn(Vec3, Vec3, f32) -> Option<Hit>> Ground for F {
    fn cast(&self, from: Vec3, dir: Vec3, max_t: f32) -> Option<Hit> {
        self(from, dir, max_t)
    }
}

/// A level plane at height `y`, solid from above only.
#[derive(Clone, Copy, Debug)]
pub struct Floor {
    pub y: f32,
}

impl Ground for Floor {
    fn cast(&self, from: Vec3, dir: Vec3, max_t: f32) -> Option<Hit> {
        if from.y < self.y || dir.y >= 0.0 {
            return None;
        }
        let t = (self.y - from.y) / dir.y;
        (t <= max_t).then_some(Hit { t, normal: Vec3::Y })
    }
}

/// The bullet mesh as the ground (`mapdata::MapData::bullet_triangles`: what
/// the game's grenade meets, collision group 0x818 against mask 8, PROVEN;
/// a player's hit model also stops it, which is not in the mesh). The
/// normal is the hit triangle's own ([`RayTree::ray_hit`]).
#[derive(Clone, Copy)]
pub struct Mesh<'a>(pub &'a RayTree);

impl Ground for Mesh<'_> {
    fn cast(&self, from: Vec3, dir: Vec3, max_t: f32) -> Option<Hit> {
        let hit = self.0.ray_hit(from, dir, max_t)?;
        Some(Hit {
            t: hit.t,
            normal: hit.normal,
        })
    }
}

/// A grenade's numbers, from its config (`weapons::GrenadeParams`, the game data's):
/// - `speed`: the throw's speed before the player's modifiers, m/s (`throw_power`, 0x309B91 into
///   item+0x2B8, used at 0x30A2D5; the bots have no modifier 27, `grenade_throw_power`, so it is the
///   speed). PROVEN.
/// - `fuse_s`: from the press to the blast, seconds (`ttl_ms`, 0x30A120). PROVEN.
/// - `splash_m`: the blast's reach, metres (`splash_radius`; lethal within about 2.4 m of the
///   centre).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grenade {
    pub speed: f32,
    pub fuse_s: f32,
    pub splash_m: f32,
}

impl Grenade {
    /// From a grenade's config numbers.
    pub fn of(params: &survarium::v026g::weapons::GrenadeParams) -> Self {
        Self {
            speed: params.throw_power,
            fuse_s: params.ttl_ms as f32 / 1000.0,
            splash_m: params.splash_radius,
        }
    }

    /// The fuse from the press, ms.
    pub fn fuse_ms(&self) -> u32 {
        (self.fuse_s * 1000.0).round() as u32
    }

    /// What is left of the fuse at the launch: a grenade still in the air at this time goes off
    /// there. LIKELY (the launch delay).
    pub fn flight_s(&self) -> f32 {
        self.fuse_s - LAUNCH_DELAY_S
    }

    /// The grenade of the bots' kit (`loadouts::BOT_GRENADE`) in the game data, read once. A game
    /// data without it has bots without grenades (`reflex` throws only what the kit holds); the
    /// 0.26g RGD-5's numbers stand in, said once.
    pub fn bots() -> &'static Grenade {
        static SHARED: std::sync::OnceLock<Grenade> = std::sync::OnceLock::new();
        SHARED.get_or_init(|| {
            let name = survarium::v026g::loadouts::BOT_GRENADE.name;
            match survarium::v026g::weapons::grenade_named(name) {
                Some(params) => Grenade::of(&params),
                None => {
                    log::error!("the game data has no grenade {name}: the 0.26g RGD-5's numbers stand in");
                    Grenade {
                        speed: 17.0,
                        fuse_s: 4.0,
                        splash_m: 10.0,
                    }
                }
            }
        })
    }

    /// Every fragmentation grenade of the game data by item name (a blast that hurts,
    /// `GrenadeParams::is_frag`), read once: what a human's pocket can hold that the live list
    /// tracks.
    pub fn frags() -> &'static [(&'static str, Grenade)] {
        static SHARED: std::sync::OnceLock<Vec<(&'static str, Grenade)>> = std::sync::OnceLock::new();
        SHARED.get_or_init(|| {
            survarium::v026g::weapons::grenades()
                .iter()
                .filter(|(_, params)| params.is_frag())
                .map(|(name, params)| (*name, Grenade::of(params)))
                .collect()
        })
    }

    /// The fragmentation grenade item `name`.
    pub fn named(name: &str) -> Option<&'static Grenade> {
        Self::frags()
            .iter()
            .find(|(frag, _)| *frag == name)
            .map(|(_, grenade)| grenade)
    }

    /// The velocity of a throw along the view (yaw, pitch at the launch).
    pub fn throw_velocity(&self, yaw: f32, pitch: f32) -> Vec3 {
        view_forward(yaw, pitch) * self.speed
    }

    /// How far from the launch point a throw at `pitch` lands on a level ground `drop_m` under the
    /// launch point (negative: a ledge above it), metres on the ground; none when it never comes
    /// down to it within [`Self::flight_s`].
    pub fn level_range(&self, pitch: f32, drop_m: f32) -> Option<f32> {
        let flight = Flight::new_by(*self, Vec3::new(0.0, drop_m, 0.0), self.throw_velocity(0.0, pitch));
        let contact = flight.first_contact(&Floor { y: 0.0 }, self.flight_s())?;
        Some(contact.point.z)
    }

    /// The pitch of the longest throw from `drop_m` over a level ground, radians:
    /// `tan θ = v / √(v² + 2 g h)`. None when the ground is above what the throw can reach.
    pub fn max_pitch(&self, drop_m: f32) -> Option<f32> {
        let k = self.speed * self.speed + 2.0 * GRAVITY * drop_m;
        (k > 0.0).then(|| (self.speed / k.sqrt()).atan())
    }

    /// The farthest a throw lands from `drop_m` over a level ground (about 30 m from an eye 1.6 m
    /// up for the RGD-5), metres.
    pub fn max_range(&self, drop_m: f32) -> Option<f32> {
        self.level_range(self.max_pitch(drop_m)?, drop_m)
    }

    /// The view pitch whose first contact is `range_m` out on a level ground `drop_m` under the
    /// launch point, radians: the flat arc (the high one lands a second or more later and is no
    /// use before a fuse). None beyond [`Self::max_range`], nearer than a throw at [`PITCH_LIMIT`]
    /// down lands, and where the longest throw would outlast [`Self::flight_s`]. Solved on the same
    /// steps as [`Flight`], so `level_range(pitch_for(d, h)?, h)` is `d`.
    pub fn pitch_for(&self, range_m: f32, drop_m: f32) -> Option<f32> {
        let top = self.max_pitch(drop_m)?;
        if self.level_range(top, drop_m)? < range_m {
            return None;
        }
        let mut low = -PITCH_LIMIT;
        if self.level_range(low, drop_m).is_some_and(|r| r > range_m) {
            return None;
        }
        let mut high = top;
        for _ in 0..24 {
            let middle = 0.5 * (low + high);
            match self.level_range(middle, drop_m) {
                Some(r) if r >= range_m => high = middle,
                _ => low = middle,
            }
        }
        Some(0.5 * (low + high))
    }
}

/// The unit direction of the view, `pitch` up.
pub fn view_forward(yaw: f32, pitch: f32) -> Vec3 {
    let [x, z] = kin::forward(yaw);
    let (sin, cos) = pitch.sin_cos();
    Vec3::new(x * cos, sin, z * cos)
}

/// Where the grenade starts for an eye at `eye`: [`HAND_DOWN_M`] under it.
/// GUESS.
pub fn hand_of(eye: Vec3) -> Vec3 {
    eye - Vec3::Y * HAND_DOWN_M
}

/// A grenade's flight from its start, before anything is hit: Bullet's
/// steps of [`STEP_S`], the velocity changed by gravity and then the body
/// moved by it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flight {
    pub start: Vec3,
    pub velocity: Vec3,
    /// The grenade that flies: its fuse bounds the flight, its blast is the landing's.
    pub grenade: Grenade,
}

impl Flight {
    /// A flight of the bots' grenade ([`Grenade::bots`]).
    pub fn new(start: Vec3, velocity: Vec3) -> Self {
        Self::new_by(*Grenade::bots(), start, velocity)
    }

    /// A flight of `grenade`.
    pub fn new_by(grenade: Grenade, start: Vec3, velocity: Vec3) -> Self {
        Self {
            start,
            velocity,
            grenade,
        }
    }

    /// The throw of a view (`yaw`, `pitch` at the launch) from the hand of
    /// an eye at `eye`, of the bots' grenade ([`Grenade::bots`]).
    pub fn thrown(eye: Vec3, yaw: f32, pitch: f32) -> Self {
        Self::thrown_by(*Grenade::bots(), eye, yaw, pitch)
    }

    /// [`Self::thrown`] of `grenade`.
    pub fn thrown_by(grenade: Grenade, eye: Vec3, yaw: f32, pitch: f32) -> Self {
        Self::new_by(grenade, hand_of(eye), grenade.throw_velocity(yaw, pitch))
    }

    /// Where the body is after `step` steps.
    pub fn point(&self, step: u32) -> Vec3 {
        let n = step as f32;
        let fall = 0.5 * GRAVITY * STEP_S * STEP_S * n * (n + 1.0);
        let t = n * STEP_S;
        Vec3::new(
            self.start.x + self.velocity.x * t,
            self.start.y + self.velocity.y * t - fall,
            self.start.z + self.velocity.z * t,
        )
    }

    /// The velocity after `step` steps.
    pub fn velocity_after(&self, step: u32) -> Vec3 {
        let mut v = self.velocity;
        v.y -= GRAVITY * STEP_S * step as f32;
        v
    }

    /// Where the body is `t` seconds after the launch: on the steps' line
    /// between two steps.
    pub fn position_at(&self, t: f32) -> Vec3 {
        let steps = (t / STEP_S).max(0.0);
        let n = steps.floor();
        let from = self.point(n as u32);
        let to = self.point(n as u32 + 1);
        from + (to - from) * (steps - n)
    }

    /// The body after each step from the start on, without end.
    pub fn points(&self) -> impl Iterator<Item = Vec3> + '_ {
        (0..).map(|step| self.point(step))
    }

    /// The first time the body touches `ground` within `max_s` seconds of
    /// the launch ([`Grenade::flight_s`] for a throw that must land before
    /// the blast), or none. Each step's line is asked of the ground; the point
    /// is the centre of the body when it is [`RADIUS_M`] from the surface.
    pub fn first_contact(&self, ground: &(impl Ground + ?Sized), max_s: f32) -> Option<Contact> {
        let steps = (max_s / STEP_S).ceil() as u32;
        let mut from = self.start;
        for step in 0..steps {
            let to = self.point(step + 1);
            let length = (to - from).length();
            if length > 1e-6 {
                let dir = (to - from) * (1.0 / length);
                if let Some(hit) = ground.cast(from, dir, length + SKIN_M) {
                    let back = RADIUS_M / (-dir.dot(hit.normal)).max(RADIUS_M / SKIN_M);
                    let t = (hit.t - back).max(0.0);
                    if t <= length {
                        return Some(Contact {
                            point: from + dir * t,
                            normal: hit.normal,
                            velocity: self.velocity_after(step + 1),
                            t: (step as f32 + t / length) * STEP_S,
                        });
                    }
                }
            }
            from = to;
        }
        None
    }

    /// Where it lands and where it rests: [`Self::first_contact`] within
    /// [`Grenade::flight_s`], with its [`Landing`].
    pub fn landing(&self, ground: &(impl Ground + ?Sized)) -> Option<Landing> {
        self.first_contact(ground, self.grenade.flight_s())
            .map(|contact| Landing::of(contact, self.grenade.splash_m))
    }
}

/// The first touch of a flight on the ground.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Contact {
    /// The grenade's centre, [`RADIUS_M`] off the surface along its normal.
    pub point: Vec3,
    /// The surface's unit normal.
    pub normal: Vec3,
    /// The velocity coming in.
    pub velocity: Vec3,
    /// Seconds from the launch.
    pub t: f32,
}

impl Contact {
    /// Whether the surface is a wall ([`WALL_NORMAL_Y`]).
    pub fn is_wall(&self) -> bool {
        self.normal.y < WALL_NORMAL_Y
    }

    /// The unit direction of the velocity on the ground; none (zero) for a
    /// grenade falling straight.
    pub fn heading(&self) -> Vec3 {
        let flat = Vec3::new(self.velocity.x, 0.0, self.velocity.z);
        let length = flat.length();
        if length > 1e-6 {
            flat * (1.0 / length)
        } else {
            Vec3::ZERO
        }
    }
}

/// How far the grenade lies from its first contact along the heading,
/// metres (the rule of `fact-grenade-rest.md` §5):
/// - a wall: 0, it falls at its foot;
/// - else [`ROLL_M`], and on a surface that slides it, `½ a t²` more with
///   `a = 10 (sin θ − 0.25 cos θ)` for `t` = [`ROLL_S`], when the heading
///   goes downhill. A body holds where `tan θ ≤` [`FRICTION`] (14°), so
///   `a > 0` is the rule's "over 14°". LIKELY (data weak: 7 cases).
pub fn roll_m(contact: &Contact) -> f32 {
    if contact.is_wall() {
        return 0.0;
    }
    let n = contact.normal;
    let sin = n.x.hypot(n.z);
    let accel = GRAVITY * (sin - FRICTION * n.y);
    let downhill = contact.heading().dot(Vec3::new(n.x, 0.0, n.z)) > 0.0;
    if accel > 0.0 && downhill {
        ROLL_M + 0.5 * accel * ROLL_S * ROLL_S
    } else {
        ROLL_M
    }
}

/// Where a throw lands and where it is likely to lie when it goes off.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Landing {
    pub contact: Contact,
    /// [`Contact::heading`].
    pub heading: Vec3,
    /// The expected blast: the contact moved [`roll_m`] along the heading,
    /// at the contact's height.
    pub rest: Vec3,
    /// The blast's reach, metres ([`Grenade::splash_m`]).
    pub splash_m: f32,
}

impl Landing {
    pub fn of(contact: Contact, splash_m: f32) -> Self {
        let heading = contact.heading();
        Self {
            contact,
            heading,
            splash_m,
            rest: contact.point + heading * roll_m(&contact),
        }
    }

    /// How far `at` is from the nearest place the blast may be: the rest
    /// point, or any point from the contact to `reach_m` along the heading
    /// ([`ROLL_P80_M`] for the usual check, [`ROLL_WORST_M`] for the worst
    /// case), metres. For a wall the segment still runs along the incoming
    /// heading (through the wall: a mate behind it is counted).
    pub fn blast_distance(&self, at: Vec3, reach_m: f32) -> f32 {
        let from = self.contact.point;
        let end = from + self.heading * reach_m;
        distance_to_segment(at, from, end).min((at - self.rest).length())
    }

    /// Whether `at` (a mate's body: the chest is the nearer than the feet to
    /// a blast above the ground) is inside the splash of a blast anywhere
    /// from the contact to `reach_m` along the heading, or at the rest point.
    pub fn splashes(&self, at: Vec3, reach_m: f32) -> bool {
        self.blast_distance(at, reach_m) < self.splash_m
    }

    /// How far the thrower (`at`) is from the blast, metres: from the
    /// contact or the rest point, the nearer. The roll moves the blast away
    /// from the thrower (restitution 0, nothing sends it back), so the
    /// contact decides; the minimum landing distance is the caller's rule.
    pub fn thrower_distance(&self, at: Vec3) -> f32 {
        (at - self.contact.point)
            .length()
            .min((at - self.rest).length())
    }

    /// Whether the thrower is inside the splash of the predicted blast.
    pub fn splashes_thrower(&self, at: Vec3) -> bool {
        self.thrower_distance(at) < self.splash_m
    }
}

/// The distance from `p` to the segment `a`..`b`.
fn distance_to_segment(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let ab = b - a;
    let length2 = ab.dot(ab);
    let along = if length2 > 0.0 {
        ((p - a).dot(ab) / length2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p - (a + ab * along)).length()
}

/// [`Grenade::level_range`] of the bots' grenade.
pub fn level_range(pitch: f32, drop_m: f32) -> Option<f32> {
    Grenade::bots().level_range(pitch, drop_m)
}

/// [`Grenade::max_pitch`] of the bots' grenade.
pub fn max_pitch(drop_m: f32) -> Option<f32> {
    Grenade::bots().max_pitch(drop_m)
}

/// [`Grenade::max_range`] of the bots' grenade.
pub fn max_range(drop_m: f32) -> Option<f32> {
    Grenade::bots().max_range(drop_m)
}

/// [`Grenade::pitch_for`] of the bots' grenade.
pub fn pitch_for(range_m: f32, drop_m: f32) -> Option<f32> {
    Grenade::bots().pitch_for(range_m, drop_m)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EYE_M: f32 = 1.6;

    /// A throw from `EYE_M` over a level floor at 0, from the eye as the
    /// note's table has it; its landing.
    fn level_landing(pitch_deg: f32) -> Contact {
        let eye = Vec3::new(0.0, EYE_M, 0.0);
        let grenade = Grenade::bots();
        Flight::new(eye, grenade.throw_velocity(0.0, pitch_deg.to_radians()))
            .first_contact(&Floor { y: 0.0 }, grenade.flight_s())
            .unwrap()
    }

    /// A plane through `point` with the unit `normal`, solid from the side
    /// it faces.
    fn plane(point: Vec3, normal: Vec3) -> impl Ground {
        move |from: Vec3, dir: Vec3, max_t: f32| {
            let slope = dir.dot(normal);
            if slope >= 0.0 || (from - point).dot(normal) < 0.0 {
                return None;
            }
            let t = (point - from).dot(normal) / slope;
            (t <= max_t).then_some(Hit { t, normal })
        }
    }

    fn near(a: f32, b: f32, tolerance: f32, what: &str) {
        assert!((a - b).abs() <= tolerance, "{what}: {a} against {b}");
    }

    /// The bots' grenade's throw velocity (the tests' flights are of it).
    fn throw_velocity(yaw: f32, pitch: f32) -> Vec3 {
        Grenade::bots().throw_velocity(yaw, pitch)
    }

    /// The bots' grenade is the game data's, and on the stock data it is the numbers the bots were
    /// built on (17 m/s, a 4 s fuse, a 10 m blast): the decisions on stock data do not move.
    #[test]
    fn the_bots_grenade_is_the_game_datas_and_the_stock_numbers() {
        let rgd = survarium::v026g::weapons::grenade_named("rgd-5").unwrap();
        assert_eq!(*Grenade::bots(), Grenade::of(&rgd));
        assert_eq!(
            (Grenade::bots().speed, Grenade::bots().fuse_s, Grenade::bots().splash_m),
            (17.0, 4.0, 10.0)
        );
        assert_eq!(Grenade::bots().fuse_ms(), rgd.ttl_ms);
        assert!((Grenade::bots().flight_s() - 2.9).abs() < 1e-6);
        // The frags the humans' pockets can hold: the RGD-5 is one, the smoke is not.
        assert_eq!(Grenade::named("rgd-5"), Some(Grenade::bots()));
        assert_eq!(Grenade::named("rdg-m"), None);
        assert_eq!(Grenade::named("no such grenade"), None);
    }

    /// A grenade patch reaches the bots: the numbers of a patched copy of the data
    /// (`with_grenade_patched`: a 3 s fuse, 20 m/s, an 8 m blast) are the grenade's, and move the
    /// range of a throw, the time it may fly, the landing's blast and what it splashes.
    #[test]
    fn a_patched_grenades_numbers_reach_the_flight_the_range_and_the_blast() {
        use survarium::v026g::{
            dictionary::Dictionary,
            game_data::GameData,
            weapons::{derive_grenades, with_grenade_patched},
        };
        let data = with_grenade_patched(
            &GameData::test_archive(),
            "rgd-5",
            &[("ttl_ms", 3_000.0), ("splash_radius", 8.0), ("throw_power", 20.0)],
        );
        let rows = derive_grenades(&data, Dictionary::shared());
        let rgd = rows.iter().find(|(name, _)| *name == "rgd-5").unwrap().1;
        let patched = Grenade::of(&rgd);
        assert_eq!(
            patched,
            Grenade {
                speed: 20.0,
                fuse_s: 3.0,
                splash_m: 8.0
            }
        );
        assert_eq!(patched.fuse_ms(), 3_000);
        let stock = *Grenade::bots();
        // Faster: a flat throw lands farther.
        let flat = |g: Grenade| g.level_range(0.0, EYE_M).unwrap();
        assert!(flat(patched) > flat(stock) + 1.0, "{} {}", flat(patched), flat(stock));
        // A shorter fuse: a high lob that stock data lets fly (2.3 s of 2.9) is out of time now
        // (2.7 s of 1.9).
        assert!(stock.level_range(0.7, EYE_M).is_some());
        assert!(patched.level_range(0.7, EYE_M).is_none());
        // The pitch for a range is the grenade's: the same range needs a lower pitch at 18 m/s (the
        // fuse kept: a fuse shorter than the longest throw's flight leaves `pitch_for` no throw).
        let faster = Grenade { speed: 18.0, ..stock };
        assert!(faster.pitch_for(15.0, EYE_M).unwrap() < stock.pitch_for(15.0, EYE_M).unwrap());
        assert_eq!(
            patched.pitch_for(15.0, EYE_M),
            None,
            "the 3 s fuse is shorter than the longest throw's flight"
        );
        // A flight carries its grenade into the landing's blast radius.
        let eye = Vec3::new(0.0, EYE_M, 0.0);
        let ground = Floor { y: 0.0 };
        let landing_of = |g: Grenade| Flight::thrown_by(g, eye, 0.0, 0.0).landing(&ground).unwrap();
        let (own, fast) = (landing_of(stock), landing_of(patched));
        assert_eq!((own.splash_m, fast.splash_m), (10.0, 8.0));
        // 9 m from the rest point: inside the stock blast, outside the patched one.
        let (near_own, near_fast) = (
            own.rest + Vec3::new(9.0, 0.0, 0.0),
            fast.rest + Vec3::new(9.0, 0.0, 0.0),
        );
        assert!(own.splashes(near_own, 0.0));
        assert!(!fast.splashes(near_fast, 0.0));
        // The thrower: his own blast is the grenade's.
        assert!(own.splashes_thrower(own.rest + Vec3::new(0.0, 0.0, 9.0)));
        assert!(!fast.splashes_thrower(fast.rest + Vec3::new(0.0, 0.0, 9.0)));
    }

    #[test]
    fn the_notes_throws_land_where_it_says() {
        // fact-grenade-arc.md §0, to the grenade_arc.py numbers (eye 1.6 m,
        // level floor, the start at the eye): 9.3, 12, 15, 22 m.
        for (deg, range) in [
            (0.0, 9.324),
            (5.0, 12.090),
            (10.0, 15.288),
            (20.0, 21.919),
            (30.0, 27.249),
        ] {
            let contact = level_landing(deg);
            near(contact.point.z, range, 0.03, &format!("{deg} deg"));
            near(
                contact.point.y,
                RADIUS_M,
                1e-3,
                "the centre rests on its radius",
            );
            assert_eq!(contact.normal, Vec3::Y);
        }
        near(level_landing(0.0).t, 0.5485, 0.01, "level flight time");
        let velocity = level_landing(0.0).velocity;
        near(velocity.z, Grenade::bots().speed, 1e-4, "speed along");
        near(velocity.y, -5.5, 1e-3, "speed down");
    }

    #[test]
    fn the_longest_throw_is_thirty_metres_at_forty_three_degrees() {
        let pitch = max_pitch(EYE_M).unwrap();
        assert!((43.0..44.0).contains(&pitch.to_degrees()), "{pitch}");
        let range = max_range(EYE_M).unwrap();
        near(range, 30.2, 0.1, "longest");
        let flight = level_landing(pitch.to_degrees()).t;
        assert!(flight < Grenade::bots().flight_s(), "lands in {flight} s");
        // Either side of it lands shorter.
        assert!(level_range(pitch - 0.05, EYE_M).unwrap() < range);
        assert!(level_range(pitch + 0.05, EYE_M).unwrap() < range);
        // Nothing reaches a ground higher than v² / 2g above the start.
        assert_eq!(max_pitch(-15.0), None);
    }

    #[test]
    fn the_hand_below_the_eye_shortens_a_level_throw() {
        // fact-grenade-arc.md §3: 9.3 m from the eye, 8.4 from 0.3 m below.
        let eye = Vec3::new(0.0, EYE_M, 0.0);
        let hand = Flight::thrown(eye, 0.0, 0.0)
            .first_contact(&Floor { y: 0.0 }, Grenade::bots().flight_s())
            .unwrap();
        near(hand.point.z, 8.358, 0.03, "hand 0.3 m below");
    }

    #[test]
    fn the_pitch_for_a_range_inverts_the_flight() {
        // fact-grenade-arc.md §3: 12 m 4.9 deg, 16 m 11.1, 20 m 17.0,
        // 25 m 25.3, 28 m 31.9.
        for (range, deg) in [
            (12.0, 4.852),
            (16.0, 11.064),
            (20.0, 17.018),
            (25.0, 25.293),
            (28.0, 31.910),
        ] {
            let pitch = pitch_for(range, EYE_M).unwrap();
            near(pitch.to_degrees(), deg, 0.05, &format!("{range} m"));
            near(
                level_range(pitch, EYE_M).unwrap(),
                range,
                0.01,
                "round trip",
            );
        }
        // From a height, a ledge above the hand, and a pit.
        for drop in [-2.0, 0.5, 6.0] {
            let pitch = pitch_for(14.0, drop).unwrap();
            near(level_range(pitch, drop).unwrap(), 14.0, 0.01, "round trip");
        }
        // The lower arc: more drop, a flatter throw for the same range.
        assert!(pitch_for(14.0, 6.0).unwrap() < pitch_for(14.0, EYE_M).unwrap());
    }

    #[test]
    fn a_throw_out_of_reach_has_no_pitch() {
        assert_eq!(pitch_for(31.0, EYE_M), None);
        assert_eq!(pitch_for(max_range(EYE_M).unwrap() + 0.1, EYE_M), None);
        // Nearer than the steepest throw lands (about 1 m from 1.6 m up).
        let nearest = level_range(-PITCH_LIMIT, EYE_M).unwrap();
        assert!(nearest > 0.5 && nearest < 1.5, "{nearest}");
        assert_eq!(pitch_for(nearest - 0.1, EYE_M), None);
        assert!(pitch_for(nearest + 0.1, EYE_M).is_some());
        // A ledge 14 m up is all but out of a 17 m/s throw's reach (14.45 m).
        assert_eq!(pitch_for(10.0, -14.0), None);
    }

    #[test]
    fn the_flight_is_bullets_steps() {
        let flight = Flight::thrown(Vec3::new(3.0, 2.0, -1.0), 0.4, 0.2);
        let mut at = flight.start;
        let mut v = flight.velocity;
        for step in 1..=150u32 {
            v.y -= GRAVITY * STEP_S;
            at = at + v * STEP_S;
            let point = flight.point(step);
            assert!(
                (point - at).length() < 2e-3,
                "step {step}: {point:?} {at:?}"
            );
            near(flight.velocity_after(step).y, v.y, 1e-3, "velocity");
        }
        assert_eq!(flight.point(0), flight.start);
        assert_eq!(flight.position_at(0.0), flight.start);
        assert_eq!(flight.position_at(-1.0), flight.start);
        // Between two steps it is on the line between them.
        let half = flight.position_at(STEP_S * 10.5);
        let middle = (flight.point(10) + flight.point(11)) * 0.5;
        assert!((half - middle).length() < 1e-4);
        assert_eq!(flight.points().nth(7), Some(flight.point(7)));
        // The yaw turns left: forward at yaw 0 is +z, at a quarter turn -x.
        let quarter = view_forward(std::f32::consts::FRAC_PI_2, 0.0);
        assert!((quarter - Vec3::new(-1.0, 0.0, 0.0)).length() < 1e-6);
    }

    #[test]
    fn it_does_not_land_before_the_blast_in_the_air() {
        let up = Flight::new(Vec3::new(0.0, 1.0, 0.0), throw_velocity(0.0, 1.2));
        assert!(up.landing(&Floor { y: 0.0 }).is_none());
        // Given the time it does come down.
        assert!(up.first_contact(&Floor { y: 0.0 }, 8.0).is_some());
        // Without any ground it never lands.
        let nothing = |_: Vec3, _: Vec3, _: f32| -> Option<Hit> { None };
        assert!(
            Flight::thrown(Vec3::Y, 0.0, 0.0)
                .landing(&nothing)
                .is_none()
        );
    }

    #[test]
    fn a_floor_rolls_two_metres_and_a_wall_none() {
        let eye = Vec3::new(0.0, EYE_M, 0.0);
        let flight = Flight::new(eye, throw_velocity(0.0, 0.0));
        let floor = flight.landing(&Floor { y: 0.0 }).unwrap();
        assert!(!floor.contact.is_wall());
        near(floor.heading.z, 1.0, 1e-5, "heading");
        near(
            (floor.rest - floor.contact.point).length(),
            ROLL_M,
            1e-4,
            "roll",
        );
        near(floor.rest.z, floor.contact.point.z + 2.0, 1e-4, "along z");

        // A wall across the throw 6 m out: the grenade is at its foot.
        let wall = plane(Vec3::new(0.0, 0.0, 6.0), Vec3::new(0.0, 0.0, -1.0));
        let wall = flight.landing(&wall).unwrap();
        assert!(wall.contact.is_wall());
        near(wall.contact.point.z, 6.0 - RADIUS_M, 1e-3, "at the wall");
        assert_eq!(wall.rest, wall.contact.point);
        assert!(wall.contact.t < floor.contact.t);
        // The wall reads as a wall on the edge: a normal at 0.5 is not one.
        let steep = Contact {
            normal: Vec3::new(0.0, 0.5, -0.866),
            ..wall.contact
        };
        assert!(!steep.is_wall());
        assert!(
            Contact {
                normal: Vec3::new(0.0, 0.49, -0.87),
                ..steep
            }
            .is_wall()
        );
    }

    #[test]
    fn a_downhill_slope_over_fourteen_degrees_rolls_further() {
        let eye = Vec3::new(0.0, EYE_M, 0.0);
        let flight = Flight::new(eye, throw_velocity(0.0, 0.0));
        // A slope falling away along +z; the normal leans the way it falls.
        let on = |deg: f32| {
            let t = deg.to_radians();
            let normal = Vec3::new(0.0, t.cos(), t.sin());
            let slope = plane(Vec3::ZERO, normal);
            flight.landing(&slope).unwrap()
        };
        // 20 deg: a = 10 (sin 20 - 0.25 cos 20) = 1.07 m/s², ½ a 2² = 2.14 m.
        let a20 = GRAVITY * (20f32.to_radians().sin() - FRICTION * 20f32.to_radians().cos());
        near(a20, 1.071, 1e-3, "acceleration");
        let steep = on(20.0);
        near(
            (steep.rest - steep.contact.point).length(),
            ROLL_M + 0.5 * a20 * ROLL_S * ROLL_S,
            1e-3,
            "20 deg",
        );
        // 30 deg: a = 2.83 m/s², 5.7 m on top.
        let steeper = on(30.0);
        near(
            (steeper.rest - steeper.contact.point).length(),
            2.0 + 5.67,
            0.03,
            "30 deg",
        );
        // A gentle slope (10 deg) holds a body: the plain roll.
        let gentle = on(10.0);
        near(
            (gentle.rest - gentle.contact.point).length(),
            ROLL_M,
            1e-4,
            "10 deg",
        );
        // Uphill (the normal leaning back at the thrower): the plain roll.
        let t = 30f32.to_radians();
        let uphill = plane(Vec3::new(0.0, 0.0, 4.0), Vec3::new(0.0, t.cos(), -t.sin()));
        let uphill = flight.landing(&uphill).unwrap();
        near(
            (uphill.rest - uphill.contact.point).length(),
            ROLL_M,
            1e-4,
            "uphill",
        );
        // A slope the throw crosses along: no push along the heading.
        let across = plane(Vec3::new(0.0, 0.0, 4.0), Vec3::new(t.sin(), t.cos(), 0.0));
        let across = flight.landing(&across).unwrap();
        near(
            (across.rest - across.contact.point).length(),
            ROLL_M,
            1e-4,
            "across",
        );
    }

    #[test]
    fn a_mate_on_the_line_beyond_the_landing_is_in_the_blast_segment() {
        let eye = Vec3::new(0.0, EYE_M, 0.0);
        let landing = Flight::new(eye, throw_velocity(0.0, 10f32.to_radians()))
            .landing(&Floor { y: 0.0 })
            .unwrap();
        let first = landing.contact.point;
        let at = |along: f32, side: f32| first + Vec3::new(side, 0.0, along);
        // 15 m past the contact: out of 10 m of it and of the rest point
        // (13 m), in range of the P80 end (7 m) and of the worst end (1 m).
        let mate = at(15.0, 0.0);
        assert!(landing.blast_distance(mate, 0.0) > Grenade::bots().splash_m);
        assert!(!landing.splashes(mate, 0.0));
        assert!(landing.splashes(mate, ROLL_P80_M));
        assert!(landing.splashes(mate, ROLL_WORST_M));
        near(
            landing.blast_distance(mate, ROLL_P80_M),
            7.0,
            1e-3,
            "to the P80 end",
        );
        // 20 m past: only the worst case reaches (4 m from its end).
        let far = at(20.0, 0.0);
        assert!(!landing.splashes(far, ROLL_P80_M));
        assert!(landing.splashes(far, ROLL_WORST_M));
        // 27 m past, or 12 m to the side: clear of all.
        assert!(!landing.splashes(at(27.0, 0.0), ROLL_WORST_M));
        assert!(!landing.splashes(at(4.0, 12.0), ROLL_WORST_M));
        // Beside the line the segment counts: 9 m aside, mid-way.
        assert!(landing.splashes(at(4.0, 9.0), ROLL_P80_M));
        // Behind the landing it is the contact that counts, as for a point.
        near(
            landing.blast_distance(at(-6.0, 0.0), ROLL_P80_M),
            6.0,
            1e-3,
            "behind",
        );
        // The rest point alone, past the P80 reach on a long downhill roll.
        let long = Landing {
            rest: first + Vec3::new(0.0, 0.0, 12.0),
            ..landing
        };
        assert!(long.splashes(at(21.0, 0.0), 0.0));
        assert!(!landing.splashes(at(21.0, 0.0), 0.0));
    }

    #[test]
    fn the_thrower_is_inside_the_splash_of_a_short_throw() {
        let thrower = Vec3::new(0.0, 0.0, 0.0);
        let eye = Vec3::new(0.0, EYE_M, 0.0);
        let level = Flight::new(eye, throw_velocity(0.0, 0.0))
            .landing(&Floor { y: 0.0 })
            .unwrap();
        // 9.3 m out: the thrower stands in the splash (the contact decides,
        // the rest point is 11.3 m out).
        near(level.thrower_distance(thrower), 9.3, 0.05, "level throw");
        assert!(level.splashes_thrower(thrower));
        // +5 deg lands 12.1 m out: clear.
        let five = Flight::new(eye, throw_velocity(0.0, 5f32.to_radians()))
            .landing(&Floor { y: 0.0 })
            .unwrap();
        assert!(five.thrower_distance(thrower) > 12.0);
        assert!(!five.splashes_thrower(thrower));
        // A thrower beside the landing is measured to the nearer of the two.
        let beside = Vec3::new(10.0, 0.0, 0.0) + level.rest;
        near(
            level.thrower_distance(beside),
            10.0,
            1e-3,
            "beside the rest point",
        );
    }

    #[test]
    fn the_same_throw_gives_the_same_landing() {
        let throw = || {
            Flight::thrown(Vec3::new(12.5, 3.2, -7.7), 1.234, 0.321)
                .landing(&Floor { y: 0.0 })
                .unwrap()
        };
        let first = throw();
        for _ in 0..4 {
            assert_eq!(throw(), first);
        }
        assert_eq!(
            pitch_for(17.0, 1.6).unwrap().to_bits(),
            pitch_for(17.0, 1.6).unwrap().to_bits()
        );
    }

    /// Two big triangles: a floor at height 0, 80 m a side.
    fn floor_triangles() -> Vec<[Vec3; 3]> {
        let (a, b) = (Vec3::new(-40.0, 0.0, -40.0), Vec3::new(40.0, 0.0, -40.0));
        let (c, d) = (Vec3::new(40.0, 0.0, 40.0), Vec3::new(-40.0, 0.0, 40.0));
        vec![[a, b, c], [a, c, d]]
    }

    /// A wall in the plane z = `z`, 10 m high, 80 m wide.
    fn wall_triangles(z: f32) -> Vec<[Vec3; 3]> {
        let (a, b) = (Vec3::new(-40.0, 0.0, z), Vec3::new(40.0, 0.0, z));
        let (c, d) = (Vec3::new(40.0, 10.0, z), Vec3::new(-40.0, 10.0, z));
        vec![[a, b, c], [a, c, d]]
    }

    #[test]
    fn the_mesh_finds_the_floor_and_the_walls_normals() {
        let eye = Vec3::new(1.0, EYE_M, -3.0);
        let flight = Flight::new(eye, throw_velocity(0.3, 5f32.to_radians()));
        let floor = RayTree::from_corners(floor_triangles());
        let on_mesh = flight.landing(&Mesh(&floor)).unwrap();
        let on_plane = flight.landing(&Floor { y: 0.0 }).unwrap();
        assert!((on_mesh.contact.normal - Vec3::Y).length() < 1e-3);
        assert!((on_mesh.contact.point - on_plane.contact.point).length() < 1e-3);
        assert!((on_mesh.rest - on_plane.rest).length() < 1e-3);

        // A floor with a wall across the throw: the wall is met first.
        let mut both = floor_triangles();
        both.extend(wall_triangles(5.0));
        let tree = RayTree::from_corners(both);
        let flight = Flight::new(Vec3::new(0.0, EYE_M, 0.0), throw_velocity(0.0, 0.0));
        let wall = flight.landing(&Mesh(&tree)).unwrap();
        assert!(wall.contact.is_wall(), "{:?}", wall.contact.normal);
        assert!(wall.contact.normal.z < -0.99, "{:?}", wall.contact.normal);
        near(wall.contact.point.z, 5.0 - RADIUS_M, 0.02, "at the wall");
        assert_eq!(wall.rest, wall.contact.point);
        // A throw from above the wall's top lands on the floor behind it.
        let over = Flight::new(Vec3::new(0.0, 12.0, 0.0), throw_velocity(0.0, 0.0));
        let behind = over.landing(&Mesh(&tree)).unwrap();
        assert!(!behind.contact.is_wall());
        assert!(behind.contact.point.z > 5.0);
    }

    /// A ramp of `deg` falling away along +z from the line z = 0, 80 m a side.
    fn ramp_triangles(deg: f32) -> Vec<[Vec3; 3]> {
        let t = deg.to_radians();
        let (rise, run) = (t.sin(), t.cos());
        let (a, b) = (Vec3::new(-40.0, 0.0, 0.0), Vec3::new(40.0, 0.0, 0.0));
        let (c, d) = (
            Vec3::new(40.0, -40.0 * rise, 40.0 * run),
            Vec3::new(-40.0, -40.0 * rise, 40.0 * run),
        );
        vec![[a, b, c], [a, c, d]]
    }

    #[test]
    fn the_mesh_reads_a_slopes_normal() {
        let t = 20f32.to_radians();
        let tree = RayTree::from_corners(ramp_triangles(20.0));
        let flight = Flight::new(Vec3::new(0.0, 1.0, -2.0), throw_velocity(0.0, 0.0));
        let landing = flight.landing(&Mesh(&tree)).unwrap();
        let normal = landing.contact.normal;
        near(normal.x, 0.0, 1e-5, "normal x");
        near(normal.y, t.cos(), 1e-5, "normal y");
        near(normal.z, t.sin(), 1e-5, "normal z");
        assert!(!landing.contact.is_wall());
        assert!(roll_m(&landing.contact) > ROLL_M);
    }

    #[test]
    fn the_mesh_normal_is_the_triangles_whatever_the_angle_of_the_ray() {
        // A grazing ray (3 degrees over a floor), a ray straight down, and one
        // at a wall: the normal is the surface's own, not the ray's.
        let mut both = floor_triangles();
        both.extend(wall_triangles(30.0));
        let tree = RayTree::from_corners(both);
        let mesh = Mesh(&tree);
        for deg in [-89.0f32, -45.0, -10.0, -3.0, -0.5] {
            let t = deg.to_radians();
            let dir = Vec3::new(0.0, t.sin(), t.cos());
            let hit = mesh.cast(Vec3::new(3.0, 0.4, -20.0), dir, 100.0).unwrap();
            assert!(
                (hit.normal - Vec3::Y).length() < 1e-6,
                "{deg} deg: {:?}",
                hit.normal
            );
        }
        // Level, toward the wall: the wall's, facing the ray.
        let hit = mesh
            .cast(Vec3::new(3.0, 1.0, -20.0), Vec3::new(0.0, 0.0, 1.0), 100.0)
            .unwrap();
        assert_eq!(hit.normal, Vec3::new(0.0, 0.0, -1.0));
        near(hit.t, 50.0, 1e-3, "wall distance");
    }

    #[test]
    fn a_step_meets_the_floor_and_the_riser_each_as_itself() {
        // A floor at 0 up to z = 6, a riser 1 m high, a floor at 1 m beyond
        // it: a flat throw into the riser is a wall, a lob onto the upper
        // floor is not, and neither reads the other's normal.
        let (lo, hi, z) = (0.0f32, 1.0f32, 6.0f32);
        let tri =
            |a: [f32; 3], b: [f32; 3], c: [f32; 3]| [a, b, c].map(|[x, y, z]| Vec3::new(x, y, z));
        let corners = vec![
            tri([-40.0, lo, -40.0], [40.0, lo, -40.0], [40.0, lo, z]),
            tri([-40.0, lo, -40.0], [40.0, lo, z], [-40.0, lo, z]),
            tri([-40.0, lo, z], [40.0, lo, z], [40.0, hi, z]),
            tri([-40.0, lo, z], [40.0, hi, z], [-40.0, hi, z]),
            tri([-40.0, hi, z], [40.0, hi, z], [40.0, hi, 40.0]),
            tri([-40.0, hi, z], [40.0, hi, 40.0], [-40.0, hi, 40.0]),
        ];
        let tree = RayTree::from_corners(corners);
        let mesh = Mesh(&tree);
        let flat = Flight::new(Vec3::new(0.0, 1.6, 0.0), throw_velocity(0.0, 0.0));
        let into = flat.landing(&mesh).unwrap();
        assert!(into.contact.is_wall(), "{:?}", into.contact);
        assert_eq!(into.contact.normal, Vec3::new(0.0, 0.0, -1.0));
        let lob = Flight::new(Vec3::new(0.0, 1.6, 0.0), throw_velocity(0.0, 0.5));
        let over = lob.landing(&mesh).unwrap();
        assert!(!over.contact.is_wall(), "{:?}", over.contact);
        assert!(over.contact.point.z > z);
        assert_eq!(over.contact.normal, Vec3::Y);
    }
}
