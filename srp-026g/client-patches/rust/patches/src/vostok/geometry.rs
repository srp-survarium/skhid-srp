//! Finite world-space geometry at the engine boundary.
#![forbid(unsafe_code)]

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldPoint([f32; 3]);

impl WorldPoint {
    pub fn parse(value: [f32; 3]) -> Option<Self> {
        value.iter().all(|v| v.is_finite()).then_some(Self(value))
    }
    pub fn coordinates(self) -> [f32; 3] { self.0 }
    /// Whether `other` is at most `distance` from here.
    pub fn within(self, other: Self, distance: Distance) -> bool {
        let mut square = 0.0;
        for k in 0..3 { square += (other.0[k] - self.0[k]) * (other.0[k] - self.0[k]); }
        square <= distance.0 * distance.0
    }
}

/// A finite native ray direction. The engine supplies its unit length;
/// keeping the native components preserves clipping and intersection arithmetic.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayDirection([f32; 3]);

impl RayDirection {
    pub fn parse(value: [f32; 3]) -> Option<Self> {
        (value.iter().all(|v| v.is_finite()) && value.iter().any(|v| *v != 0.0))
            .then_some(Self(value))
    }
    pub fn coordinates(self) -> [f32; 3] { self.0 }
    /// From `from` towards `to`; None when they are the same point.
    pub fn between(from: WorldPoint, to: WorldPoint) -> Option<Self> {
        Self::parse(core::array::from_fn(|k| to.0[k] - from.0[k]))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Distance(f32);

impl Distance {
    pub fn parse(value: f32) -> Option<Self> {
        (value.is_finite() && value >= 0.0).then_some(Self(value))
    }
}

/// Already clipped at the nearest native hit. All consumers see the same
/// segment; none can accidentally break a fixture through a wall or player.
#[derive(Clone, Copy, Debug)]
pub struct BulletSegment {
    origin: WorldPoint,
    direction: RayDirection,
    length: Distance,
}

impl BulletSegment {
    pub fn clipped(origin: WorldPoint, direction: RayDirection, length: Distance,
                   hit: Option<WorldPoint>) -> Option<Self> {
        if length.0 == 0.0 { return None; }
        let stop = match hit {
            None => length,
            Some(hit) => {
                let mut projection = 0.0;
                for k in 0..3 { projection += (hit.0[k] - origin.0[k]) * direction.0[k]; }
                Distance::parse(projection)?
            }
        };
        Some(Self { origin, direction, length: Distance(length.0.min(stop.0)) })
    }
    pub fn direction(self) -> RayDirection { self.direction }
}

#[derive(Clone, Copy, Debug)]
pub struct Bounds { low: WorldPoint, high: WorldPoint, centre: WorldPoint }

impl Bounds {
    pub fn parse(low: WorldPoint, high: WorldPoint) -> Option<Self> {
        if (0..3).any(|k| low.0[k] >= high.0[k]) { return None; }
        let centre = WorldPoint::parse(core::array::from_fn(|k| (low.0[k] + high.0[k]) * 0.5))?;
        Some(Self { low, high, centre })
    }
    pub fn centre(self) -> WorldPoint { self.centre }
    /// The point of these bounds nearest to `point` (`point` itself inside).
    pub fn nearest(self, point: WorldPoint) -> WorldPoint {
        WorldPoint(core::array::from_fn(|k| point.0[k].max(self.low.0[k]).min(self.high.0[k])))
    }
    /// Points spread along the bounds' longest extent through the centre,
    /// one per `spacing` of it (at least one, at most `most`), each in the
    /// middle of its share: the centre alone for a short fixture.
    pub fn along(self, spacing: Distance, most: core::num::NonZeroUsize) -> impl Iterator<Item = WorldPoint> {
        let extent: [f32; 3] = core::array::from_fn(|k| self.high.0[k] - self.low.0[k]);
        let axis = (0..3).fold(0, |longest, k| if extent[k] > extent[longest] { k } else { longest });
        let shares = if spacing.0 > 0.0 { extent[axis] / spacing.0 } else { 1.0 };
        let whole = shares as usize;
        let count = (whole + usize::from((whole as f32) < shares)).clamp(1, most.get());
        (0..count).map(move |i| {
            let mut point = self.centre.0;
            point[axis] = self.low.0[axis] + extent[axis] * (i as f32 + 0.5) / count as f32;
            WorldPoint(point)
        })
    }
    /// Whether `point` lies in these bounds, or within `tolerance` of them.
    pub fn holds(self, point: WorldPoint, tolerance: Distance) -> bool {
        (0..3).all(|k| point.0[k] >= self.low.0[k] - tolerance.0 && point.0[k] <= self.high.0[k] + tolerance.0)
    }
    pub fn intersects(self, ray: BulletSegment) -> bool {
        let (mut near, mut far) = (0.0_f32, ray.length.0);
        for k in 0..3 {
            let d = ray.direction.0[k];
            if d == 0.0 {
                if ray.origin.0[k] < self.low.0[k] || ray.origin.0[k] > self.high.0[k] { return false; }
            } else {
                let a = (self.low.0[k] - ray.origin.0[k]) / d;
                let b = (self.high.0[k] - ray.origin.0[k]) / d;
                near = near.max(a.min(b));
                far = far.min(a.max(b));
                if near > far { return false; }
            }
        }
        true
    }
}
