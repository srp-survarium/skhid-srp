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
//!   Acting on a cloud (watching its edges, crossing under it) is for what a human knows: the clouds he saw, heard
//!   pop or was told of ([`KnownClouds`], the one home of that rule).

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
        self.radius_after_formed(ms, self.forming_ms)
    }

    /// [`Self::radius_after`] with the cloud hiding from `forming_ms` after the blast ([`Forming`]).
    pub fn radius_after_formed(&self, ms: u32, forming_ms: u32) -> Option<f32> {
        let full = self.radius_m;
        if ms < forming_ms || ms >= self.gone_ms {
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
    super::rays::ray_cast(mesh, from, Vec3::new(0.0, -1.0, 0.0), FLOOR_BELOW_M)
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

/// [smoke.forming_client] From the blast until a cloud hides through its middle as the client draws it, ms: 1.24 s,
/// where a line through about 8 of its puffs is 75 % opaque. PROVEN, read: the puffs spawn 0.2 s after the blast and
/// fade in over 6.5 % of their 29 s (`smoke_grenade.particle`, the alpha keys), and one puff's texture
/// (`fx/lighted_smoke/smoke1`, DXT5) averages 0.291 alpha over its disc (max 0.83: none is opaque alone). LIKELY, the
/// layering: 1 - (1 - 0.291 k)^8 >= 0.75 at a fade-in key k of 0.547, 0.2 + 0.547 x 1.9 s
/// (`traces-20261028/fx-smoke-data.md` section 7; the 8 puffs on a line rest on its size GUESSes).
pub const CLIENT_FORMING_MS: u32 = 1_240;

/// From when a team's eyes take a cloud to hide (`smoke.forming_client`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Forming {
    /// The cloud shape's own `forming_ms` (the RDG-M's 0.7 s, a GUESS from the alpha keys alone).
    OfShape,
    /// [`CLIENT_FORMING_MS`]: as long as the client's puffs let a human see through it, never shorter.
    Client,
}

impl Forming {
    /// The forming time of `shape` under this rule, ms.
    pub fn ms(self, shape: &CloudShape) -> u32 {
        match self {
            Self::OfShape => shape.forming_ms,
            Self::Client => CLIENT_FORMING_MS.max(shape.forming_ms),
        }
    }

    /// The team's rule by its smoke tuning.
    pub fn of(tuning: &super::tuning::SmokeTuning) -> Self {
        if tuning.forming_client { Self::Client } else { Self::OfShape }
    }
}

impl Cloud {
    /// When it is gone.
    pub fn ends(&self) -> MatchMs {
        self.pours.plus(self.shape.life_ms())
    }

    /// Its radius that hides at `at`: none before it hides (in the air, forming) and once gone.
    pub fn radius_at(&self, at: MatchMs) -> Option<f32> {
        self.radius_formed(at, Forming::OfShape)
    }

    /// [`Self::radius_at`] under the forming rule `forming`.
    pub fn radius_formed(&self, at: MatchMs, forming: Forming) -> Option<f32> {
        if at < self.pours {
            return None;
        }
        self.shape.radius_after_formed(at.since(self.pours), forming.ms(&self.shape))
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
        self.inside_formed(a, b, at, Forming::OfShape)
    }

    /// [`Self::inside_m`] under the forming rule `forming`.
    pub fn inside_formed(&self, a: Vec3, b: Vec3, at: MatchMs, forming: Forming) -> f32 {
        let Some(r) = self.radius_formed(at, forming).filter(|r| *r > 0.0) else {
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
}

/// The clouds in the match at one moment (the view's): those not yet ended, in the order thrown.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Clouds(Vec<Cloud>);

impl Clouds {
    /// [planning] The clouds a smoke plan of `team`'s counts at `clock`, each at its full size from now (the owner: "they
    /// shouldnt throw smoke if smoke is already there"): every cloud live or forming, of either team (a puff is seen by
    /// all), and the team's own still in the air (thrown, and told: its thrower's mates know it), until it ends.
    /// Full under the team's forming rule `forming` (`smoke.forming_client`).
    pub fn planning(&self, clock: u32, team: u8, forming: Forming) -> Self {
        let at = MatchMs(clock);
        Self(
            self.0
                .iter()
                .filter(|c| at < c.ends() && (at >= c.pours || c.team == team))
                // Full now: poured at least its forming time ago (one already older keeps its age).
                .map(|c| Cloud { pours: MatchMs(c.pours.0.min(clock.saturating_sub(forming.ms(&c.shape)))), ..*c })
                .collect(),
        )
    }

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
        let forming = Forming::of(tuning);
        if !tuning.sight || !self.0.iter().any(|c| c.radius_formed(at, forming).is_some()) {
            return Veil::Clear;
        }
        Veil::Smoke {
            clouds: &self.0,
            at,
            thin_m: tuning.thin_m.get(),
            forming,
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
        /// From when a cloud hides (`SmokeTuning::forming_client`).
        forming: Forming,
    },
}

impl Veil<'_> {
    /// Whether smoke hides `b` from `a`: the line runs through one cloud for `thin_m` or more, or through
    /// several for that much in all. No ray is cast.
    pub fn hides(&self, a: Vec3, b: Vec3) -> bool {
        match *self {
            Veil::Clear => false,
            Veil::Smoke { clouds, at, thin_m, forming } => {
                let mut inside = 0.0;
                for cloud in clouds {
                    inside += cloud.inside_formed(a, b, at, forming);
                    // Strictly more, as Valve's `IsLineBlockedBySmoke`.
                    if inside > thin_m {
                        return true;
                    }
                }
                false
            }
        }
    }

    /// [`Self::hides`] by every cloud but `except` (a line to that cloud itself: is it seen past the others).
    pub fn hides_except(&self, a: Vec3, b: Vec3, except: CloudKey) -> bool {
        match *self {
            Veil::Clear => false,
            Veil::Smoke { clouds, at, thin_m, forming } => {
                let inside: f32 = clouds
                    .iter()
                    .filter(|cloud| cloud.key() != except)
                    .map(|cloud| cloud.inside_formed(a, b, at, forming))
                    .sum();
                inside > thin_m
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

/// A cloud's name: who threw it and when.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CloudKey {
    pub by: Place,
    pub thrown: MatchMs,
}

impl Cloud {
    pub fn key(&self) -> CloudKey {
        CloudKey {
            by: self.by,
            thrown: self.thrown,
        }
    }
}

/// How a bot came to know a cloud: as a player does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloudSource {
    /// It saw the cloud: smoking, its middle in the view cone within `smoke.see_m`, a clear line on the map that no
    /// other cloud hides.
    Seen,
    /// It heard the pop: within `smoke.heard_m` while the grenade begins to pour (`smoke.heard_ms`).
    Heard,
    /// Its team's: the thrower knows his own from the pop, his mates `smoke.told_ms` after it (a callout; the
    /// justified-cheat ledger's "own smoke told" row, as `avoid.told_ms` for a mate's frag).
    Told,
}

impl CloudSource {
    pub const ALL: [Self; 3] = [Self::Seen, Self::Heard, Self::Told];
}

/// The bot that knows: its place, team and eyes.
#[derive(Clone, Copy, Debug)]
pub struct Watcher {
    pub me: Place,
    pub team: u8,
    pub eye: Vec3,
}

/// The clouds one bot knows of ([`CloudSource`]): never a cloud it had no way to see, hear or be told of. Kept
/// until the cloud ends, with what the bot did about each (its edge watches). The one home of "which clouds a bot
/// knows".
#[derive(Clone, Debug, Default)]
pub struct KnownClouds {
    known: Vec<Known>,
}

#[derive(Clone, Copy, Debug)]
struct Known {
    key: CloudKey,
    source: CloudSource,
    /// When it may next watch the cloud's edge: `None` before its first watch (the one noted).
    next_watch: Option<MatchMs>,
}

/// What an edge watch came to: where to look (the edge on the cloud's floor; the glance looks a chest's height
/// over it), for which cloud, and whether it is the first for it (a note).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EdgeWatch {
    pub at: Vec3,
    pub key: CloudKey,
    pub first: bool,
}

impl KnownClouds {
    /// At a think: the clouds that ended are forgotten, and a cloud not yet known is learnt if the bot sees it
    /// (`sees` of the cloud's middle a metre over its floor: the caller's view cone and map line, within
    /// `smoke.see_m`; and no other cloud hides that line, `veil`, the team's own), hears it pop, or is told of it
    /// ([`CloudSource`]). What it learnt now, by source ([`CloudSource::ALL`]'s order).
    pub fn update(
        &mut self,
        who: Watcher,
        clouds: &Clouds,
        veil: Veil,
        now: u32,
        t: &super::tuning::SmokeTuning,
        sees: impl Fn(Vec3) -> bool,
    ) -> [u32; 3] {
        let at = MatchMs(now);
        let mut learnt = [0; 3];
        self.known.retain(|k| clouds.all().iter().any(|c| c.key() == k.key && c.live_at(at)));
        for cloud in clouds.all() {
            if at < cloud.pours || !cloud.live_at(at) || self.knows(cloud.key()) {
                continue;
            }
            let middle = cloud.centre + Vec3::new(0.0, 1.0, 0.0);
            let source = if told(cloud, who, at, t) {
                Some(CloudSource::Told)
            } else if who.eye.distance_xz(cloud.centre) <= t.heard_m.get() && at < cloud.pours.plus(t.heard_ms) {
                Some(CloudSource::Heard)
            } else if sees(middle) && !veil.hides_except(who.eye, middle, cloud.key()) {
                Some(CloudSource::Seen)
            } else {
                None
            };
            if let Some(source) = source {
                learnt[source as usize] += 1;
                self.known.push(Known {
                    key: cloud.key(),
                    source,
                    next_watch: None,
                });
            }
        }
        learnt
    }

    /// Whether it knows the cloud `key`.
    pub fn knows(&self, key: CloudKey) -> bool {
        self.known.iter().any(|k| k.key == key)
    }

    /// How it knows the cloud `key`.
    pub fn source(&self, key: CloudKey) -> Option<CloudSource> {
        self.known.iter().find(|k| k.key == key).map(|k| k.source)
    }

    /// The edge of the nearest cloud it knows and was not told is its team's (a cloud of unknown side is watched as
    /// a possible enemy's), within `smoke.watch_m`, still hiding, facing it: where a man coming through would step
    /// out. Due every `smoke.watch_every_ms` per cloud. The bot is outside it (one inside sees nothing to watch).
    pub fn edge_to_watch(
        &mut self,
        who: Watcher,
        feet: Vec3,
        clouds: &Clouds,
        now: u32,
        t: &super::tuning::SmokeTuning,
    ) -> Option<EdgeWatch> {
        let at = MatchMs(now);
        let watch_m = t.watch_m.get();
        let (index, cloud, radius) = self
            .known
            .iter()
            .enumerate()
            .filter(|(_, k)| k.next_watch.is_none_or(|next| at >= next))
            .filter_map(|(i, k)| {
                let cloud = clouds.all().iter().find(|c| c.key() == k.key)?;
                if told(cloud, who, at, t) {
                    return None;
                }
                let radius = cloud.radius_at(at)?;
                let d = feet.distance_xz(cloud.centre);
                (d > radius && d - radius <= watch_m).then_some((i, *cloud, radius))
            })
            .min_by(|a, b| {
                (feet.distance_xz(a.1.centre) - a.2).total_cmp(&(feet.distance_xz(b.1.centre) - b.2))
            })?;
        let toward = (feet - cloud.centre) * (1.0 / feet.distance_xz(cloud.centre).max(1e-3));
        let edge = cloud.centre + Vec3::new(toward.x * radius, 0.0, toward.z * radius);
        let known = &mut self.known[index];
        let first = known.next_watch.is_none();
        known.next_watch = Some(at.plus(t.watch_every_ms));
        Some(EdgeWatch {
            at: edge,
            key: cloud.key(),
            first,
        })
    }
}

/// Whether `who` knows `cloud` is his team's at `at`: he threw it (from its pop), or a mate did and the callout came
/// (`smoke.told_ms` after the pop).
fn told(cloud: &Cloud, who: Watcher, at: MatchMs, t: &super::tuning::SmokeTuning) -> bool {
    cloud.team == who.team && (cloud.by == who.me || at >= cloud.pours.plus(t.told_ms))
}

/// A smoke is thrown against an enemy the thrower's team sensed (saw or heard) within this, ms: the fairness rule
/// of BENCH-DECISIONS (no decision on an enemy not seen or heard within 15 s).
pub const FAIR_SENSED_MS: u32 = 15_000;

/// Whether the kill feed (`deaths`, `MatchView::deaths`) showed `who` dead since he was sensed at `sensed`, or within
/// `window_ms` before it (the sensing's lag): a human reads the feed and smokes no dead man's line. A death longer before
/// is an earlier life of his.
pub fn died_since(deaths: &[super::view::Death], who: Place, sensed: u32, window_ms: u32) -> bool {
    deaths.iter().any(|d| d.place == who && d.time >= sensed.saturating_sub(window_ms))
}

/// What a smoke throw is for: the uses of part 2, each its own switch (`SmokeTuning`), and the plays that call the
/// API (fx-threat-obj's Breach and spawn-exit breakout: `Breach`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SmokeUse {
    /// A bot or group about to cross a lane a held spot covers (#703's K1): between the holder and the crossing.
    CrossLane,
    /// Hit from beyond its weapon's reach: on the sniper's sightline, as near him as it reaches.
    AntiSniper,
    /// An objective actor's exposed crossing (the carrier now; a capturer entering a zone later, MODES-READY 2).
    ObjectiveActor,
    /// A Breach job's thrower (`design-p1c.md` 1), or the spawn-exit breakout.
    Breach,
}

impl SmokeUse {
    pub const ALL: [Self; 4] = [Self::CrossLane, Self::AntiSniper, Self::ObjectiveActor, Self::Breach];

    pub const fn index(self) -> usize {
        match self {
            Self::CrossLane => 0,
            Self::AntiSniper => 1,
            Self::ObjectiveActor => 2,
            Self::Breach => 3,
        }
    }
}

/// Where to put a smoke, and why: where the throw's arc comes to rest on the map (the cloud's floor), the view that
/// throws it, and how much smoke that cloud puts on the line it is for (metres, at its full radius).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SmokePlan {
    pub at: Vec3,
    pub aim: SmokeAim,
    pub for_use: SmokeUse,
    pub covers_m: f32,
    /// The line the cloud is for.
    pub line: SmokeLine,
}

/// The line a smoke is for: the threat's eye to the chest over the crossing, and the crossing (the route's first point
/// in his sight, where the bot would step into it).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SmokeLine {
    pub eye: Vec3,
    pub target: Vec3,
    pub crossing: Vec3,
}

