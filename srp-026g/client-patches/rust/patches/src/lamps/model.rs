//! Lamp simulation and presentation values. No game memory or native handles.
#![forbid(unsafe_code)]

use core::num::NonZeroU16;
use crate::vostok::parse::Reader;

pub const MAX_LAMPS: usize = 256;
const WORDS: usize = MAX_LAMPS / u32::BITS as usize;
const HISTORY_MAGIC: u32 = u32::from_le_bytes(*b"LMP1");

/// A fixture's position in the project's authored ordering, never a visual index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LampId(u16);

impl LampId {
    pub fn index(self) -> usize { usize::from(self.0) }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LampCount(NonZeroU16);

impl LampCount {
    pub fn parse(value: u32) -> Option<Self> {
        let n = NonZeroU16::new(u16::try_from(value).ok()?)?;
        (usize::from(n.get()) <= MAX_LAMPS).then_some(Self(n))
    }
    pub fn get(self) -> usize { usize::from(self.0.get()) }
    pub fn id(self, index: usize) -> Option<LampId> {
        (index < self.get()).then_some(LampId(index as u16))
    }
    fn words(self) -> usize { self.get().div_ceil(u32::BITS as usize) }
    pub fn history_bytes(self) -> usize { (2 + self.words()) * size_of::<u32>() }
}

pub use crate::vostok::geometry::{Bounds, BulletSegment, Distance, GlassLine, RayDirection, WorldPoint};

/// A breakable fixture: its bounds (its collision, what a bullet or a blast
/// must reach) and its glass (where its break plays).
#[derive(Clone, Copy, Debug)]
pub struct Fixture { pub bounds: Bounds, pub glass: GlassLine }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Condition { Intact, Broken }

#[derive(Clone, Copy, Debug)]
pub enum LampEvent { Hit, Restore(Condition) }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BreakEffect { None, Shatter }

impl Condition {
    pub fn parse(value: u32) -> Option<Self> {
        match value { 0 => Some(Self::Intact), 1 => Some(Self::Broken), _ => None }
    }
    pub fn transition(self, event: LampEvent) -> (Self, BreakEffect) {
        match (self, event) {
            (Self::Intact, LampEvent::Hit) => (Self::Broken, BreakEffect::Shatter),
            (Self::Broken, LampEvent::Hit) => (Self::Broken, BreakEffect::None),
            (_, LampEvent::Restore(state)) => (state, BreakEffect::None),
        }
    }
}

pub use crate::vostok::presentation::{Membership, PresentationEvent, RendererAction, Visibility};

use ::ragdoll::math::{Pose, Quat, Vec3};
use ::ragdoll::ragdoll::{Level, LoneBody, Motion};
pub use ::ragdoll::ragdoll::{ShardSize, WareShape};

/// The most loose pieces a project's fixtures have (a skeleton's cap, a
/// bottle's eight shards, a tin can; Vector's capsules and doors, 16 shards
/// a half, and its 50 jars, five a jar: 378). A piece is a pose (some 100
/// bytes); only those in the air have a body ([`Bodies`]).
pub const MAX_PIECES: usize = 384;
/// The most pieces in the air at once, each a body of its own (some 290
/// bytes): a capsule's 32 shards and a blast's jars' together. A piece
/// that would fly while every body flies waits where it is for one.
pub const MAX_FLYING: usize = 128;
// Each piece is a loose visual of the project's registry.
const _: () = assert!(MAX_PIECES <= crate::vostok::project::MAX_LOOSE);

/// What a loose piece is, and so how it flies off and on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PieceKind {
    /// A figure's headwear (a skeleton's cap): knocked off as a corpse's is.
    Headwear,
    /// A glass figure's shard (a bottle's, a jug's), its place `outward`
    /// from the figure's middle: thrown out that way and along the push.
    Shard { outward: Vec3 },
    /// A large pane's shard (Vector's capsule glass): it drops `out` of its
    /// frame (its glass's outward normal there), its body (a flat capsule
    /// along its X) turned from the visual by `frame` onto the shard's long
    /// side.
    PaneShard { out: Vec3, frame: Quat },
    /// A whole thing of the map (a tin can, a teapot): it jumps off along
    /// the push. Its body is `shape`, turned from the visual by `frame` (the
    /// capsule's axis, the body's X, along the visual's long axis).
    Ware { shape: WareShape, frame: Quat },
}

/// A pane shard's frame: its body's X along the shard's long side `along`,
/// its Z toward `out` (both in the visual's frame); none when either is no
/// direction or they are parallel.
pub fn pane_frame(along: Vec3, out: Vec3) -> Option<Quat> {
    let x = along.direction()?;
    let z = out.reject(x).direction()?;
    Some(Quat::from_basis(x, z.cross(x), z))
}

/// A ware's long axis in its visual's frame: the body's X there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WareAxis { X, Y, Z }

impl WareAxis {
    /// The turn from the body's frame to the visual's.
    pub fn frame(self) -> Quat {
        match self {
            WareAxis::X => Quat::IDENTITY,
            WareAxis::Y => Quat::from_axis_angle(Vec3::Z, core::f32::consts::FRAC_PI_2),
            WareAxis::Z => Quat::from_axis_angle(Vec3::Y, -core::f32::consts::FRAC_PI_2),
        }
    }
}

