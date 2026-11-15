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
//!   PROVEN. It leaves at [`launch_at`]: `max(press + 400, release) + 500` ms
//!   (the pull-pin stage, the idle stage that waits for the release, the
//!   throw clip's event on the 50 ms grid: [`PULL_PIN_MS`],
//!   [`RELEASE_TO_LAUNCH_MS`]), and the view that counts is the one **at the
//!   launch**, not at the press. PROVEN. The start is the right hand's bone
//!   at that event, not the eye (0x309F00): [`release_point`], measured.
//!   PROVEN.
//! - **The fuse runs from the press** (0x30A120 from 0x3103D7, 80 of 82
//!   recorded blasts at press + 4000 ms): [`Grenade::fuse_s`], so the flight
//!   has [`Grenade::flight_s`] left after the launch. PROVEN.
//! - **The body** (`grenade_core::load` 0x303890 → 0x357100): a capsule of
//!   radius [`RADIUS_M`], 0.3 kg, no damping, restitution 0, friction 0.5,
//!   never asleep, in Bullet 2.81 with gravity `(0, -10, 0)` (0x74E3D) and
//!   `stepSimulation(dt, 3, 1/60)` each 50 ms tick (0x75610, constant
//!   0xA2DBB4): three fixed steps of [`STEP_S`], gravity into the velocity
//!   and then the move. PROVEN.
//! - **After the first contact** the body keeps its tangential speed and
//!   loses the normal one (restitution 0), with the two frictions' product
//!   [`FRICTION`] (0x8E870, nothing sets either afterwards; the contact
//!   callback 0x1F1D3B4 is never written). PROVEN. And the physics world's
//!   swept test caps the speed at the hit: a step that moves more than the
//!   motion threshold and whose swept sphere meets something puts the body
//!   at the time of impact with its speed cut to the threshold over the
//!   step, 1.8 m/s ([`CCD_SPEED_CAP`], `integrateTransforms` 0x9CA78..
//!   0x9CB60; `docs/notes/thrown_and_placed.md`, `grenade_ccd.md`). PROVEN.
//!   [`Flight::rest_at_blast`] flies that rule to the blast (the rest of
//!   [`Landing`], `aim.combat.grenade_rest_flown`); the old rest,
//!   [`roll_m`] past the contact, was a fit of 26 recorded kills.
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
//! game clamps the view between [`PITCH_LOW`] and [`PITCH_HIGH`]).

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
/// The throw state's pull-pin stage, ms: `grenade_swing`, 12 frames at rate 1.0, 400 ms by the mixer's clock. Its idle
/// stage then waits for the key's release: it is ready once the inventory's grenade slot (+17Ch) is no longer this
/// grenade's (0x310450), which `want_throw` 0x309DC0 sets to the grenade's slot on the press and to 17h (none) on the
/// release. PROVEN (`docs/notes/thrown_and_placed.md`, 8a64c4e61; 0x310450 and 0x309DC0 re-read 2026-11-15).
pub const PULL_PIN_MS: u32 = 400;
/// From the throw stage's start (the release, or the pull-pin's end when the key was let go sooner) to the flight's
/// start, ms: the throw clip's `shoot` event (frame 16, 533 ms; 0x3104E7 -> 0x310680 -> `launch_grenade` 0x30A250) falls
/// in the tick that ends at +550, whose physics step then flies the new body that tick's 50 ms, so the flight starts at
/// +500 on the 50 ms grid. PROVEN: a key held 600 ms or more launches 500 ms after the release (140 throws, the median
/// 500.00), a tap 900 ms after the press (88 throws, P10 to P90 900 to 950) (`thrown_and_placed.md`).
pub const RELEASE_TO_LAUNCH_MS: u32 = 500;
/// The earliest launch after the press, ms (a tap, [`launch_after_ms`] of 0): the most flight a fuse leaves.
pub const TAP_LAUNCH_MS: u32 = PULL_PIN_MS + RELEASE_TO_LAUNCH_MS;
/// Where the grenade leaves the hand, metres: over the feet standing and crouched, behind the throw line and to its
/// right. PROVEN: measured on 147 throwers standing still at the throw's event, the right hand's bone
/// (`hand_transform` 0x309F00; `thrown_and_placed.md`, 8a64c4e61). The old start, 0.3 m under the eye, was about 0.55 m
/// too low.
pub const RELEASE_UP_M: (f32, f32) = (1.73, 1.29);
pub const RELEASE_BACK_M: f32 = 0.13;
pub const RELEASE_RIGHT_M: f32 = 0.21;
/// The lowest elevation the game lets the view have, radians: 67.48 degrees down, the pitch unit's -1 (PROVEN,
/// `mapdata::stands::VIEW_DOWN_DEG`; it was taken as -1 radian, -57.3 degrees).
pub const PITCH_LOW: f32 = -mapdata::stands::VIEW_DOWN_RAD;
/// The highest elevation the game lets the view have, radians: 60.0 degrees up, the pitch unit's 1 (PROVEN,
/// `mapdata::stands::VIEW_UP_DEG`; it was taken as 1 radian, 57.3 degrees).
pub const PITCH_HIGH: f32 = mapdata::stands::VIEW_UP_RAD;

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
/// The body's motion threshold for the swept test, metres a step (`m_ccdMotionThreshold` +104h, set at 0x303A0B):
/// a step that moves more than this is swept. PROVEN (`grenade_ccd.md`).
pub const CCD_MOTION_M: f32 = 0.03;
/// The speed a swept hit leaves the body at most, m/s: the threshold over the step, 0.03 / (1 / 60) = 1.8
/// (`integrateTransforms` 0x9CA78..0x9CB60 scales the linear velocity down to it). PROVEN (`thrown_and_placed.md`).
pub const CCD_SPEED_CAP: f32 = CCD_MOTION_M / STEP_S;
/// The sweep's result filter, metres: a hit whose step moves this much or less along the hit's normal is dropped, so
/// the step is not cut and the speed not capped (`(to - from) . normal >= -[cb+64h]`, 0x9C6B0's test at
/// 0x9C6DD..0x9C72D; the value the collision world's +38h, 0.04, set by its constructor at 0x9FCC5..0x9FCE7). A touch
/// at 0.04 x 60 = 2.4 m/s or less into the surface is not capped. PROVEN (exe review of #2050).
pub const SWEEP_FILTER_M: f32 = 0.04;
/// Where the client's grenade went off against the flown rest, along the throw's heading, metres: 911 recorded frag
/// flights flown by [`Flight::rest_at_blast`]'s rule from a sighting in the air, against the last sighting before the
/// blast (3D error P50 0.54 m, P80 4.34, P90 8.72; the old roll's P50 2.13 m). Along the heading: P10 1.24 m short of
/// the rest ([`REST_BEHIND_M`]), P90 3.95 m past it ([`REST_AHEAD_M`]), P95 8.92 m past it ([`REST_AHEAD_WORST_M`]);
/// across it P90 1.82 m (a body's reach covers it). 142 of the 911 had fallen through the floor (the stock CCD, before
/// exe patch 105). PROVEN (counted: the flights of `thrown_and_placed.md` flown again by this rule, the S7 tool's `fly`
/// with the sweep's filter and the capped step's stop; its errors split along and across the heading).
pub const REST_BEHIND_M: f32 = 1.24;
pub const REST_AHEAD_M: f32 = 3.95;
pub const REST_AHEAD_WORST_M: f32 = 8.92;

