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
