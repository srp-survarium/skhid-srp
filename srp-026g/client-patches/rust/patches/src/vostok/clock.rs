//! The machine's clock, kernel32's QueryPerformanceCounter: a patch that
//! steps something by the real time (the ragdolls, a frame each).

use super::hook::Thread;
use crate::raw;

/// The performance counter now, in its ticks.
pub fn counter(_: &Thread) -> u64 {
    let mut out = [0u32; 2];
    // SAFETY: the exe's import; it writes the 8 bytes it is given.
    unsafe { (raw::engine_query_performance_counter)(out.as_mut_ptr()) };
    u64::from(out[0]) | u64::from(out[1]) << 32
}

/// The counter's ticks a second.
pub fn frequency(_: &Thread) -> u64 {
    let mut out = [0u32; 2];
    // SAFETY: as counter.
    unsafe { (raw::engine_query_performance_frequency)(out.as_mut_ptr()) };
    u64::from(out[0]) | u64::from(out[1]) << 32
}

/// The counter in milliseconds (wraps: for logs of how long a step took).
pub fn milliseconds(thread: &Thread) -> u32 {
    let frequency = frequency(thread).max(1);
    (u128::from(counter(thread)) * 1000 / u128::from(frequency)) as u32
}

/// The machine's local date and time (kernel32's GetLocalTime): a line's
/// time stamp, for the owner to read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalTime {
    pub year: u16,
    pub month: u16,
    pub day: u16,
    pub hour: u16,
    pub minute: u16,
    pub second: u16,
    pub millisecond: u16,
}

/// The local time now.
pub fn local_time(_: &Thread) -> LocalTime {
    // a SYSTEMTIME: year, month, day of the week, day, hour, minute, second, millisecond (u16 each)
    let mut out = [0u32; 4];
    // SAFETY: the exe's import; it writes the 16 bytes it is given.
    unsafe { (raw::engine_get_local_time)(out.as_mut_ptr()) };
    let half = |i: usize| (out[i / 2] >> (16 * (i % 2))) as u16;
    LocalTime {
        year: half(0),
        month: half(1),
        day: half(3),
        hour: half(4),
        minute: half(5),
        second: half(6),
        millisecond: half(7),
    }
}