thread_local! {
    static REST_FLOWN: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
}

/// Whether a [`Flight::landing`]'s rest is the client's flown one ([`Flight::rest_at_blast`]) or the old roll
/// ([`roll_m`]): `aim.combat.grenade_rest_flown`, which `Bots` sets for its thread at each call. On by default.
pub fn rest_flown() -> bool {
    REST_FLOWN.with(|on| on.get())
}

/// Sets [`rest_flown`] for this thread.
pub fn set_rest_flown(on: bool) {
    REST_FLOWN.with(|cell| cell.set(on));
}

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
/// the game's grenade meets, collision group 0x818 against mask 8, PROVEN).
/// A player's body does not stop it: the grenade (group 818h, mask 8,
/// 0x303C73..0x303C7B) and a player's hit body (40h, 11h, `activate_physics`
/// 0x2D9958..0x2D995D) or his controller's ghost (824h, 3) make no pair
/// (`addOverlappingPair` 0xB3EDC..0xB3EEE), so it flies and rolls through
/// players. PROVEN (exe review of #2057). The normal is the hit triangle's
/// own ([`RayTree::ray_hit`]).
#[derive(Clone, Copy)]
pub struct Mesh<'a>(pub &'a RayTree);

impl Ground for Mesh<'_> {
    fn cast(&self, from: Vec3, dir: Vec3, max_t: f32) -> Option<Hit> {
        let hit = super::rays::ray_hit(self.0, from, dir, max_t)?;
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
/// - `most`, `least`: the blast's damage at its middle and at its edge, shares of a body's health
///   (`splash_max_damage`, `splash_min_damage`; the client's amount falls linearly between them,
///   `bullet_manager::explode` 0x339736..0x339750). PROVEN.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grenade {
    pub speed: f32,
    pub fuse_s: f32,
    pub splash_m: f32,
    pub most: f32,
    pub least: f32,
}

impl Grenade {
    /// From a grenade's config numbers.
    pub fn of(params: &survarium::v026g::weapons::GrenadeParams) -> Self {
        Self {
            speed: params.throw_power,
            fuse_s: params.ttl_ms as f32 / 1000.0,
            splash_m: params.splash_radius,
            most: params.splash_max_damage,
            least: params.splash_min_damage,
        }
    }

    /// How far from the blast's middle (a body's origin, its feet) its damage is at least `amount` of a body's health,
    /// metres: the client's linear law, `least + (most - least) (R - d) / R` (0x339736..0x339750), solved for
    /// `amount`. Zero for a blast that never does that much (a smoke), the whole reach for one that does it to the
    /// edge.
    pub fn reach_of_m(&self, amount: f32) -> f32 {
        if self.most <= amount {
            0.0
        } else if self.least >= amount {
            self.splash_m
        } else {
            self.splash_m * (self.most - amount) / (self.most - self.least)
        }
    }

    /// [`Self::reach_of_m`] of half a body's health: 6.4 m for the RGD-5 (10 m, 1.3 to 0.05), where both leg
    /// pools (0.5) empty too.
    pub fn half_health_m(&self) -> f32 {
        self.reach_of_m(0.5)
    }

    /// The fuse from the press, ms.
    pub fn fuse_ms(&self) -> u32 {
        (self.fuse_s * 1000.0).round() as u32
    }

    /// What is left of the fuse at a launch `launch_ms` after the press ([`launch_after_ms`]): a grenade still in the air
    /// at this time goes off there.
    pub fn flight_s(&self, launch_ms: u32) -> f32 {
        self.fuse_s - launch_ms as f32 / 1000.0
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
                        most: 1.3,
                        least: 0.05,
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
    /// down to it within [`Self::flight_s`] of a tap ([`TAP_LAUNCH_MS`]).
    pub fn level_range(&self, pitch: f32, drop_m: f32) -> Option<f32> {
        self.level_range_after(pitch, drop_m, TAP_LAUNCH_MS)
    }

    /// [`Self::level_range`] for a launch `launch_ms` after the press.
    pub fn level_range_after(&self, pitch: f32, drop_m: f32, launch_ms: u32) -> Option<f32> {
        let flight = Flight::new_by(*self, Vec3::new(0.0, drop_m, 0.0), self.throw_velocity(0.0, pitch));
        let contact = flight.first_contact(&Floor { y: 0.0 }, self.flight_s(launch_ms))?;
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
        self.max_range_after(drop_m, TAP_LAUNCH_MS)
    }

    /// [`Self::max_range`] for a launch `launch_ms` after the press.
    pub fn max_range_after(&self, drop_m: f32, launch_ms: u32) -> Option<f32> {
        self.level_range_after(self.max_pitch(drop_m)?, drop_m, launch_ms)
    }

    /// The view pitch whose first contact is `range_m` out on a level ground `drop_m` under the
    /// launch point, radians: the flat arc (the high one lands a second or more later and is no
    /// use before a fuse). None beyond [`Self::max_range`], nearer than a throw at [`PITCH_LOW`]
    /// down lands, and where the longest throw would outlast [`Self::flight_s`]. Solved on the same
    /// steps as [`Flight`], so `level_range(pitch_for(d, h)?, h)` is `d`.
    pub fn pitch_for(&self, range_m: f32, drop_m: f32) -> Option<f32> {
        self.pitch_for_after(range_m, drop_m, TAP_LAUNCH_MS)
    }

    /// [`Self::pitch_for`] for a launch `launch_ms` after the press.
    pub fn pitch_for_after(&self, range_m: f32, drop_m: f32, launch_ms: u32) -> Option<f32> {
        let top = self.max_pitch(drop_m)?;
        if self.level_range_after(top, drop_m, launch_ms)? < range_m {
            return None;
        }
        log_view_limits();
        let mut low = PITCH_LOW;
        if self.level_range_after(low, drop_m, launch_ms).is_some_and(|r| r > range_m) {
            return None;
        }
        let mut high = top;
        for _ in 0..24 {
            let middle = 0.5 * (low + high);
            match self.level_range_after(middle, drop_m, launch_ms) {
                Some(r) if r >= range_m => high = middle,
                _ => low = middle,
            }
        }
        Some(0.5 * (low + high))
    }
}

/// Says once, in the server's log, the view's limits the throws are searched between (the client's, [`PITCH_LOW`] and
/// [`PITCH_HIGH`]).
fn log_view_limits() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        log::info!(
            "bots: grenade arcs between the view's elevations {:.2} and {:.2} degrees (the pitch unit's clamp, 0x2E0F60)",
            PITCH_LOW.to_degrees(),
            PITCH_HIGH.to_degrees()
        );
    });
}

