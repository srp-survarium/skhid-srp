//! Patch 83's link to the DLSS helper (docs/notes/dlss-option.md, "The helper
//! interface" and "Falling back"): the one owner of the helper's process, the
//! pipe to it and what is shared with it, as one state ([`Link`]) with one
//! transition ([`Link::step`]); the Settings' gate of the DLSS checkbox
//! ([`DlssOffer`]) is read from that state.
//!
//! - The process's start decides whether anything starts ([`at_start`]): with
//!   DXVK, an NVIDIA GPU and the helper's files, the helper is asked at every
//!   start, with the tick on or off: its answer is what offers the checkbox.
//! - The first Present starts the helper and its watch; the Hello goes out
//!   once the helper has opened the pipe, with the output's real size. A "yes"
//!   with the tick off ends there (offered, the helper told to quit); with the
//!   tick on the images are made and shared (the Setup), and each Present
//!   after the Setup's answer submits a frame.
//! - A Present of another size tells the helper (a new size), retires what
//!   was shared and says Hello again.
//! - Any failure stops the link for the process: the helper ended, the
//!   timeline released, what was shared retired, the reason in
//!   `srp_patches.log`; the game's frame is its own all along (nothing reads
//!   the helper's output yet: docs/notes/dlss-option.md, "Status").
//! - The helper's watch ([`watch`]) runs on a thread of its own: DXVK's queue
//!   waits for a value only the helper signals, and the game's thread may
//!   wait for that queue (a Present), so the helper's end is seen there and
//!   the timeline signalled from there.

use core::ffi::CStr;

use super::{DLSS, DXVK_MARKER, Renderer, renderer};
use crate::vostok::dlss::{self, BackBuffer, Device, Failure, Frame, Hello, Helper, HelperWatch, Refusal, SLOTS, Shared, Size, SwapChain, Tick,
                          Watched};
use crate::vostok::log::{Piece, Piece::*, line, line_of};
use crate::vostok::settings::{file_exists, save_user_config};
use crate::vostok::renderer::environment_set;
use crate::vostok::{GameCell, Lock, Thread, hud_options_on_watch_entry};

/// Why nothing starts in this process.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotRun {
    /// The process's start has not decided (DirectX 11 never does).
    Undecided,
    /// DXVK's GPU is not NVIDIA's.
    NotNvidia,
    /// The helper's files are not in `dlss\`.
    NoFiles,
    /// The game has exited.
    Exited,
}

impl NotRun {
    /// Whether the helper can be asked at all: None when it can.
    pub fn of(nvidia: bool, files: bool) -> Option<NotRun> {
        match (nvidia, files) {
            (false, _) => Some(NotRun::NotNvidia),
            (true, false) => Some(NotRun::NoFiles),
            (true, true) => None,
        }
    }
}

/// What the helper is started for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Purpose {
    /// The tick is off: only its answer, for the checkbox.
    Ask,
    /// The tick is on: DLSS.
    Run,
    /// DLSS at a new output size: the helper said yes before.
    Resize,
}

/// The link's state. `H`, `D` and `S` are the helper, DXVK's device and what
/// is shared (vostok's; a test's own).
#[derive(Debug, PartialEq)]
pub enum Link<H, D, S> {
    /// Not started, and nothing will.
    Off(NotRun),
    /// Not started: the first Present starts the helper.
    Waiting(Purpose),
    /// The helper's process runs; it has not opened the pipe yet.
    Starting { helper: H, device: D, purpose: Purpose, since: Tick },
    /// The Hello is sent, for this output.
    Asked { helper: H, device: D, purpose: Purpose, output: Size, since: Tick },
    /// The helper said DLSS runs here, and the tick is off: nothing runs.
    Offered,
    /// The helper said yes, for this output: the images are being made.
    Sharing { helper: H, device: D, output: Size },
    /// The Setup is sent.
    Setup { helper: H, device: D, shared: S, output: Size, since: Tick },
    /// Each Present of this output submits a frame, this one next.
    Running { helper: H, device: D, shared: S, output: Size, frame: Frame },
    /// It failed before the helper said yes.
    Failed(Failure),
    /// It failed after the helper said yes.
    Stopped(Failure),
}

#[derive(Debug, PartialEq)]
pub enum Event<H, D, S> {
    /// A Present, with the output's size.
    Frame { output: Size, now: Tick },
    /// The helper's process and its watch run.
    Started { helper: H, device: D, now: Tick },
    /// The helper opened the pipe and the Hello for this output went out.
    HelloSent { output: Size, now: Tick },
    /// The Hello's answer: the render size, or no.
    HelloAnswered(Result<Size, Refusal>),
    /// The output is no more the Hello's: the helper was told a new size.
    OutputChanged { now: Tick },
    /// The images are made and the Setup went out.
    SetupSent { shared: S, output: Size, now: Tick },
    SetupAnswered(Result<(), Refusal>),
    /// The state's frame is on DXVK's queue.
    Submitted,
    /// The helper ended, the pipe broke, or a call of DXVK's device failed.
    Broken(Failure),
    /// The game exits.
    Exit,
}