impl PieceKind {
    /// Its body off its seat by a push along `push` (zero: dropped, a break
    /// no push of this client's made).
    fn off(self, seat: Pose, up: Vec3, push: Vec3) -> LoneBody {
        let seat = self.body_of(seat);
        match self {
            PieceKind::Headwear => LoneBody::shot_off(seat, up, push),
            PieceKind::Shard { outward } => {
                let outward = if push == Vec3::ZERO { Vec3::ZERO } else { outward };
                LoneBody::shattered(seat, up, push, outward)
            }
            PieceKind::PaneShard { out, .. } => {
                let out = if push == Vec3::ZERO { Vec3::ZERO } else { out };
                LoneBody::pane_shattered(seat, up, push, out)
            }
            PieceKind::Ware { shape, .. } => LoneBody::ware_shot(seat, shape, up, push),
        }
    }
    /// Its body struck again at `from` (the body's pose) along `push`.
    fn kicked(self, from: Pose, up: Vec3, push: Vec3) -> LoneBody {
        match self {
            PieceKind::Headwear => LoneBody::kicked(from, up, push),
            PieceKind::Shard { .. } => LoneBody::shard_kicked(from, ShardSize::Bottle, up, push),
            PieceKind::PaneShard { .. } => LoneBody::shard_kicked(from, ShardSize::Pane, up, push),
            PieceKind::Ware { shape, .. } => LoneBody::ware_kicked(from, shape, up, push),
        }
    }
    /// How near a shot or a blast's reach must come to its centre to knock it
    /// on: about its half length (a cap's 14 cm, a bottle's shard's 6 cm, a
    /// pane's 25, a ware's capsule's).
    pub fn reach(self) -> Distance {
        let metres = match self {
            PieceKind::Headwear => 0.14,
            PieceKind::Shard { .. } => 0.06,
            PieceKind::PaneShard { .. } => 0.25,
            PieceKind::Ware { shape, .. } => shape.reach(),
        };
        Distance::parse(metres).expect("a finite, non-negative distance")
    }
    /// The body's pose of its visual at `visual` (the same centre).
    fn body_of(self, visual: Pose) -> Pose {
        match self {
            PieceKind::Ware { frame, .. } | PieceKind::PaneShard { frame, .. } => {
                Pose::new(visual.rotation * frame, visual.position)
            }
            PieceKind::Headwear | PieceKind::Shard { .. } => visual,
        }
    }
    /// Where its visual is drawn for its body at `body`.
    pub fn drawn(self, body: Pose) -> Pose {
        match self {
            PieceKind::Ware { frame, .. } | PieceKind::PaneShard { frame, .. } => {
                Pose::new(body.rotation * frame.conjugate(), body.position)
            }
            PieceKind::Headwear | PieceKind::Shard { .. } => body,
        }
    }
}

/// How often a bell is heard at most, s (over 0, to 10 minutes).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RingEvery(f32);

impl RingEvery {
    pub fn parse(seconds: f32) -> Option<RingEvery> {
        (seconds > 0.0 && seconds <= 600.0).then_some(RingEvery(seconds))
    }
}

/// A fixture that rings (School's wall bells): a shot or a blast that
/// reaches it rings it, at most once every `every`, at the next frame; it
/// never breaks, and the history knows nothing of it. Times are the
/// project's frames' real time, s, on each client by itself. The strike's
/// seed (made of the shot: the same in every game) picks the ring's take.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bell { every: RingEvery, ringing: Ringing }

#[derive(Clone, Copy, Debug, PartialEq)]
enum Ringing {
    Silent,
    /// Struck since the last frame, by a shot of `seed`: it rings at the next.
    Due { seed: u32 },
    /// It rang at the frame of `at`.
    Rang { at: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BellEvent {
    /// A shot or a blast of `seed` reached it, the last frame at `now`.
    Struck { now: f32, seed: u32 },
    /// A frame at `now`.
    Frame { now: f32 },
}

impl Bell {
    pub fn new(every: RingEvery) -> Bell { Bell { every, ringing: Ringing::Silent } }

    /// The bell after `event`, and the seed of the ring that plays at this
    /// frame. A strike within `every` of its last ring is not heard (nor a
    /// bullet resimulated after a rollback, nor a burst's next bullets); the
    /// first strike of a frame is the one heard.
    pub fn ring(self, event: BellEvent) -> (Bell, Option<u32>) {
        let ringing = match (self.ringing, event) {
            (Ringing::Rang { at }, BellEvent::Struck { now, .. }) if now - at < self.every.0 => return (self, None),
            (Ringing::Due { .. }, BellEvent::Struck { .. }) => self.ringing,
            (Ringing::Silent | Ringing::Rang { .. }, BellEvent::Struck { seed, .. }) => Ringing::Due { seed },
            (Ringing::Due { seed }, BellEvent::Frame { now }) => {
                return (Bell { ringing: Ringing::Rang { at: now }, ..self }, Some(seed));
            }
            (Ringing::Silent | Ringing::Rang { .. }, BellEvent::Frame { .. }) => self.ringing,
        };
        (Bell { ringing, ..self }, None)
    }
}

/// FNV-1a's offset basis and prime (as bytes: a literal this large reads as
/// a game address to the patches' layer check), and the golden ratio's
/// 32-bit fraction.
const FNV_BASIS: u32 = u32::from_le_bytes([0xC5, 0x9D, 0x1C, 0x81]);
const FNV_PRIME: u32 = u32::from_le_bytes([0x93, 0x01, 0x00, 0x01]);
const GOLDEN: u32 = u32::from_le_bytes([0xB9, 0x79, 0x37, 0x9E]);

/// A shot's seed, the same in every game that simulates it: its start (a
/// bullet's muzzle, a blast's centre) to the centimetre, mixed (FNV-1a).
pub fn shot_seed(start: [f32; 3]) -> u32 {
    let mut hash = FNV_BASIS;
    for value in start {
        let centimetres = value * 100.0;
        let nearest = (centimetres + if centimetres < 0.0 { -0.5 } else { 0.5 }) as i32;
        for byte in nearest.to_le_bytes() {
            hash = (hash ^ u32::from(byte)).wrapping_mul(FNV_PRIME);
        }
    }
    hash
}

/// The seed a bell `row` of a shot of `seed` rings with: each bell of one
/// blast its own take.
pub fn bell_seed(seed: u32, row: usize) -> u32 {
    (seed ^ (row as u32).wrapping_mul(GOLDEN)).wrapping_mul(FNV_PRIME)
}

/// A fixture's loose piece (a skeleton's cap): on its seat while the fixture
/// is intact, knocked off when it breaks, as a corpse's headwear is shot off
/// (`ragdoll`'s LoneBody), then lying where it came to rest, until a shot or a
/// blast knocks it on again. Drawn only, on each client by itself: the
/// history keeps the fixture's condition, never the piece's flight.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Flight {
    Seated,
    /// Broken by a push along `push` (a bullet's ray, a blast's), not stepped yet.
    Struck { push: RayDirection },
    /// Off its seat, struck again at `from` along `push`, not stepped yet.
    Kicked { from: Pose, push: RayDirection },
    Flying(LoneBody),
    Lying(Pose),
}

impl Flight {
    /// The fixture broke by a push along `push`: its piece goes off its seat that way.
    pub fn strike(self, push: RayDirection) -> Flight {
        Flight::Struck { push }
    }

