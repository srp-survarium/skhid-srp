//! Patch 121: the camouflage/decal bake does not wait for the GPU
//! (docs/notes/baked-textures.md; client_patch.md 3.57, "The bake's GPU
//! round trips").
//!
//! Stock, a bake read its target back, uploaded it again and read each of
//! its ten compressed levels back, each time waiting for the GPU on the
//! game's thread (the hitch of a player joining with an item nobody baked
//! yet). Here (vostok::bake_gpu does the work):
//! - `on_issue`: the bake's target is copied on the GPU into the
//!   compressor's level 0, the levels are drawn and copied to stagings with
//!   no wait, the last copy flushed, and process_compression ends, its query
//!   not finished (patch 70 keeps its waiting queries waiting). No DDS
//!   memory is held while the GPU has the bake. At most IN_FLIGHT bakes are
//!   on the GPU: with every place taken, the oldest is completed first, on
//!   the spot (read back, waiting only when the GPU has not done it, and its
//!   tail run), and the new bake takes its place. A bake never waits for a
//!   place and is never asked for again.
//! - `on_dispatched`, each resources dispatch on the bakes' thread: the
//!   oldest bakes whose last staging maps without waiting are completed
//!   (their DDS allocated then, MT memory) by process_compression's own
//!   tail (`on_resume`): the texture, its sharing, its save (`on_save`,
//!   `on_free`: the descriptor owns the DDS, no copy), its record and the
//!   "camouflage/decal cook" line, while the frame's BUDGET lasts. One the
//!   GPU has had LATE is read back waiting, and logged. That is when the
//!   bakes' thread ticks frames (it comes back every frame). A thread that
//!   ticks none may not dispatch again for minutes (measured 2026-11-09:
//!   bakes left on the GPU a minute, until new ones came), so its bakes all
//!   complete before its dispatch ends, waiting if the GPU has not done them.
//! - `on_frame`, engine_world::tick: a new frame, a new budget, and the
//!   thread that ticks frames.
//! - `on_exit`, game::exit's start: every bake on the GPU completed,
//!   waiting; after it, bakes take the stock path.
//!
//! A bake whose levels cannot be read back (the device lost), that finds no
//! MT memory, whose level 0 finds no GPU memory, or whose compressor does
//! not fit its DDS fails its query, as a bake that fails does. Textures
//! only: nothing a match computes changes.

#![forbid(unsafe_code)]

use crate::vostok::bake_gpu::{self, CaptureError, Completion, CompressError, IssueFailed, Issued, Ms, Outcome,
                              Progress, ReadError, Stamp};
use crate::vostok::log::{line, parts, Limited, Piece};
use crate::vostok::{BakeAsyncOnIssueExit, BakeAsyncOnReadbackExit, BakeAsyncOnResumeExit, BakeAsyncOnSaveExit,
                    Context, Flow, Lock, NoExit, Thread, ThreadBound, ThreadId};

/// Bakes on the GPU at once (their stagings, up to 5.6 MB each).
const IN_FLIGHT: usize = 8;
/// A bake on the GPU this long is read back waiting.
const LATE: Ms = Ms::new(1000.0);
/// A frame's time for completing bakes (each makes its texture from its
/// DDS, runs its CRC and saves it); the rest complete in the next frames,
/// oldest first. A frame's first pass always completes one done bake.
const BUDGET: Ms = Ms::new(6.0);
/// A frame engine_world::tick has not ended in this long counts as ended
/// (apc's wait loops dispatch with no tick): its budget starts again.
const FRAME: Ms = Ms::new(100.0);

// ---- the places ------------------------------------------------------------------

/// A place on the GPU.
enum Slot {
    Free,
    /// A hook of the places' thread holds it ([`Ticket`]): a bake being
    /// issued into it, or one out of it being polled or completed.
    Held,
    Bake(Issued),
}

/// The GPU's places, one thread's (its immediate context does their work).
struct Places([Slot; IN_FLIGHT]);

impl Places {
    const fn new() -> Places {
        Places([const { Slot::Free }; IN_FLIGHT])
    }

    fn bakes(&self) -> usize {
        self.0.iter().filter(|slot| matches!(slot, Slot::Bake(_))).count()
    }

    fn empty(&self) -> bool {
        self.0.iter().all(|slot| matches!(slot, Slot::Free))
    }

    /// A free place, held.
    fn hold_free(&mut self) -> Option<Hold> {
        let at = self.0.iter().position(|slot| matches!(slot, Slot::Free))?;
        self.0[at] = Slot::Held;
        Some(Hold { at })
    }