/// [A208 `smoke.wait_for_cloud`] Whose cloud a hold is for: the bot's own smoke, by its use, or a mate's in the air
/// (`CloudWaiters::Crossers`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaitsFor {
    Own(SmokeUse),
    Mates,
}

impl WaitsFor {
    /// The counters' columns: the uses in [`SmokeUse::ALL`]'s order, then the mates'.
    pub const COUNT: usize = SmokeUse::ALL.len() + 1;

    pub const fn index(self) -> usize {
        match self {
            Self::Own(u) => u.index(),
            Self::Mates => SmokeUse::ALL.len(),
        }
    }
}

/// [A208 `smoke.wait_for_cloud`] Why a hold for a cloud ended (`Reflex::cloud_holding`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HoldEnd {
    /// The clouds hid the line.
    Hidden,
    /// A target that is not the line's threat took the bot.
    Fight,
    /// Hit since the hold began.
    Hit,
    /// An enemy its own senses put within `close_m` lately.
    Close,
    /// The cap ran out.
    Cap,
}

/// [A208 `smoke.wait_for_cloud`] A hold short of a crossing until a cloud hides its line: what `Reflex::start_smoke`
/// takes for a thrower (from the launch on) and `Reflex::hold_for_cloud` for a bot crossing under a mate's cloud. The
/// bot walks its route on to `hold` and stands there, looking at the threat's eye, until the clouds (its team's
/// forming rule) hide the line, or a fight, a hit, an enemy sensed close or the cap ends it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CloudWait {
    pub waits_for: WaitsFor,
    pub line: SmokeLine,
    /// The enemy the line is against, when known: his being its target is no fight that ends the hold (the reflex
    /// holds the sniper that outranges it as its target, seen or not: `Reflex::outranged_by`).
    pub threat: Option<Place>,
    /// Where it holds: the last point of its route short of the crossing that the threat's eyes do not see (its feet
    /// when there is none ahead of it).
    pub hold: Vec3,
    /// The latest end, ms after the hold's start (the press for its own smoke, the hold's begin for a mate's): the
    /// grenade's fuse, the forming and `margin_ms`.
    pub cap_ms: u32,
    pub thin_m: f32,
    pub forming: Forming,
    pub close_m: f32,
    pub close_sensed_ms: u32,
}

impl CloudWait {
    /// The hold for `plan` by a bot of a team whose tuning is `tuning` (None with `smoke.wait_for_cloud` off), with
    /// `grenade` thrown: its own smoke's, or with `mates` a mate's cloud in the air (only with
    /// `CloudWaiters::Crossers`). Every `start_smoke` caller (the lanes, the sniper, an objective actor's, a Breach
    /// thrower's) takes its wait from here.
    pub fn of(
        tuning: &super::tuning::SmokeTuning,
        grenade: &SmokeGrenade,
        line: SmokeLine,
        threat: Option<Place>,
        waits_for: WaitsFor,
        hold: Vec3,
    ) -> Option<Self> {
        let w = tuning.wait_for_cloud?;
        if waits_for == WaitsFor::Mates && w.who != super::tuning::CloudWaiters::Crossers {
            return None;
        }
        let forming = Forming::of(tuning);
        Some(Self {
            waits_for,
            line,
            threat,
            hold,
            cap_ms: grenade.cloud.fuse_ms + forming.ms(&grenade.cloud) + w.margin_ms,
            thin_m: tuning.thin_m.get(),
            forming,
            close_m: w.close_m.get(),
            close_sensed_ms: w.close_sensed_ms,
        })
    }

    /// Whether the clouds `clouds` hide its line at `now`.
    pub fn hidden(&self, clouds: &Clouds, now: u32) -> bool {
        Veil::Smoke { clouds: clouds.all(), at: MatchMs(now), thin_m: self.thin_m, forming: self.forming }.hides(self.line.eye, self.line.target)
    }
}

/// The view a smoke is thrown with from the thrower's eye (`arc::Flight::thrown_by`'s yaw and pitch), and where its
/// arc comes to rest on the bullet mesh (`arc::Flight::landing`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SmokeAim {
    pub yaw: f32,
    pub pitch: f32,
    pub rest: Vec3,
}

/// A planned cloud covers its line with this much more smoke than the sight rule, metres: the arc lands a throw
/// within a few metres of its point (median 2 m from the view's prediction, `arc.rs`), and a line half a metre over
/// the rule still hides with the cloud a little off.
pub const PLAN_MARGIN_M: f32 = 1.5;
/// The points tried along a line, metres apart.
const PLAN_STEP_M: f32 = 1.0;

/// What a smoke plan reads of the map and the thrower: whether a throw from `thrower_eye` reaches `at` (the arc's
/// flat throw, `arc::Grenade::pitch_for`, with the smoke's throw power), and the floor under a point.
pub struct Reach<'a> {
    pub mesh: &'a mapdata::RayTree,
    pub grenade: SmokeGrenade,
    pub thrower_eye: Vec3,
}

impl Reach<'_> {
    /// The floor under `point`, if there is one within [`FLOOR_BELOW_M`].
    fn floor(&self, point: Vec3) -> Option<Vec3> {
        floor_under(self.mesh, point).map(|y| Vec3::new(point.x, y, point.z))
    }

    /// The throw at `at`: the flat arc's pitch for its distance and drop, flown against the bullet mesh
    /// (`arc::Flight::landing`: the first contact and the roll after it); its rest on the floor there. None when no
    /// pitch reaches, the flight meets nothing in its time, or the rest has no floor under it.
    pub fn aim(&self, at: Vec3) -> Option<SmokeAim> {
        let eye = self.thrower_eye;
        let hand = super::arc::hand_of(eye);
        let pitch = self.grenade.flight.pitch_for(hand.distance_xz(at), hand.y - at.y)?;
        let yaw = super::kin::yaw_of([at.x - eye.x, at.z - eye.z]);
        let landing = super::arc::Flight::thrown_by(self.grenade.flight, eye, yaw, pitch)
            .landing(&super::arc::Mesh(self.mesh))?;
        let rest = self.floor(landing.rest + Vec3::new(0.0, 0.5, 0.0))?;
        Some(SmokeAim { yaw, pitch, rest })
    }

    /// The smoke a cloud of this grenade at full radius on `floor` puts on the line from `a` to `b`, metres.
    fn smoke_on_line(&self, floor: Vec3, a: Vec3, b: Vec3) -> f32 {
        let cloud = Cloud {
            by: 0,
            team: 0,
            placed: Placed::Arc,
            centre: floor,
            thrown: MatchMs(0),
            pours: MatchMs(0),
            shape: self.grenade.cloud,
        };
        cloud.inside_m(a, b, MatchMs(self.grenade.cloud.forming_ms))
    }
}