#[derive(Debug, PartialEq)]
pub enum Action<H, D, S> {
    Nothing,
    /// Start the helper and its watch.
    Start,
    /// Send the Hello for this output, if the pipe is open.
    SayHello(Size),
    /// Read the Hello's answer, if it has come.
    AwaitHello,
    /// Make the images for this render size and send the Setup; the output
    /// the Hello was for.
    Share { render: Size, output: Size },
    AwaitSetup,
    /// Submit this frame, if its command buffer is free.
    Submit(Frame),
    /// Tell the helper a new size comes and retire what was shared.
    Resize(S),
    /// The answer is all that was wanted: the helper told to quit.
    Dismiss { helper: H, device: D },
    /// Say why nothing started.
    Report { why: Failure, purpose: Purpose },
    /// End the helper, release the timeline, retire what was shared, say why.
    Stop { helper: H, device: D, shared: Option<S>, why: Failure, purpose: Purpose },
    /// The same at the game's exit, the helper given its time to quit.
    Quit { helper: H, device: D, shared: Option<S> },
}

/// How long a helper that runs may take to open the pipe, to answer the
/// Hello (NGX's start: about 1 s measured) and to answer the Setup. A timer,
/// because a helper that hangs gives no event; its end and a broken pipe are
/// events and do not wait for it.
pub const ANSWER_WAIT: u32 = 30_000;

/// What a state holds that an end must end.
struct Parts<H, D, S> {
    helper: H,
    device: D,
    shared: Option<S>,
    purpose: Purpose,
    /// The helper has said DLSS runs here.
    offered: bool,
}

impl<H, D, S> Link<H, D, S> {
    fn parts(self) -> Result<Parts<H, D, S>, Self> {
        match self {
            Link::Starting { helper, device, purpose, .. } | Link::Asked { helper, device, purpose, .. } => {
                Ok(Parts { helper, device, shared: None, purpose, offered: purpose == Purpose::Resize })
            }
            Link::Sharing { helper, device, .. } => Ok(Parts { helper, device, shared: None, purpose: Purpose::Run, offered: true }),
            Link::Setup { helper, device, shared, .. } | Link::Running { helper, device, shared, .. } => {
                Ok(Parts { helper, device, shared: Some(shared), purpose: Purpose::Run, offered: true })
            }
            other => Err(other),
        }
    }

    /// Since when a state waits for the helper, and for what.
    fn waits(&self) -> Option<(Tick, &'static str)> {
        match self {
            Link::Starting { since, .. } => Some((*since, "it did not open the pipe")),
            Link::Asked { since, .. } => Some((*since, "no answer to the Hello")),
            Link::Setup { since, .. } => Some((*since, "no answer to the Setup")),
            _ => None,
        }
    }

    /// The one transition.
    pub fn step(self, event: Event<H, D, S>) -> (Self, Action<H, D, S>) {
        match (self, event) {
            (state, Event::Exit) => match state.parts() {
                Ok(Parts { helper, device, shared, .. }) => (Link::Off(NotRun::Exited), Action::Quit { helper, device, shared }),
                Err(state) => (state, Action::Nothing),
            },
            (state, Event::Broken(why)) => match state.parts() {
                Ok(Parts { helper, device, shared, purpose, offered }) => {
                    (if offered { Link::Stopped(why) } else { Link::Failed(why) }, Action::Stop { helper, device, shared, why, purpose })
                }
                Err(Link::Waiting(purpose)) => (Link::Failed(why), Action::Report { why, purpose }),
                Err(state) => (state, Action::Nothing),
            },
            (state, Event::Frame { now, .. }) if state.waits().is_some_and(|(since, _)| now.since(since) > ANSWER_WAIT) => {
                let what = state.waits().map_or("", |(_, what)| what);
                state.step(Event::Broken(Failure::Late(what)))
            }
            (state @ Link::Waiting(_), Event::Frame { .. }) => (state, Action::Start),
            (Link::Waiting(purpose), Event::Started { helper, device, now }) => {
                (Link::Starting { helper, device, purpose, since: now }, Action::Nothing)
            }
            (state @ Link::Starting { .. }, Event::Frame { output, .. }) => (state, Action::SayHello(output)),
            (Link::Starting { helper, device, purpose, .. }, Event::HelloSent { output, now }) => {
                (Link::Asked { helper, device, purpose, output, since: now }, Action::Nothing)
            }
            (state @ Link::Asked { .. }, Event::Frame { .. }) => (state, Action::AwaitHello),
            (Link::Asked { helper, device, purpose: Purpose::Ask, .. }, Event::HelloAnswered(Ok(_))) => {
                (Link::Offered, Action::Dismiss { helper, device })
            }
            (Link::Asked { helper, device, output, .. }, Event::HelloAnswered(Ok(render))) => {
                (Link::Sharing { helper, device, output }, Action::Share { render, output })
            }
            (state @ Link::Asked { .. }, Event::HelloAnswered(Err(refusal))) => state.step(Event::Broken(Failure::Refused(refusal))),
            (Link::Sharing { helper, device, .. }, Event::OutputChanged { now }) => {
                (Link::Starting { helper, device, purpose: Purpose::Resize, since: now }, Action::Nothing)
            }
            (Link::Sharing { helper, device, .. }, Event::SetupSent { shared, output, now }) => {
                (Link::Setup { helper, device, shared, output, since: now }, Action::Nothing)
            }
            (state @ Link::Setup { .. }, Event::Frame { .. }) => (state, Action::AwaitSetup),
            (Link::Setup { helper, device, shared, output, .. }, Event::SetupAnswered(Ok(()))) => {
                (Link::Running { helper, device, shared, output, frame: Frame(0) }, Action::Nothing)
            }
            (state @ Link::Setup { .. }, Event::SetupAnswered(Err(refusal))) => state.step(Event::Broken(Failure::SetupRefused(refusal))),
            (Link::Running { helper, device, shared, output, frame }, Event::Frame { output: now_output, now }) => {
                if now_output == output {
                    (Link::Running { helper, device, shared, output, frame }, Action::Submit(frame))
                } else {
                    (Link::Starting { helper, device, purpose: Purpose::Resize, since: now }, Action::Resize(shared))
                }
            }
            (Link::Running { helper, device, shared, output, frame }, Event::Submitted) => {
                (Link::Running { helper, device, shared, output, frame: frame.next() }, Action::Nothing)
            }
            // an event that is not this state's (an answer after a stop): nothing
            (state, _) => (state, Action::Nothing),
        }
    }