    /// The oldest bake, out of its place (held).
    fn take_oldest(&mut self) -> Option<(Hold, Issued)> {
        let at = self.0.iter().enumerate()
            .filter_map(|(at, slot)| match slot {
                Slot::Bake(bake) => Some((at, bake.since())),
                Slot::Free | Slot::Held => None,
            })
            .min_by_key(|&(_, since)| since)?.0;
        match core::mem::replace(&mut self.0[at], Slot::Held) {
            Slot::Bake(bake) => Some((Hold { at }, bake)),
            other => {
                self.0[at] = other;
                None
            }
        }
    }

    /// A held place filled; the bake given back when the place is not held.
    fn fill(&mut self, hold: Hold, bake: Issued) -> Result<(), Issued> {
        match &mut self.0[hold.at] {
            slot @ Slot::Held => {
                *slot = Slot::Bake(bake);
                Ok(())
            }
            Slot::Free | Slot::Bake(_) => Err(bake),
        }
    }

    /// A held place let go.
    fn free(&mut self, hold: Hold) {
        if let slot @ Slot::Held = &mut self.0[hold.at] {
            *slot = Slot::Free;
        }
    }
}

/// A place a hook holds ([`Slot::Held`]): the table gives it, and takes it
/// back by fill or free.
#[derive(Clone, Copy)]
struct Hold {
    at: usize,
}

// ---- the table ---------------------------------------------------------------------

/// The game's state as the patch sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Playing,
    /// game::exit started: bakes on the GPU complete, waiting; new ones are stock.
    Exiting,
}

/// Whether a completion is under way (a pass, or a bake completed on the
/// spot): one at a time, so none starts inside another's tail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Work {
    Idle,
    Completing,
}

/// A frame's budget: when it began (engine_world::tick, or a pass that
/// found none for FRAME) and the time spent completing bakes in it.
#[derive(Clone, Copy, Debug)]
struct Frame {
    started: Stamp,
    spent: Ms,
}

impl Frame {
    fn new(now: Stamp) -> Frame {
        Frame { started: now, spent: Ms::default() }
    }
}

/// Where a bake goes at its issue.
enum Admit {
    /// The free place it holds.
    Place(Hold),
    /// Every place taken: the oldest bake out, to complete first, and its
    /// place held for this one.
    AfterOldest(Hold, Issued),
    /// The stock bake.
    Stock(Why),
}

/// Why a bake takes the stock path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Why {
    /// game::exit started.
    Exiting,
    /// The GPU's places are another thread's (the immediate context is one
    /// thread's).
    Thread(ThreadId),
    /// Every place taken while a completion is under way (a bake asked for
    /// inside another's tail).
    Busy,
}

/// Who has bakes on the GPU.
enum OnGpu {
    Nobody,
    Me(usize),
    Thread(ThreadId),
}

/// What a dispatch's pass does.
enum Start {
    /// It looks at this thread's bakes so.
    Pass(Look),
    /// Nothing: no bake of this thread's on the GPU, or a completion under way.
    Nothing,
    /// The GPU's bakes are this thread's: only it completes them.
    Theirs(ThreadId),
}

/// The owner of the pending bakes: the GPU's places (one thread's while it
/// has any), the thread that ticks frames, the frame's budget, the
/// completion under way, the mode. Every change of a place is a method here.
struct Table {
    gpu: Option<ThreadBound<Places>>,
    /// The thread engine_world::tick last ran on (none before a frame).
    ticker: Option<ThreadId>,
    frame: Option<Frame>,
    work: Work,
    mode: Mode,
}

static TABLE: Lock<Table> =
    Lock::new(Table { gpu: None, ticker: None, frame: None, work: Work::Idle, mode: Mode::Playing });

impl Table {
    /// The one admission: where a bake of this thread goes now.
    fn admit(&mut self, thread: &Thread) -> Admit {
        if self.mode == Mode::Exiting {
            return Admit::Stock(Why::Exiting);
        }
        let bound = self.gpu.get_or_insert_with(|| ThreadBound::new(thread, Places::new()));
        let owner = bound.owner();
        let Some(places) = bound.get_mut(thread) else { return Admit::Stock(Why::Thread(owner)) };
        if let Some(hold) = places.hold_free() {
            return Admit::Place(hold);
        }
        match self.work {
            Work::Idle => match places.take_oldest() {
                Some((hold, oldest)) => {
                    self.work = Work::Completing;
                    Admit::AfterOldest(hold, oldest)
                }
                None => Admit::Stock(Why::Busy),
            },
            Work::Completing => Admit::Stock(Why::Busy),
        }
    }

