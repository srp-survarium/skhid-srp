//! The game's optional voice DLL boundary. Load every symbol and verify the
//! version before constructing `Api`; a missing or incompatible DLL disables voice.
#![no_std]
use core::ffi::c_char;
pub const ABI_VERSION: u8 = 4;
pub type Version = unsafe extern "C" fn() -> i32;
pub type Start = unsafe extern "C" fn(*const c_char, u32, u32) -> i32;
pub type Stop = unsafe extern "C" fn();
pub type Ptt = unsafe extern "C" fn(i32);
pub type Settings = unsafe extern "C" fn(f32, i32, f32);
/// How a place is heard, all of it in one call ([`Heard::call`]): the place,
/// its plain voice's gain in the left and in the right ear, and whether its
/// radio is on (not 0).
pub type Hear = unsafe extern "C" fn(i32, f32, f32, i32);
pub type Mute = unsafe extern "C" fn(u32);
pub type Talking = unsafe extern "C" fn() -> u32;
pub type Slot = unsafe extern "C" fn() -> i32;
pub type VoiceId = unsafe extern "C" fn(i32) -> u32;
/// The options' "Test microphone" button pressed, with the "Voice chat"
/// volume and the "Microphone sensitivity" as their sliders stand (each
/// 0..1): starts the test, or stops the one that runs. With or without a
/// match's session.
pub type TestPress = unsafe extern "C" fn(f32, f32);
/// The test stopped, if one runs (the options closed, a match loads or ends).
pub type TestStop = unsafe extern "C" fn();
/// What the test does now: a [`MicTest`]'s word.
pub type TestState = unsafe extern "C" fn() -> u32;
/// The microphone test as the options' button shows it (the owner,
/// 2026-11-14: "you press button, it records for 10 seconds, then plays it
/// back"). The DLL owns the test; this is all the game knows of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MicTest {
    Idle,
    /// The microphone is recorded: the seconds left, counted up to whole ones.
    Recording(u8),
    /// The recording plays back: the seconds left of it.
    Playing(u8),
    /// No microphone, or it cannot be opened: said for a moment, then idle.
    NoMicrophone,
    /// No playback device: the same.
    NoPlayback,
}
impl MicTest {
    /// `sv_test_state`'s word: the state in its low byte, the seconds in the next.
    pub const fn word(self) -> u32 {
        match self {
            MicTest::Idle => 0,
            MicTest::Recording(seconds) => 1 | (seconds as u32) << 8,
            MicTest::Playing(seconds) => 2 | (seconds as u32) << 8,
            MicTest::NoMicrophone => 3,
            MicTest::NoPlayback => 4,
        }
    }
    /// The state of a word; None for one `word` does not make.
    pub const fn of_word(word: u32) -> Option<MicTest> {
        let seconds = (word >> 8) as u8;
        let test = match word & 0xFF {
            0 => MicTest::Idle,
            1 => MicTest::Recording(seconds),
            2 => MicTest::Playing(seconds),
            3 => MicTest::NoMicrophone,
            4 => MicTest::NoPlayback,
            _ => return None,
        };
        if test.word() == word { Some(test) } else { None }
    }
}
/// A plain voice's gain in the left and in the right ear: what the game's
/// sound engine gives a sound at the speaker's place (its pan, its gain by
/// distance). 1 in both is the voice as decoded.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ears([f32; 2]);
impl Ears {
    pub const SILENT: Ears = Ears([0.0; 2]);
    /// As decoded in both ears.
    pub const UNITY: Ears = Ears([1.0; 2]);
    /// The most an ear is given (+6 dB).
    pub const MOST: f32 = 2.0;
    /// Each held to 0..=[`Ears::MOST`]; not a number: silent.
    pub fn new(left: f32, right: f32) -> Ears {
        Ears([left, right].map(|gain| if gain.is_nan() { 0.0 } else { gain.clamp(0.0, Ears::MOST) }))
    }
    pub fn get(self) -> [f32; 2] {
        self.0
    }
}
/// How a place is heard.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Heard {
    /// Its voice plain, in the world: from its side and by its distance.
    Plain(Ears),
    /// Through the radio only, centred.
    Radio,
    /// Both: where the plain voice still reaches and the radio is already on
    /// (only when the two distances differ).
    Both(Ears),
}
impl Heard {
    /// [`Hear`]'s arguments after the place.
    pub fn call(self) -> (f32, f32, i32) {
        let (ears, radio) = match self {
            Heard::Plain(ears) => (ears, 0),
            Heard::Radio => (Ears::SILENT, 1),
            Heard::Both(ears) => (ears, 1),
        };
        (ears.0[0], ears.0[1], radio)
    }
    /// What [`Hear`]'s arguments say. A radio with a silent plain voice is
    /// the radio alone.
    pub fn of_call(left: f32, right: f32, radio: i32) -> Heard {
        let ears = Ears::new(left, right);
        match (radio != 0, ears == Ears::SILENT) {
            (false, _) => Heard::Plain(ears),
            (true, true) => Heard::Radio,
            (true, false) => Heard::Both(ears),
        }
    }
}
#[derive(Clone, Copy)]
pub struct Api {
    pub version: Version,
    pub start: Start,
    pub stop: Stop,
    pub ptt: Ptt,
    pub settings: Settings,
    pub hear: Hear,
    pub mute: Mute,
    pub talking: Talking,
    pub slot: Slot,
    pub voice_id: VoiceId,
    pub test_press: TestPress,
    pub test_stop: TestStop,
    pub test_state: TestState,
}
impl Api {
    /// Resolve and check the complete optional DLL interface.
    ///
    /// # Safety
    /// `resolve` must return either null or the DLL's callable C export for
    /// the requested name. The library must remain loaded for this Api's life.
    pub unsafe fn load(
        mut resolve: impl FnMut(&core::ffi::CStr) -> *mut core::ffi::c_void,
    ) -> Option<Self> {
        macro_rules! load {
            ($name:literal, $ty:ty) => {{
                let address = resolve($name);
                if address.is_null() {
                    return None;
                }
                // The caller supplies the named export with the ABI declared here.
                unsafe { core::mem::transmute::<*mut core::ffi::c_void, $ty>(address) }
            }};
        }
        let version = load!(c"sv_version", Version);
        if unsafe { version() } != i32::from(ABI_VERSION) {
            return None;
        }
        Some(Self {
            version,
            start: load!(c"sv_start", Start),
            stop: load!(c"sv_stop", Stop),
            ptt: load!(c"sv_ptt", Ptt),
            settings: load!(c"sv_settings", Settings),
            hear: load!(c"sv_hear", Hear),
            mute: load!(c"sv_mute", Mute),
            talking: load!(c"sv_talking", Talking),
            slot: load!(c"sv_slot", Slot),
            voice_id: load!(c"sv_voice_id", VoiceId),
            test_press: load!(c"sv_test_press", TestPress),
            test_stop: load!(c"sv_test_stop", TestStop),
            test_state: load!(c"sv_test_state", TestState),
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_test_states_word_reads_back_and_no_other_word_does() {
        let states = [MicTest::Idle, MicTest::Recording(10), MicTest::Recording(1), MicTest::Playing(10),
                      MicTest::Playing(0), MicTest::NoMicrophone, MicTest::NoPlayback];
        for state in states {
            assert_eq!(MicTest::of_word(state.word()), Some(state));
        }
        for word in [5, 0xFF, 0x100, 0x103, 0x0001_0001, u32::MAX] {
            assert_eq!(MicTest::of_word(word), None, "{word:#x}");
        }
    }
    #[test]
    fn a_place_is_heard_as_its_one_call_says() {
        let ears = Ears::new(0.25, 1.5);
        for heard in [Heard::Plain(ears), Heard::Plain(Ears::SILENT), Heard::Radio, Heard::Both(ears)] {
            let (left, right, radio) = heard.call();
            assert_eq!(Heard::of_call(left, right, radio), heard);
        }
        assert_eq!(Heard::of_call(0.0, 0.0, 7), Heard::Radio);
        assert_eq!(Ears::new(f32::NAN, 9.0).get(), [0.0, Ears::MOST]);
        assert_eq!(Ears::new(-1.0, f32::INFINITY).get(), [0.0, Ears::MOST]);
        assert_eq!(Heard::of_call(f32::NAN, f32::NAN, 1), Heard::Radio);
    }
}