    /// A shot or a blast reached the piece off its seat: it goes on along
    /// `push` from where it is. A seated one is the fixture's (a break
    /// strikes it), a struck one is already going.
    pub fn kick(self, push: RayDirection) -> Flight {
        match self {
            Flight::Flying(body) => Flight::Kicked { from: body.pose(), push },
            Flight::Lying(pose) => Flight::Kicked { from: pose, push },
            Flight::Seated | Flight::Struck { .. } | Flight::Kicked { .. } => self,
        }
    }

    /// One frame of `dt` s for the piece of a fixture in `condition`, seated
    /// at `seat`: an intact fixture's sits there; a broken one's flies off it
    /// (dropped, when the break came from the history and no push of this
    /// client's) and then lies where it rested.
    pub fn step(self, kind: PieceKind, condition: Condition, seat: Pose, dt: f32, up: Vec3,
                level: &mut impl Level) -> Flight {
        let flying = match (condition, self) {
            (Condition::Intact, _) => return Flight::Seated,
            (Condition::Broken, Flight::Lying(pose)) => return Flight::Lying(pose),
            (Condition::Broken, Flight::Seated) => kind.off(seat, up, Vec3::ZERO),
            (Condition::Broken, Flight::Struck { push }) => kind.off(seat, up, vector(push)),
            (Condition::Broken, Flight::Kicked { from, push }) => kind.kicked(from, up, vector(push)),
            (Condition::Broken, Flight::Flying(body)) => body,
        };
        let mut body = flying;
        match body.step(dt, up, level) {
            Motion::Moving => Flight::Flying(body),
            Motion::Settled => Flight::Lying(body.pose()),
        }
    }

    /// Where the piece is drawn, and where a shot reaches it: None, on its seat.
    pub fn pose(&self) -> Option<Pose> {
        match self {
            Flight::Seated | Flight::Struck { .. } | Flight::Kicked { .. } => None,
            Flight::Flying(body) => Some(body.pose()),
            Flight::Lying(pose) => Some(*pose),
        }
    }
}

/// Where a loose piece is, as a project keeps it: a [`Flight`] whose body,
/// while it flies, is one of the project's [`Bodies`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Perch {
    Seated,
    Struck { push: RayDirection },
    Kicked { from: Pose, push: RayDirection },
    Flying(Slot),
    Lying(Pose),
}

/// A body of [`Bodies`]: only `Bodies` makes one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slot(u8);
const _: () = assert!(MAX_FLYING <= u8::MAX as usize + 1);

/// The bodies of a project's pieces in the air. Each [`Perch::Flying`] owns
/// its slot: a transition that ends its flight frees it, one that starts a
/// flight takes a free one (none free: the piece stays as it was, its
/// flight waiting a frame).
pub struct Bodies { slots: [Option<LoneBody>; MAX_FLYING] }

impl Default for Bodies {
    fn default() -> Self { Self::new() }
}

impl Bodies {
    pub const fn new() -> Bodies { Bodies { slots: [None; MAX_FLYING] } }

    /// The piece's flight (a flying one with its body).
    pub fn flight(&self, perch: Perch) -> Flight {
        match perch {
            Perch::Seated => Flight::Seated,
            Perch::Struck { push } => Flight::Struck { push },
            Perch::Kicked { from, push } => Flight::Kicked { from, push },
            Perch::Lying(pose) => Flight::Lying(pose),
            // a slot is always filled while a perch holds it (`keep`); a
            // lost one drops the piece from its seat again
            Perch::Flying(Slot(slot)) => self.slots[usize::from(slot)].map_or(Flight::Seated, Flight::Flying),
        }
    }

