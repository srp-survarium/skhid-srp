//! The bench's split of the commander's time: [`span`] times a call by label
//! (calls, total and worst µs); out of tests it is the call itself.

#[cfg(test)]
use std::{cell::RefCell, collections::BTreeMap, time::Instant};

#[cfg(test)]
thread_local! {
    static SPANS: RefCell<BTreeMap<&'static str, (u64, u64, u64)>> = const { RefCell::new(BTreeMap::new()) };
}

#[cfg(test)]
pub fn span<R>(label: &'static str, f: impl FnOnce() -> R) -> R {
    let start = Instant::now();
    let out = f();
    let us = start.elapsed().as_micros() as u64;
    SPANS.with(|s| {
        let mut s = s.borrow_mut();
        let e = s.entry(label).or_insert((0, 0, 0));
        e.0 += 1;
        e.1 += us;
        e.2 = e.2.max(us);
    });
    out
}

#[cfg(not(test))]
#[inline(always)]
pub fn span<R>(_: &'static str, f: impl FnOnce() -> R) -> R {
    f()
}

/// `label: [calls, total ms, worst µs]`, so far on this thread.
#[cfg(test)]
pub fn line() -> String {
    SPANS.with(|s| {
        let rows: Vec<String> = s
            .borrow()
            .iter()
            .map(|(k, (n, us, worst))| format!("{k:?}: [{n}, {:.1}, {worst}]", *us as f64 / 1000.0))
            .collect();
        format!("{{{}}}", rows.join(", "))
    })
}
