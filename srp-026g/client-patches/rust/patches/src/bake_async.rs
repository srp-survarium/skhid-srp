//! Patch 111: the camouflage/decal bake does not wait for the GPU
//! (docs/notes/baked-textures.md; client_patch.md 3.57, "The bake's GPU
//! round trips").
//!
//! Stock, a bake read its target back, uploaded it again and read each of
//! its ten compressed levels back, each time waiting for the GPU on the
//! game's thread (the hitch of a player joining with an item nobody baked
//! yet). Here (vostok::bake_gpu does the work):
//! - `on_issue`: the bake's target is copied on the GPU into the
//!   compressor's level 0, the levels are drawn and copied to stagings with
//!   one flush each and no wait, and process_compression ends, its query
//!   not finished (patch 70 keeps its waiting queries waiting). At most
//!   IN_FLIGHT bakes are on the GPU; the next QUEUED keep their level 0 and
//!   go when one ends; past those, the stock bake;
//! - `on_dispatched`, each resources dispatch on the bake's thread: the
//!   oldest bakes whose last staging maps without waiting (FINISHED_A_FRAME
//!   of them a dispatch, the rest in the next frames, oldest first) are read
//!   back and finished by process_compression's own tail (`on_resume`), the texture,
//!   its sharing, its save (`on_save`, `on_free`: the DDS is MT memory the
//!   save descriptor owns, no copy), its record and the "camouflage/decal
//!   cook" line; one that waited LATE_MS is read back waiting, and logged;
//! - `on_exit`, game::exit: every pending bake finished, waiting.
//!
//! A bake whose levels cannot be read back (the device lost) or that finds
//! no MT memory fails its query, as a bake that fails does. Textures only:
//! nothing a match computes changes.

#![forbid(unsafe_code)]

use crate::vostok::bake_gpu::{self, Compressed, HeldTexture, Issue, Job, Stamp, ThreadId};
use crate::vostok::log::{line, Piece};
use crate::vostok::{BakeAsyncOnIssueExit, BakeAsyncOnReadbackExit, BakeAsyncOnResumeExit, BakeAsyncOnSaveExit,
                    Context, Flow, GameCell, Lock, NoExit, Thread};

/// Bakes on the GPU at once (their stagings, 3 to 6 MB each).
const IN_FLIGHT: usize = 8;
/// Bakes waiting for a place (their level 0, 16 MB of the GPU's each).
const QUEUED: usize = 8;
/// A bake on the GPU this long is read back waiting.
const LATE_MS: f32 = 1000.0;
/// Done bakes a dispatch finishes (each makes its texture from its DDS and
/// runs its CRC); the rest finish in the next frames, oldest first.
const FINISHED_A_FRAME: u32 = 2;

/// A pending bake.
enum Bake {
    /// Its target copied into level 0, waiting for a place on the GPU.
    Queued { job: Job, level0: HeldTexture },
    /// Its levels drawn and copied to stagings at `since`.
    InFlight { job: Job, gpu: Compressed, since: Stamp },
}

/// A bake's state without its resources: what [`next`] decides on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Queued,
    InFlight,
}

/// What happened to a pending bake.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Event {
    /// A place on the GPU is free (a queued bake can go).
    Room,
    /// None is.
    NoRoom,
    /// Its last staging maps without waiting: the GPU has done it; this
    /// dispatch may finish `left` more bakes (FINISHED_A_FRAME at its start).
    Done { left: u32 },
    /// The GPU has not done it after `waited_ms`.
    Busy { waited_ms: f32 },
    /// The game exits: every bake ends now.
    Drain,
}

/// What a pending bake does on an event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Next {
    Wait,
    /// Its levels drawn and copied (it is in flight then).
    Issue,
    /// Read back and finished.
    Complete,
    /// Read back waiting for the GPU (logged), then finished.
    CompleteLate,
}

/// The one transition: a pending bake's phase and an event to what it does.
fn next(phase: Phase, event: Event) -> Next {
    match (phase, event) {
        (Phase::Queued, Event::Room | Event::Drain) => Next::Issue,
        (Phase::Queued, Event::NoRoom | Event::Done { .. } | Event::Busy { .. }) => Next::Wait,
        (Phase::InFlight, Event::Done { left: 0 }) => Next::Wait,
        (Phase::InFlight, Event::Done { .. }) => Next::Complete,
        (Phase::InFlight, Event::Busy { waited_ms }) if waited_ms >= LATE_MS => Next::CompleteLate,
        (Phase::InFlight, Event::Drain) => Next::CompleteLate,
        (Phase::InFlight, Event::Busy { .. } | Event::Room | Event::NoRoom) => Next::Wait,
    }
}

