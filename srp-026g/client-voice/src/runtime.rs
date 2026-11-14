//! Session lifecycle and the Windows voice worker. The game never shares
//! codec or WinMM memory with the worker; its controls and status are atomic,
//! how each place is heard one value under a lock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TalkState {
    Idle,
    Talking { since: u32 },
    WaitingForRelease,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TalkAction {
    None,
    Start,
    Stop,
    Cap,
}
impl TalkState {
    fn update(&mut self, now: u32, pressed: bool, mic_on: bool) -> TalkAction {
        match *self {
            Self::Idle if pressed && mic_on => {
                *self = Self::Talking { since: now };
                TalkAction::Start
            }
            Self::Talking { .. } if !pressed || !mic_on => {
                *self = Self::Idle;
                TalkAction::Stop
            }
            Self::Talking { since } if now.wrapping_sub(since) >= 30_000 => {
                *self = Self::WaitingForRelease;
                TalkAction::Cap
            }
            Self::WaitingForRelease if !pressed => {
                *self = Self::Idle;
                TalkAction::None
            }
            _ => TalkAction::None,
        }
    }
}

/// A WinMM ring is consumed in submission order, including after a pause
/// completes all buffers across the physical array's wrap boundary.
pub(crate) struct BufferOrder {
    next: usize,
}
impl BufferOrder {
    pub(crate) const fn new(next: usize) -> Self {
        Self { next }
    }
    pub(crate) fn next_ready(
        &mut self,
        count: usize,
        ready: impl FnOnce(usize) -> bool,
    ) -> Option<usize> {
        if !ready(self.next) {
            return None;
        }
        let index = self.next;
        self.next = (self.next + 1) % count;
        Some(index)
    }
}

/// How often the drop log writes at most.
pub(crate) const DROP_LOG_MS: u32 = 10_000;

/// What the worker dropped since the log last said so.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Dropped {
    datagrams: u32,
    /// The first dropped datagram's reason; `None` for a receive error.
    first: Option<crate::wire::Malformed>,
}

/// The datagrams the worker drops (malformed, or a receive error) and the
/// frames its decoder refuses, for the log: the first at once, then a line
/// at most every [`DROP_LOG_MS`] with the counts, so a flood is one line.
#[derive(Default)]
pub(crate) struct DropLog {
    dropped: Option<Dropped>,
    /// The decoder's refusals ([`crate::Stats::undecodable`]) last logged.
    undecodable: u32,
    last_line: Option<u32>,
}
impl DropLog {
    pub(crate) fn drop(&mut self, reason: Option<crate::wire::Malformed>) {
        self.dropped = Some(match self.dropped {
            Some(dropped) => Dropped {
                datagrams: dropped.datagrams.saturating_add(1),
                ..dropped
            },
            None => Dropped {
                datagrams: 1,
                first: reason,
            },
        });
    }
    /// The line to log at `now`, if one is due; `undecodable` is the
    /// receiver's count so far.
    pub(crate) fn line(&mut self, now: u32, undecodable: u32) -> Option<String> {
        if self
            .last_line
            .is_some_and(|last| now.wrapping_sub(last) < DROP_LOG_MS)
        {
            return None;
        }
        let frames = undecodable.wrapping_sub(self.undecodable);
        if self.dropped.is_none() && frames == 0 {
            return None;
        }
        self.undecodable = undecodable;
        self.last_line = Some(now);
        let mut line = match self.dropped.take() {
            Some(Dropped {
                datagrams,
                first: Some(reason),
            }) => format!("dropped {datagrams} datagrams (the first: {reason})"),
            Some(Dropped {
                datagrams,
                first: None,
            }) => format!("dropped {datagrams} datagrams (the first: a receive error)"),
            None => String::new(),
        };
        if frames > 0 {
            if !line.is_empty() {
                line.push_str("; ");
            }
            line.push_str(&format!("{frames} frames the decoder refused"));
        }
        Some(line)
    }
}

/// The longest host `sv_start` takes, its NUL not counted.
pub(crate) const MAX_HOST: usize = 255;