/// A smoke between a threat and a crossing (`SmokeUse::CrossLane`, `AntiSniper`, `ObjectiveActor`, `Breach`): the
/// point on the line from the threat's eye to the crossing (a chest over it), as near the threat as the throw reaches,
/// whose cloud covers that line by `thin_m` + [`PLAN_MARGIN_M`]. Near the threat first: "the closer the smoke is
/// placed to the sniper's location, the more effective" (FM 23-10 6-8a(7)); smoke near our own side still helps,
/// so the points go on toward the crossing. None when no point of the line both reaches and covers.
pub fn smoke_between(
    reach: &Reach<'_>,
    threat_eye: Vec3,
    crossing: Vec3,
    thin_m: f32,
    for_use: SmokeUse,
) -> Option<SmokePlan> {
    let chest = crossing + Vec3::new(0.0, 1.3, 0.0);
    let length = threat_eye.distance_xz(chest);
    let steps = (length / PLAN_STEP_M).floor() as u32;
    (0..=steps).find_map(|k| {
        let t = (k as f32 * PLAN_STEP_M / length.max(1e-3)).min(1.0);
        let on_line = threat_eye + (chest - threat_eye) * t;
        let floor = reach.floor(on_line)?;
        // Judged where the arc rests, not where it was aimed.
        let aim = reach.aim(floor)?;
        let covers_m = reach.smoke_on_line(aim.rest, threat_eye, chest);
        (covers_m > thin_m + PLAN_MARGIN_M).then_some(SmokePlan {
            at: aim.rest,
            aim,
            for_use,
            covers_m,
            line: SmokeLine { eye: threat_eye, target: chest, crossing },
        })
    })
}

/// How much of a crossing's stretch a cloud at `rest` hides from `threat_eye`: of the chest points `stretch` (the route
/// from the crossing on that he sees), those its smoke covers by `thin_m` + [`PLAN_MARGIN_M`].
fn stretch_hidden(reach: &Reach<'_>, rest: Vec3, threat_eye: Vec3, stretch: &[Vec3], thin_m: f32) -> usize {
    stretch
        .iter()
        .filter(|&&p| reach.smoke_on_line(rest, threat_eye, p) > thin_m + PLAN_MARGIN_M)
        .count()
}

/// [A291 `smoke.aim_point`] A smoke between a threat and a crossing that lands mid-line: of the points of the line from
/// the threat's eye to the crossing (a chest over it), `rule.step_m` apart, whose throw reaches and whose cloud covers the
/// line by `thin_m` + [`PLAN_MARGIN_M`], the one that (1) leaves the threat outside the cloud by `rule.shooter_clear_m`, then
/// (2) hides the most of the crossing's stretch `stretch` (chest points of the route from the crossing on that the threat
/// sees), then (3) lies nearest the crossing (the wider the cloud looks from the crossing, the more of the threat's moves
/// it still screens). A cloud on the threat's feet lets him step out of it and see the crossing again; one between hides
/// the crossing wherever he steps along his cover. When no point of that line both reaches and covers at its step, the
/// first-point rule ([`smoke_between`], 1 m steps) decides, so the switch never takes a smoke away.
pub fn smoke_mid_line(
    reach: &Reach<'_>,
    threat_eye: Vec3,
    crossing: Vec3,
    stretch: &[Vec3],
    thin_m: f32,
    for_use: SmokeUse,
    rule: super::tuning::MidLine,
) -> Option<SmokePlan> {
    let chest = crossing + Vec3::new(0.0, 1.3, 0.0);
    let length = threat_eye.distance_xz(chest);
    let step = rule.step_m.get();
    let steps = (length / step).floor() as u32;
    let clear_m = reach.grenade.cloud.radius_m + rule.shooter_clear_m.get();
    let best = (0..=steps)
        .filter_map(|k| {
            let t = (k as f32 * step / length.max(1e-3)).min(1.0);
            let on_line = threat_eye + (chest - threat_eye) * t;
            let floor = reach.floor(on_line)?;
            let aim = reach.aim(floor)?;
            let covers_m = reach.smoke_on_line(aim.rest, threat_eye, chest);
            if covers_m <= thin_m + PLAN_MARGIN_M {
                return None;
            }
            let clear = aim.rest.distance_xz(threat_eye) >= clear_m;
            let hidden = stretch_hidden(reach, aim.rest, threat_eye, stretch, thin_m);
            Some(((clear, hidden, k), SmokePlan {
                at: aim.rest,
                aim,
                for_use,
                covers_m,
                line: SmokeLine { eye: threat_eye, target: chest, crossing },
            }))
        })
        .max_by_key(|(key, _)| *key)
        .map(|(_, plan)| plan);
    best.or_else(|| smoke_between(reach, threat_eye, crossing, thin_m, for_use))
}

/// [A291 `AimPoint::NearShooter`] A smoke a few metres in front of the threat: of the points of the line from his eye to
/// the crossing within `rule.min_m`..`rule.max_m` of him (1 m steps), whose throw reaches and whose cloud covers the line by
/// `thin_m` + [`PLAN_MARGIN_M`], the one whose rest is nearest `rule.short_m` from him, then the one hiding the most of the
/// crossing's stretch. When none reaches there (he is beyond the throw), the first-point rule ([`smoke_between`]): as near
/// him as the throw goes.
pub fn smoke_near_shooter(
    reach: &Reach<'_>,
    threat_eye: Vec3,
    crossing: Vec3,
    stretch: &[Vec3],
    thin_m: f32,
    for_use: SmokeUse,
    rule: super::tuning::NearShooter,
) -> Option<SmokePlan> {
    let chest = crossing + Vec3::new(0.0, 1.3, 0.0);
    let length = threat_eye.distance_xz(chest).max(1e-3);
    let (min_m, max_m, short_m) = (rule.min_m.get(), rule.max_m.get().min(length), rule.short_m.get());
    let first = min_m.ceil() as u32;
    let last = max_m.floor() as u32;
    let best = (first..=last)
        .filter_map(|m| {
            let on_line = threat_eye + (chest - threat_eye) * (m as f32 / length).min(1.0);
            let floor = reach.floor(on_line)?;
            let aim = reach.aim(floor)?;
            let off = aim.rest.distance_xz(threat_eye);
            if off < min_m {
                return None;
            }
            let covers_m = reach.smoke_on_line(aim.rest, threat_eye, chest);
            if covers_m <= thin_m + PLAN_MARGIN_M {
                return None;
            }
            let hidden = stretch_hidden(reach, aim.rest, threat_eye, stretch, thin_m);
            // Nearest `short_m` in decimetres first (smaller is better), then the most hidden.
            let miss_dm = ((off - short_m).abs() * 10.0).round() as u32;
            Some(((std::cmp::Reverse(miss_dm), hidden), SmokePlan {
                at: aim.rest,
                aim,
                for_use,
                covers_m,
                line: SmokeLine { eye: threat_eye, target: chest, crossing },
            }))
        })
        .max_by_key(|(key, _)| *key)
        .map(|(_, plan)| plan);
    best.or_else(|| smoke_between(reach, threat_eye, crossing, thin_m, for_use))
}

/// [A291 `AimPoint::BestCover`] The smoke whose cloud covers the most of the threat's line over the crossing, in 3D: of the
/// points of the line from his eye to the crossing (a chest over it), `rule.step_m` apart, each projected to the floor under
/// it and flown there ([`Reach::aim`]: the arc against the bullet mesh, its rest), those whose cloud (the server's cylinder:
/// radius 8 m, 1.3 m under its floor to 5 m over it) covers the line by `thin_m` + [`PLAN_MARGIN_M`]; the one hiding the
/// most of the crossing's stretch, then one at least `rule.avoid_m` from him, then the most smoke on the line. A raised
/// shooter's line passes over a cloud on the ground in front of him and that point never covers; one below sees up over a
/// cloud on a floor beneath his line. None when no landing covers the line: no smoke for that crossing.
pub fn smoke_best_cover(
    reach: &Reach<'_>,
    threat_eye: Vec3,
    crossing: Vec3,
    stretch: &[Vec3],
    thin_m: f32,
    for_use: SmokeUse,
    rule: super::tuning::BestCover,
) -> Option<SmokePlan> {
    let chest = crossing + Vec3::new(0.0, 1.3, 0.0);
    let length = threat_eye.distance_xz(chest);
    let step = rule.step_m.get();
    let steps = (length / step).floor() as u32;
    (0..=steps)
        .filter_map(|k| {
            let t = (k as f32 * step / length.max(1e-3)).min(1.0);
            let on_line = threat_eye + (chest - threat_eye) * t;
            let floor = reach.floor(on_line)?;
            let aim = reach.aim(floor)?;
            let covers_m = reach.smoke_on_line(aim.rest, threat_eye, chest);
            if covers_m <= thin_m + PLAN_MARGIN_M {
                return None;
            }
            let hidden = stretch_hidden(reach, aim.rest, threat_eye, stretch, thin_m);
            let apart = aim.rest.distance_xz(threat_eye) >= rule.avoid_m.get();
            let covers_dm = (covers_m * 10.0).round() as u32;
            Some(((hidden, apart, covers_dm), SmokePlan {
                at: aim.rest,
                aim,
                for_use,
                covers_m,
                line: SmokeLine { eye: threat_eye, target: chest, crossing },
            }))
        })
        .max_by_key(|(key, _)| *key)
        .map(|(_, plan)| plan)
}

/// [A291 `smoke.shooter_spot`] A smoke on the threat's own spot: the floor under his eye, flown there, kept when it rests
/// within `rule.rest_within_m` of him across the ground (he is in its cloud). Its `covers_m` is the smoke on his line to the
/// crossing, which may be under the sight rule's (a steep line leaves the cloud's bottom fast): the cloud is on HIM.
pub fn smoke_on_shooter(reach: &Reach<'_>, threat_eye: Vec3, crossing: Vec3, for_use: SmokeUse, rule: super::tuning::ShooterSpot) -> Option<SmokePlan> {
    let chest = crossing + Vec3::new(0.0, 1.3, 0.0);
    let floor = reach.floor(threat_eye)?;
    let aim = reach.aim(floor)?;
    if aim.rest.distance_xz(threat_eye) > rule.rest_within_m.get() {
        return None;
    }
    Some(SmokePlan {
        at: aim.rest,
        aim,
        for_use,
        covers_m: reach.smoke_on_line(aim.rest, threat_eye, chest),
        line: SmokeLine { eye: threat_eye, target: chest, crossing },
    })
}

