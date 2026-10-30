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

pub use crate::vostok::geometry::{Bounds, BulletSegment, Distance, RayDirection, WorldPoint};

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
    fn shards_spread_one_per_spacing_along_the_longest_extent() {
        let most = core::num::NonZeroUsize::new(4).unwrap();
        let spacing = Distance::parse(0.4).unwrap();
        let along = |high: [f32; 3]| {
            let b = Bounds::parse(point([0.0; 3]), point(high)).unwrap();
            b.along(spacing, most).map(WorldPoint::coordinates).collect::<std::vec::Vec<_>>()
        };
        assert_eq!(along([0.11, 0.34, 0.13]), [[0.055, 0.17, 0.065]]);   // a bulb: its centre
        assert_eq!(along([0.34, 0.1, 0.8]), [[0.17, 0.05, 0.2], [0.17, 0.05, 0.6]]);
        assert_eq!(along([0.81, 0.1, 0.34]).len(), 3);
        assert_eq!(along([0.34, 1.5, 0.1]).iter().map(|p| p[1]).collect::<std::vec::Vec<_>>(),
                   [0.1875, 0.5625, 0.9375, 1.3125]);   // a tube: four along y
        assert_eq!(along([3.0, 0.1, 0.1]).len(), 4);   // capped
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

}