    /// `before`'s piece is now `after`: its slot kept for its body, freed
    /// when it lands, a free one taken when it takes off.
    pub fn keep(&mut self, before: Perch, after: Flight) -> Perch {
        let held = match before { Perch::Flying(slot) => Some(slot), _ => None };
        match after {
            Flight::Flying(body) => {
                let Some(Slot(slot)) = held.or_else(|| self.free()) else { return before };
                self.slots[usize::from(slot)] = Some(body);
                Perch::Flying(Slot(slot))
            }
            settled => {
                if let Some(Slot(slot)) = held { self.slots[usize::from(slot)] = None; }
                match settled {
                    Flight::Seated => Perch::Seated,
                    Flight::Struck { push } => Perch::Struck { push },
                    Flight::Kicked { from, push } => Perch::Kicked { from, push },
                    Flight::Lying(pose) => Perch::Lying(pose),
                    Flight::Flying(_) => unreachable!("matched above"),
                }
            }
        }
    }

    fn free(&self) -> Option<Slot> {
        self.slots.iter().position(Option::is_none).map(|slot| Slot(slot as u8))
    }

    /// The fixture broke by a push along `push`.
    pub fn strike(&mut self, perch: Perch, push: RayDirection) -> Perch {
        let after = self.flight(perch).strike(push);
        self.keep(perch, after)
    }
    /// A shot or a blast reached the piece off its seat.
    pub fn kick(&mut self, perch: Perch, push: RayDirection) -> Perch {
        let after = self.flight(perch).kick(push);
        self.keep(perch, after)
    }
    /// One frame of the piece ([`Flight::step`]).
    #[allow(clippy::too_many_arguments)]
    pub fn step(&mut self, perch: Perch, kind: PieceKind, condition: Condition, seat: Pose, dt: f32, up: Vec3,
                level: &mut impl Level) -> Perch {
        let after = self.flight(perch).step(kind, condition, seat, dt, up, level);
        self.keep(perch, after)
    }
    /// Where the piece is drawn ([`Flight::pose`]).
    pub fn pose(&self, perch: Perch) -> Option<Pose> {
        self.flight(perch).pose()
    }
}

fn vector(direction: RayDirection) -> Vec3 {
    let [x, y, z] = direction.coordinates();
    Vec3::new(x, y, z)
}

/// Parsed history, including its fixture count. No state changes until all
/// words have been read and the count matches the bound project.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot { count: LampCount, bits: [u32; WORDS] }