/// The unit direction of the view, `pitch` up.
pub fn view_forward(yaw: f32, pitch: f32) -> Vec3 {
    let [x, z] = kin::forward(yaw);
    let (sin, cos) = pitch.sin_cos();
    Vec3::new(x * cos, sin, z * cos)
}

/// The match time a grenade pressed at `press` and let go at `release` leaves the hand ([`PULL_PIN_MS`],
/// [`RELEASE_TO_LAUNCH_MS`]); the times are the match's 50 ms ticks, so the launch is one too.
pub fn launch_at(press: u32, release: u32) -> u32 {
    log_throw_model();
    (press + PULL_PIN_MS).max(release) + RELEASE_TO_LAUNCH_MS
}

/// Says once, in the server's log, when and where the bots take a grenade to leave the hand (the client's: the
/// launch's timing and the measured release point).
fn log_throw_model() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        log::info!(
            "bots: a grenade leaves the hand at max(press + {PULL_PIN_MS}, release) + {RELEASE_TO_LAUNCH_MS} ms \n             (the pull-pin clip, the idle stage to the release 0x310450, the throw clip's event), \n             from {:.2} m over the feet standing ({:.2} crouched), {RELEASE_BACK_M:.2} m behind the throw line \n             and {RELEASE_RIGHT_M:.2} m to its right (the right hand at the event, 0x309F00, measured)",
            RELEASE_UP_M.0,
            RELEASE_UP_M.1
        );
    });
}

/// From the press to the launch, ms, for a key held `hold_ms`.
pub fn launch_after_ms(hold_ms: u32) -> u32 {
    launch_at(0, hold_ms)
}

/// Where a grenade leaves the hand of a thrower at `feet`, standing or `crouched`, throwing along `yaw`
/// ([`RELEASE_UP_M`], [`RELEASE_BACK_M`], [`RELEASE_RIGHT_M`]).
pub fn release_point(feet: Vec3, crouched: bool, yaw: f32) -> Vec3 {
    log_throw_model();
    let up = if crouched { RELEASE_UP_M.1 } else { RELEASE_UP_M.0 };
    let [fx, fz] = kin::forward(yaw);
    // The thrower's right: `forward(yaw - pi/2)` = (cos, sin) = (fz, -fx).
    feet + Vec3::new(fz * RELEASE_RIGHT_M - fx * RELEASE_BACK_M, up, -fx * RELEASE_RIGHT_M - fz * RELEASE_BACK_M)
}

/// A thrower as a throw's arc needs him: his feet, his stance, and how long after the press his grenade leaves the
/// hand ([`launch_after_ms`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Thrower {
    pub feet: Vec3,
    pub crouched: bool,
    pub launch_ms: u32,
}

impl Thrower {
    /// The release point of a throw along `yaw` ([`release_point`]).
    pub fn release(&self, yaw: f32) -> Vec3 {
        release_point(self.feet, self.crouched, yaw)
    }

    /// The release point's height (it does not turn with the yaw).
    pub fn release_y(&self) -> f32 {
        self.release(0.0).y
    }

    /// The yaw that throws over `at` and the release point it leaves from: the line from the release point along the
    /// yaw meets `at` on the ground (the release point turns with the yaw, so two passes from the bearing of the feet).
    pub fn aim(&self, at: Vec3) -> (f32, Vec3) {
        let bearing = |from: Vec3| kin::yaw_of([at.x - from.x, at.z - from.z]);
        let mut yaw = bearing(self.feet);
        for _ in 0..2 {
            yaw = bearing(self.release(yaw));
        }
        (yaw, self.release(yaw))
    }

    /// [`Grenade::pitch_for_after`] of `grenade` to `at` from this thrower's release point, with the yaw ([`Self::aim`]).
    pub fn pitch_for(&self, grenade: &Grenade, at: Vec3) -> Option<(f32, f32)> {
        let (yaw, release) = self.aim(at);
        let pitch = grenade.pitch_for_after(release.distance_xz(at), release.y - at.y, self.launch_ms)?;
        Some((yaw, pitch))
    }

    /// [`Grenade::max_range_after`] of `grenade` over a ground at `ground_y`.
    pub fn max_range(&self, grenade: &Grenade, ground_y: f32) -> Option<f32> {
        grenade.max_range_after(self.release_y() - ground_y, self.launch_ms)
    }
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
    /// How long it flies before the fuse ends it, seconds ([`Grenade::flight_s`] of its launch).
    pub max_s: f32,
}

impl Flight {
    /// A flight of the bots' grenade ([`Grenade::bots`]).
    pub fn new(start: Vec3, velocity: Vec3) -> Self {
        Self::new_by(*Grenade::bots(), start, velocity)
    }