    fn places(&mut self, thread: &Thread) -> Option<&mut Places> {
        self.gpu.as_mut().and_then(|bound| bound.get_mut(thread))
    }

    /// A held place filled; the bake given back (to let go out of the lock)
    /// when the place is not this thread's held one.
    fn fill(&mut self, thread: &Thread, hold: Hold, bake: Issued) -> Result<(), Issued> {
        match self.places(thread) {
            Some(places) => places.fill(hold, bake),
            None => Err(bake),
        }
    }

    /// A held place let go; with none in use, the GPU is no thread's.
    fn free(&mut self, thread: &Thread, hold: Hold) {
        let Some(places) = self.places(thread) else { return };
        places.free(hold);
        if places.empty() {
            self.gpu = None;
        }
    }

    /// A pass started on this thread: how it looks at its bakes. With
    /// game::exit started, every one ends. A thread that ticks frames comes
    /// back every frame, so its bakes wait for the frame's budget; any other
    /// (or any, before a frame is ticked) may not dispatch again for long,
    /// so its bakes end with its dispatch. Not started while a completion is
    /// under way or the thread has no bake on the GPU; the owner named when
    /// the GPU's bakes are another thread's.
    fn start_pass(&mut self, thread: &Thread) -> Start {
        let me = ThreadId::current(thread);
        let bakes = match &self.gpu {
            None => 0,
            Some(bound) => match bound.get(thread) {
                Some(places) => places.bakes(),
                None => return Start::Theirs(bound.owner()),
            },
        };
        if self.work != Work::Idle || bakes == 0 {
            return Start::Nothing;
        }
        self.work = Work::Completing;
        Start::Pass(match (self.mode, self.ticker) {
            (Mode::Exiting, _) => Look::Exit,
            (Mode::Playing, Some(ticker)) if ticker == me => Look::Pass,
            (Mode::Playing, _) => Look::Drain,
        })
    }

    /// The pass's next bake, the oldest, out of its place: while the frame's
    /// budget lasts (a pass that drains its thread's bakes, or the exit's: always).
    fn next(&mut self, thread: &Thread, now: Stamp, look: Look) -> Option<(Hold, Issued)> {
        let budget = look != Look::Pass || self.frame(now, thread).spent < BUDGET;
        if budget { self.places(thread)?.take_oldest() } else { None }
    }

    /// The time since `started` spent completing a bake, in its frame.
    fn spend(&mut self, thread: &Thread, started: Stamp) {
        let spent = started.elapsed(thread);
        self.frame(started, thread).spent += spent;
    }

    fn end_work(&mut self) {
        self.work = Work::Idle;
    }

    /// The frame's budget at `now`: this frame's, or a new one when none is
    /// or it began FRAME ago.
    fn frame(&mut self, now: Stamp, thread: &Thread) -> &mut Frame {
        let frame = self.frame.filter(|frame| frame.started.ms_to(now, thread) < FRAME);
        self.frame.insert(frame.unwrap_or(Frame::new(now)))
    }

    /// game::exit: who has bakes on the GPU now.
    fn exit(&mut self, thread: &Thread) -> OnGpu {
        self.mode = Mode::Exiting;
        match &self.gpu {
            None => OnGpu::Nobody,
            Some(bound) => match bound.get(thread) {
                Some(places) => OnGpu::Me(places.bakes()),
                None => OnGpu::Thread(bound.owner()),
            },
        }
    }
}

/// A place this hook holds ([`Slot::Held`]): filled with a bake, or freed
/// when dropped. Never dropped inside the table's lock (it takes it).
struct Ticket<'t> {
    thread: &'t Thread,
    hold: Hold,
}

impl Ticket<'_> {
    fn fill(self, bake: Issued) {
        let (thread, hold) = (self.thread, self.hold);
        core::mem::forget(self);
        let refused = TABLE.with(thread, |t| t.fill(thread, hold, bake));
        // let go out of the lock (never: the place is this hook's)
        drop(refused);
    }
}

impl Drop for Ticket<'_> {
    fn drop(&mut self) {
        let (thread, hold) = (self.thread, self.hold);
        TABLE.with(thread, |t| t.free(thread, hold));
    }
}

// ---- the transition ---------------------------------------------------------------

