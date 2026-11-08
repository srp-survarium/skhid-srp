//! Patch 121: the camouflage/decal bake does not wait for the GPU
//! (docs/notes/baked-textures.md; client_patch.md 3.57, "The bake's GPU
//! round trips").
//!
//! Stock, a bake read its target back, uploaded it again and read each of
//! its ten compressed levels back, each time waiting for the GPU on the
//! game's thread (the hitch of a player joining with an item nobody baked
//! yet). Here (vostok::bake_gpu does the work):
//! - `on_draw`: a bake the GPU has no place for (IN_FLIGHT bakes on it, or
//!   another thread's) is put off before its draw: on_textures_loaded ends
//!   as after a failed bake, but its query is not finished, and its model is
//!   held. The query is made again (finish_query's result 4, requery) once a
//!   place is free. Nothing waits for the GPU; nothing is drawn twice.
//! - `on_issue`: the bake's target is copied on the GPU into the
//!   compressor's level 0, the levels are drawn and copied to stagings with
//!   no wait, the last copy flushed, and process_compression ends, its query
//!   not finished (patch 70 keeps its waiting queries waiting). No DDS
//!   memory is held while the GPU has the bake.
//! - `on_dispatched`, each resources dispatch on the bakes' thread: the
//!   oldest bakes whose last staging maps without waiting are read back
//!   (their DDS allocated then, MT memory) and finished by
//!   process_compression's own tail (`on_resume`): the texture, its sharing,
//!   its save (`on_save`, `on_free`: the descriptor owns the DDS, no copy),
//!   its record and the "camouflage/decal cook" line, while the frame's
//!   BUDGET_MS lasts. One that waited LATE_MS is read back waiting, and
//!   logged. Then the postponed queries are made again, as many as there are
//!   free places.
//! - `on_frame`, engine_world::tick: a new frame, a new budget.
//! - `on_exit`, game::exit's start: every pending bake finished waiting and
//!   every postponed query made again (on its thread: now, or at its next
//!   pass); after it, bakes take the stock path.
//!
//! A bake whose levels cannot be read back (the device lost), that finds no
//! MT memory, or whose compressor does not fit its DDS fails its query, as a
//! bake that fails does. Textures only: nothing a match computes changes.

#![forbid(unsafe_code)]

use crate::vostok::bake_gpu::{self, CaptureError, CompressError, Job, Postponed, Progress, ReadError, Readback, Saved,
                              Stamp, ThreadId};
use crate::vostok::log::{line, parts, Limited, Piece};
use crate::vostok::{BakeAsyncOnDrawExit, BakeAsyncOnIssueExit, BakeAsyncOnReadbackExit, BakeAsyncOnResumeExit,
                    BakeAsyncOnSaveExit, Context, Flow, Lock, NoExit, Thread};

/// Bakes on the GPU at once (their stagings, up to 5.6 MB each).
const IN_FLIGHT: usize = 8;
/// Queries put off at once (16 bytes each, in the arena); past them a query
/// is made again at once (its translate and texture queries again, no draw).
const POSTPONED: usize = 512;
/// A bake on the GPU this long is read back waiting.
const LATE_MS: f32 = 1000.0;
/// A frame's time for finishing done bakes (each makes its texture from its
/// DDS, runs its CRC and saves it); the rest finish in the next frames,
/// oldest first. A frame always starts one.
const BUDGET_MS: f32 = 6.0;
/// A frame engine_world::tick has not ended in this long counts as ended
/// (apc's wait loops dispatch with no tick): its budget starts again.
const FRAME_MS: f32 = 100.0;

/// A bake's turn: a smaller number was issued (or put off) earlier. Wraps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Number(u32);

impl Number {
    /// How many numbers were given after this one, `next` the next to give.
    fn age(self, next: Number) -> u32 {
        next.0.wrapping_sub(self.0)
    }

    fn after(self) -> Number {
        Number(self.0.wrapping_add(1))
    }
}

/// A bake on the GPU: its job, its stagings, since when.
struct InFlight {
    job: Job,
    readback: Readback,
    since: Stamp,
}

/// A place on the GPU.
enum Slot {
    Free,
    /// A hook of this thread has it: a bake being issued into it, or one out
    /// of it while a pass polls or finishes it. A [`Ticket`] holds it.
    Taken(ThreadId),
    Bake(Number, InFlight),
}