impl Snapshot {
    pub fn intact(count: LampCount) -> Self { Self { count, bits: [0; WORDS] } }
    pub fn parse(reader: &mut Reader<'_>, expected: LampCount) -> Option<Self> {
        let mut staged = reader.clone();
        if staged.u32()? != HISTORY_MAGIC || LampCount::parse(staged.u32()?)? != expected { return None; }
        let mut snapshot = Self::intact(expected);
        for word in &mut snapshot.bits[..expected.words()] { *word = staged.u32()?; }
        // Bits beyond the fixture count are not state. The native C
        // reader ignored them and subsequent writes emitted zero padding.
        let used = expected.get() % u32::BITS as usize;
        if used != 0 { snapshot.bits[expected.words() - 1] &= (1u32 << used) - 1; }
        *reader = staged;
        Some(snapshot)
    }
    pub fn condition(&self, id: LampId) -> Condition {
        assert!(id.index() < self.count.get(), "fixture belongs to this snapshot");
        if self.bits[id.index() / 32] & (1 << (id.index() % 32)) == 0 { Condition::Intact } else { Condition::Broken }
    }
    pub fn transition(&mut self, id: LampId, event: LampEvent) -> BreakEffect {
        let (next, effect) = self.condition(id).transition(event);
        let mask = 1 << (id.index() % 32);
        let word = &mut self.bits[id.index() / 32];
        match next { Condition::Intact => *word &= !mask, Condition::Broken => *word |= mask }
        effect
    }
    pub fn restore(&mut self, restored: Self) -> Option<()> {
        if self.count != restored.count { return None; }
        for i in 0..self.count.get() {
            let id = self.count.id(i).expect("fixture belongs to its count");
            self.transition(id, LampEvent::Restore(restored.condition(id)));
        }
        Some(())
    }
    pub fn words(&self) -> impl Iterator<Item = u32> + '_ {
        [HISTORY_MAGIC, self.count.get() as u32].into_iter().chain(self.bits[..self.count.words()].iter().copied())
    }
    pub fn count(&self) -> LampCount { self.count }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn point(v: [f32; 3]) -> WorldPoint { WorldPoint::parse(v).unwrap() }
    fn ray(start: [f32; 3], direction: [f32; 3], length: f32, hit: Option<[f32; 3]>) -> BulletSegment {
        BulletSegment::clipped(point(start), RayDirection::parse(direction).unwrap(), Distance::parse(length).unwrap(), hit.map(point)).unwrap()
    }
    #[test]
    fn segment_edges_and_native_occlusion() {
        let b = Bounds::parse(point([0.0; 3]), point([1.0; 3])).unwrap();
        assert!(b.intersects(ray([-1.0, 0.5, 0.5], [1.0, 0.0, 0.0], 1.0, None)));
        assert!(!b.intersects(ray([-1.0, 0.5, 0.5], [1.0, 0.0, 0.0], 3.0, Some([-0.1, 0.5, 0.5]))));
        assert!(b.intersects(ray([2.0, 0.5, 0.5], [-1.0, 0.0, 0.0], 3.0, None)));
        assert!(b.intersects(ray([0.5; 3], [0.0, 1.0, 0.0], 0.1, None)));
        assert!(!b.intersects(ray([2.0, 0.5, 0.5], [0.0, 1.0, 0.0], 3.0, None)));
        assert!(WorldPoint::parse([f32::NAN, 0.0, 0.0]).is_none());
        assert!(Distance::parse(f32::INFINITY).is_none());
        assert!(Distance::parse(-1.0).is_none());
        assert!(Bounds::parse(point([1.0; 3]), point([0.0; 3])).is_none());
    }
    #[test]
    fn a_segment_passes_near_a_point_only_along_its_own_length() {
        let reach = Distance::parse(0.14).unwrap();
        let shot = ray([0.0; 3], [1.0, 0.0, 0.0], 5.0, None);
        assert!(shot.passes_within(point([2.0, 0.1, 0.0]), reach));
        assert!(!shot.passes_within(point([2.0, 0.2, 0.0]), reach));
        assert!(!shot.passes_within(point([-0.5, 0.0, 0.0]), reach), "behind the muzzle");
        assert!(shot.passes_within(point([-0.1, 0.0, 0.0]), reach));
        // a wall at 1 m stops it before a piece at 2
        assert!(!ray([0.0; 3], [1.0, 0.0, 0.0], 5.0, Some([1.0, 0.0, 0.0])).passes_within(point([2.0, 0.0, 0.0]), reach));
    }
    #[test]
    fn marks_on_the_bounds_and_their_margin_only() {
        let b = Bounds::parse(point([0.0; 3]), point([1.0; 3])).unwrap();
        let margin = Distance::parse(0.01).unwrap();
        for inside in [[0.5; 3], [0.0, 0.5, 1.0], [-0.009, 0.5, 0.5], [0.5, 1.009, 0.5]] {
            assert!(b.holds(point(inside), margin), "{inside:?}");
        }
        for outside in [[-0.02, 0.5, 0.5], [0.5, 0.5, 1.02], [2.0, 2.0, 2.0]] {
            assert!(!b.holds(point(outside), margin), "{outside:?}");
        }
        assert!(!b.holds(point([-0.009, 0.5, 0.5]), Distance::parse(0.0).unwrap()));
    }
    #[test]
    fn shards_spread_about_one_per_spacing_along_the_glass() {
        let most = core::num::NonZeroUsize::new(10).unwrap();
        let spacing = Distance::parse(0.15).unwrap();
        let along = |from: [f32; 3], to: [f32; 3]| {
            GlassLine::new(point(from), point(to)).along(spacing, most).map(WorldPoint::coordinates)
                .collect::<std::vec::Vec<_>>()
        };
        assert_eq!(along([0.0, 0.2, 0.0], [0.0, 0.0, 0.0]), [[0.0, 0.1, 0.0]]);   // a bulb: its middle
        assert_eq!(along([0.0; 3], [0.0; 3]), [[0.0; 3]]);
        assert_eq!(along([0.0; 3], [0.25, 0.0, 0.0]), [[0.0625, 0.0, 0.0], [0.1875, 0.0, 0.0]]);   // 1.7 shares: two
        assert_eq!(along([0.0; 3], [0.2, 0.0, 0.0]).len(), 1);   // 1.3 shares: one
        // A turned tube: on its own line, not a world axis of its box.
        let turned = along([0.0; 3], [0.6, 0.0, 0.6]);
        assert_eq!(turned.len(), 6);
        assert!(turned.iter().all(|p| (p[0] - p[2]).abs() < 1e-6 && p[1] == 0.0));
        assert_eq!(along([0.0; 3], [3.0, 0.0, 0.0]).len(), 10);   // capped
    }
    #[test]
    fn blasts_measure_to_the_bounds_nearest_point() {
        let b = Bounds::parse(point([0.0; 3]), point([1.0; 3])).unwrap();
        assert_eq!(b.nearest(point([0.5, 0.2, 0.9])), point([0.5, 0.2, 0.9]));
        assert_eq!(b.nearest(point([3.0, 0.5, -2.0])), point([1.0, 0.5, 0.0]));
        let radius = Distance::parse(2.0).unwrap();
        assert!(point([3.0, 0.5, 0.5]).within(b.nearest(point([3.0, 0.5, 0.5])), radius));
        assert!(!point([3.1, 0.5, 0.5]).within(b.nearest(point([3.1, 0.5, 0.5])), radius));
        assert_eq!(RayDirection::between(point([0.0; 3]), point([0.0; 3])), None);
        assert!(RayDirection::between(point([0.0; 3]), point([0.0, 1.0, 0.0])).is_some());
    }
    #[test]
    fn a_break_frame_never_lies_along_its_ray() {
        // The game's effect basis is normalize(normal x direction): a shard
        // burst's two axes may never be parallel, whichever way it flies.
        for ray in [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, -1.0], [0.6, 0.8, 0.0],
                    [-0.3, 0.9, 0.3], [1e-3, 1.0, -1e-3], [0.577, 0.577, 0.577]] {
            let d = RayDirection::parse(ray).unwrap();
            let p = d.perpendicular().coordinates();
            let dot: f32 = (0..3).map(|k| p[k] * ray[k]).sum();
            let (square, ray_square) = (p.iter().map(|v| v * v).sum::<f32>(), ray.iter().map(|v| v * v).sum::<f32>());
            assert!(dot.abs() <= 1e-6 && square >= ray_square * 2.0 / 3.0 - 1e-6, "{ray:?} -> {p:?}");
            assert_eq!(d.reversed().coordinates(), ray.map(|v| -v));
        }
    }
    #[test]
    fn break_effects_only_follow_first_hit_and_never_history() {
        for initial in [Condition::Intact, Condition::Broken] {
            for restored in [Condition::Intact, Condition::Broken] {
                assert_eq!(initial.transition(LampEvent::Restore(restored)), (restored, BreakEffect::None));
            }
        }
        let (broken, effect) = Condition::Intact.transition(LampEvent::Hit);
        assert_eq!(effect, BreakEffect::Shatter);
        assert_eq!(broken.transition(LampEvent::Hit), (broken, BreakEffect::None));
    }
    #[test]
    fn presentation_is_total_idempotent_and_never_inserts_a_detached_view_on_frame() {
        for state in [Membership::Detached, Membership::Hidden, Membership::Shown] {
            for event in [PresentationEvent::Remove, PresentationEvent::Insert(Visibility::Visible),
                          PresentationEvent::Insert(Visibility::Hidden), PresentationEvent::Present(Visibility::Visible),
                          PresentationEvent::Present(Visibility::Hidden)] {
                let (next, action) = state.transition(event);
                assert_eq!(action == RendererAction::Add, state != Membership::Shown && next == Membership::Shown);
                assert_eq!(action == RendererAction::Remove, state == Membership::Shown && next != Membership::Shown);
                assert_eq!(next.transition(event), (next, RendererAction::Keep));
            }
        }
        assert_eq!(Membership::Detached.transition(PresentationEvent::Present(Visibility::Visible)),
                   (Membership::Detached, RendererAction::Keep));
    }
    #[test]
    fn history_roundtrips_every_capacity_and_rejects_truncation_without_consuming() {
        assert!(LampCount::parse(0).is_none());
        assert!(LampCount::parse(257).is_none());
        for n in 1..=256 {
            let count = LampCount::parse(n).unwrap();
            let mut state = Snapshot::intact(count);
            for i in (0..count.get()).step_by(3) { state.transition(count.id(i).unwrap(), LampEvent::Hit); }
            let bytes: std::vec::Vec<_> = state.words().flat_map(u32::to_le_bytes).collect();
            assert_eq!(bytes.len(), count.history_bytes());
            assert_eq!(Snapshot::parse(&mut Reader::new(&bytes), count), Some(state));
            for len in 0..bytes.len() {
                let mut r = Reader::new(&bytes[..len]);
                assert!(Snapshot::parse(&mut r, count).is_none());
                assert_eq!(r.remaining(), &bytes[..len]);
            }
            let wrong = LampCount::parse(if n == 256 { 1 } else { n + 1 }).unwrap();
            let mut r = Reader::new(&bytes);
            assert!(Snapshot::parse(&mut r, wrong).is_none());
            assert_eq!(r.remaining(), bytes);
        }
    }
    #[test]
    fn history_ignores_padding_and_rejects_a_different_project_count_atomically() {
        let count = LampCount::parse(33).unwrap();
        let bytes: std::vec::Vec<_> = [HISTORY_MAGIC, 33, u32::MAX, u32::MAX].into_iter()
            .flat_map(u32::to_le_bytes).collect();
        let parsed = Snapshot::parse(&mut Reader::new(&bytes), count).unwrap();
        assert_eq!(parsed.words().collect::<std::vec::Vec<_>>(), [HISTORY_MAGIC, 33, u32::MAX, 1]);
        let mut state = Snapshot::intact(count);
        state.restore(parsed.clone()).unwrap();
        assert_eq!(state, parsed);
        let before = state.clone();
        assert!(state.restore(Snapshot::intact(LampCount::parse(1).unwrap())).is_none());
        assert_eq!(state, before);
        state.restore(Snapshot::intact(count)).unwrap();
        assert_eq!(state, Snapshot::intact(count));
    }

    /// A floor at y 0.
    struct Floor;
    impl Level for Floor {
        fn sweep(&mut self, from: Vec3, to: Vec3, radius: f32) -> ::ragdoll::ragdoll::Sweep {
            let (a, b) = (from.y - radius, to.y - radius);
            if a > 0.0 && b >= 0.0 { return ::ragdoll::ragdoll::Sweep::Clear; }
            let t = if a <= 0.0 { 0.0 } else { a / (a - b) };
            let p = from.lerp(to, t);
            ::ragdoll::ragdoll::Sweep::Hit { point: Vec3::new(p.x, 0.0, p.z), normal: Vec3::Y }
        }
    }
    fn fly(mut flight: Flight, condition: Condition, seat: Pose) -> Flight {
        for _ in 0..600 {
            flight = flight.step(PieceKind::Headwear, condition, seat, 1.0 / 60.0, Vec3::Y, &mut Floor);
            if !matches!(flight, Flight::Flying(_)) { break; }
        }
        flight
    }
    #[test]
    fn a_piece_flies_off_along_its_push_and_lies_where_it_rests() {
        let seat = Pose::new(::ragdoll::math::Quat::IDENTITY, Vec3::new(0.0, 0.8, 0.0));
        assert_eq!(Flight::Seated.step(PieceKind::Headwear, Condition::Intact, seat, 1.0 / 60.0, Vec3::Y, &mut Floor), Flight::Seated);
        assert_eq!(Flight::Seated.pose(), None);
        let struck = Flight::Seated.strike(RayDirection::parse([1.0, -0.5, 0.0]).unwrap());
        assert_eq!(struck.pose(), None, "drawn on its seat until stepped");
        let first = struck.step(PieceKind::Headwear, Condition::Broken, seat, 1.0 / 60.0, Vec3::Y, &mut Floor);
        assert!(matches!(first, Flight::Flying(_)), "{first:?}");
        let Flight::Lying(rest) = fly(first, Condition::Broken, seat) else { panic!("it never rested") };
        assert!(rest.position.x > 0.1 && rest.position.z.abs() < 0.05, "along the push: {:?}", rest.position);
        assert!(rest.position.y < 0.15, "on the floor: {:?}", rest.position);
        assert_eq!(Flight::Lying(rest).step(PieceKind::Headwear, Condition::Broken, seat, 1.0 / 60.0, Vec3::Y, &mut Floor), Flight::Lying(rest));
        // a rollback to intact seats it again; the history's break drops it
        assert_eq!(Flight::Lying(rest).step(PieceKind::Headwear, Condition::Intact, seat, 1.0 / 60.0, Vec3::Y, &mut Floor), Flight::Seated);
        let Flight::Lying(dropped) = fly(Flight::Seated, Condition::Broken, seat) else { panic!("it never rested") };
        assert!(dropped.position.x.abs() < 0.01 && dropped.position.z.abs() < 0.01, "{:?}", dropped.position);
    }
    #[test]
    fn a_bell_rings_at_the_frame_after_its_strike_and_not_again_within_its_interval() {
        let bell = Bell::new(RingEvery::parse(4.0).unwrap());
        assert_eq!(bell.ring(BellEvent::Frame { now: 1.0 }), (bell, None), "silent until struck");
        let (struck, now) = bell.ring(BellEvent::Struck { now: 1.0, seed: 7 });
        assert_eq!(now, None, "a strike plays at the next frame, on the game's thread");
        let (again, _) = struck.ring(BellEvent::Struck { now: 1.0, seed: 8 });
        assert_eq!(again, struck, "a burst's bullets in one frame: one ring, the first's");
        let (rang, play) = struck.ring(BellEvent::Frame { now: 1.02 });
        assert_eq!(play, Some(7));
        assert_eq!(rang.ring(BellEvent::Frame { now: 1.04 }), (rang, None), "once");
        assert_eq!(rang.ring(BellEvent::Struck { now: 4.9, seed: 9 }), (rang, None), "within 4 s of its ring");
        let (late, _) = rang.ring(BellEvent::Struck { now: 5.03, seed: 9 });
        assert_eq!(late.ring(BellEvent::Frame { now: 5.05 }).1, Some(9), "past it, again");
        // every game's seed of one shot is the same; another shot's another
        assert_eq!(shot_seed([12.341, 19.4, -3.0]), shot_seed([12.3412, 19.4001, -3.0]));
        assert_ne!(shot_seed([12.345, 19.4, -3.0]), shot_seed([12.4, 19.4, -3.0]));
        let takes: std::collections::BTreeSet<_> = (0..40).map(|k| bell_seed(shot_seed([k as f32, 0.0, 0.0]), 2) % 10).collect();
        assert!(takes.len() >= 8, "the takes all come up: {takes:?}");
        assert_ne!(bell_seed(5, 0) % 10, bell_seed(5, 1) % 10, "two bells of one blast");
        assert!(RingEvery::parse(0.0).is_none() && RingEvery::parse(f32::NAN).is_none() && RingEvery::parse(601.0).is_none());
    }
    #[test]
    fn a_piece_flies_in_a_body_of_the_pool_and_waits_for_one_when_all_fly() {
        let seat = Pose::new(Quat::IDENTITY, Vec3::new(0.0, 1.0, 0.0));
        let push = RayDirection::parse([1.0, 0.0, 0.0]).unwrap();
        let mut bodies = Bodies::new();
        let step = |bodies: &mut Bodies, perch| {
            bodies.step(perch, PieceKind::Headwear, Condition::Broken, seat, 1.0 / 60.0, Vec3::Y, &mut Floor)
        };
        // every body taken
        let mut flying = [Perch::Seated; MAX_FLYING];
        for perch in flying.iter_mut() {
            let struck = bodies.strike(Perch::Seated, push);
            *perch = step(&mut bodies, struck);
            assert!(matches!(perch, Perch::Flying(_)), "{perch:?}");
        }
        // one more waits, struck, drawn on its seat
        let struck = bodies.strike(Perch::Seated, push);
        assert_eq!(step(&mut bodies, struck), struck);
        assert_eq!(bodies.pose(struck), None);
        // one lands: its body is free, the waiting piece takes off
        let mut first = flying[0];
        for _ in 0..900 {
            first = step(&mut bodies, first);
            if !matches!(first, Perch::Flying(_)) { break; }
        }
        assert!(matches!(first, Perch::Lying(_)), "{first:?}");
        assert!(matches!(step(&mut bodies, struck), Perch::Flying(_)));
        // a rollback seats a flying piece and frees its body
        let seated = bodies.step(flying[1], PieceKind::Headwear, Condition::Intact, seat, 1.0 / 60.0, Vec3::Y, &mut Floor);
        assert_eq!(seated, Perch::Seated);
        let again = bodies.strike(Perch::Seated, push);
        assert!(matches!(step(&mut bodies, again), Perch::Flying(_)));
    }
    #[test]
    fn a_pane_shard_lies_along_its_glass_and_drops_out_of_its_frame() {
        // a capsule's shard standing 1.5 m up, its long side the visual's Y,
        // its glass facing the visual's +Z; the capsule turned a quarter
        // about up, so out is the world's +X
        let frame = pane_frame(Vec3::new(0.0, 2.0, 0.0), Vec3::new(0.0, 0.3, 1.0)).unwrap();
        assert!((frame.rotate(Vec3::X) - Vec3::Y).length() < 1e-6 && (frame.rotate(Vec3::Z) - Vec3::Z).length() < 1e-6);
        assert!(pane_frame(Vec3::ZERO, Vec3::Z).is_none() && pane_frame(Vec3::Z, Vec3::Z * 2.0).is_none());
        let turn = Quat::from_axis_angle(Vec3::Y, core::f32::consts::FRAC_PI_2);
        let seat = Pose::new(turn, Vec3::new(0.0, 1.5, 0.0));
        let kind = PieceKind::PaneShard { out: turn.rotate(Vec3::Z), frame };
        assert!((kind.body_of(seat).rotation.rotate(Vec3::X) - Vec3::Y).length() < 1e-6, "the capsule along the glass");
        // shot from outside, toward -X: it falls out, toward +X
        let mut flight = Flight::Seated.strike(RayDirection::parse([-1.0, 0.0, 0.0]).unwrap());
        for _ in 0..900 {
            flight = flight.step(kind, Condition::Broken, seat, 1.0 / 60.0, Vec3::Y, &mut Floor);
            if !matches!(flight, Flight::Flying(_)) { break; }
        }
        let Flight::Lying(rest) = flight else { panic!("it never rested: {flight:?}") };
        assert!(rest.position.x > 0.2 && rest.position.y < 0.05, "{:?}", rest.position);
    }
    #[test]
    fn a_ware_flies_as_its_body_and_is_drawn_upright_from_it() {
        // a tin can on a table 0.8 m up, its long axis the visual's Y
        let kind = PieceKind::Ware { shape: WareShape::new(0.06, 0.1).unwrap(), frame: WareAxis::Y.frame() };
        let seat = Pose::new(Quat::IDENTITY, Vec3::new(0.0, 0.96, 0.0));
        assert!((kind.body_of(seat).rotation.rotate(Vec3::X) - Vec3::Y).length() < 1e-6, "the capsule along its height");
        let drawn = kind.drawn(kind.body_of(seat));
        assert!((drawn.rotation.rotate(Vec3::Y) - Vec3::Y).length() < 1e-6 && drawn.position == seat.position);
        assert!((WareAxis::Z.frame().rotate(Vec3::X) - Vec3::Z).length() < 1e-6);
        let mut flight = Flight::Seated.strike(RayDirection::parse([0.0, 0.0, 1.0]).unwrap());
        for _ in 0..900 {
            flight = flight.step(kind, Condition::Broken, seat, 1.0 / 60.0, Vec3::Y, &mut Floor);
            if !matches!(flight, Flight::Flying(_)) { break; }
        }
        let Flight::Lying(rest) = flight else { panic!("it never rested: {flight:?}") };
        assert!(rest.position.z > 0.8 && (rest.position.y - 0.1).abs() < 0.03, "{:?}", rest.position);
        assert!((kind.drawn(rest).rotation.rotate(Vec3::Y).y).abs() < 0.2, "lying on its side: {:?}", kind.drawn(rest));
        assert_eq!(kind.reach(), Distance::parse(0.16).unwrap());
    }
    #[test]
    fn a_lying_piece_is_knocked_on_from_where_it_lies() {
        let seat = Pose::new(::ragdoll::math::Quat::IDENTITY, Vec3::new(0.0, 0.8, 0.0));
        let Flight::Lying(rest) = fly(Flight::Seated, Condition::Broken, seat) else { panic!("it never rested") };
        let push = RayDirection::parse([0.0, -0.3, 1.0]).unwrap();
        assert_eq!(Flight::Seated.kick(push), Flight::Seated, "a seated piece is its fixture's");
        let kicked = Flight::Lying(rest).kick(push);
        assert_eq!(kicked, Flight::Kicked { from: rest, push });
        assert_eq!(kicked.pose(), None, "stepped before it is drawn: the present steps, then places");
        assert_eq!(kicked.kick(RayDirection::parse([1.0, 0.0, 0.0]).unwrap()), kicked, "one push a frame");
        let Flight::Lying(again) = fly(kicked, Condition::Broken, seat) else { panic!("it never rested") };
        assert!(again.position.z > rest.position.z + 0.5 && (again.position.x - rest.position.x).abs() < 0.15,
                "on along the push: {:?} from {:?}", again.position, rest.position);
        // a flying one goes on from where it is
        let flying = Flight::Seated.strike(push).step(PieceKind::Headwear, Condition::Broken, seat, 1.0 / 60.0, Vec3::Y, &mut Floor);
        let Flight::Flying(body) = flying else { panic!("{flying:?}") };
        assert_eq!(flying.kick(push), Flight::Kicked { from: body.pose(), push });
    }

}
