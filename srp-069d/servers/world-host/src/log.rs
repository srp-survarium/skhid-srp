//! Host log: one file (plus stderr for host lines). The exe's own log arrives through
//! OutputDebugStringA and is written with the prefix "dbg: ".

use std::fs::File;
use std::io::Write;
use std::sync::Mutex;
use std::time::Instant;

static SINK: Mutex<Option<File>> = Mutex::new(None);
static START: Mutex<Option<Instant>> = Mutex::new(None);
static QUIET: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn open(path: &str) {
    let f = File::create(path).unwrap_or_else(|e| panic!("cannot create log {path}: {e}"));
    *SINK.lock().unwrap() = Some(f);
    *START.lock().unwrap() = Some(Instant::now());
}

/// Do not echo "dbg:" lines to stderr (they still go to the file).
pub fn set_quiet_dbg(q: bool) {
    QUIET.store(q, std::sync::atomic::Ordering::Relaxed);
}

fn stamp() -> f64 {
    START.lock().unwrap().map(|s| s.elapsed().as_secs_f64()).unwrap_or(0.0)
}

pub fn line(s: &str) {
    let t = stamp();
    let text = format!("{t:9.3} {s}\n");
    let is_dbg = s.starts_with("dbg:");
    if !is_dbg || !QUIET.load(std::sync::atomic::Ordering::Relaxed) {
        let _ = std::io::stderr().write_all(text.as_bytes());
    }
    if let Some(f) = SINK.lock().unwrap().as_mut() {
        let _ = f.write_all(text.as_bytes());
    }
}

pub fn flush() {
    if let Some(f) = SINK.lock().unwrap().as_mut() {
        let _ = f.flush();
    }
}

/// Raw write for signal handlers (no locks, no allocation).
pub fn raw(s: &[u8]) {
    unsafe {
        libc::write(2, s.as_ptr() as *const libc::c_void, s.len());
    }
}

#[macro_export]
macro_rules! hlog {
    ($($t:tt)*) => { $crate::log::line(&format!($($t)*)) };
}