impl Slot {
    fn thread(&self) -> Option<ThreadId> {
        match self {
            Slot::Free => None,
            Slot::Taken(thread) => Some(*thread),
            Slot::Bake(_, bake) => Some(bake.job.thread()),
        }
    }
}

/// Up to N values packed at the front: the first empty slot ends them.
struct Pack<T, const N: usize>([Option<T>; N]);

impl<T, const N: usize> Pack<T, N> {
    const fn new() -> Self {
        Pack([const { None }; N])
    }

    fn len(&self) -> usize {
        self.iter().count()
    }

    fn iter(&self) -> impl Iterator<Item = &T> {
        self.0.iter().map_while(Option::as_ref)
    }

    /// The value at the end; given back when there is no room.
    fn push(&mut self, value: T) -> Result<(), T> {
        let n = self.len();
        match self.0.get_mut(n) {
            Some(slot) => {
                *slot = Some(value);
                Ok(())
            }
            None => Err(value),
        }
    }

    /// The value at `at` taken out, the last moved into its place.
    fn remove(&mut self, at: usize) -> Option<T> {
        let last = self.len().checked_sub(1).filter(|&last| at <= last)?;
        let value = self.0[at].take();
        self.0.swap(at, last);
        value
    }
}

/// Why a bake is put off.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Why {
    /// The GPU's IN_FLIGHT places are taken.
    Full,
    /// The bakes on the GPU are another thread's (the immediate context is
    /// one thread's).
    Thread(ThreadId),
    /// No texture for its level 0 (create_texture2d: the GPU's memory).
    Level0,
}

/// Where a bake of this thread goes, at its draw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Admit {
    /// The free place at that slot.
    Place(usize),
    Postpone(Why),
    /// The game exits: the stock bake.
    Stock,
}

/// Where a bake of this thread goes, at its issue: as [`Admit`], its place
/// taken.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Reserved {
    Place { at: usize, number: Number },
    Postpone(Why),
    Stock,
}

/// The game's state as the patch sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Playing,
    /// game::exit started: pending bakes end, waiting; new ones are stock.
    Exiting,
}

/// A frame's budget: when it began (engine_world::tick, or a pass that
/// found none for FRAME_MS), the ms its passes spent finishing bakes, and
/// the queries they made again.
#[derive(Clone, Copy, Debug)]
struct Frame {
    started: Stamp,
    spent_ms: f32,
    requeried: usize,
}

impl Frame {
    fn new(now: Stamp) -> Frame {
        Frame { started: now, spent_ms: 0.0, requeried: 0 }
    }
}

/// Whether a pass is under way (on any thread): a dispatch reached from
/// inside one (the tail it runs) or beside it does not start another.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pass {
    Idle,
    Running,
}

/// The owner of the pending bakes: the GPU's places, the postponed queries
/// (oldest first by their numbers), the frame's budget, the pass, the mode.
struct Table {
    slots: [Slot; IN_FLIGHT],
    postponed: Pack<(Number, Postponed), POSTPONED>,
    next: Number,
    frame: Option<Frame>,
    pass: Pass,
    mode: Mode,
}

static TABLE: Lock<Table> = Lock::new(Table {
    slots: [const { Slot::Free }; IN_FLIGHT],
    postponed: Pack::new(),
    next: Number(0),
    frame: None,
    pass: Pass::Idle,
    mode: Mode::Playing,
});

impl Table {
    /// The thread whose bakes the GPU has (one at a time).
    fn owner(&self) -> Option<ThreadId> {
        self.slots.iter().find_map(Slot::thread)
    }

    fn in_flight(&self) -> usize {
        self.slots.iter().filter(|slot| !matches!(slot, Slot::Free)).count()
    }

    fn number(&mut self) -> Number {
        let number = self.next;
        self.next = number.after();
        number
    }

    /// The one admission: where a bake of `me` goes now.
    fn admit(&self, me: ThreadId) -> Admit {
        let free = self.slots.iter().position(|slot| matches!(slot, Slot::Free));
        match (self.mode, self.owner(), free) {
            (Mode::Exiting, _, _) => Admit::Stock,
            (Mode::Playing, Some(owner), _) if owner != me => Admit::Postpone(Why::Thread(owner)),
            (Mode::Playing, _, Some(at)) => Admit::Place(at),
            (Mode::Playing, _, None) => Admit::Postpone(Why::Full),
        }
    }