/// Where a new bake goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Admit {
    InFlight,
    Queued,
    /// The stock bake (waiting for the GPU): no place, or the pending bakes
    /// are another thread's.
    Stock,
}

fn admit(in_flight: usize, queued: usize, same_thread: bool) -> Admit {
    match (same_thread, in_flight < IN_FLIGHT, queued < QUEUED) {
        (false, _, _) => Admit::Stock,
        (true, true, _) => Admit::InFlight,
        (true, false, true) => Admit::Queued,
        (true, false, false) => Admit::Stock,
    }
}

impl Bake {
    fn phase(&self) -> Phase {
        match self {
            Bake::Queued { .. } => Phase::Queued,
            Bake::InFlight { .. } => Phase::InFlight,
        }
    }

    fn job(&self) -> &Job {
        match self {
            Bake::Queued { job, .. } | Bake::InFlight { job, .. } => job,
        }
    }
}

/// The pending bakes, oldest first by their number; all of one thread.
struct Table {
    bakes: [Option<(u32, Bake)>; IN_FLIGHT + QUEUED],
    next: u32,
}

static TABLE: Lock<Table> = Lock::new(Table { bakes: [const { None }; IN_FLIGHT + QUEUED], next: 0 });

impl Table {
    fn count(&self, phase: Phase) -> usize {
        self.bakes.iter().flatten().filter(|(_, bake)| bake.phase() == phase).count()
    }

    fn thread(&self) -> Option<ThreadId> {
        self.bakes.iter().flatten().next().map(|(_, bake)| bake.job().thread())
    }

    /// A new bake, the youngest.
    fn push(&mut self, bake: Bake) {
        let number = self.next;
        self.next = self.next.wrapping_add(1);
        self.put(number, bake);
    }

    /// A bake back in its place (it was taken this hook: there is room).
    fn put(&mut self, number: u32, bake: Bake) {
        if let Some(slot) = self.bakes.iter_mut().find(|slot| slot.is_none()) {
            *slot = Some((number, bake));
        }
    }

    /// The oldest bake of `phase` (any phase: None), taken out.
    fn take_oldest(&mut self, phase: Option<Phase>) -> Option<(u32, Bake)> {
        let next = self.next;
        let age = |number: u32| next.wrapping_sub(number);
        let at = self.bakes.iter().enumerate()
            .filter_map(|(i, slot)| slot.as_ref().map(|(number, bake)| (i, *number, bake.phase())))
            .filter(|&(_, _, p)| phase.is_none_or(|phase| p == phase))
            .max_by_key(|&(_, number, _)| age(number))?.0;
        self.bakes[at].take()
    }
}

/// The table's thread is this one (a bake of it is pending).
fn ours(thread: &Thread) -> bool {
    let me = ThreadId::current(thread);
    TABLE.with(thread, |t| t.thread() == Some(me))
}

fn log_job(job: &Job, what: &str, number: u32) {
    line(&[Piece::Text("bake_async: "), Piece::Text(job.name()), Piece::Text(what), Piece::Dec(number)]);
}

// ---- the hooks -------------------------------------------------------------------------