/// [A291] Why a held lane got no covering landing, judged at the shooter's own spot (the scan's first point).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoPlan {
    /// No floor under his eye within reach of the floor test.
    NoFloor,
    /// No throw's pitch reaches his spot (beyond the flat throw), or its flight meets nothing in its time.
    OutOfReach,
    /// The throw comes to rest farther than `ShooterSpot`'s default 4 m from him (it bounced or rolled off his ledge).
    Bounced,
    /// It rests on him, but its cloud does not put the sight rule's smoke on his line (a steep line leaves its bottom).
    NoCover,
}

impl NoPlan {
    pub const ALL: [Self; 4] = [Self::NoFloor, Self::OutOfReach, Self::Bounced, Self::NoCover];

    pub const fn index(self) -> usize {
        match self {
            Self::NoFloor => 0,
            Self::OutOfReach => 1,
            Self::Bounced => 2,
            Self::NoCover => 3,
        }
    }

    /// The reason for a lane from `threat_eye` to `crossing` that no landing covered.
    pub fn of(reach: &Reach<'_>, threat_eye: Vec3, crossing: Vec3) -> Self {
        let Some(floor) = reach.floor(threat_eye) else {
            return Self::NoFloor;
        };
        let Some(aim) = reach.aim(floor) else {
            return Self::OutOfReach;
        };
        if aim.rest.distance_xz(threat_eye) > super::tuning::ShooterSpot::default().rest_within_m.get() {
            Self::Bounced
        } else {
            let _ = crossing;
            Self::NoCover
        }
    }
}

/// [A291] How a shooter stands to the crossing he covers: his eye over the chest over it by more than [`LEVEL_M`], within
/// it, or under it by more.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Height {
    Above,
    Level,
    Below,
}

/// [A291] A floor's height, about: a shooter more than this over or under the crossing's chest is on another level.
pub const LEVEL_M: f32 = 2.5;

impl Height {
    pub const ALL: [Self; 3] = [Self::Above, Self::Level, Self::Below];

    pub fn of(eye: Vec3, chest: Vec3) -> Self {
        let up = eye.y - chest.y;
        if up > LEVEL_M {
            Self::Above
        } else if up < -LEVEL_M {
            Self::Below
        } else {
            Self::Level
        }
    }

    pub const fn index(self) -> usize {
        match self {
            Self::Above => 0,
            Self::Level => 1,
            Self::Below => 2,
        }
    }
}

/// [A291 `smoke.aim_point`] The smoke on a threat's line to a crossing by `rule`, or by the first-point rule with none.
pub fn smoke_aimed(
    reach: &Reach<'_>,
    threat_eye: Vec3,
    crossing: Vec3,
    stretch: &[Vec3],
    thin_m: f32,
    for_use: SmokeUse,
    rule: Option<super::tuning::AimPoint>,
) -> Option<SmokePlan> {
    use super::tuning::AimPoint;
    match rule {
        None => smoke_between(reach, threat_eye, crossing, thin_m, for_use),
        Some(AimPoint::MidLine(mid)) => smoke_mid_line(reach, threat_eye, crossing, stretch, thin_m, for_use, mid),
        Some(AimPoint::NearShooter(near)) => smoke_near_shooter(reach, threat_eye, crossing, stretch, thin_m, for_use, near),
        Some(AimPoint::BestCover(best)) => smoke_best_cover(reach, threat_eye, crossing, stretch, thin_m, for_use, best),
    }
}

/// [A291] What the landing of a smoke plan did to its line, for the counters (switch on or off): whether its cloud stands
/// on the threat (centre within the cloud's radius + [`ON_SHOOTER_CLEAR_M`] of him), how far it is from him and from the
/// crossing, and how much of the crossing's stretch it hides.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Landing {
    pub on_shooter: bool,
    pub shooter_m: f32,
    pub crossing_m: f32,
    pub hidden: usize,
    pub stretch: usize,
}

/// [A291] A cloud whose centre is within its radius and this of the threat stands on him (the default `MidLine`'s).
pub const ON_SHOOTER_CLEAR_M: f32 = 2.0;

impl Landing {
    pub fn of(reach: &Reach<'_>, plan: &SmokePlan, stretch: &[Vec3], thin_m: f32) -> Self {
        let shooter_m = plan.at.distance_xz(plan.line.eye);
        Self {
            on_shooter: shooter_m < reach.grenade.cloud.radius_m + ON_SHOOTER_CLEAR_M,
            shooter_m,
            crossing_m: plan.at.distance_xz(plan.line.crossing),
            hidden: stretch_hidden(reach, plan.at, plan.line.eye, stretch, thin_m),
            stretch: stretch.len(),
        }
    }
}

