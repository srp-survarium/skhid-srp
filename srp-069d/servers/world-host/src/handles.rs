//! Kernel objects behind fake HANDLEs: events, mutexes, semaphores, waitable timers, threads,
//! completion ports and files. One global lock + condvar; waits poll the state under it
//! (simple and correct; these objects are not on a hot path).

use crate::files::FileObj;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

pub const WAIT_OBJECT_0: u32 = 0;
pub const WAIT_TIMEOUT: u32 = 0x102;
pub const WAIT_FAILED: u32 = 0xFFFF_FFFF;
pub const INFINITE: u32 = 0xFFFF_FFFF;
pub const PSEUDO_PROCESS: u64 = u64::MAX;
pub const PSEUDO_THREAD: u64 = u64::MAX - 1;

pub enum Obj {
    Event { manual: bool, signaled: bool },
    Mutex { owner: u64, count: u32 },
    Semaphore { count: i64, max: i64 },
    Timer { manual: bool, signaled: bool, due: Option<Instant>, period: Duration },
    Thread { tid: u64, done: bool, exit_code: u32, suspended: u32 },
    Iocp { queue: VecDeque<(u32, u64, u64)> },
    File(Arc<Mutex<FileObj>>),
    Find(Arc<Mutex<crate::files::FindObj>>),
    Process,
}

pub struct State {
    pub objs: HashMap<u64, Obj>,
    pub next: u64,
    /// file handle -> (completion port, key)
    pub iocp_assoc: HashMap<u64, (u64, u64)>,
    /// The exe's timer-loop threads seen in their idle wait (`wait_marked`): thread id -> inside that wait now. Set and
    /// cleared under this lock, so a thread marked here cannot leave the wait while the lock is held (the fork census,
    /// `run::fork_world`).
    pub idle_waiters: HashMap<u64, bool>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);
static CV: Condvar = Condvar::new();

pub fn lock() -> MutexGuard<'static, Option<State>> {
    let mut g = STATE.lock().unwrap();
    if g.is_none() {
        *g = Some(State { objs: HashMap::new(), next: 0x1000, iocp_assoc: HashMap::new(), idle_waiters: HashMap::new() });
    }
    g
}

/// The fork census over the timer-loop threads' marks: (in their idle wait now, known).
pub fn timer_census(marks: impl IntoIterator<Item = bool>) -> (usize, usize) {
    marks.into_iter().fold((0, 0), |(k, n), m| (k + usize::from(m), n + 1))
}

pub fn notify() {
    CV.notify_all();
}

pub fn insert(o: Obj) -> u64 {
    let mut g = lock();
    let s = g.as_mut().unwrap();
    let h = s.next;
    s.next += 4;
    s.objs.insert(h, o);
    h
}

pub fn close(h: u64) -> bool {
    let mut g = lock();
    let s = g.as_mut().unwrap();
    s.iocp_assoc.remove(&h);
    s.objs.remove(&h).is_some()
}

pub fn file(h: u64) -> Option<Arc<Mutex<FileObj>>> {
    let g = lock();
    match g.as_ref().unwrap().objs.get(&h) {
        Some(Obj::File(f)) => Some(f.clone()),
        _ => None,
    }
}

pub fn find(h: u64) -> Option<Arc<Mutex<crate::files::FindObj>>> {
    let g = lock();
    match g.as_ref().unwrap().objs.get(&h) {
        Some(Obj::Find(f)) => Some(f.clone()),
        _ => None,
    }
}

fn update_timers(s: &mut State, now: Instant) {
    for o in s.objs.values_mut() {
        if let Obj::Timer { signaled, due, period, .. } = o {
            if let Some(d) = *due {
                if now >= d {
                    *signaled = true;
                    *due = if period.is_zero() { None } else { Some(d + *period) };
                }
            }
        }
    }
}

fn is_signaled(s: &State, h: u64, tid: u64) -> Option<bool> {
    Some(match s.objs.get(&h)? {
        Obj::Event { signaled, .. } => *signaled,
        Obj::Mutex { owner, .. } => *owner == 0 || *owner == tid,
        Obj::Semaphore { count, .. } => *count > 0,
        Obj::Timer { signaled, .. } => *signaled,
        Obj::Thread { done, .. } => *done,
        Obj::Process => false,
        Obj::File(_) => true,
        _ => return None,
    })
}

fn consume(s: &mut State, h: u64, tid: u64) {
    match s.objs.get_mut(&h) {
        Some(Obj::Event { manual: false, signaled }) => *signaled = false,
        Some(Obj::Mutex { owner, count }) => {
            *owner = tid;
            *count += 1;
        }
        Some(Obj::Semaphore { count, .. }) => *count -= 1,
        Some(Obj::Timer { manual: false, signaled, .. }) => *signaled = false,
        _ => {}
    }
}

fn next_timer_due(s: &State) -> Option<Instant> {
    s.objs
        .values()
        .filter_map(|o| if let Obj::Timer { due, .. } = o { *due } else { None })
        .min()
}

