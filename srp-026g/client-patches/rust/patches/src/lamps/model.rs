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

use ::ragdoll::math::{Pose, Vec3};
use ::ragdoll::ragdoll::{Level, LoneBody, Motion};

/// The most loose pieces a project's fixtures have (a skeleton's cap, a
/// bottle's eight shards: School's three bottles have 24).
pub const MAX_PIECES: usize = 32;
// Each piece is a loose visual of the project's registry.
const _: () = assert!(MAX_PIECES <= crate::vostok::project::MAX_LOOSE);

/// What a loose piece is, and so how it flies off and on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PieceKind {
    /// A figure's headwear (a skeleton's cap): knocked off as a corpse's is.
    Headwear,
    /// A glass figure's shard (a bottle's), its place `outward` from the
    /// figure's middle: thrown out that way and along the push.
    Shard { outward: Vec3 },
}

impl PieceKind {
    /// Its body off its seat by a push along `push` (zero: dropped, a break
    /// no push of this client's made).
    fn off(self, seat: Pose, up: Vec3, push: Vec3) -> LoneBody {
        match self {
            PieceKind::Headwear => LoneBody::shot_off(seat, up, push),
            PieceKind::Shard { outward } => {
                let outward = if push == Vec3::ZERO { Vec3::ZERO } else { outward };
                LoneBody::shattered(seat, up, push, outward)
            }
        }
    }
    /// Its body struck again at `from` along `push`.
    fn kicked(self, from: Pose, up: Vec3, push: Vec3) -> LoneBody {
        match self {
            PieceKind::Headwear => LoneBody::kicked(from, up, push),
            PieceKind::Shard { .. } => LoneBody::shard_kicked(from, up, push),
        }
    }
    /// How near a shot or a blast's reach must come to its centre to knock it
    /// on: about its half length (a cap's 14 cm, a shard's 6 cm).
    pub fn reach(self) -> Distance {
        let metres = match self { PieceKind::Headwear => 0.14, PieceKind::Shard { .. } => 0.06 };
        Distance::parse(metres).expect("a finite, non-negative distance")
    }
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