/// A smoke on a spot (a held spot, its holder's eye `spot_eye`): the floor under it, if the throw reaches it. Its
/// `covers_m` is the smoke on the line from the spot's eye to `toward` (the thrower's side), its line that one (the
/// crossing at `toward`).
pub fn smoke_on(reach: &Reach<'_>, spot_eye: Vec3, toward: Vec3, for_use: SmokeUse) -> Option<SmokePlan> {
    let floor = reach.floor(spot_eye)?;
    let aim = reach.aim(floor)?;
    Some(SmokePlan {
        at: aim.rest,
        aim,
        for_use,
        covers_m: reach.smoke_on_line(aim.rest, spot_eye, toward),
        line: SmokeLine { eye: spot_eye, target: toward, crossing: toward },
    })
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
        /// A glance at a cloud's edge (`smoke.react_watch`), taken.
        EdgeWatch,
        /// An edge watch due but not taken: something else held the look (`Reflex::watch_smoke`).
        WatchBusy,
        /// A cloud learnt by a bot ([`super::CloudSource`]): seen, heard pop, told.
        LearntSeen,
        LearntHeard,
        LearntTold,
        /// [smoke.anti_sniper] A think outranged by a sniper it saw within 15 s, with a smoke in the kit and the hands
        /// free: a plan made, or none (no point of the line both in the throw's reach and covering it).
        Outranged,
        OutrangedNoPlan,
        /// [smoke.anti_sniper] A think outranged by a sniper it saw within 15 s, whatever its kit or hands.
        OutrangedAny,
        /// [smoke.spot_rules] A smoke plan refused: smoke is there already (a cloud live or forming, or the team's in the
        /// air), or a mate's live claim is within `claim_m` of it or on the same shooter.
        CloudThere,
        ClaimRefused,
        /// [smoke.spot_rules] A bot's route ahead (`lane_ahead_m`) enters ground a held lane covers but a cloud hides from
        /// its shooter now: it crosses under the cloud (counted once a crossing, 10 s apart a bot).
        UnderCloud,
        /// A think with a smoke plan to make (an outranging sniper not in sight of the bot, or a held lane the bot is not in) that
        /// the bot's held sprint (`sprinting` in `think::smoke_uses`) shut off: a short gap is sprinted, not smoked.
        SprintSkip,
        /// [A291 `smoke.shooter_spot`] A plan made on the shooter's own spot because no landing covered his line.
        ShooterSpotPlan,
    }

    impl Event {
        pub fn learnt(source: super::CloudSource) -> Self {
            match source {
                super::CloudSource::Seen => Self::LearntSeen,
                super::CloudSource::Heard => Self::LearntHeard,
                super::CloudSource::Told => Self::LearntTold,
            }
        }
    }

    pub const EVENTS: usize = 17;

    /// What became of the bots' smoke throws: begun, thrown (the key let go), given up (the view never came round),
    /// died (the life ended with it under way). Every throw begun ends in one of the last three.
    pub const THROW_STEPS: usize = 4;
    const ROLES: usize = survarium::v026g::loadouts::BotRole::ALL.len();
    /// By team, role and use ([`super::SmokeUse::ALL`]), each step.
    pub type Throws = [[[[u64; THROW_STEPS]; super::SmokeUse::ALL.len()]; ROLES]; 2];

    thread_local! {
        static THROWS: Cell<Throws> = const { Cell::new([[[[0; THROW_STEPS]; super::SmokeUse::ALL.len()]; ROLES]; 2]) };
        /// The role of the bot whose reflex ticks now, for the throw steps the reflex counts.
        static ROLE: Cell<usize> = const { Cell::new(0) };
    }

    /// The bot ticking now is of role `role` (its index in `BotRole::ALL`).
    pub fn set_role(role: usize) {
        ROLE.with(|r| r.set(role.min(ROLES - 1)));
    }

    fn step(team: u8, for_use: super::SmokeUse, step: usize) {
        let role = ROLE.with(Cell::get);
        THROWS.with(|t| {
            let mut throws = t.get();
            throws[usize::from(team & 1)][role][for_use.index()][step] += 1;
            t.set(throws);
        });
    }

    /// A smoke throw begun.
    pub fn begun(team: u8, for_use: super::SmokeUse) {
        step(team, for_use, 0);
    }

    /// A smoke throw let go (`true`) or given up (`false`).
    pub fn thrown(team: u8, for_use: super::SmokeUse, done: bool) {
        step(team, for_use, if done { 1 } else { 2 });
    }

    /// A smoke throw under way when its thrower's life ended.
    pub fn died(team: u8, for_use: super::SmokeUse) {
        step(team, for_use, 3);
    }

    /// [A208 `smoke.wait_for_cloud`] What became of the holds for a cloud, and (switch on or off) of each thrown smoke's
    /// crossing, by team and whose cloud (`WaitsFor::index`).
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Wait {
        /// A hold begun; it ends in one of the next six.
        Begun,
        /// The cloud hid the line.
        Hidden,
        /// A target took the bot.
        Fight,
        /// Hit since the hold began.
        Hit,
        /// An enemy its own senses put within `close_m`.
        Close,
        /// The cap ran out.
        Cap,
        /// The life ended in the hold.
        Died,
        /// The time held, ms (summed).
        HeldMs,
        /// A thrown smoke's bot came to its crossing (within 2.5 m) with the line hidden from the client's eyes, or
        /// open, or not within 15 s of the launch (its route went elsewhere, or it died).
        CrossedHidden,
        CrossedOpen,
        CrossedNever,
    }

    impl Wait {
        pub fn ended(end: super::HoldEnd) -> Self {
            match end {
                super::HoldEnd::Hidden => Self::Hidden,
                super::HoldEnd::Fight => Self::Fight,
                super::HoldEnd::Hit => Self::Hit,
                super::HoldEnd::Close => Self::Close,
                super::HoldEnd::Cap => Self::Cap,
            }
        }
    }

    pub const WAITS: usize = 11;
    pub type Waits = [[[u64; WAITS]; super::WaitsFor::COUNT]; 2];

    thread_local! {
        static WAIT_COUNTS: Cell<Waits> = const { Cell::new([[[0; WAITS]; super::WaitsFor::COUNT]; 2]) };
    }

    pub fn wait(team: u8, waits_for: super::WaitsFor, what: Wait, n: u64) {
        WAIT_COUNTS.with(|c| {
            let mut counts = c.get();
            counts[usize::from(team & 1)][waits_for.index()][what as usize] += n;
            c.set(counts);
        });
    }

    pub fn waits() -> Waits {
        WAIT_COUNTS.with(Cell::get)
    }

    /// [A291 `smoke.aim_point`] Where the smoke plans begun landed and what became of their throwers, by team and use
    /// (switch on or off): [plans, on the shooter, shooter-to-cloud dm summed, crossing-to-cloud dm summed, stretch points
    /// hidden, stretch points, thrower alive at 15 s, died to the plan's shooter within 15 s, died to another, then over the
    /// follow (alive) ticks followed, ticks the thrower was hidden by smoke from the shooter's eye, mate-ticks whose line to
    /// the shooter's chest smoke hid (own fire blocked), mate-ticks (mates alive within `SEE_MATES_M` of him)].
    pub const LANDING_SLOTS: usize = 13;
    /// Mates this near the plan's shooter count for the own-fire measure, metres (the bots' `smoke.see_m`).
    pub const SEE_MATES_M: f32 = 80.0;
    /// A plan's thrower is followed this long, ms.
    pub const LANDING_FOLLOW_MS: u32 = 15_000;
    pub type Landings = [[[u64; LANDING_SLOTS]; super::SmokeUse::ALL.len()]; 2];
    /// A plan whose thrower is followed: (place, team, use, shooter, begun, the shooter's height to the crossing).
    type Followed = (usize, u8, super::SmokeUse, Option<usize>, u32, super::Height);

    thread_local! {
        static LANDINGS: Cell<Landings> = const { Cell::new([[[0; LANDING_SLOTS]; super::SmokeUse::ALL.len()]; 2]) };
        static LANDING_OPEN: std::cell::RefCell<Vec<Followed>> = const { std::cell::RefCell::new(Vec::new()) };
    }

    pub fn landing_add(team: u8, for_use: super::SmokeUse, slot: usize, n: u64) {
        LANDINGS.with(|c| {
            let mut counts = c.get();
            counts[usize::from(team & 1)][for_use.index()][slot] += n;
            c.set(counts);
        });
    }

    /// A smoke plan begun by the bot at `place` against `shooter` (at `height` to the crossing) at `now`, landing so.
    pub fn landed(place: usize, team: u8, for_use: super::SmokeUse, shooter: Option<usize>, now: u32, landing: super::Landing, height: super::Height) {
        height_plan(team, height, landing);
        let dm = |m: f32| (m * 10.0).round().max(0.0) as u64;
        landing_add(team, for_use, 0, 1);
        landing_add(team, for_use, 1, u64::from(landing.on_shooter));
        landing_add(team, for_use, 2, dm(landing.shooter_m));
        landing_add(team, for_use, 3, dm(landing.crossing_m));
        landing_add(team, for_use, 4, landing.hidden as u64);
        landing_add(team, for_use, 5, landing.stretch as u64);
        LANDING_OPEN.with(|o| {
            let mut open = o.borrow_mut();
            open.retain(|&(p, ..)| p != place);
            open.push((place, team, for_use, shooter, now, height));
        });
    }

    /// The followed thrower at `place`: its fate at `tick` when it died (killed by `killer`) or its time ran out alive.
    pub fn landing_fate(place: usize, tick: u32, died: bool, killer: Option<usize>) {
        LANDING_OPEN.with(|o| {
            let mut open = o.borrow_mut();
            let Some(i) = open.iter().position(|&(p, ..)| p == place) else {
                return;
            };
            let (_, team, for_use, shooter, begun, height) = open[i];
            let ran_out = tick.saturating_sub(begun) >= LANDING_FOLLOW_MS;
            if !died && !ran_out {
                return;
            }
            open.swap_remove(i);
            let slot = match (died && !ran_out, killer) {
                (false, _) => 6,
                (true, Some(k)) if Some(k) == shooter => 7,
                (true, _) => 8,
            };
            landing_add(team, for_use, slot, 1);
            height_add(team, height, slot - 1, 1);
        });
    }

    /// A tick of the followed thrower at `place` alive: hidden by smoke from its shooter's eye or not, and of `mates` of its
    /// team near him, `blocked` with smoke on their line to him.
    pub fn landing_tick(place: usize, hidden: bool, blocked: usize, mates: usize) {
        let Some((team, for_use, _, height)) = landing_followed(place) else {
            return;
        };
        landing_add(team, for_use, 9, 1);
        landing_add(team, for_use, 10, u64::from(hidden));
        landing_add(team, for_use, 11, blocked as u64);
        landing_add(team, for_use, 12, mates as u64);
        height_add(team, height, 8, 1);
        height_add(team, height, 9, u64::from(hidden));
    }

    /// The followed plan of the thrower at `place`, if any: (team, use, shooter, height).
    pub fn landing_followed(place: usize) -> Option<(u8, super::SmokeUse, Option<usize>, super::Height)> {
        LANDING_OPEN.with(|o| o.borrow().iter().find(|&&(p, ..)| p == place).map(|&(_, team, u, shooter, _, h)| (team, u, shooter, h)))
    }

    pub fn landings() -> Landings {
        LANDINGS.with(Cell::get)
    }

    /// [A291] By team and the shooter's height to the crossing ([`super::Height::ALL`]): [plans, on the shooter, stretch
    /// points hidden, stretch points, held lanes with no landing that covers the line (no smoke), thrower alive at 15 s,
    /// died to the plan's shooter within 15 s, died to another, ticks followed, ticks hidden from the shooter], CrossLane,
    /// AntiSniper and the objective smoke together, switch on or off.
    pub const HEIGHT_SLOTS: usize = 10;
    pub type Heights = [[[u64; HEIGHT_SLOTS]; super::Height::ALL.len()]; 2];

    thread_local! {
        static HEIGHTS: Cell<Heights> = const { Cell::new([[[0; HEIGHT_SLOTS]; super::Height::ALL.len()]; 2]) };
    }

    fn height_add(team: u8, height: super::Height, slot: usize, n: u64) {
        HEIGHTS.with(|c| {
            let mut counts = c.get();
            counts[usize::from(team & 1)][height.index()][slot] += n;
            c.set(counts);
        });
    }

    /// A plan begun against a shooter at `height` to its crossing, landing so.
    fn height_plan(team: u8, height: super::Height, landing: super::Landing) {
        height_add(team, height, 0, 1);
        height_add(team, height, 1, u64::from(landing.on_shooter));
        height_add(team, height, 2, landing.hidden as u64);
        height_add(team, height, 3, landing.stretch as u64);
    }

    /// A held lane to smoke whose line no landing covered.
    pub fn height_no_plan(team: u8, height: super::Height) {
        height_add(team, height, 4, 1);
    }

    pub fn heights() -> Heights {
        HEIGHTS.with(Cell::get)
    }

    /// [A291] What the plan's shooter did, by team and where the cloud went (between, on him): [plans followed, he moved
    /// more than [`SHOOTER_MOVED_M`] from where he stood 5 s after the plan within the next 5 s, his kills within 15 s of the
    /// plan], switch on or off.
    pub const SHOOTER_SLOTS: usize = 3;
    pub const SHOOTER_MOVED_M: f32 = 3.0;
    pub type Shooters = [[[u64; SHOOTER_SLOTS]; 2]; 2];
    /// (shooter, team, on him, begun, where he stood at begun + 5 s, moved).
    type Watched = (usize, u8, bool, u32, Option<super::Vec3>, bool);

    thread_local! {
        static SHOOTERS: Cell<Shooters> = const { Cell::new([[[0; SHOOTER_SLOTS]; 2]; 2]) };
        static SHOOTER_OPEN: std::cell::RefCell<Vec<Watched>> = const { std::cell::RefCell::new(Vec::new()) };
    }

    /// A plan begun at `now` against `shooter`, its cloud on him or not.
    pub fn shooter_watch(team: u8, shooter: usize, on_him: bool, now: u32) {
        SHOOTER_OPEN.with(|o| o.borrow_mut().push((shooter, team, on_him, now, None, false)));
    }

    /// The body at `place` alive at `tick` with its feet at `feet`: a watched shooter's stand and move.
    pub fn shooter_tick(place: usize, tick: u32, feet: super::Vec3) {
        SHOOTER_OPEN.with(|o| {
            for w in o.borrow_mut().iter_mut().filter(|w| w.0 == place) {
                match w.4 {
                    None if tick >= w.3.saturating_add(5_000) => w.4 = Some(feet),
                    Some(at) if tick <= w.3.saturating_add(10_000) && at.distance_xz(feet) > SHOOTER_MOVED_M => w.5 = true,
                    _ => {}
                }
            }
        });
    }

    /// At `tick`, the watches 15 s old end: counted with the shooter's kills (`kills`: time, killer, victim) since the plan.
    pub fn shooter_close(tick: u32, kills: &[(u32, usize, usize)]) {
        SHOOTER_OPEN.with(|o| {
            o.borrow_mut().retain(|&(shooter, team, on_him, begun, _, moved)| {
                if tick < begun.saturating_add(15_000) {
                    return true;
                }
                let his = kills.iter().filter(|&&(t, k, _)| k == shooter && t > begun && t <= begun + 15_000).count() as u64;
                SHOOTERS.with(|c| {
                    let mut counts = c.get();
                    let row = &mut counts[usize::from(team & 1)][usize::from(on_him)];
                    row[0] += 1;
                    row[1] += u64::from(moved);
                    row[2] += his;
                    c.set(counts);
                });
                false
            });
        });
    }

    pub fn shooters() -> Shooters {
        SHOOTERS.with(Cell::get)
    }

    /// [A291] The lanes with no covering landing by team, the shooter's height and the reason ([`super::NoPlan::ALL`]).
    pub type NoPlans = [[[u64; super::NoPlan::ALL.len()]; super::Height::ALL.len()]; 2];

    thread_local! {
        static NO_PLANS: Cell<NoPlans> = const { Cell::new([[[0; super::NoPlan::ALL.len()]; super::Height::ALL.len()]; 2]) };
    }

    pub fn no_plan_reason(team: u8, height: super::Height, why: super::NoPlan) {
        NO_PLANS.with(|c| {
            let mut counts = c.get();
            counts[usize::from(team & 1)][height.index()][why.index()] += 1;
            c.set(counts);
        });
    }

    pub fn no_plans() -> NoPlans {
        NO_PLANS.with(Cell::get)
    }

    pub fn throws() -> Throws {
        THROWS.with(Cell::get)
    }

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

    thread_local! {
        static THREW: std::cell::RefCell<std::collections::BTreeMap<usize, u32>> =
            const { std::cell::RefCell::new(std::collections::BTreeMap::new()) };
    }

    thread_local! {
        /// By place: a sniper crossing under way (the sniper, when it began, whether a smoke was begun for it).
        static SNIPER_CROSSINGS: std::cell::RefCell<std::collections::BTreeMap<usize, (usize, u32, bool)>> =
            const { std::cell::RefCell::new(std::collections::BTreeMap::new()) };
    }

    /// The bot at `place` is about to cross ground the sniper `who` sees (`smoke_uses`, either arm): kept until it is
    /// resolved (the smoke topic: died to him, died otherwise or crossed within its time). One at a time a bot.
    pub fn sniper_crossing(place: usize, who: usize, now: u32) {
        SNIPER_CROSSINGS.with(|s| {
            s.borrow_mut().entry(place).or_insert((who, now, false));
        });
    }

    /// A smoke was begun for the bot at `place`'s sniper crossing.
    pub fn sniper_smoked(place: usize) {
        SNIPER_CROSSINGS.with(|s| {
            if let Some(c) = s.borrow_mut().get_mut(&place) {
                c.2 = true;
            }
        });
    }

    /// The sniper crossing under way of the bot at `place`, taken if `done` says it is resolved.
    pub fn take_sniper_crossing(place: usize, done: impl FnOnce((usize, u32, bool)) -> bool) -> Option<(usize, u32, bool)> {
        SNIPER_CROSSINGS.with(|s| {
            let mut s = s.borrow_mut();
            let c = *s.get(&place)?;
            done(c).then(|| s.remove(&place)).flatten()
        })
    }

    /// An anti-sniper smoke as planned: the thrower, its team, the sniper, his eye as sensed, the crossing, when begun, and
    /// when the thrower's route first no longer ran by the crossing (a new route: the plan changed after the throw), and what
    /// ended the route it had (`RouteEnd`'s name), if anything did.
    #[derive(Clone, Copy, Debug)]
    pub struct SniperPlan {
        pub place: usize,
        pub team: u8,
        pub who: usize,
        pub eye: super::Vec3,
        pub crossing: super::Vec3,
        pub time: u32,
        pub route_left: Option<(u32, &'static str)>,
    }

    /// How long after its begin a plan's route is watched, ms (the throw classes' reach time).
    const SNIPER_ROUTE_WATCH_MS: u32 = 15_000;

    /// The bot at `place` at `now`: each of its plans begun within [`SNIPER_ROUTE_WATCH_MS`] whose crossing `on_route` no
    /// longer holds for is marked as left, with `why` (what last ended a route of the bot's).
    pub fn sniper_routes(place: usize, now: u32, why: &'static str, on_route: impl Fn(super::Vec3) -> bool) {
        SNIPER_PLANS.with(|s| {
            for p in s.borrow_mut().iter_mut() {
                if p.place == place && p.route_left.is_none() && now.saturating_sub(p.time) <= SNIPER_ROUTE_WATCH_MS && !on_route(p.crossing) {
                    p.route_left = Some((now, why));
                }
            }
        });
    }

    thread_local! {
        static SNIPER_CANDIDATE: std::cell::RefCell<std::collections::BTreeMap<usize, (usize, super::Vec3, super::Vec3)>> =
            const { std::cell::RefCell::new(std::collections::BTreeMap::new()) };
        static SNIPER_PLANS: std::cell::RefCell<Vec<SniperPlan>> = const { std::cell::RefCell::new(Vec::new()) };
    }

    /// The bot at `place` planned a smoke against `who` (his eye `eye`) for its crossing at `crossing`.
    pub fn sniper_candidate(place: usize, who: usize, eye: super::Vec3, crossing: super::Vec3) {
        SNIPER_CANDIDATE.with(|s| s.borrow_mut().insert(place, (who, eye, crossing)));
    }

    /// The bot at `place` began its anti-sniper smoke at `now`: its candidate is a plan.
    pub fn sniper_begun(place: usize, team: u8, now: u32) {
        if let Some((who, eye, crossing)) = SNIPER_CANDIDATE.with(|s| s.borrow().get(&place).copied()) {
            SNIPER_PLANS.with(|s| s.borrow_mut().push(SniperPlan { place, team, who, eye, crossing, time: now, route_left: None }));
        }
    }

    /// The anti-sniper plans begun so far this match.
    pub fn sniper_plans() -> Vec<SniperPlan> {
        SNIPER_PLANS.with(|s| s.borrow().clone())
    }

    /// A match begins on this thread: the per-place records of the last one go (places are reused).
    pub fn begin_match() {
        SNIPER_CANDIDATE.with(|s| s.borrow_mut().clear());
        SNIPER_PLANS.with(|s| s.borrow_mut().clear());
        SNIPER_CROSSINGS.with(|s| s.borrow_mut().clear());
        UNDER_CLOUD.with(|s| s.borrow_mut().clear());
        THREW.with(|s| s.borrow_mut().clear());
        SEEN_LOOKS.with(|s| s.borrow_mut().clear());
        SPRAYED.with(|s| s.borrow_mut().clear());
        LANDING_OPEN.with(|s| s.borrow_mut().clear());
        SHOOTER_OPEN.with(|s| s.borrow_mut().clear());
    }

    /// The bot at `place` let go of a smoke at `now`.
    pub fn threw(place: usize, now: u32) {
        THREW.with(|s| s.borrow_mut().insert(place, now));
    }

    /// When the bot at `place` last let go of a smoke.
    pub fn last_threw(place: usize) -> Option<u32> {
        THREW.with(|s| s.borrow().get(&place).copied())
    }

    thread_local! {
        static UNDER_CLOUD: std::cell::RefCell<std::collections::BTreeMap<usize, u32>> =
            const { std::cell::RefCell::new(std::collections::BTreeMap::new()) };
    }

    /// The bot at `place` crosses under an existing cloud (`commander::under_cloud`): counted once 10 s apart.
    pub fn under_cloud(place: usize, team: u8, now: u32) {
        let fresh = UNDER_CLOUD.with(|s| {
            let mut s = s.borrow_mut();
            let fresh = s.get(&place).is_none_or(|&t| now.saturating_sub(t) >= 10_000);
            if fresh {
                s.insert(place, now);
            }
            fresh
        });
        if fresh {
            add(team, Event::UnderCloud);
        }
    }

    /// When the bot at `place` last fired a spray.
    pub fn last_spray(place: usize) -> Option<u32> {
        SPRAYED.with(|s| s.borrow().iter().find(|&&(p, _)| p == place).map(|&(_, t)| t))
    }

    /// A look that found an enemy (`aimpoint::look_parts` showed him) as the bot made it: the view's clock, the
    /// bot's eye as its view had it, his feet and crouch as its view had them. The fairness count replays it.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct SeenLook {
        pub time: u32,
        pub eye: super::Vec3,
        pub feet: super::Vec3,
        pub crouched: bool,
    }

    thread_local! {
        static SEEN_LOOKS: std::cell::RefCell<std::collections::BTreeMap<(usize, usize), SeenLook>> =
            const { std::cell::RefCell::new(std::collections::BTreeMap::new()) };
    }

    /// The bot at `place` saw `who` with `look` (the think's look or the aim's).
    pub fn looked_seen(place: usize, who: usize, look: SeenLook) {
        SEEN_LOOKS.with(|s| s.borrow_mut().insert((place, who), look));
    }

    /// The last look of the bot at `place` that found `who`.
    pub fn seen_look(place: usize, who: usize) -> Option<SeenLook> {
        SEEN_LOOKS.with(|s| s.borrow().get(&(place, who)).copied())
    }

    /// The look of `me` that found `enemy` at `clock`.
    pub fn record_seen(me: &crate::v026g::bots::view::PlaceView, enemy: &crate::v026g::bots::view::PlaceView, clock: u32) {
        looked_seen(
            me.place,
            enemy.place,
            SeenLook { time: clock, eye: me.eye(), feet: enemy.feet, crouched: enemy.crouched() },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::bots::tuning::SmokeTuning;

    #[test]
    fn a_sniper_the_kill_feed_showed_dead_is_smoked_against_no_more() {
        use crate::v026g::bots::view::Death;
        let death = |place, time| Death { place, at: Vec3::ZERO, time };
        // Sensed at 20 s, the window 5 s.
        assert!(died_since(&[death(7, 21_000)], 7, 20_000, 5_000), "dead since the sensing");
        assert!(died_since(&[death(7, 18_000)], 7, 20_000, 5_000), "dead within the sensing's lag");
        assert!(!died_since(&[death(7, 9_000)], 7, 20_000, 5_000), "an earlier life of his: sensed again since");
        assert!(!died_since(&[death(8, 21_000)], 7, 20_000, 5_000), "another player's death");
        assert!(!died_since(&[], 7, 20_000, 5_000));
    }

    fn cloud(at: Vec3) -> Cloud {
        SmokeThrow {
            by: 0,
            team: 0,
            press: MatchMs(1_000),
            grenade: SmokeGrenade::RDG_M,
        }
        .cloud_at(at, |_| None)
    }

    /// [smoke.forming_client] A forming cloud hides from the client's 1.24 s after the blast, not the shape's 0.7 s;
    /// once formed the two read alike.
    #[test]
    fn with_the_client_forming_a_cloud_hides_from_its_client_time() {
        let c = cloud(Vec3::ZERO);
        let pours = c.pours.0;
        let at = |ms: u32| MatchMs(pours + ms);
        assert_eq!(c.radius_formed(at(1_000), Forming::OfShape), Some(8.0), "the shape's: formed at 0.7 s");
        assert_eq!(c.radius_formed(at(1_000), Forming::Client), None, "the client's: still forming");
        assert_eq!(c.radius_formed(at(CLIENT_FORMING_MS), Forming::Client), Some(8.0));
        assert_eq!(c.radius_formed(at(17_499), Forming::Client), c.radius_formed(at(17_499), Forming::OfShape));
        // A line through its middle 1 s after the blast: seen with the client's forming, hidden without.
        let tuning = |client: bool| SmokeTuning { forming_client: client, ..SmokeTuning::default() };
        let clouds = Clouds::new(vec![c]);
        let (a, b) = (Vec3::new(-20.0, 1.5, 0.0), Vec3::new(20.0, 1.5, 0.0));
        assert!(clouds.covers(pours + 1_000, &tuning(false), a, b));
        assert!(!clouds.covers(pours + 1_000, &tuning(true), a, b));
        assert!(clouds.covers(pours + 1_300, &tuning(true), a, b));
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
        let full = MatchMs(c.pours.0 + Forming::of(&SmokeTuning::default()).ms(&c.shape) + 100);
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
        let full = c.pours.0 + Forming::of(&on).ms(&c.shape);
        assert!(!clouds.veil(full, &on).is_clear());
        assert!(clouds.veil(full, &off).is_clear());
        assert!(clouds.veil(c.thrown.0, &on).is_clear(), "still in the air");
        assert!(clouds.veil(c.ends().0, &on).is_clear(), "gone");
        assert!(Clouds::default().veil(full, &on).is_clear());
    }

    #[test]
    fn a_bot_knows_the_clouds_it_sees_hears_or_is_told_of_and_watches_the_near_edge_of_one_not_its_teams() {
        let t = SmokeTuning::default();
        // Team 0's (`cloud`, thrown by place 0) 30 m ahead; team 1's 30 m aside, thrown by place 4.
        let enemy = cloud(Vec3::new(0.0, 0.0, 30.0));
        let mate = Cloud {
            team: 1,
            by: 4,
            ..cloud(Vec3::new(30.0, 0.0, 0.0))
        };
        let clouds = Clouds::new(vec![enemy, mate]);
        let who = Watcher {
            me: 5,
            team: 1,
            eye: Vec3::new(0.0, 1.6, 0.0),
        };
        let pop = enemy.pours.0;
        let update = |known: &mut KnownClouds, who: Watcher, seen: bool, at: u32| {
            known.update(who, &clouds, Veil::Clear, at, &t, |_| seen)
        };
        // Not seen, not near: nothing, its own team's neither before the callout.
        let mut known = KnownClouds::default();
        assert_eq!(update(&mut known, who, false, pop + t.told_ms - 50), [0, 0, 0]);
        // The callout: its mate's is told; the enemy's is not.
        assert_eq!(update(&mut known, who, false, pop + t.told_ms), [0, 0, 1]);
        assert_eq!(known.source(mate.key()), Some(CloudSource::Told));
        assert!(!known.knows(enemy.key()));
        // The thrower knows his own from the pop.
        let mut thrower = KnownClouds::default();
        update(&mut thrower, Watcher { me: 4, ..who }, false, pop);
        assert_eq!(thrower.source(mate.key()), Some(CloudSource::Told));
        // Before it smokes, nothing to see; seen: known.
        update(&mut known, who, true, pop - 100);
        assert!(!known.knows(enemy.key()));
        assert_eq!(update(&mut known, who, true, pop + 1_000), [1, 0, 0]);
        assert_eq!(known.source(enemy.key()), Some(CloudSource::Seen));
        // Heard: within `heard_m` while it begins to pour only; a 20 s old cloud near is not heard.
        let close = Clouds::new(vec![cloud(Vec3::new(0.0, 0.0, 12.0))]);
        let mut near = KnownClouds::default();
        near.update(who, &close, Veil::Clear, pop + t.heard_ms - 10, &t, |_| false);
        assert_eq!(near.source(close.all()[0].key()), Some(CloudSource::Heard));
        let mut late = KnownClouds::default();
        late.update(who, &close, Veil::Clear, pop + 20_000, &t, |_| false);
        assert!(!late.knows(close.all()[0].key()));
        // Seen only past the other clouds: a cloud between hides the one behind it.
        let behind = cloud(Vec3::new(0.0, 0.0, 40.0));
        let screen = Cloud {
            by: 3,
            ..cloud(Vec3::new(0.0, 0.0, 15.0))
        };
        let two = Clouds::new(vec![behind, screen]);
        let thin = SmokeTuning::default();
        let mut through = KnownClouds::default();
        let formed = pop + Forming::of(&thin).ms(&screen.shape) + 300;
        let learnt = through.update(who, &two, two.veil(formed, &thin), formed, &thin, |_| true);
        assert!(through.knows(screen.key()) && !through.knows(behind.key()), "{learnt:?}");
        // The enemy cloud's edge facing the bot, on its floor, 22 m off (8 m radius at 30 m), within the 40 m watch.
        let now = pop + 1_000;
        let watch = known.edge_to_watch(who, Vec3::ZERO, &clouds, now, &t).unwrap();
        assert_eq!(watch.key, enemy.key());
        assert!(watch.first && (watch.at - Vec3::new(0.0, 0.0, 22.0)).length() < 1e-3, "{watch:?}");
        // Again only after `watch_every_ms`, and no longer the first.
        assert_eq!(known.edge_to_watch(who, Vec3::ZERO, &clouds, now + 1_000, &t), None);
        let again = known.edge_to_watch(who, Vec3::ZERO, &clouds, now + t.watch_every_ms, &t).unwrap();
        assert!(!again.first);
        // A mate's cloud seen before the callout is of unknown side: watched; once told, never. (With the default
        // callout, 400 ms, a mate's cloud is told before it hides: forming takes 700 ms. A later one shows the rule.)
        let slow = SmokeTuning {
            told_ms: 3_000,
            ..SmokeTuning::default()
        };
        let mut early = KnownClouds::default();
        let mates = Clouds::new(vec![mate]);
        early.update(who, &mates, Veil::Clear, pop + 1_000, &slow, |_| true);
        assert_eq!(early.source(mate.key()), Some(CloudSource::Seen));
        let side = Vec3::new(10.0, 0.0, 0.0);
        assert!(early.edge_to_watch(who, side, &mates, pop + 1_000, &slow).is_some());
        assert_eq!(early.edge_to_watch(who, side, &mates, pop + 3_000 + slow.watch_every_ms, &slow), None);
        // Inside the cloud, or beyond the watch, nothing.
        let later = now + 3 * t.watch_every_ms;
        assert_eq!(known.edge_to_watch(who, Vec3::new(0.0, 0.0, 28.0), &clouds, later, &t), None);
        assert_eq!(known.edge_to_watch(who, Vec3::new(0.0, 0.0, -30.0), &clouds, later, &t), None);
        // Gone: forgotten.
        update(&mut known, who, true, enemy.ends().0);
        assert!(!known.knows(enemy.key()));
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
        let formed = Forming::of(&SmokeTuning::default()).ms(&SmokeGrenade::RDG_M.cloud);
        assert_eq!(looks(&view(me, him, &cloud, formed - 50)), SEEN, "still forming");
        assert_eq!(looks(&view(me, him, &cloud, formed)), SMOKED);
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

    /// A floor at 0, 200 m a side.
    fn flat() -> RayTree {
        let (a, b) = (Vec3::new(-100.0, 0.0, -100.0), Vec3::new(100.0, 0.0, -100.0));
        let (c, d) = (Vec3::new(100.0, 0.0, 100.0), Vec3::new(-100.0, 0.0, 100.0));
        RayTree::from_corners([[a, b, c], [a, c, d]])
    }

    #[test]
    fn a_smoke_between_goes_as_near_the_threat_as_the_throw_reaches_and_covers_the_line() {
        let mesh = flat();
        let reach = Reach {
            mesh: &mesh,
            grenade: SmokeGrenade::RDG_M,
            thrower_eye: Vec3::new(0.0, 1.6, 0.0),
        };
        let tuning = SmokeTuning::default();
        let thin = tuning.thin_m.get();
        // A holder 50 m out, a crossing 15 m to the thrower's side: the line runs from 50 m to 15 m out.
        let (threat, crossing) = (Vec3::new(0.0, 1.6, 50.0), Vec3::new(10.0, 0.0, 15.0));
        let plan = smoke_between(&reach, threat, crossing, thin, SmokeUse::CrossLane).unwrap();
        let out = plan.at.distance_xz(reach.thrower_eye);
        // The RDG-M's flat throw lands about 30 m out at most: the point is the line's nearest-the-holder reachable one.
        assert!((20.0..32.0).contains(&out), "{plan:?} at {out} m");
        assert!(plan.covers_m > thin + PLAN_MARGIN_M && plan.at.y.abs() < 1e-3, "{plan:?}");
        // The cloud there hides the crossing from him.
        let cloud = Cloud {
            by: 0,
            team: 0,
            placed: Placed::Arc,
            centre: plan.at,
            thrown: MatchMs(0),
            pours: MatchMs(0),
            shape: SmokeGrenade::RDG_M.cloud,
        };
        assert!(Clouds::new(vec![cloud]).covers(5_000, &tuning, threat, crossing + Vec3::new(0.0, 1.3, 0.0)));
        // Nearer, the first point of the line (by the holder) is in reach: on it.
        let near = smoke_between(&reach, Vec3::new(0.0, 1.6, 20.0), crossing, thin, SmokeUse::AntiSniper).unwrap();
        // (Where its arc rests: the contact and the roll after it.)
        assert!(near.at.distance_xz(Vec3::new(0.0, 0.0, 20.0)) < 4.0, "{near:?}");
        // A line wholly out of reach: none.
        let far = (Vec3::new(0.0, 1.6, 90.0), Vec3::new(10.0, 0.0, 70.0));
        assert_eq!(smoke_between(&reach, far.0, far.1, thin, SmokeUse::CrossLane), None);
    }

    #[test]
    fn a_mid_line_smoke_leaves_a_near_shooter_outside_its_cloud_and_hides_the_crossings_stretch() {
        let mesh = flat();
        let reach = Reach {
            mesh: &mesh,
            grenade: SmokeGrenade::RDG_M,
            thrower_eye: Vec3::new(0.0, 1.6, 0.0),
        };
        let thin = SmokeTuning::default().thin_m.get();
        let rule = crate::v026g::bots::tuning::MidLine::default();
        // A holder 25 m out (within the throw's reach), the thrower's route crossing his lane 5 m out and running on east.
        let threat = Vec3::new(0.0, 1.6, 25.0);
        let crossing = Vec3::new(0.0, 0.0, 5.0);
        let stretch: Vec<Vec3> = (0..=12).map(|d| Vec3::new(d as f32, 1.3, 5.0)).collect();
        // The first-point rule puts it on his feet.
        let first = smoke_between(&reach, threat, crossing, thin, SmokeUse::CrossLane).unwrap();
        let first_landing = Landing::of(&reach, &first, &stretch, thin);
        assert!(first_landing.on_shooter, "{first:?} {first_landing:?}");
        // Mid-line: he is outside the cloud by the rule's margin, and it hides at least as much of the stretch.
        let mid = smoke_mid_line(&reach, threat, crossing, &stretch, thin, SmokeUse::CrossLane, rule).unwrap();
        let mid_landing = Landing::of(&reach, &mid, &stretch, thin);
        assert!(!mid_landing.on_shooter, "{mid:?} {mid_landing:?}");
        assert!(mid.at.distance_xz(threat) >= SmokeGrenade::RDG_M.cloud.radius_m + rule.shooter_clear_m.get(), "{mid:?}");
        assert!(mid_landing.hidden >= first_landing.hidden && mid_landing.hidden > 0, "{mid_landing:?} vs {first_landing:?}");
        // It covers the line, and a step aside along his cover (3 m east or west) still does not see the crossing.
        assert!(mid.covers_m > thin + PLAN_MARGIN_M, "{mid:?}");
        let cloud = Cloud {
            by: 0,
            team: 0,
            placed: Placed::Arc,
            centre: mid.at,
            thrown: MatchMs(0),
            pours: MatchMs(0),
            shape: SmokeGrenade::RDG_M.cloud,
        };
        let clouds = Clouds::new(vec![cloud]);
        let tuning = SmokeTuning::default();
        for side in [-3.0, 0.0, 3.0] {
            let eye = threat + Vec3::new(side, 0.0, 0.0);
            assert!(clouds.covers(5_000, &tuning, eye, crossing + Vec3::new(0.0, 1.3, 0.0)), "from {side}: {mid:?}");
        }
        // A line wholly out of reach: none, as the first-point rule.
        let far = (Vec3::new(0.0, 1.6, 90.0), Vec3::new(10.0, 0.0, 70.0));
        assert_eq!(smoke_mid_line(&reach, far.0, far.1, &[], thin, SmokeUse::CrossLane, rule), None);
        // Near the shooter: a few metres in front of him, never on his feet, and it still covers the line.
        let near_rule = crate::v026g::bots::tuning::NearShooter::default();
        let near = smoke_near_shooter(&reach, threat, crossing, &stretch, thin, SmokeUse::CrossLane, near_rule).unwrap();
        let off = near.at.distance_xz(threat);
        assert!((near_rule.min_m.get()..=near_rule.max_m.get() + 2.0).contains(&off), "{near:?} at {off} m");
        assert!(near.covers_m > thin + PLAN_MARGIN_M, "{near:?}");
        // The dispatcher picks each rule.
        use crate::v026g::bots::tuning::AimPoint;
        assert_eq!(smoke_aimed(&reach, threat, crossing, &stretch, thin, SmokeUse::CrossLane, None), Some(first));
        assert_eq!(smoke_aimed(&reach, threat, crossing, &stretch, thin, SmokeUse::CrossLane, Some(AimPoint::MidLine(rule))), Some(mid));
        assert_eq!(
            smoke_aimed(&reach, threat, crossing, &stretch, thin, SmokeUse::CrossLane, Some(AimPoint::NearShooter(near_rule))),
            Some(near)
        );
    }

    #[test]
    fn a_raised_shooters_line_passes_over_a_ground_cloud_in_front_of_him_and_best_cover_takes_one_that_covers_it() {
        // Ground at 0 and a roof at 12 m over z 21..30: the holder stands on the roof's edge, the crossing is on the ground.
        let (a, b) = (Vec3::new(-100.0, 0.0, -100.0), Vec3::new(100.0, 0.0, -100.0));
        let (c, d) = (Vec3::new(100.0, 0.0, 100.0), Vec3::new(-100.0, 0.0, 100.0));
        let (e, f) = (Vec3::new(-10.0, 12.0, 21.0), Vec3::new(10.0, 12.0, 21.0));
        let (g, h) = (Vec3::new(10.0, 12.0, 30.0), Vec3::new(-10.0, 12.0, 30.0));
        let mesh = RayTree::from_corners([[a, b, c], [a, c, d], [e, f, g], [e, g, h]]);
        let reach = Reach {
            mesh: &mesh,
            grenade: SmokeGrenade::RDG_M,
            thrower_eye: Vec3::new(0.0, 1.6, 0.0),
        };
        let thin = SmokeTuning::default().thin_m.get();
        let threat = Vec3::new(0.0, 13.6, 22.0);
        let crossing = Vec3::new(0.0, 0.0, 5.0);
        let chest = crossing + Vec3::new(0.0, 1.3, 0.0);
        let stretch: Vec<Vec3> = (0..=12).map(|d| Vec3::new(d as f32, 1.3, 5.0)).collect();
        assert_eq!(Height::of(threat, chest), Height::Above);
        // A cloud on the ground 6 m in front of him (under his line) puts no smoke on it to speak of.
        let under = Vec3::new(0.0, 0.0, 16.0);
        assert!(reach.smoke_on_line(under, threat, chest) <= thin + PLAN_MARGIN_M, "{}", reach.smoke_on_line(under, threat, chest));
        // Best cover: a landing whose cloud covers the line, and it hides at least as much of the stretch as the other rules.
        let best = smoke_best_cover(&reach, threat, crossing, &stretch, thin, SmokeUse::CrossLane, crate::v026g::bots::tuning::BestCover::default())
            .expect("a covering landing");
        assert!(best.covers_m > thin + PLAN_MARGIN_M, "{best:?}");
        let best_hidden = Landing::of(&reach, &best, &stretch, thin).hidden;
        let near = smoke_near_shooter(&reach, threat, crossing, &stretch, thin, SmokeUse::CrossLane, crate::v026g::bots::tuning::NearShooter::default());
        for plan in [smoke_between(&reach, threat, crossing, thin, SmokeUse::CrossLane), near].into_iter().flatten() {
            // Every rule only takes landings that cover the line in 3D.
            assert!(plan.covers_m > thin + PLAN_MARGIN_M, "{plan:?}");
            assert!(best_hidden >= Landing::of(&reach, &plan, &stretch, thin).hidden, "{best:?} vs {plan:?}");
        }
    }

    #[test]
    fn a_shooter_spot_smoke_rests_on_him_within_reach_and_not_beyond() {
        let mesh = flat();
        let reach = Reach {
            mesh: &mesh,
            grenade: SmokeGrenade::RDG_M,
            thrower_eye: Vec3::new(0.0, 1.6, 0.0),
        };
        let rule = crate::v026g::bots::tuning::ShooterSpot::default();
        let crossing = Vec3::new(0.0, 0.0, 5.0);
        let near = Vec3::new(0.0, 1.6, 20.0);
        let plan = smoke_on_shooter(&reach, near, crossing, SmokeUse::CrossLane, rule).expect("in reach");
        assert!(plan.at.distance_xz(near) <= rule.rest_within_m.get(), "{plan:?}");
        // His eye is inside the cloud.
        let cloud = Cloud { by: 0, team: 0, placed: Placed::Arc, centre: plan.at, thrown: MatchMs(0), pours: MatchMs(0), shape: SmokeGrenade::RDG_M.cloud };
        assert!(cloud.holds(near, MatchMs(5_000)), "{plan:?}");
        // Beyond the throw's reach: none.
        assert_eq!(smoke_on_shooter(&reach, Vec3::new(0.0, 1.6, 80.0), crossing, SmokeUse::CrossLane, rule), None);
    }

    #[test]
    fn a_smoke_on_a_spot_lands_on_its_floor_when_the_throw_reaches_it() {
        let mesh = flat();
        let reach = Reach {
            mesh: &mesh,
            grenade: SmokeGrenade::RDG_M,
            thrower_eye: Vec3::new(0.0, 1.6, 0.0),
        };
        let toward = Vec3::new(0.0, 1.6, 0.0);
        let plan = smoke_on(&reach, Vec3::new(5.0, 1.6, 22.0), toward, SmokeUse::Breach).unwrap();
        // Where its arc rests on the floor: the contact and the roll after it.
        assert!(plan.at.distance_xz(Vec3::new(5.0, 0.0, 22.0)) < 4.0 && plan.at.y.abs() < 1e-3, "{plan:?}");
        assert!(plan.covers_m > 2.0, "{plan:?}");
        assert_eq!(smoke_on(&reach, Vec3::new(5.0, 1.6, 45.0), toward, SmokeUse::Breach), None);
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

    /// The cost of what a bot knows of clouds at a think (`smoke.react_watch` on): the update (a ray on the map for
    /// each cloud not yet known, in view; none once known) and the edge to watch, with 1 and 4 live clouds. Two
    /// rows each: every cloud new (the worst, a ray each) and every cloud known (a later think).
    /// `cargo test -p match-server --release known_clouds_cost -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn known_clouds_cost() {
        let sight = RayTree::build(&[]);
        let t = SmokeTuning::default();
        let who = Watcher {
            me: 5,
            team: 0,
            eye: Vec3::new(0.0, 1.6, 0.0),
        };
        let at = [
            Vec3::new(0.0, 0.0, 30.0),
            Vec3::new(20.0, 0.0, 30.0),
            Vec3::new(-20.0, 0.0, 30.0),
            Vec3::new(0.0, 0.0, 60.0),
        ];
        const N: u32 = 200_000;
        for count in [1, 4] {
            let v = view(Vec3::ZERO, Vec3::new(0.0, 0.0, 90.0), &at[..count], 5_000);
            let veil = v.smoke.veil(v.clock, &t);
            for fresh in [true, false] {
                let mut known = KnownClouds::default();
                let start = std::time::Instant::now();
                let mut watches = 0u32;
                for i in 0..N {
                    if fresh || i == 0 {
                        known = KnownClouds::default();
                    }
                    known.update(who, &v.smoke, veil, v.clock, &t, |middle| {
                        crate::v026g::bots::rays::visible(&sight, who.eye, middle)
                    });
                    watches += u32::from(known.edge_to_watch(who, Vec3::ZERO, &v.smoke, v.clock + i * 10, &t).is_some());
                }
                let ns = start.elapsed().as_nanos() as f64 / f64::from(N);
                let row = if fresh { "all new" } else { "all known" };
                println!("known_clouds_cost: {count} clouds, {row}: {ns:.0} ns a think ({watches} watches)");
            }
        }
    }

    /// A plan counts every cloud live or forming, of either team, and its own team's in the air, each at its full size;
    /// not another team's in the air, nor one ended.
    #[test]
    fn a_plan_counts_clouds_forming_and_its_teams_in_the_air() {
        let cloud = |team: u8, pours: u32| Cloud {
            by: 0,
            team,
            placed: Placed::Arc,
            centre: Vec3::new(10.0, 0.0, 0.0),
            thrown: MatchMs(pours.saturating_sub(4_000)),
            pours: MatchMs(pours),
            shape: SmokeGrenade::RDG_M.cloud,
        };
        let t = SmokeTuning::default();
        let (eye, point) = (Vec3::new(0.0, 1.6, 0.0), Vec3::new(20.0, 1.3, 0.0));
        let covers = |c: Cloud, team: u8, clock: u32| Clouds::new(vec![c]).planning(clock, team, Forming::of(&t)).covers(clock, &t, eye, point);
        // In the air (pours later): its team's counts, the other's not; live clouds miss it.
        assert!(covers(cloud(0, 5_000), 0, 2_000), "own in the air");
        assert!(!covers(cloud(0, 5_000), 1, 2_000), "the other team's in the air");
        assert!(!Clouds::new(vec![cloud(0, 5_000)]).covers(2_000, &t, eye, point), "#801's test missed it");
        // Forming (poured 300 ms ago): either team's.
        assert!(covers(cloud(0, 5_000), 1, 5_300), "forming, the other team");
        // Ended: none.
        let ended = cloud(0, 5_000).ends().0 + 1;
        assert!(!covers(cloud(0, 5_000), 0, ended), "ended");
    }
}