/// `sv_start`'s host: UTF-8 of 1 to [`MAX_HOST`] bytes before its NUL. A
/// string with no NUL in its first `MAX_HOST + 1` bytes is refused without
/// reading further.
///
/// # Safety
/// `host` is null, or readable up to its NUL or `MAX_HOST + 1` bytes,
/// whichever comes first.
pub(crate) unsafe fn host_arg<'a>(host: *const std::ffi::c_char) -> Option<&'a str> {
    if host.is_null() {
        return None;
    }
    let mut len = 0;
    // SAFETY: each byte read is at most the NUL or MAX_HOST + 1 bytes in.
    while unsafe { *host.add(len) } != 0 {
        len += 1;
        if len > MAX_HOST {
            return None;
        }
    }
    if len == 0 {
        return None;
    }
    // SAFETY: the `len` bytes before the NUL were just read.
    let bytes = unsafe { std::slice::from_raw_parts(host.cast::<u8>(), len) };
    std::str::from_utf8(bytes).ok()
}

#[cfg(windows)]
mod platform {
    use super::{DropLog, TalkAction, TalkState, host_arg};
    use crate::{
        Core, Encoder, FRAME, Heard, Hearing, Preprocessor, SLOTS, Sender,
        mic_test::{Ask, Devices, Door, Event, Shown, Sliders, Test},
        radio::unit,
        receiver::centred,
        windows::{Input, Output},
        wire::{self, Message},
    };
    use std::{
        ffi::c_char,
        io,
        net::{ToSocketAddrs, UdpSocket},
        sync::{
            Arc, Mutex, MutexGuard,
            atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering::SeqCst},
        },
        thread::{self, JoinHandle},
        time::{Duration, Instant},
    };
    use windows_sys::Win32::System::{
        Diagnostics::Debug::OutputDebugStringA,
        LibraryLoader::{
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_PIN,
            GetModuleHandleExA,
        },
    };

    struct Controls {
        ptt: AtomicBool,
        mic: AtomicBool,
        volume: AtomicU32,
        sensitivity: AtomicU32,
        /// How each slot is heard: a slot's whole state in one `sv_hear`.
        hearing: Mutex<Hearing>,
        muted: AtomicU32,
        hello: AtomicBool,
    }
    static CONTROL: Controls = Controls {
        ptt: AtomicBool::new(false),
        mic: AtomicBool::new(true),
        volume: AtomicU32::new(1.0_f32.to_bits()),
        sensitivity: AtomicU32::new(0.5_f32.to_bits()),
        hearing: Mutex::new(Hearing::UNITY),
        muted: AtomicU32::new(0),
        hello: AtomicBool::new(false),
    };
    fn volume() -> f32 {
        f32::from_bits(CONTROL.volume.load(SeqCst))
    }

    struct Status {
        stop: AtomicBool,
        slot: AtomicI32,
        heard: AtomicU32,
        transmitting: AtomicBool,
        ids: [AtomicU32; SLOTS],
        /// The microphone test as the options' button shows it (a
        /// [`Shown`]'s word).
        test: AtomicU32,
        /// What the game asks of the microphone test, for this worker.
        door: Mutex<Door>,
    }
    impl Status {
        /// `asked`: what the worker finds asked at its first pass.
        fn new(asked: Option<Ask>) -> Self {
            Self {
                stop: AtomicBool::new(false),
                slot: AtomicI32::new(-1),
                heard: AtomicU32::new(0),
                transmitting: AtomicBool::new(false),
                ids: std::array::from_fn(|_| AtomicU32::new(0)),
                test: AtomicU32::new(Shown::Idle.word()),
                door: Mutex::new(Door::Open(asked)),
            }
        }
        fn door(&self) -> MutexGuard<'_, Door> {
            self.door.lock().unwrap_or_else(|error| error.into_inner())
        }
    }
    struct Session {
        host: String,
        port: u16,
        session: u32,
        status: Arc<Status>,
        worker: JoinHandle<()>,
    }
    enum Lifecycle {
        Stopped,
        Running(Session),
        /// No match: a worker for the microphone test alone ([`run_test`]).
        Testing {
            status: Arc<Status>,
            worker: JoinHandle<()>,
        },
        Broken,
    }
    static SESSION: Mutex<Lifecycle> = Mutex::new(Lifecycle::Stopped);
    // Reconnecting to the same match must not reuse an utterance id that
    // listeners already closed. Only the one active worker increments this.
    static NEXT_UTTERANCE: AtomicU32 = AtomicU32::new(1);

    fn say(message: &str) {
        let text = format!("skhid_voice: {message}\n\0");
        // SAFETY: text is NUL-terminated and lives for the synchronous call.
        unsafe {
            OutputDebugStringA(text.as_ptr());
        }
    }
    fn send(socket: &UdpSocket, bytes: &[u8]) {
        if !bytes.is_empty() {
            let _ = socket.send(bytes);
        }
    }
    fn stop_locked(state: &mut Lifecycle) {
        CONTROL.ptt.store(false, SeqCst);
        let previous = std::mem::replace(state, Lifecycle::Stopped);
        match previous {
            Lifecycle::Running(Session { status, worker, .. })
            | Lifecycle::Testing { status, worker } => {
                status.stop.store(true, SeqCst);
                let deadline = Instant::now() + Duration::from_secs(3);
                while !worker.is_finished() && Instant::now() < deadline {
                    thread::sleep(Duration::from_millis(5));
                }
                if worker.is_finished() {
                    if worker.join().is_err() {
                        say("voice worker panicked");
                    }
                } else {
                    // The thread owns its buffers. Detaching preserves that
                    // ownership; a new session must not share its controls.
                    say("audio driver did not stop; voice stays off until restart");
                    *state = Lifecycle::Broken;
                }
            }
            Lifecycle::Broken => *state = Lifecycle::Broken,
            Lifecycle::Stopped => (),
        }
    }

    /// The worker's WinMM devices as the microphone test finds them.
    struct Winmm {
        playback: bool,
    }
    impl Devices for Winmm {
        type Microphone = Input;
        fn microphone(&mut self) -> Option<Input> {
            Input::open().ok()
        }
        fn playback(&self) -> bool {
            self.playback
        }
    }
    /// A pass of the microphone test: what the game asked of it since the
    /// last, then what its microphone captured and what is due.
    fn test_pass(test: &mut Test<Input>, status: &Status, now: u32, playback: bool, talking: bool) {
        let mut devices = Winmm { playback };
        // The door is not held while the test opens or closes a device.
        let asked = status.door().take();
        match asked {
            Some(Ask::Press(sliders)) => {
                test.apply(Event::Press { sliders, talking }, now, &mut devices);
            }
            Some(Ask::Stop) => {
                test.apply(Event::Stop, now, &mut devices);
            }
            None => (),
        }
        test.apply(Event::Pass, now, &mut devices);
    }
    /// The worker of a microphone test with no match (the lobby): the two
    /// devices and the test, no socket and nothing of the relay. It leaves
    /// when the test is over, closing its devices on its own thread.
    fn run_test(status: Arc<Status>) {
        let clock = Instant::now();
        let mut test = Test::Idle;
        // None: the press finds no playback device, and the button says so.
        let mut output = Output::open().ok();
        while !status.stop.load(SeqCst) {
            let now = clock.elapsed().as_millis() as u32;
            test_pass(&mut test, &status, now, output.is_some(), false);
            if let Some(device) = &mut output {
                let mut devices = Winmm { playback: true };
                let play = || {
                    let own = test.apply(Event::Play, now, &mut devices);
                    centred(&own.unwrap_or([0.; FRAME]))
                };
                if device.pump(play).is_err() {
                    output = None;
                }
            }
            status.test.store(test.shown(now).word(), SeqCst);
            if test.is_idle() && status.door().shut() {
                // The device still holds the last frames it was handed.
                thread::sleep(Duration::from_millis(100));
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        status.test.store(Shown::Idle.word(), SeqCst);
    }

    fn run(
        socket: UdpSocket,
        session: u32,
        status: Arc<Status>,
        mut core: Core,
        mut encoder: Encoder,
        mut preprocessor: Preprocessor,
    ) {
        let clock = Instant::now();
        let mut sender = Sender::new(session, NEXT_UTTERANCE.load(SeqCst) as u16);
        let mut talk = TalkState::Idle;
        let mut input: Option<Input> = None;
        let mut output: Option<Output> = None;
        let mut output_retry: Option<u32> = None;
        let mut next_mix = 0_u32;
        let mut last_hello = None;
        let mut ending: Option<(Vec<u8>, u8, u32)> = None;
        let mut applied_mute = u32::MAX;
        let mut receiving = true;
        let mut test = Test::Idle;
        let mut drops = DropLog::default();
        let mut bytes = [0_u8; 464];
        while !status.stop.load(SeqCst) {
            let now = clock.elapsed().as_millis() as u32;
            let hears = volume() > 0.0;
            let muted = CONTROL.muted.load(SeqCst);
            if muted != applied_mute || hears != receiving {
                core.mute(if hears { muted } else { u32::MAX });
                applied_mute = muted;
                receiving = hears;
            }
            // Bound each poll even if the peer floods the socket, so capture,
            // playback and stop cannot be starved by network traffic.
            // The socket is connected to the relay: the system passes on no
            // datagram from another address. What the relay sends is still
            // parsed once here; anything malformed is dropped and logged.
            for _ in 0..64 {
                let received = match socket.recv(&mut bytes) {
                    Ok(count) => bytes.get(..count).unwrap_or(&[]),
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                    // A datagram larger than the buffer, or an ICMP answer:
                    // that one is gone, the next may be good.
                    Err(_) => {
                        drops.drop(None);
                        continue;
                    }
                };
                match wire::parse(received) {
                    Ok(Message::Welcome { slot }) => status.slot.store(i32::from(slot), SeqCst),
                    Ok(Message::Roster { entries }) => {
                        let mut roster = [0_u32; SLOTS];
                        for (slot, id) in entries {
                            if let Some(place) = roster.get_mut(usize::from(slot)) {
                                *place = id;
                            }
                        }
                        for ((slot, id), known) in roster.into_iter().enumerate().zip(&status.ids) {
                            let previous = known.swap(id, SeqCst);
                            if previous != 0 && previous != id {
                                // Decoder history and utterance tombstones belong
                                // to an account, not the place a late joiner reused.
                                core.reset_slot(slot as u8);
                            }
                        }
                    }
                    Ok(
                        message @ (Message::RelayStart { .. }
                        | Message::RelayAudio { .. }
                        | Message::RelayEnd { .. }),
                    ) => {
                        if hears {
                            core.receive(&message, now);
                        }
                    }
                    Ok(
                        Message::Hello { .. }
                        | Message::Start { .. }
                        | Message::Audio { .. }
                        | Message::End { .. },
                    ) => drops.drop(Some(wire::Malformed::FromClient)),
                    Err(reason) => drops.drop(Some(reason)),
                }
            }
            if let Some(line) = drops.line(now, core.stats().undecodable) {
                say(&line);
            }
            // Until WELCOME every 250 ms: the relay knows the seat's IP only
            // once the match connection that sv_start begins is made.
            let every = if status.slot.load(SeqCst) < 0 { 250 } else { 2_000 };
            if CONTROL.hello.swap(false, SeqCst)
                || last_hello.is_none_or(|last| now.wrapping_sub(last) >= every)
            {
                send(&socket, &wire::hello(session, hears, muted));
                last_hello = Some(now);
            }

            if output.is_none() && output_retry.is_none_or(|last| now.wrapping_sub(last) >= 5_000) {
                match Output::open() {
                    Ok(device) => output = Some(device),
                    Err(_) => {
                        output_retry = Some(now);
                        say("no output device; retry in 5 seconds");
                    }
                }
            }
            let hearing = *CONTROL
                .hearing
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let level = volume();
            if let Some(device) = &mut output {
                // The microphone test's playback under the voices: nothing
                // of the test goes anywhere else from here.
                let mut devices = Winmm { playback: true };
                let mix = || {
                    let own = test.apply(Event::Play, now, &mut devices);
                    core.mix_over(own.as_ref(), level, &hearing, now)
                };
                if device.pump(mix).is_err() {
                    output = None;
                    output_retry = Some(now);
                    say("output device failed; retry in 5 seconds");
                }
                next_mix = now;
            } else {
                // Keep timeout and speaker status current without an output
                // device. A long scheduling pause cannot cause a busy loop.
                // Signed: a next mix still ahead of now is not a pause (as
                // unsigned it was, and every 5 ms pass mixed a frame: the
                // jitter buffers drained 4 times too fast).
                if (now.wrapping_sub(next_mix) as i32) > 200 {
                    next_mix = now;
                }
                let mut count = 0;
                while (now.wrapping_sub(next_mix) as i32) >= 0 && count < 10 {
                    core.mix(level, &hearing, next_mix);
                    next_mix = next_mix.wrapping_add(20);
                    count += 1;
                }
            }
            status.heard.store(core.talking(now), SeqCst);

            let pressed = CONTROL.ptt.load(SeqCst);
            test_pass(&mut test, &status, now, output.is_some(), pressed);
            let action = talk.update(now, pressed, CONTROL.mic.load(SeqCst));
            if action == TalkAction::Start {
                // Talking wins: the test ends, and its capture device is
                // closed before the transmission opens its own.
                let playback = output.is_some();
                test.apply(Event::Talk, now, &mut Winmm { playback });
            }
            match action {
                TalkAction::Start => match Input::open() {
                    Ok(device) => {
                        input = Some(device);
                        status.transmitting.store(true, SeqCst);
                        ending = None;
                        send(&socket, &sender.start(now));
                        NEXT_UTTERANCE.fetch_add(1, SeqCst);
                    }
                    Err(_) => {
                        talk = TalkState::WaitingForRelease;
                        say("no microphone; retry on the next push-to-talk press");
                    }
                },
                TalkAction::Stop | TalkAction::Cap => {
                    status.transmitting.store(false, SeqCst);
                    if let Some(bytes) = sender.flush() {
                        send(&socket, &bytes);
                    }
                    if let Some(bytes) = sender.end() {
                        send(&socket, &bytes);
                        ending = Some((bytes, 2, now.wrapping_add(30)));
                    }
                    // Close on release: no buffered microphone samples from
                    // this press can be sent at the start of the next one.
                    input = None;
                }
                TalkAction::None => (),
            }
            if let Some(device) = &mut input {
                let sensitivity = f32::from_bits(CONTROL.sensitivity.load(SeqCst));
                let result = device.pump(|mut pcm| {
                    preprocessor.run(&mut pcm, sensitivity);
                    if let Ok(frame) = encoder.encode(&pcm)
                        && let Some(bytes) = sender.frame(&frame)
                    {
                        send(&socket, &bytes);
                    }
                });
                if result.is_err() {
                    status.transmitting.store(false, SeqCst);
                    if let Some(bytes) = sender.flush() {
                        send(&socket, &bytes);
                    }
                    if let Some(bytes) = sender.end() {
                        send(&socket, &bytes);
                    }
                    input = None;
                    talk = TalkState::WaitingForRelease;
                    say("microphone failed; retry on the next press");
                }
            }
            if let Some((bytes, remaining, due)) = &mut ending
                && (now.wrapping_sub(*due) as i32) >= 0
            {
                send(&socket, bytes);
                *remaining -= 1;
                *due = now.wrapping_add(30);
                if *remaining == 0 {
                    ending = None;
                }
            }
            status.test.store(test.shown(now).word(), SeqCst);
            thread::sleep(Duration::from_millis(5));
        }
        if let Some(bytes) = sender.flush() {
            send(&socket, &bytes);
        }
        if let Some(bytes) = sender.end() {
            send(&socket, &bytes);
        }
        status.transmitting.store(false, SeqCst);
        // Devices, codec and socket are dropped on their owning worker.
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn sv_version() -> i32 {
        i32::from(voice_abi::ABI_VERSION)
    }
    /// The host may FreeLibrary while a worker owns executable code.
    /// Pinning keeps code and static controls alive until process exit.
    fn pin() -> bool {
        let mut module = std::ptr::null_mut();
        // SAFETY: an address of this module and a valid output pointer.
        let pinned = unsafe {
            GetModuleHandleExA(
                GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_PIN,
                sv_version as *const () as *const u8,
                &mut module,
            )
        };
        pinned != 0
    }

    /// Start voice for a match. Returns zero on success.
    ///
    /// # Safety
    /// `host` is either null or points to a readable NUL-terminated string
    /// for this call, as required by the shared voice ABI.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn sv_start(host: *const c_char, port: u32, session: u32) -> i32 {
        if port == 0 || port > u32::from(u16::MAX) {
            return 1;
        }
        // SAFETY: the caller supplies null or a readable NUL-terminated
        // string; host_arg reads no further than its NUL or 256 bytes.
        let Some(host) = (unsafe { host_arg(host) }) else {
            return 1;
        };
        let mut state = SESSION.lock().unwrap_or_else(|error| error.into_inner());
        if matches!(*state, Lifecycle::Broken) {
            return 8;
        }
        if let Lifecycle::Running(current) = &*state
            && current.host == host
            && current.port == port as u16
            && current.session == session
        {
            return 0;
        }
        stop_locked(&mut state);
        if matches!(*state, Lifecycle::Broken) {
            return 8;
        }
        let Ok(mut addresses) = (host, port as u16).to_socket_addrs() else {
            return 4;
        };
        let Some(address) = addresses.find(|address| address.is_ipv4()) else {
            return 4;
        };
        let Ok(socket) = UdpSocket::bind("0.0.0.0:0") else {
            return 5;
        };
        if socket.connect(address).is_err() || socket.set_nonblocking(true).is_err() {
            return 5;
        }
        let Ok(core) = Core::new() else {
            return 6;
        };
        let Ok(encoder) = Encoder::new() else {
            return 6;
        };
        let Ok(preprocessor) = Preprocessor::new() else {
            return 6;
        };
        if !pin() {
            return 7;
        }
        let status = Arc::new(Status::new(None));
        let worker_status = Arc::clone(&status);
        let Ok(worker) = thread::Builder::new()
            .name("skhid-voice".into())
            .spawn(move || run(socket, session, worker_status, core, encoder, preprocessor))
        else {
            return 7;
        };
        *state = Lifecycle::Running(Session {
            host: host.into(),
            port: port as u16,
            session,
            status,
            worker,
        });
        0
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn sv_stop() {
        stop_locked(&mut SESSION.lock().unwrap_or_else(|error| error.into_inner()));
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn sv_ptt(down: i32) {
        CONTROL.ptt.store(down != 0, SeqCst);
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn sv_settings(volume: f32, mic_on: i32, sensitivity: f32) {
        let level = unit(volume);
        let previous = f32::from_bits(CONTROL.volume.swap(level.to_bits(), SeqCst));
        CONTROL
            .sensitivity
            .store(unit(sensitivity).to_bits(), SeqCst);
        CONTROL.mic.store(mic_on != 0, SeqCst);
        if (previous > 0.0) != (level > 0.0) {
            CONTROL.hello.store(true, SeqCst);
        }
    }
    /// How the slot is heard from now on (`voice_abi::Hear`): its plain
    /// voice's gain in each ear and whether its radio is on. A slot past the
    /// 20 changes nothing.
    #[unsafe(no_mangle)]
    pub extern "C" fn sv_hear(slot: i32, left: f32, right: f32, radio: i32) {
        if let Ok(slot) = usize::try_from(slot) {
            CONTROL
                .hearing
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .set(slot, Heard::of_call(left, right, radio));
        }
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn sv_mute(mask: u32) {
        if CONTROL.muted.swap(mask, SeqCst) != mask {
            CONTROL.hello.store(true, SeqCst);
        }
    }
    // Both sides compile against the same signature table; changing an export
    // without changing the shared ABI is a compile error.
    const _: voice_abi::Api = voice_abi::Api {
        version: sv_version,
        start: sv_start,
        stop: sv_stop,
        ptt: sv_ptt,
        settings: sv_settings,
        hear: sv_hear,
        mute: sv_mute,
        talking: sv_talking,
        slot: sv_slot,
        voice_id: sv_voice_id,
        test_press: sv_test_press,
        test_stop: sv_test_stop,
        test_state: sv_test_state,
    };
    fn read_status<T>(default: T, read: impl FnOnce(&Status) -> T) -> T {
        let state = SESSION.lock().unwrap_or_else(|error| error.into_inner());
        if let Lifecycle::Running(session) = &*state {
            read(&session.status)
        } else {
            default
        }
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn sv_talking() -> u32 {
        read_status(0, |status| {
            let mut mask = if volume() > 0.0 {
                status.heard.load(SeqCst) & !CONTROL.muted.load(SeqCst)
            } else {
                0
            };
            if CONTROL.ptt.load(SeqCst)
                && CONTROL.mic.load(SeqCst)
                && status.transmitting.load(SeqCst)
            {
                let slot = status.slot.load(SeqCst);
                mask |= if slot >= 0 && slot < SLOTS as i32 {
                    1 << slot
                } else {
                    1 << 31
                };
            }
            mask
        })
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn sv_slot() -> i32 {
        read_status(-1, |status| status.slot.load(SeqCst))
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn sv_voice_id(slot: i32) -> u32 {
        read_status(0, |status| {
            usize::try_from(slot)
                .ok()
                .and_then(|slot| status.ids.get(slot))
                .map_or(0, |id| id.load(SeqCst))
        })
    }
    /// The options' "Test microphone" button, with the "Voice chat" volume
    /// and the "Microphone sensitivity" as their sliders stand (each 0..1).
    /// The test is the worker's: a match's, or with no match one of its own.
    #[unsafe(no_mangle)]
    pub extern "C" fn sv_test_press(volume: f32, sensitivity: f32) {
        let ask = Ask::Press(Sliders::new(volume, sensitivity));
        let mut state = SESSION.lock().unwrap_or_else(|error| error.into_inner());
        let taken = match &*state {
            Lifecycle::Running(Session { status, .. }) | Lifecycle::Testing { status, .. } => {
                status.door().ask(ask)
            }
            Lifecycle::Stopped => false,
            Lifecycle::Broken => return,
        };
        if taken || !pin() {
            return;
        }
        // No worker, or one that has left as its test was over (it closes
        // its devices by itself; its handle goes here): a new one, which
        // finds the press at its first pass.
        let status = Arc::new(Status::new(Some(ask)));
        let worker_status = Arc::clone(&status);
        *state = match thread::Builder::new()
            .name("skhid-voice-test".into())
            .spawn(move || run_test(worker_status))
        {
            Ok(worker) => Lifecycle::Testing { status, worker },
            Err(_) => Lifecycle::Stopped,
        };
    }
    /// The test stopped, if one runs.
    #[unsafe(no_mangle)]
    pub extern "C" fn sv_test_stop() {
        let state = SESSION.lock().unwrap_or_else(|error| error.into_inner());
        if let Lifecycle::Running(Session { status, .. }) | Lifecycle::Testing { status, .. } =
            &*state
        {
            status.door().ask(Ask::Stop);
        }
    }
    /// What the test does now, as its button shows it ([`Shown::word`]).
    #[unsafe(no_mangle)]
    pub extern "C" fn sv_test_state() -> u32 {
        let state = SESSION.lock().unwrap_or_else(|error| error.into_inner());
        match &*state {
            Lifecycle::Running(Session { status, .. }) | Lifecycle::Testing { status, .. } => {
                status.test.load(SeqCst)
            }
            Lifecycle::Stopped | Lifecycle::Broken => Shown::Idle.word(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn completed_buffers_keep_capture_order_after_a_pause() {
        let mut order = BufferOrder::new(2);
        // Buffers 0/1 were requeued before the worker was descheduled. The
        // next four captured PCM frames occupy physical slots 2, 3, 0, 1.
        let frames = [30, 40, 10, 20];
        let mut ready = [true; 4];
        let mut sent = Vec::new();
        while let Some(index) = order.next_ready(4, |index| ready[index]) {
            sent.push(frames[index]);
            ready[index] = false;
        }
        assert_eq!(sent, [10, 20, 30, 40]);
        ready[3] = true;
        assert_eq!(order.next_ready(4, |index| ready[index]), None);
        ready[2] = true;
        assert_eq!(order.next_ready(4, |index| ready[index]), Some(2));
    }
    #[test]
    fn thirty_seconds_requires_release_even_when_mic_is_toggled() {
        let mut state = TalkState::Idle;
        assert_eq!(state.update(7, true, true), TalkAction::Start);
        assert_eq!(state.update(30_006, true, true), TalkAction::None);
        assert_eq!(state.update(30_007, true, true), TalkAction::Cap);
        assert_eq!(state.update(30_008, true, false), TalkAction::None);
        assert_eq!(state.update(30_009, true, true), TalkAction::None);
        assert_eq!(state.update(30_010, false, true), TalkAction::None);
        assert_eq!(state.update(30_011, true, true), TalkAction::Start);
    }
    #[test]
    fn disabling_microphone_ends_current_utterance() {
        let mut state = TalkState::Idle;
        assert_eq!(state.update(0, true, false), TalkAction::None);
        assert_eq!(state.update(1, true, true), TalkAction::Start);
        assert_eq!(state.update(2, true, false), TalkAction::Stop);
        assert_eq!(state.update(3, false, true), TalkAction::None);
    }
    #[test]
    fn talk_limit_survives_millisecond_wrap() {
        let mut state = TalkState::Idle;
        assert_eq!(state.update(u32::MAX - 5, true, true), TalkAction::Start);
        assert_eq!(state.update(29_993, true, true), TalkAction::None);
        assert_eq!(state.update(29_994, true, true), TalkAction::Cap);
    }
}