    /// A flight of `grenade`, launched by a tap ([`TAP_LAUNCH_MS`]: the most flight its fuse leaves).
    pub fn new_by(grenade: Grenade, start: Vec3, velocity: Vec3) -> Self {
        Self {
            start,
            velocity,
            grenade,
            max_s: grenade.flight_s(TAP_LAUNCH_MS),
        }
    }

    /// The throw of a view (`yaw`, `pitch` at the launch) by `thrower`, from his release point, of the bots' grenade
    /// ([`Grenade::bots`]).
    pub fn thrown(thrower: &Thrower, yaw: f32, pitch: f32) -> Self {
        Self::thrown_by(*Grenade::bots(), thrower, yaw, pitch)
    }

    /// [`Self::thrown`] of `grenade`.
    pub fn thrown_by(grenade: Grenade, thrower: &Thrower, yaw: f32, pitch: f32) -> Self {
        Self {
            start: thrower.release(yaw),
            velocity: grenade.throw_velocity(yaw, pitch),
            grenade,
            max_s: grenade.flight_s(thrower.launch_ms),
        }
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
    /// the launch ([`Self::max_s`] for a throw that must land before
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
    /// [`Self::max_s`], with its [`Landing`]: the rest flown to the blast
    /// ([`Self::rest_at_blast`]) or, with [`rest_flown`] off, the old roll.
    pub fn landing(&self, ground: &(impl Ground + ?Sized)) -> Option<Landing> {
        let contact = self.first_contact(ground, self.max_s)?;
        Some(self.landing_at(contact, ground))
    }

    /// [`Self::landing`] of the flight's own first contact `contact` (a caller that weighs the contact first flies the
    /// rest only when it needs it).
    pub fn landing_at(&self, contact: Contact, ground: &(impl Ground + ?Sized)) -> Landing {
        if rest_flown() {
            Landing::flown(contact, self.grenade.splash_m, self.rest_at_blast(ground))
        } else {
            Landing::of(contact, self.grenade.splash_m)
        }
    }

    /// Where the body is when the fuse ends ([`Self::max_s`] after the launch), flown through every contact by the rule
    /// the 911 recorded flights were counted with (`thrown_and_placed.md`, "After the first contact"):
    /// - steps of [`STEP_S`], gravity into the velocity and then the move, no damping; the body a sphere of
    ///   [`RADIUS_M`] (the capsule's radius);
    /// - a step's move that meets the ground stops at the surface. The client's swept test
    ///   (`integrateTransforms`) takes it when the move is longer than [`CCD_MOTION_M`] and goes more than
    ///   [`SWEEP_FILTER_M`] along the hit's normal (the sweep's result filter 0x9C6B0): the speed is cut to
    ///   [`CCD_SPEED_CAP`] (0x9CA78..0x9CB60), the contact made ([`touch`]), and the step ends there (the rest of it is
    ///   not moved, 0x9CB6D);
    /// - any other hit is the contact alone: the velocity into the surface is lost (restitution 0 and friction 0.5 a
    ///   body: the collision object's defaults 0x8E8B1..0x8E8D5 and the grenade's own info 0x303919..0x303944) and the
    ///   tangential speed loses [`FRICTION`] times it; what is left of the step's move is made once more the same way;
    /// - then the support: a body within [`RADIUS_M`] + 2 cm over a surface is set on it and touches it, so it slides
    ///   where the slope's pull beats friction (`tan θ >` [`FRICTION`]) and holds elsewhere.
    ///
    /// Which hits are capped and how a capped step ends are the client's. The stand-ins are the contact solving: a hit
    /// is met at the capsule's radius (the client's swept sphere is smaller, 0.01 stock and 0.04 with exe patch 105;
    /// by the recount the capsule's own contact stops it, the narrowphase and the solver not read: flown with the
    /// sweep's radius as the reach the 911 flights' error is worse, P50 0.63 m at 0.01, 0.54 at 0.04), and an uncapped
    /// contact by the support's ray and the rest of the move
    /// (without the support ray the error is worse, P90 10.6 m). The error the checks take ([`REST_AHEAD_M`]) was
    /// counted with this rule.
    ///
    /// A step that leaves a still body where it was is every later step too: the flight stops there.
    pub fn rest_at_blast(&self, ground: &(impl Ground + ?Sized)) -> Vec3 {
        /// The support's reach under the sphere, metres.
        const SUPPORT_M: f32 = 0.02;
        let down = Vec3::new(0.0, -1.0, 0.0);
        let (mut p, mut v) = (self.start, self.velocity);
        let steps = (self.max_s / STEP_S).ceil() as u32;
        for _ in 0..steps {
            let (was, was_v) = (p, v);
            v.y -= GRAVITY * STEP_S;
            let d = v * STEP_S;
            let length = d.length();
            if length > 1e-7 {
                let dir = d * (1.0 / length);
                match ground.cast(p, dir, length + RADIUS_M) {
                    Some(hit) => {
                        let travel = (hit.t - RADIUS_M).max(0.0);
                        p = p + dir * travel;
                        let swept = swept(d, hit.normal);
                        if swept {
                            let speed = v.length();
                            if speed > CCD_SPEED_CAP {
                                v = v * (CCD_SPEED_CAP / speed);
                            }
                        }
                        touch(&mut v, hit.normal);
                        let d2 = v * (STEP_S * (length - travel) / length);
                        let l2 = d2.length();
                        if !swept && l2 > 1e-7 {
                            let dir2 = d2 * (1.0 / l2);
                            match ground.cast(p, dir2, l2 + RADIUS_M) {
                                Some(again) => {
                                    p = p + dir2 * (again.t - RADIUS_M).max(0.0);
                                    touch(&mut v, again.normal);
                                }
                                None => p = p + d2,
                            }
                        }
                    }
                    None => p = p + d,
                }
            }
            if let Some(hit) = ground.cast(p, down, RADIUS_M + SUPPORT_M) {
                if hit.t < RADIUS_M {
                    p.y += RADIUS_M - hit.t;
                }
                touch(&mut v, hit.normal);
            }
            if p == was && v == Vec3::ZERO && was_v == Vec3::ZERO {
                break;
            }
        }
        p
    }
}

/// Whether the client's swept test takes a step's move `d` that meets a surface of unit normal `normal` (its speed
/// then capped at [`CCD_SPEED_CAP`] and the step ended at the hit): the move is longer than [`CCD_MOTION_M`]
/// (0x9C84E..0x9C887) and goes more than [`SWEEP_FILTER_M`] into the surface (the result filter 0x9C6B0).
fn swept(d: Vec3, normal: Vec3) -> bool {
    d.length() > CCD_MOTION_M && -d.dot(normal) > SWEEP_FILTER_M
}

/// A contact with a surface of unit normal `normal`: the velocity into it is lost (restitution 0) and the tangential
/// speed loses [`FRICTION`] times that (Coulomb, the two bodies' 0.5 x 0.5); a velocity leaving the surface is kept.
fn touch(v: &mut Vec3, normal: Vec3) {
    let into = v.dot(normal);
    if into >= 0.0 {
        return;
    }
    *v = *v - normal * into;
    let along = v.length();
    if along > 0.0 {
        *v = *v * ((along + FRICTION * into).max(0.0) / along);
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

/// Which rule gave a [`Landing`]'s rest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RestRule {
    /// The old one: [`roll_m`] past the contact along the heading; the blast may be anywhere from the contact to
    /// [`ROLL_P80_M`] (usual) or [`ROLL_WORST_M`] (worst) along it.
    Rolled,
    /// The client's flight to the blast ([`Flight::rest_at_blast`]); the blast may be anywhere on its slide from the
    /// contact to the rest, or along the heading from [`REST_BEHIND_M`] short of the rest to [`REST_AHEAD_M`] (usual)
    /// or [`REST_AHEAD_WORST_M`] (worst) past it.
    Flown,
}

/// How far a check reaches past a landing's rest ([`Landing::blast_distance`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spread {
    /// The contact and the rest only.
    Rest,
    /// The usual check: the 90th percentile past the rest (flown) or the roll's 80th (rolled).
    Usual,
    /// The worst case: the 95th percentile past the rest (flown) or the longest recorded roll (rolled).
    Worst,
}

/// Where a throw lands and where it is likely to lie when it goes off.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Landing {
    pub contact: Contact,
    /// [`Contact::heading`].
    pub heading: Vec3,
    /// The expected blast, by [`Self::rule`].
    pub rest: Vec3,
    /// The old rule's rest whatever the rule: the contact moved [`roll_m`] along the heading, at the contact's height
    /// (the logs compare the two).
    pub rolled: Vec3,
    pub rule: RestRule,
    /// The blast's reach, metres ([`Grenade::splash_m`]).
    pub splash_m: f32,
}