    /// The helper and DXVK's device of a state that has them.
    fn ends(&self) -> Option<(&H, &D)> {
        match self {
            Link::Starting { helper, device, .. }
            | Link::Asked { helper, device, .. }
            | Link::Sharing { helper, device, .. }
            | Link::Setup { helper, device, .. }
            | Link::Running { helper, device, .. } => Some((helper, device)),
            _ => None,
        }
    }

    fn shared(&self) -> Option<&S> {
        match self {
            Link::Setup { shared, .. } | Link::Running { shared, .. } => Some(shared),
            _ => None,
        }
    }

    /// Whether a Present has nothing to do for this state.
    fn rests(&self) -> bool {
        matches!(self, Link::Off(_) | Link::Offered | Link::Failed(_) | Link::Stopped(_))
    }
}

/// Whether this process offers DLSS, or why not: DLSS runs only in its
/// 64-bit helper, on DXVK's GPU, an NVIDIA one (docs/notes/dlss-option.md,
/// "The option"). Not offered, its checkbox is greyed in the Settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DlssOffer {
    /// The helper said at this start that DLSS runs here.
    Offered,
    /// DirectX 11 runs: no Vulkan device to share the frame from.
    NoDxvk,
    /// DXVK's GPU is not NVIDIA's.
    NotNvidia,
    /// The helper's files are not there.
    NoFiles,
    /// The helper has not answered yet.
    NotYet,
    /// The helper did not start, said no, or ended before it answered.
    Failed(Failure),
}

impl DlssOffer {
    /// From this process's renderer and the link's state.
    pub fn of<H, D, S>(dxvk: bool, link: &Link<H, D, S>) -> DlssOffer {
        match link {
            _ if !dxvk => DlssOffer::NoDxvk,
            Link::Off(NotRun::NotNvidia) => DlssOffer::NotNvidia,
            Link::Off(NotRun::NoFiles) => DlssOffer::NoFiles,
            Link::Off(NotRun::Undecided | NotRun::Exited) | Link::Waiting(Purpose::Ask | Purpose::Run) => DlssOffer::NotYet,
            Link::Starting { purpose, .. } | Link::Asked { purpose, .. } if *purpose != Purpose::Resize => DlssOffer::NotYet,
            Link::Failed(why) => DlssOffer::Failed(*why),
            Link::Waiting(Purpose::Resize)
            | Link::Starting { .. }
            | Link::Asked { .. }
            | Link::Offered
            | Link::Sharing { .. }
            | Link::Setup { .. }
            | Link::Running { .. }
            | Link::Stopped(_) => DlssOffer::Offered,
        }
    }

    /// This process's, now.
    pub fn now(thread: &Thread) -> DlssOffer {
        let dxvk = matches!(renderer(thread), Renderer::Dxvk(_));
        LINK.with(thread, |link| DlssOffer::of(dxvk, link))
    }
}

/// The link as the game has it.
type Live = Link<Helper, Device, Shared>;

/// A lock, as `RENDERER`'s: every user is a hook on the game's main thread
/// (the start, each Present, the Settings, the login screen, the exit).
static LINK: Lock<Live> = Lock::new(Link::Off(NotRun::Undecided));

/// The link's progress lines, a local diagnostic: off for players, on in a
/// game started with `SKHID_DLSS_LOG` set (read once, at the start).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Progress {
    Unsaid,
    /// A line at every frame whose number is a multiple of [`PROGRESS_EVERY`].
    Said,
}

static PROGRESS: GameCell<Progress> = GameCell::new(Progress::Unsaid);
const PROGRESS_EVERY: u64 = 300;

/// DXVK's mip bias with DLSS at Quality, `d3d11.samplerLodBias`:
/// log2(render / output) - 1 with the render size 1 / 1.5 of the output's
/// each way. It depends on the mode alone, so the start sets it without the
/// helper's answer (DXVK reads its options before the first frame).
pub const MIP_BIAS: &CStr = c"-1.585";

