//! Session lifecycle and the Windows voice worker. The game never shares
//! codec or WinMM memory with the worker; its controls and status are atomic.
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

#[cfg(windows)]
mod platform {
    use super::{TalkAction, TalkState};
    use crate::{
        Core, Encoder, SLOTS, Sender,
        radio::mic_gain,
        windows::{Input, Output},
        wire::{self, Message},
    };
    use std::{
        ffi::{CStr, c_char},
        io,
        net::{ToSocketAddrs, UdpSocket},
        sync::{
            Arc, Mutex,
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
        far: AtomicU32,
        muted: AtomicU32,
        hello: AtomicBool,
    }
    static CONTROL: Controls = Controls {
        ptt: AtomicBool::new(false),
        mic: AtomicBool::new(true),
        volume: AtomicU32::new(1.0_f32.to_bits()),
        sensitivity: AtomicU32::new(0.5_f32.to_bits()),
        far: AtomicU32::new(0),
        muted: AtomicU32::new(0),
        hello: AtomicBool::new(false),
    };
    fn volume() -> f32 {
        f32::from_bits(CONTROL.volume.load(SeqCst))
    }
    fn unit(value: f32) -> f32 {
        if value.is_nan() {
            0.0
        } else {
            value.clamp(0.0, 1.0)
        }
    }

    struct Status {
        stop: AtomicBool,
        slot: AtomicI32,
        heard: AtomicU32,
        last: AtomicI32,
        transmitting: AtomicBool,
        ids: [AtomicU32; SLOTS],
    }
    impl Status {
        fn new() -> Self {
            Self {
                stop: AtomicBool::new(false),
                slot: AtomicI32::new(-1),
                heard: AtomicU32::new(0),
                last: AtomicI32::new(-1),
                transmitting: AtomicBool::new(false),
                ids: std::array::from_fn(|_| AtomicU32::new(0)),
            }
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
            Lifecycle::Running(session) => {
                session.status.stop.store(true, SeqCst);
                let deadline = Instant::now() + Duration::from_secs(3);
                while !session.worker.is_finished() && Instant::now() < deadline {
                    thread::sleep(Duration::from_millis(5));
                }
                if session.worker.is_finished() {
                    if session.worker.join().is_err() {
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

    fn run(
        socket: UdpSocket,
        session: u32,
        status: Arc<Status>,
        mut core: Core,
        mut encoder: Encoder,
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
            for _ in 0..64 {
                match socket.recv(&mut bytes) {
                    Ok(count) => {
                        if let Some(message) = wire::parse(&bytes[..count]) {
                            match &message {
                                Message::Welcome { slot } if usize::from(*slot) < SLOTS => {
                                    status.slot.store(i32::from(*slot), SeqCst)
                                }
                                Message::Roster { entries } => {
                                    let mut roster = [0_u32; SLOTS];
                                    for &(slot, id) in entries {
                                        if let Some(place) = roster.get_mut(usize::from(slot)) {
                                            *place = id;
                                        }
                                    }
                                    for (slot, id) in roster.into_iter().enumerate() {
                                        let previous = status.ids[slot].swap(id, SeqCst);
                                        if previous != 0 && previous != id {
                                            // Decoder history and utterance tombstones belong
                                            // to an account, not the place a late joiner reused.
                                            core.reset_slot(slot as u8);
                                        }
                                    }
                                }
                                _ if hears => core.receive(&message, now),
                                _ => (),
                            }
                        }
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                    Err(_) => break,
                }
            }
            if CONTROL.hello.swap(false, SeqCst)
                || last_hello.is_none_or(|last| now.wrapping_sub(last) >= 2_000)
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
            let far = CONTROL.far.load(SeqCst);
            let level = volume();
            if let Some(device) = &mut output {
                if device.pump(|| core.mix(level, far, now)).is_err() {
                    output = None;
                    output_retry = Some(now);
                    say("output device failed; retry in 5 seconds");
                }
                next_mix = now;
            } else {
                // Keep timeout and speaker status current without an output
                // device. A long scheduling pause cannot cause a busy loop.
                if now.wrapping_sub(next_mix) > 200 {
                    next_mix = now;
                }
                let mut count = 0;
                while (now.wrapping_sub(next_mix) as i32) >= 0 && count < 10 {
                    core.mix(level, far, next_mix);
                    next_mix = next_mix.wrapping_add(20);
                    count += 1;
                }
            }
            status.heard.store(core.talking(now), SeqCst);
            status
                .last
                .store(core.last_speaker().map_or(-1, i32::from), SeqCst);

            let pressed = CONTROL.ptt.load(SeqCst);
            match talk.update(now, pressed, CONTROL.mic.load(SeqCst)) {
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
                let gain = mic_gain(f32::from_bits(CONTROL.sensitivity.load(SeqCst)));
                let result = device.pump(|mut pcm| {
                    for sample in &mut pcm {
                        *sample = (f32::from(*sample) * gain).clamp(-32768.0, 32767.0) as i16;
                    }
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

    /// Start voice for a match. Returns zero on success.
    ///
    /// # Safety
    /// `host` is either null or points to a readable NUL-terminated string
    /// for this call, as required by the shared voice ABI.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn sv_start(host: *const c_char, port: u32, session: u32) -> i32 {
        if host.is_null() || port == 0 || port > u32::from(u16::MAX) {
            return 1;
        }
        // SAFETY: caller supplies a readable NUL-terminated host string.
        let Ok(host) = unsafe { CStr::from_ptr(host) }.to_str() else {
            return 1;
        };
        if host.is_empty() || host.len() >= 256 {
            return 1;
        }
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
        // The host may FreeLibrary while the worker owns executable code.
        // Pinning keeps code and static controls alive until process exit.
        let mut module = std::ptr::null_mut();
        let pinned = unsafe {
            GetModuleHandleExA(
                GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_PIN,
                sv_version as *const () as *const u8,
                &mut module,
            )
        };
        if pinned == 0 {
            return 7;
        }
        let status = Arc::new(Status::new());
        let worker_status = Arc::clone(&status);
        let Ok(worker) = thread::Builder::new()
            .name("skhid-voice".into())
            .spawn(move || run(socket, session, worker_status, core, encoder))
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
    #[unsafe(no_mangle)]
    pub extern "C" fn sv_radio(mask: u32) {
        CONTROL.far.store(mask, SeqCst);
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
        radio: sv_radio,
        mute: sv_mute,
        talking: sv_talking,
        slot: sv_slot,
        last_speaker: sv_last_speaker,
        voice_id: sv_voice_id,
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
    pub extern "C" fn sv_last_speaker() -> i32 {
        read_status(-1, |status| status.last.load(SeqCst))
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
