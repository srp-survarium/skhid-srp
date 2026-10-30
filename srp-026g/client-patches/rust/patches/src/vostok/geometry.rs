//! Finite world-space geometry at the engine boundary.
#![forbid(unsafe_code)]

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldPoint([f32; 3]);

impl WorldPoint {
    pub fn parse(value: [f32; 3]) -> Option<Self> {
        value.iter().all(|v| v.is_finite()).then_some(Self(value))
    }
    pub fn coordinates(self) -> [f32; 3] { self.0 }
    fn square_distance(self, other: Self) -> f32 {
        let mut square = 0.0;
        for k in 0..3 { square += (other.0[k] - self.0[k]) * (other.0[k] - self.0[k]); }
        square
    }
    /// Whether `other` is at most `distance` from here.
    pub fn within(self, other: Self, distance: Distance) -> bool {
        self.square_distance(other) <= distance.0 * distance.0
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

/// A fixture's glass in world space, where its break plays: a segment along
/// its tube or through its bulb, authored with the fixture's placement.
#[derive(Clone, Copy, Debug)]
pub struct GlassLine { from: WorldPoint, to: WorldPoint }

impl GlassLine {
    pub fn new(from: WorldPoint, to: WorldPoint) -> Self { Self { from, to } }
    pub fn middle(self) -> WorldPoint { self.at(0.5) }
    fn at(self, t: f32) -> WorldPoint {
        WorldPoint(core::array::from_fn(|k| self.from.0[k] + (self.to.0[k] - self.from.0[k]) * t))
    }
    /// Points spread along the line, one per `spacing` of it rounded (at
    /// least one, at most `most`), each in the middle of its share: the
    /// middle alone for a bulb.
    pub fn along(self, spacing: Distance, most: core::num::NonZeroUsize) -> impl Iterator<Item = WorldPoint> {
        let square = self.from.square_distance(self.to);
        let mut count = 1;
        // The length's share count rounded, without a square root:
        // one more while (count + 1/2) spacings still fit.
        while count < most.get() && {
            let fit = (count as f32 + 0.5) * spacing.0;
            fit * fit <= square
        } { count += 1; }
        (0..count).map(move |i| self.at((i as f32 + 0.5) / count as f32))
    }
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
