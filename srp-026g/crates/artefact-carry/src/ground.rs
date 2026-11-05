//! A dropped artefact falls vertically to a simulation ray's static surface.
//! The immutable release state is enough to reproduce its motion after rollback.
use crate::carry::Time;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drop { origin: [f32; 3], floor: f32, released: Time }
impl Drop {
    pub fn new(origin: [f32; 3], floor: f32, released: Time) -> Option<Self> {
        (origin.iter().all(|x| x.is_finite()) && floor.is_finite() && floor <= origin[1])
            .then_some(Self { origin, floor, released })
    }
    pub fn position(self, now: Time) -> [f32; 3] {
        let elapsed = now.0.wrapping_sub(self.released.0);
        let seconds = if elapsed > i32::MAX as u32 { 0.0 } else { elapsed.min(60_000) as f32 * 0.001 };
        [self.origin[0], (self.origin[1] - 4.905 * seconds * seconds).max(self.floor), self.origin[2]]
    }
    pub fn snapshot(self) -> [u8; 20] {
        let mut bytes = [0; 20];
        for (n, value) in self.origin.into_iter().chain([self.floor]).enumerate() {
            bytes[n*4..n*4+4].copy_from_slice(&value.to_le_bytes());
        }
        bytes[16..20].copy_from_slice(&self.released.0.to_le_bytes()); bytes
    }
    pub fn restore(bytes: [u8; 20]) -> Option<Self> {
        let values: [f32; 4] = core::array::from_fn(|n| f32::from_le_bytes(bytes[n*4..n*4+4].try_into().unwrap()));
        Self::new([values[0], values[1], values[2]], values[3], Time(u32::from_le_bytes(bytes[16..20].try_into().unwrap())))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn falls_settles_and_restores_across_clock_wrap() {
        let drop = Drop::new([1., 2., 3.], 0., Time(u32::MAX - 99)).unwrap();
        assert_eq!(Drop::restore(drop.snapshot()), Some(drop));
        assert_eq!(drop.position(Time(u32::MAX - 100)), [1., 2., 3.]);
        let halfway = drop.position(Time(400));
        assert!((halfway[1] - 0.77375).abs() < 0.0001);
        assert_eq!(drop.position(Time(900)), [1., 0., 3.]);
        assert_eq!(drop.position(Time(2000)), [1., 0., 3.]);
    }
    #[test] fn rejects_nonfinite_or_uphill_floor() {
        assert!(Drop::new([0., 1., 0.], 2., Time(0)).is_none());
        assert!(Drop::new([f32::NAN, 1., 0.], 0., Time(0)).is_none());
        assert!(Drop::restore([255; 20]).is_none());
    }
}