/// `bake.issue`: the bake handed to the GPU (or queued), process_compression
/// left by its epilogue with its query not finished; the stock bake when
/// there is no place or the bake is not one this can take.
pub fn on_issue(ctx: &mut Context<'_>) -> Flow<BakeAsyncOnIssueExit> {
    let Some(issue) = Issue::at(ctx) else { return Flow::Continue };
    let Some((mut job, source)) = issue.job() else { return Flow::Continue };
    let thread = issue.thread();
    let me = ThreadId::current(thread);
    let place = TABLE.with(thread, |t| {
        admit(t.count(Phase::InFlight), t.count(Phase::Queued), t.thread().is_none_or(|owner| owner == me))
    });
    let Some(level0) = (place != Admit::Stock).then(|| bake_gpu::level0(&source)).flatten() else {
        if place == Admit::Stock {
            log_job(&job, ": baked waiting (no place); pending ", (IN_FLIGHT + QUEUED) as u32);
        }
        return Flow::Continue;
    };
    let bake = match place {
        Admit::InFlight => {
            let Some(gpu) = bake_gpu::compress(thread, &job, &level0) else { return Flow::Continue };
            drop(level0);
            issue.issued(&mut job);
            Bake::InFlight { job, gpu, since: Stamp::now(thread) }
        }
        Admit::Queued | Admit::Stock => {
            issue.issued(&mut job);
            Bake::Queued { job, level0 }
        }
    };
    TABLE.with(issue.thread(), |t| t.push(bake));
    issue.leave();
    Flow::Exit(BakeAsyncOnIssueExit::Issued)
}

/// `bake.resume`: a bake's tail, frames later.
pub fn on_resume(ctx: &mut Context<'_>) -> Flow<BakeAsyncOnResumeExit> {
    if bake_gpu::resume(ctx) { Flow::Exit(BakeAsyncOnResumeExit::Tail) } else { Flow::Continue }
}

/// `bake.save`: the tail's save takes the bake's DDS.
pub fn on_save(ctx: &mut Context<'_>) -> Flow<BakeAsyncOnSaveExit> {
    if bake_gpu::save(ctx) { Flow::Exit(BakeAsyncOnSaveExit::Made) } else { Flow::Continue }
}

/// `bake.free`: the tail does not free that DDS to the render allocator.
pub fn on_free(ctx: &mut Context<'_>) -> Flow<NoExit> {
    bake_gpu::free(ctx);
    Flow::Continue
}

/// `compress_mip.readback`: a level's staging kept, not mapped.
pub fn on_readback(ctx: &mut Context<'_>) -> Flow<BakeAsyncOnReadbackExit> {
    if bake_gpu::readback(ctx) { Flow::Exit(BakeAsyncOnReadbackExit::Kept) } else { Flow::Continue }
}

/// A bake the GPU has (or must have) done: its levels read back and the
/// tail run; a level lost fails it.
fn complete(thread: &Thread, job: Job, gpu: Compressed) {
    let started = Stamp::now(thread);
    let Compressed { mut dds, readback } = gpu;
    match readback.copy_into(&mut dds) {
        Ok(()) => bake_gpu::finish(thread, job, dds, started),
        Err(_) => {
            log_job(&job, ": its levels could not be read back (the device?); failed, size ", job.size());
            drop(dds);
            bake_gpu::fail(job);
        }
    }
}

/// A queued bake's levels drawn and copied; one that finds no MT room fails.
fn issue_queued(thread: &Thread, job: Job, level0: HeldTexture) -> Option<Bake> {
    match bake_gpu::compress(thread, &job, &level0) {
        Some(gpu) => Some(Bake::InFlight { job, gpu, since: Stamp::now(thread) }),
        None => {
            log_job(&job, ": no MT memory for its DDS; failed, size ", job.size());
            bake_gpu::fail(job);
            None
        }
    }
}

/// One pass over the pending bakes for `event` (a dispatch's: what the GPU
/// says; the exit's: Drain): the oldest in flight finished while done (at
/// most FINISHED_A_FRAME a dispatch) or late, then the queued issued while
/// there is room.
fn pass(thread: &Thread, drain: bool) {
    let mut left = FINISHED_A_FRAME;
    loop {
        let Some((number, bake)) = TABLE.with(thread, |t| t.take_oldest(Some(Phase::InFlight))) else { break };
        let Bake::InFlight { job, gpu, since } = bake else { break };
        let event = if drain {
            Event::Drain
        } else if gpu.readback.ready() {
            Event::Done { left }
        } else {
            Event::Busy { waited_ms: since.elapsed_ms(thread) }
        };
        match next(Phase::InFlight, event) {
            Next::Complete => {
                left = left.saturating_sub(1);
                complete(thread, job, gpu);
            }
            Next::CompleteLate => {
                if !drain {
                    log_job(&job, ": the GPU had it 1 s; read back waiting, ms ", since.elapsed_ms(thread) as u32);
                }
                complete(thread, job, gpu);
            }
            Next::Wait | Next::Issue => {
                TABLE.with(thread, |t| t.put(number, Bake::InFlight { job, gpu, since }));
                break;
            }
        }
    }
    loop {
        let room = TABLE.with(thread, |t| t.count(Phase::InFlight) < IN_FLIGHT);
        let event = if drain { Event::Drain } else if room { Event::Room } else { Event::NoRoom };
        if next(Phase::Queued, event) != Next::Issue {
            break;
        }
        let Some((number, bake)) = TABLE.with(thread, |t| t.take_oldest(Some(Phase::Queued))) else { break };
        let Bake::Queued { job, level0 } = bake else { break };
        if let Some(issued) = issue_queued(thread, job, level0) {
            if drain {
                if let Bake::InFlight { job, gpu, .. } = issued {
                    complete(thread, job, gpu);
                }
            } else {
                TABLE.with(thread, |t| t.put(number, issued));
            }
        }
    }
}