/// WaitForMultipleObjects semantics. Returns WAIT_OBJECT_0+i, WAIT_TIMEOUT or WAIT_FAILED.
pub fn wait(hs: &[u64], all: bool, timeout_ms: u32) -> u32 {
    wait_marked(hs, all, timeout_ms, false)
}

/// [`wait`]; `idle` = this is an exe timer-loop thread's idle wait: the thread is marked in `idle_waiters` from its
/// first look at the handles to its return, both under the lock.
pub fn wait_marked(hs: &[u64], all: bool, timeout_ms: u32, idle: bool) -> u32 {
    let tid = crate::teb::gettid();
    let mut g = lock();
    if idle {
        g.as_mut().unwrap().idle_waiters.insert(tid, true);
    }
    let (mut g, r) = wait_in(g, hs, all, timeout_ms, tid);
    if idle {
        g.as_mut().unwrap().idle_waiters.insert(tid, false);
    }
    r
}

type Guard = MutexGuard<'static, Option<State>>;

fn wait_in(mut g: Guard, hs: &[u64], all: bool, timeout_ms: u32, tid: u64) -> (Guard, u32) {
    let deadline = if timeout_ms == INFINITE { None } else { Some(Instant::now() + Duration::from_millis(timeout_ms as u64)) };
    loop {
        let now = Instant::now();
        let s = g.as_mut().unwrap();
        update_timers(s, now);
        let mut sig = Vec::with_capacity(hs.len());
        for &h in hs {
            match is_signaled(s, h, tid) {
                Some(v) => sig.push(v),
                None => {
                    crate::hlog!("wait: unknown handle {h:#x}");
                    return (g, WAIT_FAILED);
                }
            }
        }
        if all {
            if sig.iter().all(|&x| x) {
                for &h in hs {
                    consume(s, h, tid);
                }
                return (g, WAIT_OBJECT_0);
            }
        } else if let Some(i) = sig.iter().position(|&x| x) {
            consume(s, hs[i], tid);
            return (g, WAIT_OBJECT_0 + i as u32);
        }
        if let Some(d) = deadline {
            if now >= d {
                return (g, WAIT_TIMEOUT);
            }
        }
        let mut until = deadline.unwrap_or(now + Duration::from_millis(50));
        if let Some(t) = next_timer_due(s) {
            until = until.min(t);
        }
        let dur = until.saturating_duration_since(now).max(Duration::from_micros(50)).min(Duration::from_millis(50));
        g = CV.wait_timeout(g, dur).unwrap().0;
    }
}

pub fn with<R>(h: u64, f: impl FnOnce(&mut Obj) -> R) -> Option<R> {
    let mut g = lock();
    let r = g.as_mut().unwrap().objs.get_mut(&h).map(f);
    drop(g);
    notify();
    r
}

/// IOCP: post a completion.
pub fn iocp_post(port: u64, bytes: u32, key: u64, ov: u64) -> bool {
    let ok = with(port, |o| {
        if let Obj::Iocp { queue } = o {
            queue.push_back((bytes, key, ov));
            true
        } else {
            false
        }
    });
    ok == Some(true)
}

/// IOCP: wait for a completion.
pub fn iocp_get(port: u64, timeout_ms: u32) -> Option<(u32, u64, u64)> {
    let deadline = if timeout_ms == INFINITE { None } else { Some(Instant::now() + Duration::from_millis(timeout_ms as u64)) };
    let mut g = lock();
    loop {
        if let Some(Obj::Iocp { queue }) = g.as_mut().unwrap().objs.get_mut(&port) {
            if let Some(x) = queue.pop_front() {
                return Some(x);
            }
        } else {
            return None;
        }
        let now = Instant::now();
        if let Some(d) = deadline {
            if now >= d {
                return None;
            }
        }
        g = CV.wait_timeout(g, Duration::from_millis(20)).unwrap().0;
    }
}

pub fn iocp_for_file(h: u64) -> Option<(u64, u64)> {
    lock().as_ref().unwrap().iocp_assoc.get(&h).copied()
}
pub fn iocp_associate(file: u64, port: u64, key: u64) {
    lock().as_mut().unwrap().iocp_assoc.insert(file, (port, key));
}

/// Block until a CREATE_SUSPENDED thread has been resumed.
pub fn wait_resumed(h: u64) {
    let mut g = lock();
    loop {
        match g.as_ref().unwrap().objs.get(&h) {
            Some(Obj::Thread { suspended, .. }) if *suspended > 0 => {}
            _ => return,
        }
        g = CV.wait_timeout(g, Duration::from_millis(20)).unwrap().0;
    }
}

#[cfg(test)]
mod tests {
    use super::timer_census;

    #[test]
    fn the_census_counts_the_marked_timer_threads_of_the_known_ones() {
        assert_eq!(timer_census([]), (0, 0), "no timer thread seen yet: nothing to wait for");
        assert_eq!(timer_census([true, true]), (2, 2));
        assert_eq!(timer_census([true, false]), (1, 2), "one is out of its idle wait: the fork waits");
        assert_eq!(timer_census([false, false, true]), (1, 3));
    }
}