impl Landing {
    /// The old rule's landing: the rest [`roll_m`] past the contact.
    pub fn of(contact: Contact, splash_m: f32) -> Self {
        let heading = contact.heading();
        let rolled = contact.point + heading * roll_m(&contact);
        Self { contact, heading, splash_m, rest: rolled, rolled, rule: RestRule::Rolled }
    }

    /// The client's landing: the rest flown to the blast ([`Flight::rest_at_blast`]).
    pub fn flown(contact: Contact, splash_m: f32, rest: Vec3) -> Self {
        Self { rest, rule: RestRule::Flown, ..Self::of(contact, splash_m) }
    }

    /// How far `at` is from the nearest place the blast may be, metres, by the rule:
    /// - flown: any point of the slide from the contact to the rest (a straight line), or along the heading from
    ///   [`REST_BEHIND_M`] short of the rest to [`REST_AHEAD_M`] (`Usual`) or [`REST_AHEAD_WORST_M`] (`Worst`) past it;
    /// - rolled: the rest point, or any point from the contact to [`ROLL_P80_M`] (`Usual`) or [`ROLL_WORST_M`]
    ///   (`Worst`) along the heading. For a wall the segment still runs along the incoming heading (through the
    ///   wall: a mate behind it is counted).
    ///
    /// `Rest`: the slide (flown), the contact and the rest (rolled), with no spread.
    pub fn blast_distance(&self, at: Vec3, spread: Spread) -> f32 {
        let to_rest = (at - self.rest).length();
        match self.rule {
            RestRule::Flown => {
                let (behind, ahead) = match spread {
                    Spread::Rest => (0.0, 0.0),
                    Spread::Usual => (REST_BEHIND_M, REST_AHEAD_M),
                    Spread::Worst => (REST_BEHIND_M, REST_AHEAD_WORST_M),
                };
                let slide = distance_to_segment(at, self.contact.point, self.rest);
                let spread = distance_to_segment(at, self.rest - self.heading * behind, self.rest + self.heading * ahead);
                slide.min(spread)
            }
            RestRule::Rolled => {
                let reach_m = match spread {
                    Spread::Rest => 0.0,
                    Spread::Usual => ROLL_P80_M,
                    Spread::Worst => ROLL_WORST_M,
                };
                let from = self.contact.point;
                distance_to_segment(at, from, from + self.heading * reach_m).min(to_rest)
            }
        }
    }

    /// Whether `at` (a mate's body: the chest is the nearer than the feet to
    /// a blast above the ground) is inside the splash of a blast anywhere
    /// the `spread` puts it ([`Self::blast_distance`]).
    pub fn splashes(&self, at: Vec3, spread: Spread) -> bool {
        self.blast_distance(at, spread) < self.splash_m
    }

