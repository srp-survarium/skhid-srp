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

/// A moment of the wall clock: kernel32's FILETIME, 100 ns since 1601 UTC.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct FileTime(u64);

/// FILETIME's ticks a second.
const FILE_TIME_SECOND: u64 = 10u64.pow(7);

impl FileTime {
    /// Now (GetSystemTimeAsFileTime).
    pub fn now(_: &Thread) -> FileTime {
        let mut out = [0u32; 2];
        // SAFETY: the exe's import; it writes the 8 bytes it is given.
        unsafe { (raw::engine_get_system_time_as_file_time)(out.as_mut_ptr()) };
        FileTime(u64::from(out[0]) | u64::from(out[1]) << 32)
    }

    /// `seconds` later.
    pub fn after_seconds(self, seconds: u32) -> FileTime {
        FileTime(self.0.saturating_add(u64::from(seconds) * FILE_TIME_SECOND))
    }

    /// The whole seconds from `self` to `later` (at most u32::MAX); 0 when
    /// it is not later.
    pub fn seconds_until(self, later: FileTime) -> u32 {
        let seconds = divide(later.0.saturating_sub(self.0), FILE_TIME_SECOND);
        u32::try_from(seconds).unwrap_or(u32::MAX)
    }

    /// The moment in the machine's time zone (FileTimeToLocalFileTime, then
    /// FileTimeToSystemTime); None when kernel32 refuses it.
    pub fn local(self, _: &Thread) -> Option<LocalTime> {
        let utc = [self.0 as u32, (self.0 >> 32) as u32];
        let mut local = [0u32; 2];
        // SYSTEMTIME: eight u16, two to a word: year, month; weekday, day;
        // hour, minute; second, milliseconds.
        let mut parts = [0u32; 4];
        // SAFETY: the exe's imports; each reads the 8 bytes of its first
        // argument and writes its second's 8 (16) bytes.
        let done = unsafe {
            (raw::engine_file_time_to_local_file_time)(utc.as_ptr(), local.as_mut_ptr()) != 0
                && (raw::engine_file_time_to_system_time)(local.as_ptr(), parts.as_mut_ptr()) != 0
        };
        done.then(|| LocalTime {
            month: (parts[0] >> 16) as u8,
            day: (parts[1] >> 16) as u8,
            hour: parts[2] as u8,
            minute: (parts[2] >> 16) as u8,
        })
    }
}

/// A wall-clock moment as the player's machine shows it, to the minute (the
/// year left out: what a ban's end needs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalTime {
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
}

/// `n / d` by shifts and subtractions: the arena links no `__udivdi3`.
fn divide(n: u64, d: u64) -> u64 {
    let (mut quotient, mut rest) = (0u64, 0u64);
    for bit in (0..64).rev() {
        rest = rest << 1 | (n >> bit & 1);
        if rest >= d {
            rest -= d;
            quotient |= 1 << bit;
        }
    }
    quotient
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seconds_between_file_times() {
        let start = FileTime(133_000_000_000_000_000);
        assert_eq!(start.seconds_until(start.after_seconds(1800)), 1800);
        assert_eq!(start.after_seconds(1800).seconds_until(start), 0);
        assert_eq!(start.seconds_until(FileTime(start.after_seconds(1).0 - 1)), 0);
        assert_eq!(start.seconds_until(start.after_seconds(u32::MAX)), u32::MAX);
        assert_eq!(divide(u64::MAX, 3), u64::MAX / 3);
    }
}
