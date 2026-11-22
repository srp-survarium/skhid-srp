//! [CPU] The calling thread's CPU time (Linux: `CLOCK_THREAD_CPUTIME_ID`; zero elsewhere): what the thread ran, without the wall
//! time it waited for a core (TEST ONLY: this module exists in test builds). The wall clock (`Instant`) of a loaded machine counts
//! that wait too, so a cost read on it follows the machine's load; this clock follows the work. Read beside the wall clock by the
//! kept controller's clock (`body::kept_clock`), the searches (`path`) and the bench's poll parts (`tests/bench.rs`,
//! `bench_cpu`). A read is a system call (about 0.5 µs under WSL 2), so the server keeps the wall clock alone. Decision-neutral:
//! nothing reads it back.

use std::time::Duration;

/// The calling thread's CPU time so far.
pub(crate) fn thread() -> Duration {
    CALLS.with(|calls| calls.set(calls.get() + 1));
    #[cfg(target_os = "linux")]
    {
        let mut t = libc::timespec { tv_sec: 0, tv_nsec: 0 };
        // SAFETY: `t` is a valid, writable timespec; the call only writes it.
        let read = unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut t) };
        assert_eq!(read, 0, "clock_gettime(CLOCK_THREAD_CPUTIME_ID): {}", std::io::Error::last_os_error());
        Duration::new(u64::try_from(t.tv_sec).unwrap_or(0), u32::try_from(t.tv_nsec).unwrap_or(0))
    }
    #[cfg(not(target_os = "linux"))]
    {
        Duration::ZERO
    }
}

thread_local! {
    /// The reads of [`thread`] on this thread: the bench prints them with a read's own cost, so the clock's share of a cost is known.
    static CALLS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// [`thread`]'s reads on this thread so far.
pub(crate) fn calls() -> u64 {
    CALLS.with(std::cell::Cell::get)
}

/// One read of [`thread`] on this thread, ns: the least of 5 rounds of 20,000 reads (the clock's own cost, for the bench's line).
pub(crate) fn read_ns() -> f64 {
    (0..5)
        .map(|_| {
            let started = thread();
            for _ in 0..20_000 {
                std::hint::black_box(thread());
            }
            (thread() - started).as_nanos() as f64 / 20_001.0
        })
        .fold(f64::INFINITY, f64::min)
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_thread_clock_counts_work_and_not_sleep() {
        let started = super::thread();
        std::thread::sleep(std::time::Duration::from_millis(30));
        let slept = super::thread() - started;
        let mut x = 0u64;
        let busy = super::thread();
        while super::thread() - busy < std::time::Duration::from_millis(5) {
            x = std::hint::black_box(x.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1));
        }
        let worked = super::thread() - busy;
        if cfg!(target_os = "linux") {
            assert!(slept < std::time::Duration::from_millis(10), "a sleep is not the thread's CPU: {slept:?}");
            assert!(worked >= std::time::Duration::from_millis(5), "work is: {worked:?}");
        }
    }
}