/// Why a bake on the GPU is looked at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Look {
    /// A dispatch's pass on the thread that ticks frames, while the frame's
    /// budget lasts: what it leaves comes back the next frame.
    Pass,
    /// A dispatch's pass on a thread that ticks no frames (or before a
    /// frame): it may not dispatch again for long, so every bake ends now.
    Drain,
    /// The pass after game::exit: every bake ends.
    Exit,
    /// Its place is wanted by a new bake: it ends now.
    Place,
}

/// What is seen of a bake on the GPU.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Seen {
    /// Its last staging maps without waiting: the GPU has done it.
    Done,
    /// The GPU has not, after `waited`.
    Busy { waited: Ms },
}

/// What a bake on the GPU does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    /// Stays (and the ones younger with it: the GPU runs them in order).
    Wait,
    /// Read back and completed.
    Complete,
    /// Read back waiting for the GPU (late: logged), then completed.
    CompleteLate,
    /// Read back waiting for the GPU (draining a thread that ticks no
    /// frames, at the exit, or for its place), then
    /// completed.
    CompleteWaiting,
}

/// A bake on the GPU: the one transition.
fn step(look: Look, seen: Seen) -> Step {
    match (look, seen) {
        (_, Seen::Done) => Step::Complete,
        (Look::Pass, Seen::Busy { waited }) if waited >= LATE => Step::CompleteLate,
        (Look::Pass, Seen::Busy { .. }) => Step::Wait,
        (Look::Drain | Look::Exit | Look::Place, Seen::Busy { .. }) => Step::CompleteWaiting,
    }
}

fn seen(thread: &Thread, bake: &Issued) -> Seen {
    match bake.progress() {
        Progress::Done => Seen::Done,
        Progress::Busy => Seen::Busy { waited: bake.since().elapsed(thread) },
    }
}

// ---- the log ---------------------------------------------------------------------

/// The bakes completed for their place (one line a second, the count between).
static FOR_PLACE: Limited = Limited::new("completed for their place");
/// The bakes that took the stock path while playing.
static STOCK: Limited = Limited::new("stock bakes");
/// The dispatches of a thread whose bakes are another's.
static THEIRS: Limited = Limited::new("such dispatches");
/// The passes that drained a thread that ticks no frames.
static DRAINED: Limited = Limited::new("such passes");

/// A dispatch of a thread that ticks frames while the GPU's bakes are
/// another thread's (only that one completes them, at its own dispatches).
fn log_theirs(thread: &Thread, owner: ThreadId) {
    use Piece::{Hex, Text};
    let me = ThreadId::current(thread);
    if TABLE.with(thread, |t| t.ticker) == Some(me) {
        THEIRS.line(thread, &[Text("bake_async: the GPU's bakes are thread "), Hex(owner.id()),
                              Text("'s, not the frame thread "), Hex(me.id()),
                              Text("'s: they complete at that thread's dispatches")]);
    }
}

/// A pass of a thread that ticks no frames: its bakes completed before its
/// dispatch ends, `waited` of them read back waiting for the GPU.
fn log_drained(thread: &Thread, drained: u32, waited: u32) {
    use Piece::{Dec, Hex, Text};
    let me = ThreadId::current(thread).id();
    let ticker = TABLE.with(thread, |t| t.ticker).map_or(0, ThreadId::id);
    DRAINED.line(thread, &[Text("bake_async: thread "), Hex(me), Text(" ticks no frames (the frame thread "),
                           Hex(ticker), Text("): its "), Dec(drained), Text(" bakes completed at its dispatch's end, "),
                           Dec(waited), Text(" read back waiting")]);
}


/// What a completion that did not end well says.
fn log_completion(completion: &Completion) {
    use Piece::{Dec, Text};
    let name = completion.name.as_str();
    let head: &[Piece] = &[Text("bake_async: "), Text(name)];
    let size = completion.size.bytes();
    match completion.outcome {
        Outcome::Saved => {}
        Outcome::Unsaved => parts(&[head, &[Text(": no memory for its save; used, not saved")]]),
        Outcome::NoSave => parts(&[head, &[Text(": its tail did not reach the save")]]),
        Outcome::Failed(ReadError::NoMemory) => {
            parts(&[head, &[Text(": no MT memory for its DDS of "), Dec(size), Text("; failed")]])
        }
        Outcome::Failed(ReadError::Lost) => parts(&[head, &[
            Text(": its levels could not be read back (the device?); failed, size "), Dec(size)]]),
    }
}

