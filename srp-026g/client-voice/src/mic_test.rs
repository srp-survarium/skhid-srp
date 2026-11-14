//! The options' microphone test (the owner, 2026-11-14: "you press button,
//! it records for 10 seconds, then plays it back"; docs/notes/voice-chat.md,
//! "Microphone test"). One machine, [`Test`], with one transition,
//! [`Test::step`]. The player hears what a teammate beside it would: the
//! microphone goes through a transmission's own path (noise suppression, the
//! automatic gain, the codec) and is played plain and centred. Nothing of it
//! is sent or written: a recording lives in its state and goes with it. The
//! devices are the worker's ([`Devices`]), so the tests run the machine with
//! none.
use crate::{Encoder, FRAME, Preprocessor, radio::unit, receiver::Direct, wire::our_frame};
pub use voice_abi::MicTest as Shown;

/// A recording's length: 10 s, 500 frames of 20 ms.
pub const RECORD_MS: u32 = 10_000;
const FRAMES_A_SECOND: usize = crate::RATE / FRAME;
const RECORD_FRAMES: usize = RECORD_MS as usize / 1000 * FRAMES_A_SECOND;
/// How long the button says what is missing ("for a moment").
pub const NOTICE_MS: u32 = 2_000;

/// The options' "Voice chat" volume and "Microphone sensitivity" as their
/// sliders stood at the press (each 0..1), accepted or not.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sliders {
    volume: f32,
    sensitivity: f32,
}
impl Sliders {
    pub fn new(volume: f32, sensitivity: f32) -> Self {
        Self {
            volume: unit(volume),
            sensitivity: unit(sensitivity),
        }
    }
}

/// An open capture device. Dropped, it is closed.
pub trait Microphone {
    /// Gives `frame` each 20 ms captured since the last call, oldest first.
    /// An error: the device failed.
    fn capture(&mut self, frame: impl FnMut([i16; FRAME])) -> Result<(), u32>;
}
/// The worker's devices, as a step finds them.
pub trait Devices {
    type Microphone: Microphone;
    /// Opens the capture device; None when there is none or it cannot be.
    fn microphone(&mut self) -> Option<Self::Microphone>;
    /// Whether the playback device is open.
    fn playback(&self) -> bool;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    /// The options' button. `talking`: push-to-talk is held.
    Press { sliders: Sliders, talking: bool },
    /// The options closed or left their sound tab, a match loads or ends.
    Stop,
    /// Push-to-talk starts a transmission: talking wins, the capture device
    /// is the transmission's.
    Talk,
    /// A pass of the worker: what the microphone captured, what is due.
    Pass,
    /// The playback device takes its next 20 ms.
    Play,
}

/// What the test cannot do without.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Missing {
    Microphone,
    Playback,
}

pub enum Test<M> {
    Idle,
    Recording(Recording<M>),
    Playing(Playing),
    /// The button says what is missing until the clock passes `until`.
    Missing { what: Missing, until: u32 },
}
pub struct Recording<M> {
    /// The capture device, the test's while it records.
    microphone: M,
    sliders: Sliders,
    /// When the 10 s are over.
    until: u32,
    preprocessor: Preprocessor,
    encoder: Encoder,
    /// What is recorded so far: the frames a transmission would have sent.
    frames: Vec<Vec<u8>>,
}
pub struct Playing {
    volume: f32,
    voice: Direct,
    /// What is left to play.
    frames: std::vec::IntoIter<Vec<u8>>,
}

fn due(now: u32, at: u32) -> bool {
    now.wrapping_sub(at) as i32 >= 0
}
fn missing<M>(what: Missing, now: u32) -> Test<M> {
    Test::Missing {
        what,
        until: now.wrapping_add(NOTICE_MS),
    }
}
/// `left` as whole seconds counted up, `a_second` of it to one: 1 at least.
fn seconds(left: usize, a_second: usize) -> u8 {
    left.div_ceil(a_second).clamp(1, usize::from(u8::MAX)) as u8
}