/// The process's start with DXVK: whether the helper will be asked, from
/// DXVK's GPU and the helper's files, and what for, from the tick. Whether
/// DLSS is to run (the mip bias goes with it).
pub fn at_start(thread: &Thread, nvidia: bool, files: bool, tick: bool) -> bool {
    let purpose = if tick { Purpose::Run } else { Purpose::Ask };
    let link = NotRun::of(nvidia, files).map_or(Link::Waiting(purpose), Link::Off);
    match &link {
        Link::Off(NotRun::NotNvidia) if tick => line(&[Text("dlss: on, not run: DXVK's GPU is not NVIDIA's")]),
        Link::Off(NotRun::NoFiles) if tick => line(&[Text("dlss: on, not run: the helper's files are not in dlss\\")]),
        _ => {}
    }
    let runs = matches!(link, Link::Waiting(Purpose::Run));
    if runs && environment_set(thread, c"SKHID_DLSS_LOG") {
        PROGRESS.set(thread, Progress::Said);
    }
    LINK.with(thread, |state| *state = link);
    runs
}

/// What a Present gives the link.
struct Present {
    chain: SwapChain,
    back: BackBuffer,
}

/// A Present: the link's step for it.
pub fn on_present(thread: &Thread, chain: SwapChain) {
    if LINK.with(thread, |link| link.rests()) {
        return;
    }
    match chain.back_buffer(thread) {
        Ok(back) => drive(thread, Some(&Present { chain, back }), Event::Frame { output: back.size, now: Tick::now(thread) }),
        Err(why) => drive(thread, None, Event::Broken(why)),
    }
}

/// The game's exit: the helper told to quit and everything of the link ended.
pub fn on_exit(thread: &Thread) {
    drive(thread, None, Event::Exit);
}

/// The login screen is shown (user.cfg has run): a tick the helper refused
/// before it goes off now.
pub fn on_login(thread: &Thread) {
    if LINK.with(thread, |link| matches!(link, Link::Failed(Failure::Refused(_)))) {
        tick_off(thread);
    }
}

/// `event` through [`Link::step`], its action done; an action's outcome is
/// the next event.
fn drive(thread: &Thread, present: Option<&Present>, event: Event<Helper, Device, Shared>) {
    LINK.with(thread, |link| {
        let mut event = Some(event);
        while let Some(now) = event.take() {
            let (next, action) = core::mem::replace(link, Link::Off(NotRun::Undecided)).step(now);
            *link = next;
            event = act(thread, link, present, action);
        }
    });
}

/// Whether a frame's command buffer is free: the timeline is past the copy
/// of the frame that used it last.
pub fn free(frame: Frame, reached: u64) -> bool {
    frame.0 < SLOTS || reached >= Frame(frame.0 - SLOTS).copied()
}