    /// The places the blast is checked at for a walk out of it: the contact, the rest, and, along the heading, the
    /// usual and the worst spread's ends (from the rest by [`REST_AHEAD_M`] and [`REST_AHEAD_WORST_M`] when flown;
    /// from the contact by [`ROLL_P80_M`] and [`ROLL_WORST_M`] when rolled).
    pub fn blast_points(&self) -> [Vec3; 4] {
        let (from, usual, worst) = match self.rule {
            RestRule::Flown => (self.rest, REST_AHEAD_M, REST_AHEAD_WORST_M),
            RestRule::Rolled => (self.contact.point, ROLL_P80_M, ROLL_WORST_M),
        };
        [self.contact.point, self.rest, from + self.heading * usual, from + self.heading * worst]
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
            .first_contact(&Floor { y: 0.0 }, grenade.flight_s(TAP_LAUNCH_MS))
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
    /// The old rule's landing of `flight` on `ground` (`aim.combat.grenade_rest_flown` off): the rest [`roll_m`] past
    /// the first contact.
    fn rolled(flight: &Flight, ground: &(impl Ground + ?Sized)) -> Landing {
        Landing::of(flight.first_contact(ground, flight.max_s).unwrap(), flight.grenade.splash_m)
    }

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
        assert!((Grenade::bots().flight_s(TAP_LAUNCH_MS) - 3.1).abs() < 1e-6);
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
                splash_m: 8.0,
                most: rgd.splash_max_damage,
                least: rgd.splash_min_damage,
            }
        );
        // Half a body's health 6.4 m out of the stock blast (10 m, 1.3 to 0.05); 5.12 m of the patched 8 m.
        near(Grenade::bots().half_health_m(), 6.4, 1e-4, "stock half health");
        near(patched.half_health_m(), 5.12, 1e-4, "patched half health");
        // A smoke does no half a body's health anywhere; a blast that does it at its edge, all its reach.
        assert_eq!(Grenade { most: 0.0, least: 0.0, ..*Grenade::bots() }.half_health_m(), 0.0);
        assert_eq!(Grenade { most: 0.9, least: 0.6, ..*Grenade::bots() }.half_health_m(), 10.0);
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
        let thrower = Thrower { feet: Vec3::ZERO, crouched: false, launch_ms: TAP_LAUNCH_MS };
        let ground = Floor { y: 0.0 };
        let landing_of = |g: Grenade| Flight::thrown_by(g, &thrower, 0.0, 0.0).landing(&ground).unwrap();
        let (own, fast) = (landing_of(stock), landing_of(patched));
        assert_eq!((own.splash_m, fast.splash_m), (10.0, 8.0));
        // 9 m from the rest point: inside the stock blast, outside the patched one.
        let (near_own, near_fast) = (
            own.rest + Vec3::new(9.0, 0.0, 0.0),
            fast.rest + Vec3::new(9.0, 0.0, 0.0),
        );
        assert!(own.splashes(near_own, Spread::Rest));
        assert!(!fast.splashes(near_fast, Spread::Rest));
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
        assert!(flight < Grenade::bots().flight_s(TAP_LAUNCH_MS), "lands in {flight} s");
        // Either side of it lands shorter.
        assert!(level_range(pitch - 0.05, EYE_M).unwrap() < range);
        assert!(level_range(pitch + 0.05, EYE_M).unwrap() < range);
        // Nothing reaches a ground higher than v² / 2g above the start.
        assert_eq!(max_pitch(-15.0), None);
    }

    /// The grenade leaves the measured hand: 1.73 m over the feet standing (1.29 crouched), 0.13 m behind the throw line
    /// and 0.21 m to its right, turned with the yaw; a throw starts there.
    #[test]
    fn a_throw_starts_at_the_measured_release_point() {
        let feet = Vec3::new(5.0, 2.0, -3.0);
        // Facing +z (yaw 0): the right is +x, behind is -z.
        let r = release_point(feet, false, 0.0);
        near(r.x, 5.21, 1e-5, "right");
        near(r.y, 3.73, 1e-5, "up standing");
        near(r.z, -3.13, 1e-5, "behind");
        near(release_point(feet, true, 0.0).y, 3.29, 1e-5, "up crouched");
        // Facing +x (yaw -pi/2): the right is -z, behind is -x.
        let r = release_point(feet, false, -std::f32::consts::FRAC_PI_2);
        near(r.x, 4.87, 1e-5, "behind, turned");
        near(r.z, -3.21, 1e-5, "right, turned");
        let thrower = Thrower { feet, crouched: false, launch_ms: TAP_LAUNCH_MS };
        assert_eq!(Flight::thrown(&thrower, 0.3, 0.1).start, thrower.release(0.3));
        // The aim's line from the release point along its yaw passes over the place.
        let at = Vec3::new(15.0, 0.0, 9.0);
        let (yaw, from) = thrower.aim(at);
        let [x, z] = kin::forward(yaw);
        let d = from.distance_xz(at);
        near(from.x + x * d, at.x, 1e-3, "aimed x");
        near(from.z + z * d, at.z, 1e-3, "aimed z");
    }

    /// The launch: `max(press + 400, release) + 500` ms; a tap 900 ms after the press, a key held 700 ms 1,200 ms after.
    #[test]
    fn the_launch_is_the_release_and_the_throw_clip() {
        assert_eq!(launch_at(1_000, 1_000), 1_900);
        assert_eq!(launch_at(1_000, 1_250), 1_900);
        assert_eq!(launch_at(1_000, 1_700), 2_200);
        assert_eq!(launch_after_ms(0), TAP_LAUNCH_MS);
        assert_eq!(launch_after_ms(800), 1_300);
        // The flight left of the fuse after it.
        near(Grenade::bots().flight_s(launch_after_ms(700)), 2.8, 1e-5, "4 s fuse, launch at 1.2 s");
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
        // Nearer than the steepest throw lands (about 0.65 m from 1.6 m up, 67.48 degrees down).
        let nearest = level_range(PITCH_LOW, EYE_M).unwrap();
        assert!(nearest > 0.5 && nearest < 1.5, "{nearest}");
        assert_eq!(pitch_for(nearest - 0.1, EYE_M), None);
        assert!(pitch_for(nearest + 0.1, EYE_M).is_some());
        // A ledge 14 m up is all but out of a 17 m/s throw's reach (14.45 m).
        assert_eq!(pitch_for(10.0, -14.0), None);
    }

    #[test]
    fn the_flight_is_bullets_steps() {
        let flight = Flight::new(Vec3::new(3.0, 2.0, -1.0), Grenade::bots().throw_velocity(0.4, 0.2));
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
            Flight::new(Vec3::Y, Grenade::bots().throw_velocity(0.0, 0.0))
                .landing(&nothing)
                .is_none()
        );
    }

    #[test]
    fn the_old_rule_rolls_two_metres_on_a_floor_and_none_at_a_wall() {
        let eye = Vec3::new(0.0, EYE_M, 0.0);
        let flight = Flight::new(eye, throw_velocity(0.0, 0.0));
        let floor = rolled(&flight, &Floor { y: 0.0 });
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
        let wall = rolled(&flight, &wall);
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
    fn the_old_rule_rolls_further_down_a_slope_over_fourteen_degrees() {
        let eye = Vec3::new(0.0, EYE_M, 0.0);
        let flight = Flight::new(eye, throw_velocity(0.0, 0.0));
        // A slope falling away along +z; the normal leans the way it falls.
        let on = |deg: f32| {
            let t = deg.to_radians();
            let normal = Vec3::new(0.0, t.cos(), t.sin());
            let slope = plane(Vec3::ZERO, normal);
            rolled(&flight, &slope)
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
        let uphill = rolled(&flight, &uphill);
        near(
            (uphill.rest - uphill.contact.point).length(),
            ROLL_M,
            1e-4,
            "uphill",
        );
        // A slope the throw crosses along: no push along the heading.
        let across = plane(Vec3::new(0.0, 0.0, 4.0), Vec3::new(t.sin(), t.cos(), 0.0));
        let across = rolled(&flight, &across);
        near(
            (across.rest - across.contact.point).length(),
            ROLL_M,
            1e-4,
            "across",
        );
    }

    #[test]
    fn the_old_rule_puts_a_mate_on_the_line_beyond_the_landing_in_the_blast_segment() {
        let eye = Vec3::new(0.0, EYE_M, 0.0);
        let landing = rolled(&Flight::new(eye, throw_velocity(0.0, 10f32.to_radians())), &Floor { y: 0.0 });
        let first = landing.contact.point;
        let at = |along: f32, side: f32| first + Vec3::new(side, 0.0, along);
        // 15 m past the contact: out of 10 m of it and of the rest point
        // (13 m), in range of the P80 end (7 m) and of the worst end (1 m).
        let mate = at(15.0, 0.0);
        assert!(landing.blast_distance(mate, Spread::Rest) > Grenade::bots().splash_m);
        assert!(!landing.splashes(mate, Spread::Rest));
        assert!(landing.splashes(mate, Spread::Usual));
        assert!(landing.splashes(mate, Spread::Worst));
        near(
            landing.blast_distance(mate, Spread::Usual),
            7.0,
            1e-3,
            "to the P80 end",
        );
        // 20 m past: only the worst case reaches (4 m from its end).
        let far = at(20.0, 0.0);
        assert!(!landing.splashes(far, Spread::Usual));
        assert!(landing.splashes(far, Spread::Worst));
        // 27 m past, or 12 m to the side: clear of all.
        assert!(!landing.splashes(at(27.0, 0.0), Spread::Worst));
        assert!(!landing.splashes(at(4.0, 12.0), Spread::Worst));
        // Beside the line the segment counts: 9 m aside, mid-way.
        assert!(landing.splashes(at(4.0, 9.0), Spread::Usual));
        // Behind the landing it is the contact that counts, as for a point.
        near(
            landing.blast_distance(at(-6.0, 0.0), Spread::Usual),
            6.0,
            1e-3,
            "behind",
        );
        // The rest point alone, past the P80 reach on a long downhill roll.
        let long = Landing {
            rest: first + Vec3::new(0.0, 0.0, 12.0),
            ..landing
        };
        assert!(long.splashes(at(21.0, 0.0), Spread::Rest));
        assert!(!landing.splashes(at(21.0, 0.0), Spread::Rest));
    }

    /// A ground that counts the rays it is asked.
    struct Counted<G> {
        ground: G,
        casts: std::cell::Cell<u32>,
    }

    impl<G: Ground> Ground for Counted<G> {
        fn cast(&self, from: Vec3, dir: Vec3, max_t: f32) -> Option<Hit> {
            self.casts.set(self.casts.get() + 1);
            self.ground.cast(from, dir, max_t)
        }
    }

    /// A grenade 0.2 m over a floor falling fast (12 m/s down, 6 along): its first step's move meets the floor.
    fn steep_fall() -> Flight {
        Flight::new(Vec3::new(0.0, 0.2, 0.0), Vec3::new(0.0, -12.0, 6.0))
    }

    #[test]
    fn the_flown_rest_caps_the_speed_at_a_swept_hit_and_lies_by_the_contact() {
        near(CCD_SPEED_CAP, 1.8, 1e-5, "the cap");
        let floor = Counted { ground: Floor { y: 0.0 }, casts: std::cell::Cell::new(0) };
        let landing = steep_fall().landing(&floor).unwrap();
        assert_eq!(landing.rule, RestRule::Flown);
        // The step's move (0.22 m) is over the threshold and meets the floor: 13.4 m/s cut to 1.8, the 1.6 into the
        // floor lost and the 0.8 along it less 0.25 x 1.6: 0.4 m/s, which friction stops within a few centimetres.
        let slid = landing.rest.z - landing.contact.point.z;
        assert!((0.0..0.1).contains(&slid), "{slid}");
        near(landing.rest.x, 0.0, 1e-5, "along the throw");
        near(landing.rest.y, RADIUS_M, 1e-4, "on the floor");
        // The old rule's rest is kept for the logs: 2 m past the contact.
        near(landing.rolled.z - landing.contact.point.z, ROLL_M, 1e-4, "the old roll");
        // [CPU] It stops once it lies still: the whole landing's casts, the first contact's and the flight's.
        assert!(floor.casts.get() < 40, "{} casts", floor.casts.get());
    }

    #[test]
    fn the_swept_test_takes_a_move_over_its_threshold_that_goes_more_than_four_centimetres_into_the_surface() {
        // Straight into a wall at 15 m/s: 0.25 m a step into it.
        assert!(swept(Vec3::new(0.0, 0.0, 0.25), Vec3::new(0.0, 0.0, -1.0)));
        // Along a floor, 3 cm a step down into it (1.8 m/s): dropped by the filter, whatever the speed along it.
        assert!(!swept(Vec3::new(0.0, -0.03, 0.25), Vec3::Y));
        // 4 cm is still dropped (2.4 m/s), 5 cm is taken.
        assert!(!swept(Vec3::new(0.0, -0.04, 0.25), Vec3::Y));
        assert!(swept(Vec3::new(0.0, -0.05, 0.25), Vec3::Y));
        // A move of 3 cm or less is not swept at all.
        assert!(!swept(Vec3::new(0.0, -0.02, 0.02), Vec3::Y));
        near(SWEEP_FILTER_M * 60.0, 2.4, 1e-5, "the speed into the surface under which a hit is not capped");
    }

    #[test]
    fn a_flight_the_support_meets_first_is_not_capped_and_slides_on() {
        // A level throw from the eye on a perfect plane: in its last step in the air the body comes within reach of
        // the support (2 cm under the sphere) before a move meets the floor, so no swept hit cuts its speed: the
        // support's contact takes the speed into the floor, friction takes a quarter of that, and it slides on to the
        // blast, as 209 of the 911 counted flights did (they kept 10 m/s or more after it). On real ground the next
        // rise of the mesh meets a move that goes far enough into it and cuts it.
        let floor = Counted { ground: Floor { y: 0.0 }, casts: std::cell::Cell::new(0) };
        let landing = Flight::new(Vec3::new(0.0, EYE_M, 0.0), throw_velocity(0.0, 0.0)).landing(&floor).unwrap();
        let slid = landing.rest.z - landing.contact.point.z;
        assert!(slid > 20.0, "{slid}");
        near(landing.rest.y, RADIUS_M, 1e-4, "on the floor");
        // [CPU] It slides to the fuse's end: two casts a step at most, and the first contact's.
        let steps = (landing_fuse_s() / STEP_S).ceil() as u32;
        assert!(floor.casts.get() <= 3 * steps, "{} casts for {steps} steps", floor.casts.get());
    }

    /// The flight time a tap's throw has, s.
    fn landing_fuse_s() -> f32 {
        Grenade::bots().flight_s(TAP_LAUNCH_MS)
    }

    #[test]
    fn the_flown_rest_slides_down_a_slope_over_fourteen_degrees_and_holds_on_a_gentler_one() {
        let on = |deg: f32| {
            let t = deg.to_radians();
            let slope = plane(Vec3::ZERO, Vec3::new(0.0, t.cos(), t.sin()));
            Flight::new(Vec3::new(0.0, 0.3, 0.0), Vec3::new(0.0, -12.0, 3.0)).landing(&slope).unwrap()
        };
        let slid = |l: &Landing| (l.rest - l.contact.point).length();
        // 30 deg: the pull (10 sin 30 = 5) beats friction (2.5 cos 30 = 2.2): it slides on down to the blast.
        let steep = on(30.0);
        // 10 deg holds a body (tan 10 deg under 0.25): the capped speed's few centimetres only.
        let gentle = on(10.0);
        assert!(slid(&gentle) < 0.3, "{}", slid(&gentle));
        assert!(slid(&steep) > 5.0, "{}", slid(&steep));
        // Down the slope.
        assert!(steep.rest.z > steep.contact.point.z && steep.rest.y < steep.contact.point.y);
    }

    #[test]
    fn the_flown_blast_spreads_over_the_slide_and_along_the_heading_by_its_measured_error() {
        let landing = Flight::new(Vec3::new(0.0, EYE_M, 0.0), throw_velocity(0.0, 0.0)).landing(&Floor { y: 0.0 }).unwrap();
        let (contact, rest) = (landing.contact.point, landing.rest);
        assert!(rest.z > contact.z + 20.0);
        // On the slide, half-way: in the blast's reach whatever the spread.
        let mid = (contact + rest) * 0.5;
        assert!(landing.blast_distance(mid, Spread::Rest) < 0.1);
        // Past the rest along the heading: the spread's ends.
        let at = |along: f32| rest + Vec3::new(0.0, 0.0, along);
        near(landing.blast_distance(at(15.0), Spread::Rest), 15.0, 1e-3, "to the rest");
        near(landing.blast_distance(at(15.0), Spread::Usual), 15.0 - REST_AHEAD_M, 1e-3, "usual");
        near(landing.blast_distance(at(15.0), Spread::Worst), 15.0 - REST_AHEAD_WORST_M, 1e-3, "worst");
        // Beside the slide: its distance across.
        near(landing.blast_distance(mid + Vec3::new(7.0, 0.0, 0.0), Spread::Usual), 7.0, 1e-3, "beside");
        // Behind the contact: the contact.
        near(landing.blast_distance(contact - Vec3::new(0.0, 0.0, 6.0), Spread::Usual), 6.0, 1e-3, "behind");
        // The walk out of the blast is checked at the contact, the rest and the spread's ends along the heading.
        let [first, at_rest, usual, worst] = landing.blast_points();
        assert_eq!((first, at_rest), (contact, rest));
        near(usual.z - rest.z, REST_AHEAD_M, 1e-4, "usual end");
        near(worst.z - rest.z, REST_AHEAD_WORST_M, 1e-4, "worst end");
        // A rest by the contact: the spread reaches behind it by its P10.
        let short = steep_fall().landing(&Floor { y: 0.0 }).unwrap();
        let back = short.rest - Vec3::new(0.0, 0.0, REST_BEHIND_M + 1.0);
        near(short.blast_distance(back, Spread::Usual), 1.0, 0.05, "short of the rest");
    }

    #[test]
    fn the_switch_off_gives_the_old_roll() {
        let flight = Flight::new(Vec3::new(0.0, EYE_M, 0.0), throw_velocity(0.0, 0.0));
        set_rest_flown(false);
        let off = flight.landing(&Floor { y: 0.0 }).unwrap();
        set_rest_flown(true);
        assert_eq!(off, rolled(&flight, &Floor { y: 0.0 }));
        assert_eq!(off.rule, RestRule::Rolled);
        assert_ne!(flight.landing(&Floor { y: 0.0 }).unwrap().rest, off.rest);
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
            Flight::new(Vec3::new(12.5, 3.2, -7.7), Grenade::bots().throw_velocity(1.234, 0.321))
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
        // It falls down the wall (no bounce) and lies at its foot.
        near(wall.rest.z, 5.0 - RADIUS_M, 0.05, "at the wall's foot");
        near(wall.rest.y, RADIUS_M, 0.01, "on the floor");
        assert_eq!(wall.rolled, wall.contact.point);
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
