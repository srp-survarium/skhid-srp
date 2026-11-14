//! The game's optional voice DLL boundary. Load every symbol and verify the
//! version before constructing `Api`; a missing or incompatible DLL disables voice.
#![no_std]
use core::ffi::c_char;
pub const ABI_VERSION: u8 = 3;
pub type Version = unsafe extern "C" fn() -> i32;
pub type Start = unsafe extern "C" fn(*const c_char, u32, u32) -> i32;
pub type Stop = unsafe extern "C" fn();
pub type Ptt = unsafe extern "C" fn(i32);
pub type Settings = unsafe extern "C" fn(f32, i32, f32);
pub type Radio = unsafe extern "C" fn(u32);
/// A place's [`Pan`] (its sine), for when it is heard directly.
pub type Direction = unsafe extern "C" fn(i32, f32);
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
/// Where a speaker heard directly (not through the radio) is, left to right:
/// the sine of its bearing from where the listener faces, -1 full left, 0
/// ahead or behind, 1 full right.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pan(f32);
impl Pan {
    pub const CENTRE: Pan = Pan(0.0);
    /// Held to -1..1; not a number: the centre.
    pub fn new(sine: f32) -> Pan {
        if sine.is_nan() {
            Pan::CENTRE
        } else {
            Pan(sine.clamp(-1.0, 1.0))
        }
    }
    pub fn get(self) -> f32 {
        self.0
    }
}
#[derive(Clone, Copy)]
pub struct Api {
    pub version: Version,
    pub start: Start,
    pub stop: Stop,
    pub ptt: Ptt,
    pub settings: Settings,
    pub radio: Radio,
    pub direction: Direction,
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
            radio: load!(c"sv_radio", Radio),
            direction: load!(c"sv_direction", Direction),
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
    use super::MicTest;
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
}