/// An action done on the state it left: the event that follows, if any.
fn act(thread: &Thread, link: &Live, present: Option<&Present>, action: Action<Helper, Device, Shared>) -> Option<Event<Helper, Device, Shared>> {
    let now = Tick::now(thread);
    // an action on the helper first sees whether its process still runs
    let ends = || {
        let (helper, device) = link.ends()?;
        Some(match helper.ended(thread) {
            Some(code) => Err(Event::Broken(Failure::Ended(code))),
            None => Ok((helper, device)),
        })
    };
    let or_broken = |outcome: Result<Option<Event<Helper, Device, Shared>>, Failure>| outcome.unwrap_or_else(|why| Some(Event::Broken(why)));
    match action {
        Action::Nothing => None,
        Action::Start => Some(match start(thread, present?.chain) {
            Ok((helper, device)) => {
                line(&[Text("dlss: the helper is started (dlss\\dlss_helper.exe)")]);
                Event::Started { helper, device, now }
            }
            Err(why) => Event::Broken(why),
        }),
        Action::SayHello(output) => match ends()? {
            Err(broken) => Some(broken),
            Ok((helper, device)) => or_broken(helper.hello(thread, device, output).map(|sent| match sent {
                Hello::Sent => Some(Event::HelloSent { output, now }),
                Hello::NotOpen => None,
            })),
        },
        Action::AwaitHello => {
            let (helper, _) = link.ends()?;
            or_broken(answer(thread, helper, helper.hello_answer(thread)).map(|answer| answer.map(Event::HelloAnswered)))
        }
        Action::Share { render, output } => {
            let (helper, device) = link.ends()?;
            let back = &present?.back;
            if back.size != output {
                return or_broken(helper.new_size(thread).map(|()| Some(Event::OutputChanged { now })));
            }
            or_broken(Shared::make(thread, device, helper, back, render).map(|shared| {
                line(&[Text("dlss: offered: DLSS at Quality renders "), Dec(render.width), Text("x"), Dec(render.height), Text(" for "),
                       Dec(output.width), Text("x"), Dec(output.height), Text("; the Setup is sent, "), Dec((shared.bytes() >> 20) as u32),
                       Text(" MiB shared")]);
                Some(Event::SetupSent { shared, output, now })
            }))
        }
        Action::AwaitSetup => {
            let (helper, _) = link.ends()?;
            or_broken(answer(thread, helper, helper.setup_answer(thread)).map(|answer| {
                if matches!(answer, Some(Ok(()))) {
                    line(&[Text("dlss: running: the helper took the Setup; each Present submits a frame")]);
                }
                answer.map(Event::SetupAnswered)
            }))
        }
        Action::Submit(frame) => match ends()? {
            Err(broken) => Some(broken),
            Ok((helper, device)) => {
                let (shared, back) = (link.shared()?, &present?.back);
                let submitted = helper.frames_done(thread).and_then(|()| shared.reached(thread, device)).and_then(|reached| {
                    if !free(frame, reached) {
                        // DXVK's queue is that far behind: this Present has no frame of the link
                        return Ok(None);
                    }
                    if PROGRESS.get(thread) == Progress::Said && frame.0.is_multiple_of(PROGRESS_EVERY) {
                        line(&[Text("dlss: frame "), Dec(frame.0 as u32), Text(" is next: the timeline at "), Dec(reached as u32),
                               Text(" (the helper's output of the frame before is "), Dec(frame.copied() as u32 - 1), Text(")")]);
                    }
                    shared.submit(thread, device, helper, back, frame).map(|()| Some(Event::Submitted))
                });
                if frame.0 == 0 && matches!(submitted, Ok(Some(_))) {
                    line(&[Text("dlss: the first frame is submitted")]);
                }
                or_broken(submitted)
            }
        },
        Action::Resize(shared) => {
            let (helper, device) = link.ends()?;
            shared.retire(thread, device);
            if let Some(present) = present {
                line(&[Text("dlss: the output is "), Dec(present.back.size.width), Text("x"), Dec(present.back.size.height),
                       Text(" now: a new Hello and Setup")]);
            }
            or_broken(helper.new_size(thread).map(|()| None))
        }
        Action::Dismiss { helper, device } => {
            line(&[Text("dlss: offered: the helper says DLSS runs here (the tick is off: nothing runs)")]);
            helper.quit(thread);
            helper.close(thread);
            device.release(thread);
            None
        }
        Action::Report { why, purpose } => {
            say(purpose, &why);
            None
        }
        Action::Stop { helper, device, shared, why, purpose } => {
            helper.terminate(thread);
            helper.wait(thread, 2000);
            end(thread, helper, device, shared);
            say(purpose, &why);
            // the login screen was shown (its marker is gone): user.cfg has run, the tick can be saved off
            if matches!(why, Failure::Refused(_)) && !file_exists(thread, DXVK_MARKER) {
                tick_off(thread);
            }
            None
        }
        Action::Quit { helper, device, shared } => {
            line(&[Text("dlss: the game exits: the helper is told to quit")]);
            helper.quit(thread);
            // it finishes the frames it was told, then quits
            if !helper.wait(thread, 3000) {
                helper.terminate(thread);
                helper.wait(thread, 2000);
            }
            end(thread, helper, device, shared);
            None
        }
    }
}

/// What a read of the helper's answer found, the answer first: a helper that
/// says no ends right after, and its reason is in the pipe; only without an
/// answer does a helper that has ended count as that.
fn answer<T>(thread: &Thread, helper: &Helper, read: Result<Option<T>, Failure>) -> Result<Option<T>, Failure> {
    match (read, helper.ended(thread)) {
        (Ok(Some(answer)), _) => Ok(Some(answer)),
        (_, Some(code)) => Err(Failure::Ended(code)),
        (read, None) => read,
    }
}

/// A helper that is gone: the timeline released, what was shared retired,
/// the handles closed.
fn end(thread: &Thread, helper: Helper, device: Device, shared: Option<Shared>) {
    released(dlss::release(thread));
    if let Some(shared) = shared {
        shared.retire(thread, &device);
    }
    helper.close(thread);
    device.release(thread);
}

/// The line of a timeline value the game signalled itself.
fn released(value: Option<u64>) {
    if let Some(value) = value {
        line(&[Text("dlss: the helper is gone with a frame waited for: the timeline signalled to "), Dec(value as u32),
               Text(" from the game")]);
    }
}

/// The saved tick off (a refusal: DLSS does not run on this PC, so the next
/// start sets no mip bias for it), user.cfg written.
fn tick_off(thread: &Thread) {
    if DLSS.on(thread) {
        DLSS.value.set(thread, false);
        save_user_config(thread);
        line(&[Text("dlss: the saved tick is off: the helper says DLSS does not run on this PC")]);
    }
}

/// DXVK's device of the swap chain, the helper started, its watch started.
fn start(thread: &Thread, chain: SwapChain) -> Result<(Helper, Device), Failure> {
    let device = Device::of(thread, chain)?;
    // the game's environment as it is: what the helper needs of Wine it sets up itself (client/dlss-helper/README.md)
    let helper = match Helper::start(thread) {
        Ok(helper) => helper,
        Err(why) => {
            device.release(thread);
            return Err(why);
        }
    };
    if let Err(why) = helper.watch(thread, hud_options_on_watch_entry()) {
        helper.terminate(thread);
        helper.close(thread);
        device.release(thread);
        return Err(why);
    }
    Ok((helper, device))
}