    /// As admit, the free place taken for `me` when there is one.
    fn reserve(&mut self, me: ThreadId) -> Reserved {
        match self.admit(me) {
            Admit::Place(at) => {
                self.slots[at] = Slot::Taken(me);
                Reserved::Place { at, number: self.number() }
            }
            Admit::Postpone(why) => Reserved::Postpone(why),
            Admit::Stock => Reserved::Stock,
        }
    }

    /// A query put off, the youngest: the count put off now, or the query
    /// given back when there is no room.
    fn postpone(&mut self, postponed: Postponed) -> Result<usize, Postponed> {
        let number = self.number();
        self.postponed.push((number, postponed)).map_err(|(_, postponed)| postponed)?;
        Ok(self.postponed.len())
    }

    /// The frame's budget at `now`: this frame's, or a new one when none is
    /// or it began FRAME_MS ago.
    fn frame(&mut self, now: Stamp, thread: &Thread) -> &mut Frame {
        let frame = self.frame.filter(|frame| frame.started.ms_to(now, thread) < FRAME_MS);
        self.frame.insert(frame.unwrap_or(Frame::new(now)))
    }

    /// A pass started for `me` (its mode), when none runs and `me` has a
    /// bake on the GPU or a query put off.
    fn start_pass(&mut self, me: ThreadId) -> Option<Mode> {
        let mine = self.slots.iter().any(|slot| matches!(slot, Slot::Bake(_, bake) if bake.job.thread() == me))
            || self.postponed.iter().any(|(_, postponed)| postponed.thread() == me);
        (self.pass == Pass::Idle && mine).then(|| {
            self.pass = Pass::Running;
            self.mode
        })
    }

    /// The oldest bake of `me` on the GPU, out of its slot (taken while out).
    fn take_oldest(&mut self, me: ThreadId) -> Option<(usize, Number, InFlight)> {
        let next = self.next;
        let (at, number) = self.slots.iter().enumerate()
            .filter_map(|(at, slot)| match slot {
                Slot::Bake(number, bake) if bake.job.thread() == me => Some((at, *number)),
                _ => None,
            })
            .max_by_key(|&(_, number)| number.age(next))?;
        match core::mem::replace(&mut self.slots[at], Slot::Taken(me)) {
            Slot::Bake(_, bake) => Some((at, number, bake)),
            Slot::Free | Slot::Taken(_) => None,
        }
    }

    /// The oldest query `me` put off, taken out.
    fn take_postponed(&mut self, me: ThreadId) -> Option<Postponed> {
        let next = self.next;
        let at = self.postponed.iter().enumerate()
            .filter(|(_, (_, postponed))| postponed.thread() == me)
            .max_by_key(|(_, (number, _))| number.age(next))?.0;
        self.postponed.remove(at).map(|(_, postponed)| postponed)
    }

    /// Whether a place is free this frame for a query of `me`'s put off:
    /// a free slot (none while another thread's bakes are on the GPU) that
    /// the queries made again this frame do not come back to take.
    fn room(&mut self, me: ThreadId, now: Stamp, thread: &Thread) -> Room {
        let free = match self.admit(me) {
            Admit::Place(_) => self.slots.iter().filter(|slot| matches!(slot, Slot::Free)).count(),
            Admit::Postpone(_) => 0,
            Admit::Stock => return Room::Exit,
        };
        if free > self.frame(now, thread).requeried { Room::Free } else { Room::None }
    }
}

/// A place on the GPU this hook holds ([`Slot::Taken`]): filled with a bake,
/// or freed when dropped. Never made or dropped inside the table's lock.
struct Ticket<'t> {
    at: usize,
    number: Number,
    thread: &'t Thread,
}

impl Ticket<'_> {
    fn fill(self, bake: InFlight) {
        let (at, number) = (self.at, self.number);
        TABLE.with(self.thread, |t| t.slots[at] = Slot::Bake(number, bake));
        core::mem::forget(self);
    }
}

impl Drop for Ticket<'_> {
    fn drop(&mut self) {
        let at = self.at;
        TABLE.with(self.thread, |t| t.slots[at] = Slot::Free);
    }
}

// ---- the transitions ---------------------------------------------------------------

/// What a pass sees of the oldest bake on the GPU (the frame's budget not
/// spent, or the game exiting).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Seen {
    /// Its last staging maps without waiting: the GPU has done it.
    Done,
    /// The GPU has not, after `waited_ms`.
    Busy { waited_ms: f32 },
}

/// What a bake on the GPU does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    /// Stays (and the ones younger with it: the GPU runs them in order).
    Wait,
    /// Read back and finished.
    Finish,
    /// Read back waiting for the GPU (late: logged), then finished.
    FinishLate,
    /// Read back waiting at the game's exit, then finished.
    FinishAtExit,
}