/// What a compressor's failure says.
fn log_failed(failed: &IssueFailed) {
    use Piece::{Dec, Text};
    let head: &[Piece] = &[Text("bake_async: "), Text(failed.job.name())];
    let tail: &[Piece] = &[Text("; failed")];
    match failed.error {
        CompressError::NoDevice => parts(&[head, &[Text(": no render device")], tail]),
        CompressError::Levels(n) => parts(&[head, &[Text(": the compressor gives "), Dec(n), Text(" levels")], tail]),
        CompressError::Size { data, size } => parts(&[head, &[
            Text(": its levels' "), Dec(data), Text(" bytes do not fit its DDS of "), Dec(size.bytes())], tail]),
        CompressError::Busy => parts(&[head, &[Text(": another compress is being captured")], tail]),
        CompressError::Level0 => parts(&[head, &[Text(": no texture for its level 0 (the GPU's memory?)")], tail]),
        CompressError::Capture(CaptureError::Site) => {
            parts(&[head, &[Text(": a level's staging or cursor missing")], tail])
        }
        CompressError::Capture(CaptureError::TooMany) => {
            parts(&[head, &[Text(": more levels than a texture has")], tail])
        }
        CompressError::Capture(CaptureError::Fit { offset, size }) => parts(&[head, &[
            Text(": a level at "), Dec(offset.bytes()), Text(" not inside its DDS of "), Dec(size.bytes())], tail]),
        CompressError::End { end, size } => parts(&[head, &[
            Text(": the levels end at "), Dec(end.bytes()), Text(", past its DDS of "), Dec(size.bytes())], tail]),
    }
}

/// A bake on the stock path while playing, and why.
fn log_stock(thread: &Thread, why: Why) {
    use Piece::{Hex, Text};
    let me = ThreadId::current(thread).id();
    match why {
        Why::Exiting => {}
        Why::Thread(owner) => STOCK.line(thread, &[Text("bake_async: a bake of thread "), Hex(me),
                                                   Text(" while the GPU has thread "), Hex(owner.id()),
                                                   Text("'s: the stock bake")]),
        Why::Busy => STOCK.line(thread, &[Text("bake_async: a bake inside another's tail, every place in use: the stock \
                                                bake")]),
    }
}

// ---- the hooks -------------------------------------------------------------------------

/// A bake completed on this thread, logged when it did not end well;
/// given back, untouched, while another completion is under way.
fn complete(thread: &Thread, bake: Issued) -> Result<(), Issued> {
    let completion = bake.complete(thread)?;
    log_completion(&completion);
    Ok(())
}

/// The oldest bake completed for its place (every place taken), waiting
/// when the GPU has not done it; given back when it cannot be now.
fn complete_for_place(thread: &Thread, oldest: Issued) -> Result<(), Issued> {
    let started = Stamp::now(thread);
    let step = step(Look::Place, seen(thread, &oldest));
    let completed = complete(thread, oldest);
    TABLE.with(thread, |t| {
        t.spend(thread, started);
        t.end_work();
    });
    completed?;
    use Piece::{Dec, Text};
    let how = if step == Step::CompleteWaiting { "read back waiting" } else { "the GPU had done it" };
    FOR_PLACE.line(thread, &[Text("bake_async: the GPU's "), Dec(IN_FLIGHT as u32),
                             Text(" places taken: the oldest completed for a new bake ("), Text(how), Text(", ms "),
                             Dec(started.elapsed(thread).whole()), Text(")")]);
    Ok(())
}