impl<M: Microphone> Test<M> {
    /// The one transition: the state after `event` at `now` (the worker's
    /// clock in ms, which wraps) and, for [`Event::Play`], the 20 ms of the
    /// recording to play, at the press's volume.
    pub fn step(
        self,
        event: Event,
        now: u32,
        devices: &mut impl Devices<Microphone = M>,
    ) -> (Self, Option<[f32; FRAME]>) {
        let next = match (self, event) {
            // A press while it records or plays stops it.
            (Test::Recording(_) | Test::Playing(_), Event::Press { .. }) => Test::Idle,
            // It cannot start while push-to-talk is held.
            (state @ (Test::Idle | Test::Missing { .. }), Event::Press { talking: true, .. }) => {
                state
            }
            (
                Test::Idle | Test::Missing { .. },
                Event::Press {
                    sliders,
                    talking: false,
                },
            ) => Self::start(sliders, now, devices),
            (_, Event::Stop) => Test::Idle,
            (Test::Recording(_) | Test::Playing(_), Event::Talk) => Test::Idle,
            (state @ (Test::Idle | Test::Missing { .. }), Event::Talk) => state,
            (Test::Recording(recording), Event::Pass) => recording.pass(now),
            (Test::Playing(_), Event::Pass) if !devices.playback() => {
                missing(Missing::Playback, now)
            }
            (Test::Missing { until, .. }, Event::Pass) if due(now, until) => Test::Idle,
            (state @ (Test::Idle | Test::Playing(_) | Test::Missing { .. }), Event::Pass) => state,
            (Test::Playing(mut playing), Event::Play) => {
                return match playing.next() {
                    Some(pcm) => (Test::Playing(playing), Some(pcm)),
                    None => (Test::Idle, None),
                };
            }
            (state @ (Test::Idle | Test::Recording(_) | Test::Missing { .. }), Event::Play) => {
                state
            }
        };
        (next, None)
    }

    fn start(sliders: Sliders, now: u32, devices: &mut impl Devices<Microphone = M>) -> Self {
        let Some(microphone) = devices.microphone() else {
            return missing(Missing::Microphone, now);
        };
        if !devices.playback() {
            return missing(Missing::Playback, now);
        }
        // A transmission's path, new for each test as for a match's first
        // press. Without it nothing can be recorded: as without a microphone.
        let (Ok(preprocessor), Ok(encoder)) = (Preprocessor::new(), Encoder::new()) else {
            return missing(Missing::Microphone, now);
        };
        Test::Recording(Recording {
            microphone,
            sliders,
            until: now.wrapping_add(RECORD_MS),
            preprocessor,
            encoder,
            frames: Vec::with_capacity(RECORD_FRAMES),
        })
    }

    /// [`Test::step`] in place.
    pub fn apply(
        &mut self,
        event: Event,
        now: u32,
        devices: &mut impl Devices<Microphone = M>,
    ) -> Option<[f32; FRAME]> {
        let (next, play) = std::mem::replace(self, Test::Idle).step(event, now, devices);
        *self = next;
        play
    }

    pub fn is_idle(&self) -> bool {
        matches!(self, Test::Idle)
    }

    /// What the options' button shows at `now`.
    pub fn shown(&self, now: u32) -> Shown {
        match self {
            Test::Idle => Shown::Idle,
            Test::Recording(recording) => {
                let left = (recording.until.wrapping_sub(now) as i32).max(0);
                Shown::Recording(seconds(left as usize, 1000))
            }
            Test::Playing(playing) => {
                Shown::Playing(seconds(playing.frames.len(), FRAMES_A_SECOND))
            }
            Test::Missing {
                what: Missing::Microphone,
                ..
            } => Shown::NoMicrophone,
            Test::Missing {
                what: Missing::Playback,
                ..
            } => Shown::NoPlayback,
        }
    }
}

impl<M: Microphone> Recording<M> {
    fn pass(mut self, now: u32) -> Test<M> {
        let Recording {
            microphone,
            sliders,
            preprocessor,
            encoder,
            frames,
            ..
        } = &mut self;
        let captured = microphone.capture(|mut pcm| {
            // 10 s of frames are kept; a device ahead of the clock gives more.
            if frames.len() == RECORD_FRAMES {
                return;
            }
            preprocessor.run(&mut pcm, sliders.sensitivity);
            // As a transmission (`Sender::frame`): a frame that cannot be
            // encoded, or that the relay would not forward, is not sent.
            if let Ok(frame) = encoder.encode(&pcm)
                && our_frame(&frame)
            {
                frames.push(frame);
            }
        });
        match captured {
            Err(_) => missing(Missing::Microphone, now),
            Ok(()) if !due(now, self.until) => Test::Recording(self),
            // A microphone that gave nothing in 10 s is none.
            Ok(()) if self.frames.is_empty() => missing(Missing::Microphone, now),
            // The microphone closes here, with the rest of the recording's state.
            Ok(()) => match Direct::new() {
                Ok(voice) => Test::Playing(Playing {
                    volume: self.sliders.volume,
                    voice,
                    frames: self.frames.into_iter(),
                }),
                Err(_) => missing(Missing::Playback, now),
            },
        }
    }
}