/// The line that says why DLSS does not run.
fn say(purpose: Purpose, why: &Failure) {
    let head: &[Piece] = match purpose {
        Purpose::Ask => &[Text("dlss: not offered: ")],
        Purpose::Run | Purpose::Resize => &[Text("dlss: on, not run: ")],
    };
    match why {
        Failure::Start(what, error) => {
            line_of(&[head, &[Text("the helper was not started ("), Text(what), Text(", error "), Dec(*error), Text(")")]])
        }
        Failure::Device(what, result) => line_of(&[head, &[Text("DXVK's Vulkan device: "), Text(what), Text(", result "), Hex(*result)]]),
        Failure::Refused(refusal) => line_of(&[head, &[Text("the helper says DLSS does not run here: "), Text(refusal.text()),
                                                        Text(" (NGX "), Hex(refusal.ngx), Text(")")]]),
        Failure::SetupRefused(refusal) => line_of(&[head, &[Text("the helper did not take the shared images: "), Text(refusal.text()),
                                                             Text(" (NGX "), Hex(refusal.ngx), Text(")")]]),
        Failure::Ended(code) => line_of(&[head, &[Text("the helper ended, exit code "), Dec(*code)]]),
        Failure::Pipe(what, error) => line_of(&[head, &[Text("the helper's pipe closed ("), Text(what), Text(", error "), Dec(*error), Text(")")]]),
        Failure::Late(what) => line_of(&[head, &[Text("the helper runs and "), Text(what), Text(" in "), Dec(ANSWER_WAIT / 1000), Text(" s")]]),
    }
}

/// How long the watch waits for the helper's end between its looks at the
/// timeline.
const LOOK: u32 = 500;
/// The looks in a row (5 s) that find the helper's turn on the timeline
/// unmoved before the watch ends the helper. A timer, because a helper that
/// hangs gives no event, and DXVK's queue (and soon the game's thread, in a
/// Present) waits for its signal; a frame of DLSS takes it about 1 ms.
pub const STUCK_LOOKS: u32 = 10;

/// The watch's count of looks that found the same wait unmoved; None when
/// the timeline is not behind, or it is not the helper's turn.
pub fn looks(before: Option<(dlss::Behind, u32)>, now: Option<dlss::Behind>) -> Option<(dlss::Behind, u32)> {
    let now = now.filter(|behind| behind.helpers_turn())?;
    match before {
        Some((same, looks)) if same == now => Some((now, looks + 1)),
        _ => Some((now, 1)),
    }
}