/// `bake.issue`: the bake handed to the GPU (process_compression left by its
/// epilogue, its query not finished), in a free place or in the oldest's,
/// completed first; failed when its compressor does not fit its DDS or its
/// level 0 finds no GPU memory. The stock bake when the game exits, the
/// GPU has another thread's bakes, or the bake is not one this can read.
pub fn on_issue(ctx: &mut Context<'_>) -> Flow<BakeAsyncOnIssueExit> {
    let ctx: &Context<'_> = ctx;
    let thread = ctx.thread();
    let Some(issue) = bake_gpu::Issue::at(ctx) else { return Flow::Continue };
    let ticket = match TABLE.with(thread, |t| t.admit(thread)) {
        Admit::Place(hold) => Ticket { thread, hold },
        Admit::AfterOldest(hold, oldest) => {
            let ticket = Ticket { thread, hold };
            if let Err(oldest) = complete_for_place(thread, oldest) {
                ticket.fill(oldest);
                log_stock(thread, Why::Busy);
                return Flow::Continue;
            }
            ticket
        }
        Admit::Stock(why) => {
            log_stock(thread, why);
            return Flow::Continue;
        }
    };
    let Some(bake) = issue.take() else {
        line(&[Piece::Text("bake_async: a bake whose frame this does not read; the stock bake")]);
        return Flow::Continue;
    };
    match bake.issue() {
        Ok(issued) => ticket.fill(issued),
        Err(failed) => {
            drop(ticket);
            log_failed(&failed);
            failed.job.fail();
        }
    }
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

/// One pass of this thread's bakes, oldest first: each the GPU has done
/// completed while the frame's budget lasts, a busy one ending the pass
/// unless it is late; on a thread that ticks no frames, or at the exit,
/// every one, waiting.
fn pass(thread: &Thread) {
    let look = match TABLE.with(thread, |t| t.start_pass(thread)) {
        Start::Pass(look) => look,
        Start::Nothing => return,
        Start::Theirs(owner) => return log_theirs(thread, owner),
    };
    let (mut drained, mut waited) = (0u32, 0u32);
    loop {
        let now = Stamp::now(thread);
        let Some((hold, bake)) = TABLE.with(thread, |t| t.next(thread, now, look)) else { break };
        let ticket = Ticket { thread, hold };
        let step = step(look, seen(thread, &bake));
        match step {
            Step::Wait => {
                ticket.fill(bake);
                break;
            }
            Step::CompleteLate => {
                parts(&[&[Piece::Text("bake_async: "), Piece::Text(bake.name()),
                          Piece::Text(": the GPU had it 1 s; read back waiting, ms "),
                          Piece::Dec(bake.since().elapsed(thread).whole())]]);
            }
            Step::Complete | Step::CompleteWaiting => {}
        }
        if let Err(bake) = complete(thread, bake) {
            ticket.fill(bake);
            break;
        }
        drop(ticket);
        TABLE.with(thread, |t| t.spend(thread, now));
        drained += 1;
        waited += u32::from(step == Step::CompleteWaiting);
    }
    TABLE.with(thread, Table::end_work);
    if look == Look::Drain && drained > 0 {
        log_drained(thread, drained, waited);
    }
}

/// `resources.dispatched`: each dispatch, a pass for the thread's bakes.
pub fn on_dispatched(ctx: &mut Context<'_>) -> Flow<NoExit> {
    pass(ctx.thread());
    Flow::Continue
}

/// `engine.tick`: engine_world::tick, a new frame and its budget, and the
/// thread that ticks frames (its bakes wait for the next frame's budget).
pub fn on_frame(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let now = Stamp::now(thread);
    let me = ThreadId::current(thread);
    TABLE.with(thread, |t| {
        t.frame = Some(Frame::new(now));
        t.ticker = Some(me);
    });
    Flow::Continue
}

/// `game.exit_start`: game::exit, every bake on the GPU completed (waiting),
/// here when they are this thread's, else at their thread's next pass; new
/// bakes the stock path.
pub fn on_exit(ctx: &mut Context<'_>) -> Flow<NoExit> {
    use Piece::{Dec, Hex, Text};
    let thread = ctx.thread();
    match TABLE.with(thread, |t| t.exit(thread)) {
        OnGpu::Nobody | OnGpu::Me(0) => {}
        OnGpu::Me(n) => line(&[Text("bake_async: the game exits: bakes on the GPU "), Dec(n as u32)]),
        OnGpu::Thread(owner) => {
            line(&[Text("bake_async: the game exits: the GPU has the bakes of thread "), Hex(owner.id()),
                   Text(", completed at its next pass")])
        }
    }
    pass(thread);
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bake_on_the_gpu_ends_when_done_late_at_exit_or_for_its_place() {
        let busy = |ms| Seen::Busy { waited: Ms::new(ms) };
        for look in [Look::Pass, Look::Drain, Look::Exit, Look::Place] {
            assert_eq!(step(look, Seen::Done), Step::Complete);
        }
        assert_eq!(step(Look::Pass, busy(16.0)), Step::Wait);
        assert_eq!(step(Look::Pass, busy(999.9)), Step::Wait);
        assert_eq!(step(Look::Pass, busy(1000.0)), Step::CompleteLate);
        assert_eq!(step(Look::Drain, busy(0.0)), Step::CompleteWaiting);
        assert_eq!(step(Look::Exit, busy(0.0)), Step::CompleteWaiting);
        assert_eq!(step(Look::Place, busy(0.0)), Step::CompleteWaiting);
    }
}
