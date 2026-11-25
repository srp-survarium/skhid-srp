//! [srchash oracle] Coverage windows: what a source digest's code really runs, for the check that each digest's closure
//! (`mapdata-srchash`) holds every function the build executes in its part (`executed ⊆ closure`,
//! `crates/mapdata-srchash/coverage_oracle.py`).
//!
//! Only a build with `--cfg srp_coverage` and `-C instrument-coverage` has them, and only with `SRP_COVERAGE_DIR` set: each
//! window boundary writes the counters of the segment it ends to `<dir>/<n>-<label>.profraw` and clears them. A window's own
//! segments carry its label (a memo site's id, `world`, `sight`); the build's other segments are `between`, and what ran before
//! the grid's build `pre`. The grid's code is every segment from its start to its end. Without the cfg, a window is its call.

/// Runs `f` as a window labelled `name` (a memo site's jobs, the shapes' build, the sight tree's).
#[cfg(not(srp_coverage))]
#[inline(always)]
pub(crate) fn window<R>(_name: &str, f: impl FnOnce() -> R) -> R {
    f()
}

/// Runs `f`, the grid's whole build: the segment before it is `pre`, its own segments `between` or a window's.
#[cfg(not(srp_coverage))]
#[inline(always)]
pub(crate) fn grid<R>(f: impl FnOnce() -> R) -> R {
    f()
}

#[cfg(srp_coverage)]
pub(crate) fn window<R>(name: &str, f: impl FnOnce() -> R) -> R {
    boundary("between");
    let r = f();
    boundary(name);
    r
}

#[cfg(srp_coverage)]
pub(crate) fn grid<R>(f: impl FnOnce() -> R) -> R {
    boundary("pre");
    let r = f();
    boundary("between");
    r
}

/// Writes the counters since the last boundary as `<n>-<label>.profraw` in `SRP_COVERAGE_DIR`, then clears them.
#[cfg(srp_coverage)]
fn boundary(label: &str) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    unsafe extern "C" {
        fn __llvm_profile_set_filename(name: *const std::ffi::c_char);
        fn __llvm_profile_write_file() -> i32;
        fn __llvm_profile_reset_counters();
    }
    let Some(dir) = std::env::var_os("SRP_COVERAGE_DIR") else { return };
    let n = NEXT.fetch_add(1, Ordering::SeqCst);
    let path = std::path::Path::new(&dir).join(format!("{n:04}-{label}.profraw"));
    // The runtime keeps the name's pointer: each name is leaked (a few bytes a boundary, in a coverage build only).
    let leak = |p: std::path::PathBuf| std::ffi::CString::new(p.to_string_lossy().into_owned()).expect("a path without a NUL").into_raw();
    let rest = std::path::Path::new(&dir).join("rest-%p.profraw");
    // SAFETY: the profiler runtime's own entry points, present in a `-C instrument-coverage` build; the names live on. After the
    // write the default file is `rest-<pid>`, so what runs after the last boundary never lands in a window's file.
    unsafe {
        __llvm_profile_set_filename(leak(path));
        __llvm_profile_write_file();
        __llvm_profile_reset_counters();
        __llvm_profile_set_filename(leak(rest));
    }
}
