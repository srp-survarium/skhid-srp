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

/// A minute of the local day: 0 (00:00) to 1439 (23:59).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DayMinute(u16);

impl DayMinute {
    /// `hour`:`minute`, each cut to its last (23, 59).
    pub const fn at(hour: u16, minute: u16) -> DayMinute {
        let hour = if hour > 23 { 23 } else { hour };
        let minute = if minute > 59 { 59 } else { minute };
        DayMinute(hour * 60 + minute)
    }
}

/// The machine's local time of day now (kernel32's GetLocalTime, its
/// SYSTEMTIME's hour and minute: the player's clock, the time zone and
/// summer time its).
pub fn local_time(_: &Thread) -> DayMinute {
    // SYSTEMTIME: year, month, day of week, day, hour, minute, second, ms
    let mut time = [0u16; 8];
    // SAFETY: the exe's import; it writes the 16 bytes of a SYSTEMTIME.
    unsafe { (raw::engine_get_local_time)(time.as_mut_ptr().cast()) };
    DayMinute::at(time[4], time[5])
}