/// A bake on the GPU: the one transition.
fn step(mode: Mode, seen: Seen) -> Step {
    match (mode, seen) {
        (_, Seen::Done) => Step::Finish,
        (Mode::Playing, Seen::Busy { waited_ms }) if waited_ms >= LATE_MS => Step::FinishLate,
        (Mode::Playing, Seen::Busy { .. }) => Step::Wait,
        (Mode::Exiting, Seen::Busy { .. }) => Step::FinishAtExit,
    }
}

/// What a pass sees for the oldest postponed query.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Room {
    /// A place on the GPU is free for it this frame.
    Free,
    None,
    /// The game exits.
    Exit,
}

/// What a postponed query does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Requery {
    Wait,
    Now,
}

/// A postponed query: the one transition.
fn requery(room: Room) -> Requery {
    match room {
        Room::Free | Room::Exit => Requery::Now,
        Room::None => Requery::Wait,
    }
}

// ---- the log ---------------------------------------------------------------------

/// The postponed bakes' lines (one a second, with the count between).
static POSTPONED_LINES: Limited = Limited::new("postponed");

/// A bake put off (named when its job was read), why, and how many are now.
fn log_postponed(thread: &Thread, name: Option<&str>, why: Why, count: usize) {
    let owner;
    let reason: &[Piece] = match why {
        Why::Full => &[Piece::Text("the GPU's places are taken")],
        Why::Level0 => &[Piece::Text("no texture for its level 0 (the GPU's memory?)")],
        Why::Thread(thread) => {
            owner = [Piece::Text("the GPU has the bakes of thread "), Piece::Hex(thread.id())];
            &owner
        }
    };
    POSTPONED_LINES.parts(thread, &[&[Piece::Text("bake_async: "), Piece::Text(name.unwrap_or("a bake")),
                                      Piece::Text(" postponed: ")],
                                    reason, &[Piece::Text("; postponed now "), Piece::Dec(count as u32)]]);
}

fn log_job(job: &Job, what: &[Piece]) {
    parts(&[&[Piece::Text("bake_async: "), Piece::Text(job.name())], what]);
}

/// What a compressor's failure says.
fn compress_error(error: CompressError, write: impl FnOnce(&[Piece])) {
    use Piece::{Dec, Text};
    match error {
        CompressError::NoDevice => write(&[Text(": no render device")]),
        CompressError::Levels(n) => write(&[Text(": the compressor gives "), Dec(n), Text(" levels")]),
        CompressError::Size { data, size } => {
            write(&[Text(": its levels' "), Dec(data), Text(" bytes do not fit its DDS of "), Dec(size)])
        }
        CompressError::Level0 => write(&[Text(": no texture for its level 0")]),
        CompressError::Capture(CaptureError::Site) => write(&[Text(": a level's staging or cursor missing")]),
        CompressError::Capture(CaptureError::TooMany) => write(&[Text(": more levels than a texture has")]),
        CompressError::Capture(CaptureError::Fit { offset, size }) => {
            write(&[Text(": a level at "), Dec(offset), Text(" not inside its DDS of "), Dec(size)])
        }
        CompressError::End { end, size } => {
            write(&[Text(": the levels end at "), Dec(end), Text(", past its DDS of "), Dec(size)])
        }
    }
}

// ---- the hooks -------------------------------------------------------------------------

/// `bake.draw`: a bake the GPU has no place for now put off before its draw
/// (on_textures_loaded left as after a failed bake, its query not
/// finished); else drawn. With no room to keep it, it is made again at once.
pub fn on_draw(ctx: &mut Context<'_>) -> Flow<BakeAsyncOnDrawExit> {
    let Some(draw) = bake_gpu::Draw::at(ctx) else { return Flow::Continue };
    let thread = draw.thread();
    let me = ThreadId::current(thread);
    let why = match TABLE.with(thread, |t| t.admit(me)) {
        Admit::Place(_) | Admit::Stock => return Flow::Continue,
        Admit::Postpone(why) => why,
    };
    let Some(postponed) = draw.postpone() else { return Flow::Continue };
    keep(thread, None, why, postponed);
    Flow::Exit(BakeAsyncOnDrawExit::Postponed)
}