/// Whether a pass is under way: a dispatch reached from inside one (the
/// tail it runs) does not start another.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Passing {
    Idle,
    Running,
}

static PASSING: GameCell<Passing> = GameCell::new(Passing::Idle);

/// A pass on the bakes' thread, none inside another.
fn guarded(thread: &Thread, drain: bool) {
    if PASSING.get(thread) == Passing::Running || !ours(thread) {
        return;
    }
    PASSING.set(thread, Passing::Running);
    pass(thread, drain);
    PASSING.set(thread, Passing::Idle);
}

/// `resources.dispatched`: each dispatch on the bakes' thread, the GPU asked.
pub fn on_dispatched(ctx: &mut Context<'_>) -> Flow<NoExit> {
    guarded(ctx.thread(), false);
    Flow::Continue
}

/// `game.exit_bakes`: game::exit, every pending bake finished (waiting).
pub fn on_exit(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    if ours(thread) {
        let count = TABLE.with(thread, |t| t.bakes.iter().flatten().count());
        line(&[Piece::Text("bake_async: finishing at the game's exit, bakes "), Piece::Dec(count as u32)]);
    }
    guarded(thread, true);
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_queued_bake_goes_when_there_is_room_or_at_exit() {
        assert_eq!(next(Phase::Queued, Event::Room), Next::Issue);
        assert_eq!(next(Phase::Queued, Event::Drain), Next::Issue);
        assert_eq!(next(Phase::Queued, Event::NoRoom), Next::Wait);
        assert_eq!(next(Phase::Queued, Event::Done { left: 2 }), Next::Wait);
        assert_eq!(next(Phase::Queued, Event::Busy { waited_ms: 5000.0 }), Next::Wait);
    }

    #[test]
    fn a_bake_in_flight_ends_when_done_late_or_at_exit() {
        assert_eq!(next(Phase::InFlight, Event::Done { left: FINISHED_A_FRAME }), Next::Complete);
        assert_eq!(next(Phase::InFlight, Event::Done { left: 1 }), Next::Complete);
        assert_eq!(next(Phase::InFlight, Event::Done { left: 0 }), Next::Wait);
        assert_eq!(next(Phase::InFlight, Event::Busy { waited_ms: 16.0 }), Next::Wait);
        assert_eq!(next(Phase::InFlight, Event::Busy { waited_ms: 999.9 }), Next::Wait);
        assert_eq!(next(Phase::InFlight, Event::Busy { waited_ms: 1000.0 }), Next::CompleteLate);
        assert_eq!(next(Phase::InFlight, Event::Drain), Next::CompleteLate);
        assert_eq!(next(Phase::InFlight, Event::Room), Next::Wait);
        assert_eq!(next(Phase::InFlight, Event::NoRoom), Next::Wait);
    }

    #[test]
    fn places_fill_in_flight_then_the_queue_then_the_stock_bake() {
        assert_eq!(admit(0, 0, true), Admit::InFlight);
        assert_eq!(admit(IN_FLIGHT - 1, 0, true), Admit::InFlight);
        assert_eq!(admit(IN_FLIGHT, 0, true), Admit::Queued);
        assert_eq!(admit(IN_FLIGHT, QUEUED - 1, true), Admit::Queued);
        assert_eq!(admit(IN_FLIGHT, QUEUED, true), Admit::Stock);
        assert_eq!(admit(0, 0, false), Admit::Stock);
    }
}