impl Playing {
    fn next(&mut self) -> Option<[f32; FRAME]> {
        let frame = self.frames.next()?;
        let pcm = self.voice.frame(&frame).unwrap_or([0.; FRAME]);
        Some(pcm.map(|sample| sample * self.volume))
    }
}

/// What the game asks of the test.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ask {
    Press(Sliders),
    Stop,
}

/// How the game's asks reach the worker that runs the test. The worker takes
/// what was asked at each pass. A worker that runs for a test alone (no
/// match) shuts the door as it leaves, so an ask is never left with a worker
/// that will not look again: the game starts another for it.
#[derive(Debug, PartialEq)]
pub enum Door {
    Open(Option<Ask>),
    Shut,
}
impl Door {
    /// The game's: leaves `ask` for the worker, over one it has not taken
    /// yet (the last stands). False: the worker has left.
    pub fn ask(&mut self, ask: Ask) -> bool {
        match self {
            Door::Open(asked) => {
                *asked = Some(ask);
                true
            }
            Door::Shut => false,
        }
    }
    /// The worker's: what was asked since it last looked.
    pub fn take(&mut self) -> Option<Ask> {
        match self {
            Door::Open(asked) => asked.take(),
            Door::Shut => None,
        }
    }
    /// The worker's, leaving: shut when nothing is asked. False: something
    /// is, so the worker stays.
    pub fn shut(&mut self) -> bool {
        match self {
            Door::Open(Some(_)) => false,
            Door::Open(None) | Door::Shut => {
                *self = Door::Shut;
                true
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Core, Hearing, Stereo,
        receiver::centred,
        tests::voiced,
        wire::Message,
    };
    use std::{cell::Cell, rc::Rc};

    /// What the fake devices did, for the assertions.
    #[derive(Default)]
    struct Seen {
        opened: Cell<u32>,
        closed: Cell<u32>,
    }
    /// How the fake microphone behaves.
    #[derive(Clone, Copy, PartialEq)]
    enum Mic {
        /// A voice (`voiced`), a frame for every 20 ms of the clock.
        Voice,
        /// Open, and nothing ever comes.
        Mute,
        /// It fails at its 50th frame.
        Breaks,
    }
    struct FakeMic {
        kind: Mic,
        /// The clock, which the test moves.
        clock: Rc<Cell<u32>>,
        opened_at: u32,
        given: usize,
        seen: Rc<Seen>,
    }
    impl Microphone for FakeMic {
        fn capture(&mut self, mut frame: impl FnMut([i16; FRAME])) -> Result<(), u32> {
            if self.kind == Mic::Mute {
                return Ok(());
            }
            let due = (self.clock.get().wrapping_sub(self.opened_at) / 20) as usize;
            while self.given < due {
                if self.kind == Mic::Breaks && self.given == 50 {
                    return Err(1);
                }
                frame(voiced(self.given, 8000., 8));
                self.given += 1;
            }
            Ok(())
        }
    }
    impl Drop for FakeMic {
        fn drop(&mut self) {
            self.seen.closed.set(self.seen.closed.get() + 1);
        }
    }
    struct Fake {
        microphone: Option<Mic>,
        playback: bool,
        clock: Rc<Cell<u32>>,
        seen: Rc<Seen>,
    }
    impl Fake {
        fn new(microphone: Option<Mic>) -> Self {
            Self {
                microphone,
                playback: true,
                clock: Rc::new(Cell::new(0)),
                seen: Rc::default(),
            }
        }
        fn open(&self) -> u32 {
            self.seen.opened.get() - self.seen.closed.get()
        }
    }
    impl Devices for Fake {
        type Microphone = FakeMic;
        fn microphone(&mut self) -> Option<FakeMic> {
            let kind = self.microphone?;
            self.seen.opened.set(self.seen.opened.get() + 1);
            Some(FakeMic {
                kind,
                clock: Rc::clone(&self.clock),
                opened_at: self.clock.get(),
                given: 0,
                seen: Rc::clone(&self.seen),
            })
        }
        fn playback(&self) -> bool {
            self.playback
        }
    }

    const START: u32 = u32::MAX - 4_000;
    fn press(volume: f32, sensitivity: f32) -> Event {
        Event::Press {
            sliders: Sliders::new(volume, sensitivity),
            talking: false,
        }
    }
    /// The worker's passes, one every 5 ms, until the clock is `to` past
    /// [`START`] (which wraps within a recording).
    fn run(test: &mut Test<FakeMic>, fake: &mut Fake, to: u32) {
        while fake.clock.get().wrapping_sub(START) < to {
            fake.clock.set(fake.clock.get().wrapping_add(5));
            test.apply(Event::Pass, fake.clock.get(), fake);
        }
    }
    fn shown(test: &Test<FakeMic>, fake: &Fake) -> Shown {
        test.shown(fake.clock.get())
    }
    /// A started test: pressed at [`START`].
    fn started(microphone: Mic, volume: f32, sensitivity: f32) -> (Test<FakeMic>, Fake) {
        let mut fake = Fake::new(Some(microphone));
        fake.clock.set(START);
        let mut test = Test::Idle;
        test.apply(press(volume, sensitivity), START, &mut fake);
        (test, fake)
    }
    /// The playback device's pulls until the test gives nothing more, as the
    /// worker ends each frame ([`centred`]).
    fn played(test: &mut Test<FakeMic>, fake: &mut Fake) -> Vec<Stereo> {
        let mut out = Vec::new();
        while let Some(pcm) = test.apply(Event::Play, fake.clock.get(), fake) {
            out.push(centred(&pcm));
        }
        out
    }
    /// What a teammate's game plays of the same microphone sent at
    /// `sensitivity`, at its `volume`, near and centred: a transmission's
    /// path, the relay's messages, the jitter buffer and the mix.
    fn heard_by_a_teammate(volume: f32, sensitivity: f32) -> Vec<Stereo> {
        let mut preprocessor = Preprocessor::new().unwrap();
        let mut encoder = Encoder::new().unwrap();
        let mut core = Core::new().unwrap();
        let mut out = Vec::new();
        let hearing = Hearing::radio(0);
        for pair in 0..RECORD_FRAMES / 2 {
            let frames = [2 * pair, 2 * pair + 1]
                .map(|frame| {
                    let mut pcm = voiced(frame, 8000., 8);
                    preprocessor.run(&mut pcm, sensitivity);
                    encoder.encode(&pcm).unwrap()
                })
                .to_vec();
            core.receive(
                &Message::RelayAudio {
                    slot: 3,
                    utt: 1,
                    seq: pair as u16,
                    frames,
                },
                0,
            );
            out.extend([(); 2].map(|()| core.mix(volume, &hearing, 0)));
        }
        core.receive(
            &Message::RelayEnd {
                slot: 3,
                utt: 1,
                reason: 0,
            },
            0,
        );
        while core.talking(1_000) != 0 {
            out.push(core.mix(volume, &hearing, 1_000));
        }
        assert_eq!(core.stats().decoded as usize, RECORD_FRAMES);
        assert_eq!(core.stats().concealed + core.stats().underruns, 0);
        // The jitter buffer's wait before the first frame is not the voice's.
        let voice = out.split_off(out.len() - RECORD_FRAMES);
        assert!(out.iter().all(|frame| *frame == [[0; 2]; FRAME]));
        voice
    }
    fn rms(frames: &[Stereo]) -> f32 {
        let sum: f32 = frames
            .iter()
            .flatten()
            .map(|[left, _]| (f32::from(*left) / 32768.).powi(2))
            .sum();
        (sum / (frames.len() * FRAME) as f32).sqrt()
    }

    #[test]
    fn ten_seconds_are_recorded_then_played_back_once() {
        let (mut test, mut fake) = started(Mic::Voice, 1., 0.5);
        assert_eq!(shown(&test, &fake), Shown::Recording(10));
        assert_eq!(fake.open(), 1);
        // The seconds left, counted up, through the clock's wrap.
        for (at, left) in [(5, 10), (1_000, 9), (1_005, 9), (4_000, 6), (8_995, 2), (9_000, 1), (9_995, 1)] {
            run(&mut test, &mut fake, at);
            assert_eq!(shown(&test, &fake), Shown::Recording(left), "at {at} ms");
        }
        assert!(test.apply(Event::Play, fake.clock.get(), &mut fake).is_none());
        run(&mut test, &mut fake, 10_000);
        // The microphone is closed as the playback begins.
        assert_eq!(shown(&test, &fake), Shown::Playing(10));
        assert_eq!((fake.seen.opened.get(), fake.open()), (1, 0));
        let mut frames = 0;
        for left in (1..=10).rev() {
            for _ in 0..FRAMES_A_SECOND {
                assert_eq!(shown(&test, &fake), Shown::Playing(left), "frame {frames}");
                assert!(test.apply(Event::Play, fake.clock.get(), &mut fake).is_some());
                frames += 1;
                // Passes go on as it plays.
                test.apply(Event::Pass, fake.clock.get(), &mut fake);
            }
        }
        assert_eq!(frames, RECORD_FRAMES);
        // Once: after the last frame the button is as before.
        assert_eq!(shown(&test, &fake), Shown::Playing(1));
        assert!(test.apply(Event::Play, fake.clock.get(), &mut fake).is_none());
        assert!(test.is_idle());
        assert_eq!(shown(&test, &fake), Shown::Idle);
        assert_eq!(fake.seen.opened.get(), 1);
    }

    #[test]
    fn the_playback_is_what_a_teammate_beside_the_player_hears() {
        // Sample for sample, at the sliders of the press, whatever they are.
        for (volume, sensitivity) in [(1., 0.5), (0.4, 1.), (1., 0.)] {
            let (mut test, mut fake) = started(Mic::Voice, volume, sensitivity);
            run(&mut test, &mut fake, RECORD_MS);
            let own = played(&mut test, &mut fake);
            let theirs = heard_by_a_teammate(volume, sensitivity);
            assert_eq!(own.len(), RECORD_FRAMES);
            assert!(own == theirs, "{volume} {sensitivity}");
            assert!(rms(&own[250..]) > 0.005, "{volume} {sensitivity}: {}", rms(&own[250..]));
        }
    }

    #[test]
    fn the_sliders_of_the_press_set_the_level() {
        let level = |volume, sensitivity| {
            let (mut test, mut fake) = started(Mic::Voice, volume, sensitivity);
            run(&mut test, &mut fake, RECORD_MS);
            20. * rms(&played(&mut test, &mut fake)[250..]).log10()
        };
        let (quiet, default, loud) = (level(1., 0.), level(1., 0.5), level(1., 1.));
        // The sensitivity's ends are 12 dB apart, the default between them.
        assert!((10.0..14.0).contains(&(loud - quiet)), "{quiet} {default} {loud}");
        assert!((5.0..7.0).contains(&(default - quiet)), "{quiet} {default} {loud}");
        // Half the volume is 6 dB less; none is silence.
        assert!((5.9..6.1).contains(&(default - level(0.5, 0.5))));
        assert_eq!(level(0., 0.5), f32::NEG_INFINITY);
    }

    #[test]
    fn a_press_stops_a_recording_and_a_playback() {
        let (mut test, mut fake) = started(Mic::Voice, 1., 0.5);
        run(&mut test, &mut fake, 3_000);
        test.apply(press(1., 0.5), fake.clock.get(), &mut fake);
        assert!(test.is_idle());
        // The microphone is closed and what was recorded is gone.
        assert_eq!((fake.seen.opened.get(), fake.open()), (1, 0));
        assert!(test.apply(Event::Play, fake.clock.get(), &mut fake).is_none());
        // The next press is a new test, from its start.
        test.apply(press(1., 0.5), fake.clock.get(), &mut fake);
        assert_eq!(shown(&test, &fake), Shown::Recording(10));
        run(&mut test, &mut fake, 13_000);
        assert_eq!(shown(&test, &fake), Shown::Playing(10));
        for _ in 0..120 {
            assert!(test.apply(Event::Play, fake.clock.get(), &mut fake).is_some());
        }
        assert_eq!(shown(&test, &fake), Shown::Playing(8));
        test.apply(press(1., 0.5), fake.clock.get(), &mut fake);
        assert!(test.is_idle());
        assert!(test.apply(Event::Play, fake.clock.get(), &mut fake).is_none());
        assert_eq!((fake.seen.opened.get(), fake.open()), (2, 0));
    }

    #[test]
    fn talking_wins() {
        // Held, push-to-talk keeps the test from starting: no microphone opened.
        let mut fake = Fake::new(Some(Mic::Voice));
        let mut test = Test::Idle;
        let held = Event::Press {
            sliders: Sliders::new(1., 0.5),
            talking: true,
        };
        test.apply(held, 0, &mut fake);
        assert!(test.is_idle());
        assert_eq!(fake.seen.opened.get(), 0);
        // Pressed while the test records: the test ends and its microphone
        // is closed, for the transmission to open.
        let (mut test, mut fake) = started(Mic::Voice, 1., 0.5);
        run(&mut test, &mut fake, 2_000);
        test.apply(Event::Talk, fake.clock.get(), &mut fake);
        assert!(test.is_idle());
        assert_eq!(fake.open(), 0);
        // And while it plays: nothing more of it is played.
        let (mut test, mut fake) = started(Mic::Voice, 1., 0.5);
        run(&mut test, &mut fake, RECORD_MS);
        assert!(test.apply(Event::Play, fake.clock.get(), &mut fake).is_some());
        test.apply(Event::Talk, fake.clock.get(), &mut fake);
        assert!(test.is_idle());
        assert!(test.apply(Event::Play, fake.clock.get(), &mut fake).is_none());
        // It does not touch an idle test or a notice.
        let mut none = Fake::new(None);
        let mut test = Test::Idle;
        test.apply(Event::Talk, 0, &mut none);
        assert!(test.is_idle());
        test.apply(press(1., 0.5), 0, &mut none);
        test.apply(Event::Talk, 1, &mut none);
        assert_eq!(test.shown(1), Shown::NoMicrophone);
        // A press under the notice with push-to-talk held changes nothing either.
        test.apply(held, 2, &mut none);
        assert_eq!(test.shown(2), Shown::NoMicrophone);
    }

    #[test]
    fn it_stops_when_the_options_close_or_a_match_loads_or_ends() {
        let (mut test, mut fake) = started(Mic::Voice, 1., 0.5);
        run(&mut test, &mut fake, 500);
        test.apply(Event::Stop, fake.clock.get(), &mut fake);
        assert!(test.is_idle());
        assert_eq!(fake.open(), 0);
        let (mut test, mut fake) = started(Mic::Voice, 1., 0.5);
        run(&mut test, &mut fake, RECORD_MS);
        test.apply(Event::Stop, fake.clock.get(), &mut fake);
        assert!(test.is_idle());
        assert!(test.apply(Event::Play, fake.clock.get(), &mut fake).is_none());
        let mut none = Fake::new(None);
        let mut test = Test::Idle;
        test.apply(press(1., 0.5), 0, &mut none);
        test.apply(Event::Stop, 1, &mut none);
        assert!(test.is_idle());
        test.apply(Event::Stop, 2, &mut none);
        assert!(test.is_idle());
    }

    #[test]
    fn no_microphone_is_said_for_a_moment() {
        // None to open.
        let mut fake = Fake::new(None);
        fake.clock.set(START);
        let mut test = Test::Idle;
        test.apply(press(1., 0.5), START, &mut fake);
        assert_eq!(shown(&test, &fake), Shown::NoMicrophone);
        run(&mut test, &mut fake, NOTICE_MS - 5);
        assert_eq!(shown(&test, &fake), Shown::NoMicrophone);
        run(&mut test, &mut fake, NOTICE_MS);
        assert!(test.is_idle());
        // A press under the notice tries again (a microphone plugged in since).
        test.apply(press(1., 0.5), fake.clock.get(), &mut fake);
        assert_eq!(shown(&test, &fake), Shown::NoMicrophone);
        fake.microphone = Some(Mic::Voice);
        test.apply(press(1., 0.5), fake.clock.get(), &mut fake);
        assert_eq!(shown(&test, &fake), Shown::Recording(10));
        // One that fails as it records.
        let (mut test, mut fake) = started(Mic::Breaks, 1., 0.5);
        run(&mut test, &mut fake, 995);
        assert_eq!(shown(&test, &fake), Shown::Recording(10));
        run(&mut test, &mut fake, 1_020);
        assert_eq!(shown(&test, &fake), Shown::NoMicrophone);
        assert_eq!(fake.open(), 0);
        assert!(test.apply(Event::Play, fake.clock.get(), &mut fake).is_none());
        // One that opens and gives nothing: none, after its 10 s.
        let (mut test, mut fake) = started(Mic::Mute, 1., 0.5);
        run(&mut test, &mut fake, RECORD_MS - 5);
        assert_eq!(shown(&test, &fake), Shown::Recording(1));
        run(&mut test, &mut fake, RECORD_MS);
        assert_eq!(shown(&test, &fake), Shown::NoMicrophone);
        assert_eq!(fake.open(), 0);
        run(&mut test, &mut fake, RECORD_MS + NOTICE_MS);
        assert!(test.is_idle());
    }

    #[test]
    fn no_playback_device_is_said_for_a_moment() {
        // None at the press: nothing is recorded.
        let mut fake = Fake::new(Some(Mic::Voice));
        fake.playback = false;
        fake.clock.set(START);
        let mut test = Test::Idle;
        test.apply(press(1., 0.5), START, &mut fake);
        assert_eq!(shown(&test, &fake), Shown::NoPlayback);
        assert_eq!(fake.open(), 0);
        run(&mut test, &mut fake, NOTICE_MS);
        assert!(test.is_idle());
        // Without either it is the microphone the button names.
        fake.microphone = None;
        test.apply(press(1., 0.5), fake.clock.get(), &mut fake);
        assert_eq!(shown(&test, &fake), Shown::NoMicrophone);
        // Lost while the recording plays: the rest is dropped.
        let (mut test, mut fake) = started(Mic::Voice, 1., 0.5);
        run(&mut test, &mut fake, RECORD_MS);
        assert!(test.apply(Event::Play, fake.clock.get(), &mut fake).is_some());
        fake.playback = false;
        test.apply(Event::Pass, fake.clock.get(), &mut fake);
        assert_eq!(shown(&test, &fake), Shown::NoPlayback);
        assert!(test.apply(Event::Play, fake.clock.get(), &mut fake).is_none());
        // Lost while it records: said when the playback is due.
        let (mut test, mut fake) = started(Mic::Voice, 1., 0.5);
        fake.playback = false;
        run(&mut test, &mut fake, RECORD_MS);
        assert_eq!(shown(&test, &fake), Shown::Playing(10));
        run(&mut test, &mut fake, RECORD_MS + 5);
        assert_eq!(shown(&test, &fake), Shown::NoPlayback);
    }

    #[test]
    fn a_fast_microphone_is_kept_to_ten_seconds() {
        struct Flood;
        impl Microphone for Flood {
            fn capture(&mut self, mut frame: impl FnMut([i16; FRAME])) -> Result<(), u32> {
                (0..40).for_each(|n| frame(voiced(n, 8000., 8)));
                Ok(())
            }
        }
        struct Floods;
        impl Devices for Floods {
            type Microphone = Flood;
            fn microphone(&mut self) -> Option<Flood> {
                Some(Flood)
            }
            fn playback(&self) -> bool {
                true
            }
        }
        let mut test = Test::Idle;
        test.apply(press(1., 0.5), 0, &mut Floods);
        for now in (5..=RECORD_MS).step_by(5) {
            test.apply(Event::Pass, now, &mut Floods);
        }
        let mut frames = 0;
        while test.apply(Event::Play, RECORD_MS, &mut Floods).is_some() {
            frames += 1;
        }
        assert_eq!(frames, RECORD_FRAMES);
    }

    #[test]
    fn an_ask_is_never_left_with_a_worker_that_has_gone() {
        let press = Ask::Press(Sliders::new(1., 0.5));
        // A match's worker: its door stays open, the last ask stands.
        let mut door = Door::Open(None);
        assert_eq!(door.take(), None);
        assert!(door.ask(press) && door.ask(Ask::Stop));
        assert_eq!(door.take(), Some(Ask::Stop));
        assert_eq!(door.take(), None);
        // A test's own worker, leaving: not while something is asked.
        assert!(door.ask(press));
        assert!(!door.shut());
        assert_eq!(door.take(), Some(press));
        assert!(door.shut());
        // Gone: the game is told, and starts another.
        assert!(!door.ask(press));
        assert_eq!(door.take(), None);
        assert_eq!(door, Door::Shut);
        // The sliders are held to 0..1.
        assert_eq!(Sliders::new(7., f32::NAN), Sliders::new(1., 0.));
    }
}