/// A query put off kept, or made again at once when no room is left.
fn keep(thread: &Thread, name: Option<&str>, why: Why, postponed: Postponed) {
    match TABLE.with(thread, |t| t.postpone(postponed)) {
        Ok(count) => log_postponed(thread, name, why, count),
        Err(postponed) => {
            POSTPONED_LINES.line(thread, &[Piece::Text("bake_async: a bake made again at once: "),
                                           Piece::Dec(POSTPONED as u32), Piece::Text(" postponed already")]);
            postponed.requery();
        }
    }
}

/// `bake.issue`: the bake handed to the GPU (process_compression left by its
/// epilogue, its query not finished); put off when its place went since its
/// draw or its level 0 found no GPU memory; failed when its compressor does
/// not fit its DDS. The stock bake when the game exits or the bake is not
/// one this can read.
pub fn on_issue(ctx: &mut Context<'_>) -> Flow<BakeAsyncOnIssueExit> {
    let Some(issue) = bake_gpu::Issue::at(ctx) else { return Flow::Continue };
    let thread = issue.thread();
    let Some((mut job, source)) = issue.job() else {
        line(&[Piece::Text("bake_async: a bake whose frame this does not read; the stock bake")]);
        return Flow::Continue;
    };
    let me = ThreadId::current(thread);
    let ticket = match TABLE.with(thread, |t| t.reserve(me)) {
        Reserved::Place { at, number } => Ticket { at, number, thread },
        Reserved::Stock => return Flow::Continue,
        Reserved::Postpone(why) => {
            let name = job.name_copy();
            keep(thread, Some(name.as_str()), why, job.postpone());
            issue.leave();
            return Flow::Exit(BakeAsyncOnIssueExit::Issued);
        }
    };
    match bake_gpu::compress(thread, &job, &source) {
        Ok(readback) => {
            issue.issued(&mut job);
            ticket.fill(InFlight { job, readback, since: Stamp::now(thread) });
        }
        Err(CompressError::Level0) => {
            drop(ticket);
            let name = job.name_copy();
            keep(thread, Some(name.as_str()), Why::Level0, job.postpone());
        }
        Err(error) => {
            drop(ticket);
            compress_error(error, |what| {
                parts(&[&[Piece::Text("bake_async: "), Piece::Text(job.name())], what, &[Piece::Text("; failed")]])
            });
            job.fail();
        }
    }
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

/// A bake the GPU has (or must have) done: its DDS allocated, its levels
/// read back and the tail run; no MT memory or a level lost fails it.
fn complete(thread: &Thread, bake: InFlight) {
    let started = Stamp::now(thread);
    let InFlight { job, readback, .. } = bake;
    match readback.into_dds() {
        Ok(dds) => {
            let name = job.name_copy();
            let what: &[Piece] = match bake_gpu::finish(thread, job, dds, started) {
                Saved::Yes => return,
                Saved::NoDescriptor => &[Piece::Text(": no memory for its save; used, not saved")],
                Saved::NotReached => &[Piece::Text(": its tail did not reach the save")],
            };
            parts(&[&[Piece::Text("bake_async: "), Piece::Text(name.as_str())], what]);
        }
        Err(ReadError::NoMemory) => {
            log_job(&job, &[Piece::Text(": no MT memory for its DDS of "), Piece::Dec(job.size()), Piece::Text("; failed")]);
            job.fail();
        }
        Err(ReadError::Lost) => {
            log_job(&job, &[Piece::Text(": its levels could not be read back (the device?); failed, size "),
                            Piece::Dec(job.size())]);
            job.fail();
        }
    }
}

/// The bakes of `me` on the GPU, oldest first: each the GPU has done
/// finished while the frame's budget lasts (at the exit every one, waiting);
/// a busy one stops the pass, unless it is late.
fn finish_bakes(thread: &Thread, me: ThreadId, mode: Mode) {
    loop {
        let now = Stamp::now(thread);
        let taken = TABLE.with(thread, |t| {
            let spent = t.frame(now, thread).spent_ms;
            (mode == Mode::Exiting || spent < BUDGET_MS).then(|| t.take_oldest(me)).flatten()
        });
        let Some((at, number, bake)) = taken else { break };
        let ticket = Ticket { at, number, thread };
        let seen = match bake.readback.progress() {
            Progress::Done => Seen::Done,
            Progress::Busy => Seen::Busy { waited_ms: bake.since.elapsed_ms(thread) },
        };
        match step(mode, seen) {
            Step::Wait => {
                ticket.fill(bake);
                break;
            }
            Step::Finish | Step::FinishAtExit => complete(thread, bake),
            Step::FinishLate => {
                log_job(&bake.job, &[Piece::Text(": the GPU had it 1 s; read back waiting, ms "),
                                     Piece::Dec(bake.since.elapsed_ms(thread) as u32)]);
                complete(thread, bake);
            }
        }
        drop(ticket);
        let spent = now.elapsed_ms(thread);
        TABLE.with(thread, |t| t.frame(now, thread).spent_ms += spent);
    }
}

/// The queries `me` put off, oldest first, made again while the frame has
/// places for them (at the exit, every one).
fn requery_postponed(thread: &Thread, me: ThreadId) {
    loop {
        let now = Stamp::now(thread);
        let next = TABLE.with(thread, |t| match requery(t.room(me, now, thread)) {
            Requery::Wait => None,
            Requery::Now => {
                let postponed = t.take_postponed(me)?;
                t.frame(now, thread).requeried += 1;
                Some(postponed)
            }
        });
        let Some(postponed) = next else { break };
        postponed.requery();
    }
}

/// One pass of `thread`'s, none inside or beside another.
fn pass(thread: &Thread) {
    let me = ThreadId::current(thread);
    let Some(mode) = TABLE.with(thread, |t| t.start_pass(me)) else { return };
    finish_bakes(thread, me, mode);
    requery_postponed(thread, me);
    TABLE.with(thread, |t| t.pass = Pass::Idle);
}

/// `resources.dispatched`: each dispatch, a pass for the thread's bakes.
pub fn on_dispatched(ctx: &mut Context<'_>) -> Flow<NoExit> {
    pass(ctx.thread());
    Flow::Continue
}

/// `engine.tick`: engine_world::tick, a new frame and its budget.
pub fn on_frame(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let now = Stamp::now(thread);
    TABLE.with(thread, |t| t.frame = Some(Frame::new(now)));
    Flow::Continue
}

/// `game.exit_start`: game::exit, every pending bake finished (waiting) and
/// every postponed query made again, here when they are this thread's, else
/// at their thread's next pass; new bakes the stock path.
pub fn on_exit(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let (in_flight, postponed) = TABLE.with(thread, |t| {
        t.mode = Mode::Exiting;
        (t.in_flight(), t.postponed.len())
    });
    if in_flight + postponed > 0 {
        line(&[Piece::Text("bake_async: the game exits: bakes on the GPU "), Piece::Dec(in_flight as u32),
               Piece::Text(", postponed "), Piece::Dec(postponed as u32)]);
    }
    pass(thread);
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bake_on_the_gpu_ends_when_done_late_or_at_exit() {
        assert_eq!(step(Mode::Playing, Seen::Done), Step::Finish);
        assert_eq!(step(Mode::Exiting, Seen::Done), Step::Finish);
        assert_eq!(step(Mode::Playing, Seen::Busy { waited_ms: 16.0 }), Step::Wait);
        assert_eq!(step(Mode::Playing, Seen::Busy { waited_ms: 999.9 }), Step::Wait);
        assert_eq!(step(Mode::Playing, Seen::Busy { waited_ms: LATE_MS }), Step::FinishLate);
        assert_eq!(step(Mode::Exiting, Seen::Busy { waited_ms: 0.0 }), Step::FinishAtExit);
    }

    #[test]
    fn a_postponed_query_comes_again_when_there_is_room_or_at_exit() {
        assert_eq!(requery(Room::Free), Requery::Now);
        assert_eq!(requery(Room::Exit), Requery::Now);
        assert_eq!(requery(Room::None), Requery::Wait);
    }

    #[test]
    fn a_pack_keeps_its_values_at_the_front() {
        let mut pack: Pack<u32, 3> = Pack::new();
        assert_eq!((pack.push(1), pack.push(2), pack.push(3), pack.push(4)), (Ok(()), Ok(()), Ok(()), Err(4)));
        assert_eq!(pack.remove(0), Some(1));
        assert_eq!(pack.iter().copied().collect::<std::vec::Vec<_>>(), [3, 2]);
        assert_eq!(pack.remove(1), Some(2));
        assert_eq!(pack.remove(0), Some(3));
        assert_eq!((pack.len(), pack.remove(0)), (0, None));
    }

    #[test]
    fn numbers_age_across_the_wrap() {
        let old = Number(u32::MAX);
        let next = old.after().after();
        assert_eq!(next, Number(1));
        assert_eq!(old.age(next), 2);
        assert!(old.age(next) > Number(0).age(next));
    }
}
