//! The game's optional voice DLL boundary. Load every symbol and verify the
//! version before constructing `Api`; a missing or incompatible DLL disables voice.
#![no_std]
use core::ffi::c_char;
pub const ABI_VERSION: u8 = 1;
pub type Version = unsafe extern "C" fn() -> i32;
pub type Start = unsafe extern "C" fn(*const c_char, u32, u32) -> i32;
pub type Stop = unsafe extern "C" fn();
pub type Ptt = unsafe extern "C" fn(i32);
pub type Settings = unsafe extern "C" fn(f32, i32, f32);
pub type Radio = unsafe extern "C" fn(u32);
pub type Mute = unsafe extern "C" fn(u32);
pub type Talking = unsafe extern "C" fn() -> u32;
pub type Slot = unsafe extern "C" fn() -> i32;
pub type LastSpeaker = unsafe extern "C" fn() -> i32;
pub type VoiceId = unsafe extern "C" fn(i32) -> u32;
#[derive(Clone, Copy)]
pub struct Api {
    pub version: Version,
    pub start: Start,
    pub stop: Stop,
    pub ptt: Ptt,
    pub settings: Settings,
    pub radio: Radio,
    pub mute: Mute,
    pub talking: Talking,
    pub slot: Slot,
    pub last_speaker: LastSpeaker,
    pub voice_id: VoiceId,
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
            mute: load!(c"sv_mute", Mute),
            talking: load!(c"sv_talking", Talking),
            slot: load!(c"sv_slot", Slot),
            last_speaker: load!(c"sv_last_speaker", LastSpeaker),
            voice_id: load!(c"sv_voice_id", VoiceId),
        })
    }
}
