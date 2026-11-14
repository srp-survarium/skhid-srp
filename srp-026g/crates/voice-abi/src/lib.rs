//! The game's optional voice DLL boundary. Load every symbol and verify the
//! version before constructing `Api`; a missing or incompatible DLL disables voice.
#![no_std]
use core::ffi::{CStr, c_char};
pub const ABI_VERSION: u8 = 5;
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
/// The options' Microphone list: the capture devices there are now, in
/// Windows' order, written into the room given (at most its length); how
/// many were written. With or without a match.
pub type Microphones = unsafe extern "C" fn(*mut Microphone, u32) -> u32;
/// The capture device push-to-talk and the microphone test open from now
/// on: a [`MicrophoneId`]'s text (NUL-terminated; the DLL keeps a copy), or
/// null for Windows' default. A device that is not there when it is opened
/// (unplugged): the default.
pub type ChooseMicrophone = unsafe extern "C" fn(*const c_char);
/// The room of a capture device's endpoint ID, its NUL counted.
pub const MICROPHONE_ID: usize = 128;
/// The room of a capture device's name (UTF-8), its NUL counted.
pub const MICROPHONE_NAME: usize = 256;
/// A capture device's endpoint ID, as WinMM reports it for its device
/// (`DRV_QUERYFUNCTIONINSTANCEID`: `{0.0.1.00000000}.{<a GUID>}` on Windows
/// and Wine), what the choice is saved and found again by: the same while
/// the device is unplugged and plugged in again, whatever WinMM's number for
/// it. 1 to 127 of ASCII's printable characters, no space; two are the same
/// whatever the case of their letters (a GUID's hex digits).
#[derive(Clone, Copy)]
pub struct MicrophoneId {
    /// The text and a zero after it, zeros to the end.
    text: [u8; MICROPHONE_ID],
}
impl MicrophoneId {
    pub fn parse(bytes: &[u8]) -> Option<MicrophoneId> {
        if bytes.is_empty() || bytes.len() >= MICROPHONE_ID || !bytes.iter().all(u8::is_ascii_graphic) {
            return None;
        }
        let mut text = [0; MICROPHONE_ID];
        text[..bytes.len()].copy_from_slice(bytes);
        Some(MicrophoneId { text })
    }
    pub fn as_bytes(&self) -> &[u8] {
        let end = self.text.iter().position(|&byte| byte == 0).unwrap_or(MICROPHONE_ID);
        &self.text[..end]
    }
    pub fn as_c_str(&self) -> &CStr {
        CStr::from_bytes_until_nul(&self.text).unwrap_or(c"")
    }
}
impl PartialEq for MicrophoneId {
    fn eq(&self, other: &MicrophoneId) -> bool {
        self.as_bytes().eq_ignore_ascii_case(other.as_bytes())
    }
}
impl Eq for MicrophoneId {}
impl core::fmt::Debug for MicrophoneId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("MicrophoneId").field(&self.as_c_str()).finish()
    }
}
/// A capture device as [`Microphones`] lists it: its endpoint ID and its
/// name as Windows shows it. The DLL writes it; the game reads it once with
/// [`Microphone::read`].
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Microphone {
    id: [u8; MICROPHONE_ID],
    name: [u8; MICROPHONE_NAME],
}
impl Microphone {
    pub const EMPTY: Microphone = Microphone { id: [0; MICROPHONE_ID], name: [0; MICROPHONE_NAME] };
    /// A name longer than its room is cut at a character; one with a zero
    /// in it ends there.
    pub fn new(id: MicrophoneId, name: &str) -> Microphone {
        let name = name.split('\0').next().unwrap_or("");
        let mut end = name.len().min(MICROPHONE_NAME - 1);
        while !name.is_char_boundary(end) {
            end -= 1;
        }
        let mut room = Microphone { id: id.text, ..Microphone::EMPTY };
        room.name[..end].copy_from_slice(&name.as_bytes()[..end]);
        room
    }
    /// Its ID and its name; None unless both are as [`Microphone::new`]
    /// makes them (an ID, and a name of UTF-8 that is not empty).
    pub fn read(&self) -> Option<(MicrophoneId, &CStr)> {
        let id = CStr::from_bytes_until_nul(&self.id).ok()?;
        let name = CStr::from_bytes_until_nul(&self.name).ok()?;
        let name = name.to_str().ok().filter(|name| !name.is_empty()).map(|_| name)?;
        Some((MicrophoneId::parse(id.to_bytes())?, name))
    }
}
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
    pub microphones: Microphones,
    pub choose_microphone: ChooseMicrophone,
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
            microphones: load!(c"sv_microphones", Microphones),
            choose_microphone: load!(c"sv_choose_microphone", ChooseMicrophone),
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
    fn a_microphone_reads_back_as_it_was_made_and_only_so() {
        let id = MicrophoneId::parse(b"{0.0.1.00000000}.{6b1a1d3c-1f9e-4c0e-9a59-0d2b5e3f7a10}").unwrap();
        let made = Microphone::new(id, "Микрофон (Realtek(R) Audio)");
        let (read, name) = made.read().unwrap();
        assert_eq!(read, id);
        assert_eq!(name.to_str(), Ok("Микрофон (Realtek(R) Audio)"));
        // the same device whatever the case of its GUID
        assert_eq!(MicrophoneId::parse(b"{0.0.1.00000000}.{6B1A1D3C-1F9E-4C0E-9A59-0D2B5E3F7A10}"), Some(id));
        assert_ne!(MicrophoneId::parse(b"{0.0.1.00000000}.{6b1a1d3c}"), Some(id));
        // a long name cut at a character, within its room with its zero
        let long = "Ж".repeat(200);
        let made = Microphone::new(id, &long);
        let (_, cut) = made.read().unwrap();
        assert!(cut.count_bytes() < MICROPHONE_NAME && long.starts_with(cut.to_str().unwrap()));
        assert_eq!(cut.count_bytes(), 254);
        assert_eq!(Microphone::new(id, "Mic\0hidden").read().unwrap().1, c"Mic");
        // none of these is a device
        assert!(Microphone::EMPTY.read().is_none());
        assert!(Microphone::new(id, "").read().is_none());
        let mut broken = Microphone::new(id, "Mic");
        broken.name = [b'a'; MICROPHONE_NAME];
        assert!(broken.read().is_none());
        broken = Microphone::new(id, "Mic");
        broken.name[..2].copy_from_slice(&[0xD0, 0]);
        assert!(broken.read().is_none());
        for bad in [&b""[..], b"a b", b"\xD0\x9C", b"a\0", &[b'a'; MICROPHONE_ID]] {
            assert_eq!(MicrophoneId::parse(bad), None, "{bad:?}");
        }
        assert_eq!(MicrophoneId::parse(&[b'a'; MICROPHONE_ID - 1]).map(|id| id.as_bytes().len()), Some(127));
        assert_eq!(id.as_c_str().to_bytes(), id.as_bytes());
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