/// The helper's watch, on its own thread until the helper's process ends:
/// then the timeline is signalled to the value DXVK's queue waits for, if
/// the helper did not reach it. A helper that runs and leaves its turn on
/// the timeline unmoved for [`STUCK_LOOKS`] looks is ended first.
pub fn watch(watch: HelperWatch<'_>) -> u32 {
    let thread = watch.thread();
    let mut stuck = None;
    while watch.wait(LOOK) == Watched::Runs {
        stuck = looks(stuck, dlss::behind(thread));
        if let Some((behind, STUCK_LOOKS)) = stuck {
            line(&[Text("dlss: the helper runs and did not signal "), Dec(behind.queued as u32), Text(" (the timeline at "),
                   Dec(behind.reached as u32), Text(") in "), Dec(STUCK_LOOKS * LOOK / 1000), Text(" s: ended")]);
            watch.terminate();
        }
    }
    released(dlss::release(thread));
    watch.close();
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A link of things a test can make.
    type Test = Link<&'static str, &'static str, &'static str>;

    const HD: Size = Size { width: 1920, height: 1080 };
    const RENDER: Size = Size { width: 1280, height: 720 };
    const QHD: Size = Size { width: 2560, height: 1440 };

    fn at(ms: u32) -> Tick {
        Tick::of(ms)
    }

    fn frame(output: Size, ms: u32) -> Event<&'static str, &'static str, &'static str> {
        Event::Frame { output, now: at(ms) }
    }

    fn refusal() -> Refusal {
        Refusal { text: [b'n'; dlss::REFUSAL_TEXT], ngx: 0xBAD0_0002 }
    }

    /// The link from its start through the events, each with the action it gives.
    fn run(purpose: Purpose) -> Test {
        let (link, action) = Test::Waiting(purpose).step(frame(HD, 0));
        assert_eq!(action, Action::Start);
        let (link, action) = link.step(Event::Started { helper: "helper", device: "device", now: at(10) });
        assert_eq!(action, Action::Nothing);
        // the pipe not open yet: the Hello is tried at each Present
        let (link, action) = link.step(frame(HD, 20));
        assert_eq!(action, Action::SayHello(HD));
        let (link, action) = link.step(frame(HD, 30));
        assert_eq!(action, Action::SayHello(HD));
        let (link, action) = link.step(Event::HelloSent { output: HD, now: at(30) });
        assert_eq!(action, Action::Nothing);
        let (link, action) = link.step(frame(HD, 40));
        assert_eq!(action, Action::AwaitHello);
        link
    }

    fn running() -> Test {
        let (link, action) = run(Purpose::Run).step(Event::HelloAnswered(Ok(RENDER)));
        assert_eq!(action, Action::Share { render: RENDER, output: HD });
        assert_eq!(DlssOffer::of(true, &link), DlssOffer::Offered);
        let (link, action) = link.step(Event::SetupSent { shared: "shared", output: HD, now: at(1000) });
        assert_eq!(action, Action::Nothing);
        let (link, action) = link.step(frame(HD, 1010));
        assert_eq!(action, Action::AwaitSetup);
        let (link, action) = link.step(Event::SetupAnswered(Ok(())));
        assert_eq!(action, Action::Nothing);
        link
    }

    #[test]
    fn with_the_tick_on_the_helper_is_asked_then_set_up_then_each_present_is_a_frame() {
        let link = running();
        let (link, action) = link.step(frame(HD, 1020));
        assert_eq!(action, Action::Submit(Frame(0)));
        // a Present whose command buffer is not free yet is no frame: the same one next
        let (link, action) = link.step(frame(HD, 1030));
        assert_eq!(action, Action::Submit(Frame(0)));
        let (link, action) = link.step(Event::Submitted);
        assert_eq!(action, Action::Nothing);
        let (link, action) = link.step(frame(HD, 1040));
        assert_eq!(action, Action::Submit(Frame(1)));
        assert_eq!(link, Link::Running { helper: "helper", device: "device", shared: "shared", output: HD, frame: Frame(1) });
        assert_eq!(DlssOffer::of(true, &link), DlssOffer::Offered);
    }

    #[test]
    fn with_the_tick_off_the_helpers_yes_offers_and_nothing_runs() {
        let link = run(Purpose::Ask);
        assert_eq!(DlssOffer::of(true, &link), DlssOffer::NotYet);
        let (link, action) = link.step(Event::HelloAnswered(Ok(RENDER)));
        assert_eq!(action, Action::Dismiss { helper: "helper", device: "device" });
        assert_eq!(link, Link::Offered);
        assert_eq!(DlssOffer::of(true, &link), DlssOffer::Offered);
        assert_eq!(link.step(frame(HD, 5000)), (Link::Offered, Action::Nothing));
    }

    #[test]
    fn a_no_is_not_offered_and_everything_ends() {
        for purpose in [Purpose::Ask, Purpose::Run] {
            let (link, action) = run(purpose).step(Event::HelloAnswered(Err(refusal())));
            let why = Failure::Refused(refusal());
            assert_eq!(action, Action::Stop { helper: "helper", device: "device", shared: None, why, purpose });
            assert_eq!(link, Link::Failed(why));
            assert_eq!(DlssOffer::of(true, &link), DlssOffer::Failed(why));
            // failed for the process: a Present starts nothing again
            assert_eq!(link.step(frame(HD, 9000)), (Link::Failed(why), Action::Nothing));
        }
    }

    #[test]
    fn a_failure_at_any_point_stops_with_what_there_is() {
        let ended = Failure::Ended(3);
        // nothing started
        assert_eq!(Test::Waiting(Purpose::Run).step(Event::Broken(ended)),
                   (Link::Failed(ended), Action::Report { why: ended, purpose: Purpose::Run }));
        // before the answer: not offered
        let (link, action) = run(Purpose::Run).step(Event::Broken(ended));
        assert_eq!(action, Action::Stop { helper: "helper", device: "device", shared: None, why: ended, purpose: Purpose::Run });
        assert_eq!(DlssOffer::of(true, &link), DlssOffer::Failed(ended));
        // the Setup refused, and a break while running: what was shared goes too, and the PC still offers DLSS
        let (link, action) = running().step(Event::Broken(ended));
        assert_eq!(action, Action::Stop { helper: "helper", device: "device", shared: Some("shared"), why: ended, purpose: Purpose::Run });
        assert_eq!(link, Link::Stopped(ended));
        assert_eq!(DlssOffer::of(true, &link), DlssOffer::Offered);
        assert_eq!(link.step(Event::Submitted), (Link::Stopped(ended), Action::Nothing));
        let (link, _) = run(Purpose::Run).step(Event::HelloAnswered(Ok(RENDER)));
        let (link, _) = link.step(Event::SetupSent { shared: "shared", output: HD, now: at(1000) });
        let (link, action) = link.step(Event::SetupAnswered(Err(refusal())));
        let why = Failure::SetupRefused(refusal());
        assert_eq!(action, Action::Stop { helper: "helper", device: "device", shared: Some("shared"), why, purpose: Purpose::Run });
        assert_eq!(link, Link::Stopped(why));
    }

    #[test]
    fn a_helper_that_runs_and_does_not_answer_is_late_after_the_wait() {
        // the Hello was sent at 30
        let (link, action) = run(Purpose::Run).step(frame(HD, 30 + ANSWER_WAIT));
        assert_eq!(action, Action::AwaitHello);
        let (link, action) = link.step(frame(HD, 31 + ANSWER_WAIT));
        let why = Failure::Late("no answer to the Hello");
        assert_eq!(action, Action::Stop { helper: "helper", device: "device", shared: None, why, purpose: Purpose::Run });
        assert_eq!(link, Link::Failed(why));
        // a pipe never opened, a Setup never answered
        let (link, _) = Test::Waiting(Purpose::Ask).step(Event::Started { helper: "helper", device: "device", now: at(u32::MAX - 5) });
        let (_, action) = link.step(frame(HD, ANSWER_WAIT));
        assert!(matches!(action, Action::Stop { why: Failure::Late("it did not open the pipe"), .. }));
        let (link, _) = run(Purpose::Run).step(Event::HelloAnswered(Ok(RENDER)));
        let (link, _) = link.step(Event::SetupSent { shared: "shared", output: HD, now: at(1000) });
        let (link, action) = link.step(frame(HD, 1001 + ANSWER_WAIT));
        assert!(matches!(action, Action::Stop { why: Failure::Late("no answer to the Setup"), shared: Some("shared"), .. }));
        assert!(matches!(link, Link::Stopped(_)));
    }

    #[test]
    fn a_present_of_another_size_is_a_new_hello_and_setup() {
        let (link, action) = running().step(frame(QHD, 2000));
        assert_eq!(action, Action::Resize("shared"));
        assert_eq!(link, Link::Starting { helper: "helper", device: "device", purpose: Purpose::Resize, since: at(2000) });
        // offered all along: the helper said yes before
        assert_eq!(DlssOffer::of(true, &link), DlssOffer::Offered);
        let (link, action) = link.step(frame(QHD, 2010));
        assert_eq!(action, Action::SayHello(QHD));
        let (link, _) = link.step(Event::HelloSent { output: QHD, now: at(2010) });
        let (link, action) = link.step(Event::HelloAnswered(Ok(Size { width: 1707, height: 960 })));
        assert_eq!(action, Action::Share { render: Size { width: 1707, height: 960 }, output: QHD });
        // the output changed again before the images were made: again
        let (link, action) = link.step(Event::OutputChanged { now: at(2500) });
        assert_eq!((&link, action), (&Link::Starting { helper: "helper", device: "device", purpose: Purpose::Resize, since: at(2500) },
                                     Action::Nothing));
        // a break then is a stop, not a failure
        let (link, _) = link.step(Event::Broken(Failure::Ended(0)));
        assert_eq!(link, Link::Stopped(Failure::Ended(0)));
    }

    #[test]
    fn the_exit_quits_what_runs() {
        assert_eq!(running().step(Event::Exit),
                   (Link::Off(NotRun::Exited), Action::Quit { helper: "helper", device: "device", shared: Some("shared") }));
        assert_eq!(run(Purpose::Ask).step(Event::Exit),
                   (Link::Off(NotRun::Exited), Action::Quit { helper: "helper", device: "device", shared: None }));
        for rests in [Test::Off(NotRun::NoFiles), Test::Waiting(Purpose::Run), Test::Offered, Test::Failed(Failure::Ended(1))] {
            assert!(matches!(rests.step(Event::Exit), (_, Action::Nothing)));
        }
    }

    #[test]
    fn dlss_is_offered_with_dxvk_once_the_helper_said_yes() {
        assert_eq!(NotRun::of(true, true), None);
        assert_eq!(NotRun::of(false, true), Some(NotRun::NotNvidia));
        assert_eq!(NotRun::of(true, false), Some(NotRun::NoFiles));
        assert_eq!(DlssOffer::of(true, &Test::Off(NotRun::NotNvidia)), DlssOffer::NotNvidia);
        assert_eq!(DlssOffer::of(true, &Test::Off(NotRun::NoFiles)), DlssOffer::NoFiles);
        for purpose in [Purpose::Ask, Purpose::Run] {
            assert_eq!(DlssOffer::of(true, &Test::Waiting(purpose)), DlssOffer::NotYet);
        }
        // DirectX 11 runs: never, whatever the link
        for link in [Test::Off(NotRun::Undecided), Test::Offered, running()] {
            assert_eq!(DlssOffer::of(false, &link), DlssOffer::NoDxvk);
        }
        assert!(Test::Offered.rests() && Test::Off(NotRun::NoFiles).rests() && !running().rests() && !Test::Waiting(Purpose::Ask).rests());
    }

    #[test]
    fn a_command_buffer_is_free_once_the_timeline_passed_its_last_copy() {
        for early in 0..SLOTS {
            assert!(free(Frame(early), 0));
        }
        // frame 4 takes frame 0's: free once frame 0's copy (value 1) is done
        assert!(!free(Frame(SLOTS), 0) && free(Frame(SLOTS), 1));
        assert!(!free(Frame(SLOTS + 3), Frame(3).copied() - 1) && free(Frame(SLOTS + 3), Frame(3).copied()));
    }

    #[test]
    fn the_watch_counts_looks_at_the_same_unmoved_turn_of_the_helper() {
        let turn = dlss::Behind { queued: Frame(5).upscaled(), reached: Frame(5).copied() };
        assert!(turn.helpers_turn());
        let mut stuck = None;
        for look in 1..=STUCK_LOOKS {
            stuck = looks(stuck, Some(turn));
            assert_eq!(stuck, Some((turn, look)));
        }
        // the timeline moved, or caught up: from the start
        let next = dlss::Behind { queued: Frame(6).upscaled(), reached: Frame(6).copied() };
        assert_eq!(looks(stuck, Some(next)), Some((next, 1)));
        assert_eq!(looks(stuck, None), None);
        // the game's own copy has not run (DXVK's queue is behind): not the helper's turn
        let ours = dlss::Behind { queued: Frame(5).upscaled(), reached: Frame(4).upscaled() };
        assert!(!ours.helpers_turn());
        assert_eq!(looks(stuck, Some(ours)), None);
    }
}
