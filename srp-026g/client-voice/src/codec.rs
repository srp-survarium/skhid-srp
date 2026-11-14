//! The only unsafe portable boundary: owned Opus and speexdsp objects,
//! fixed-size PCM frames.
use crate::{FRAME, RATE, radio::unit};
use std::{
    ffi::{c_int, c_void},
    ptr::NonNull,
};
unsafe extern "C" {
    fn opus_encoder_create(
        rate: c_int,
        channels: c_int,
        application: c_int,
        error: *mut c_int,
    ) -> *mut c_void;
    fn opus_encoder_destroy(encoder: *mut c_void);
    fn opus_encoder_ctl(encoder: *mut c_void, request: c_int, ...) -> c_int;
    fn opus_encode(
        encoder: *mut c_void,
        pcm: *const i16,
        frame_size: c_int,
        data: *mut u8,
        max_bytes: c_int,
    ) -> c_int;
    fn opus_decoder_create(rate: c_int, channels: c_int, error: *mut c_int) -> *mut c_void;
    fn opus_decoder_destroy(decoder: *mut c_void);
    fn opus_decoder_ctl(decoder: *mut c_void, request: c_int, ...) -> c_int;
    fn opus_decode_float(
        decoder: *mut c_void,
        data: *const u8,
        len: c_int,
        pcm: *mut f32,
        frame_size: c_int,
        fec: c_int,
    ) -> c_int;
    fn speex_preprocess_state_init(frame_size: c_int, sampling_rate: c_int) -> *mut c_void;
    fn speex_preprocess_state_destroy(state: *mut c_void);
    fn speex_preprocess_ctl(state: *mut c_void, request: c_int, value: *mut c_void) -> c_int;
    fn speex_preprocess_run(state: *mut c_void, pcm: *mut i16) -> c_int;
}
// speex_preprocess.h's requests.
const SET_DENOISE: c_int = 0;
const SET_AGC: c_int = 2;
const SET_AGC_LEVEL: c_int = 6;
/// The loudness automatic gain aims at, of speexdsp's 1..32768: 0.69's
/// `voice_chat_preprocess_gain` default (speexdsp's own is 8000).
pub const AGC_LEVEL: f32 = 12_000.;
/// The game's "Microphone sensitivity" (0..1) moves the aim by up to 6 dB
/// either way: 0.5 is 0.69's level.
pub fn agc_level(sensitivity: f32) -> f32 {
    AGC_LEVEL * 2f32.powf((unit(sensitivity) - 0.5) * 2.)
}
/// The microphone's processing before Opus, as 0.69's voice_recorder:
/// speexdsp's noise suppression and automatic gain, which brings a quiet
/// microphone up and a loud one down to the same loudness. One per session,
/// so the gain it has learnt carries from one push-to-talk to the next.
pub struct Preprocessor {
    state: NonNull<c_void>,
    level: f32,
}
impl Preprocessor {
    pub fn new() -> Result<Self, String> {
        // speexdsp owns the returned object; Drop frees it, and all accesses are exclusive.
        let ptr = unsafe { speex_preprocess_state_init(FRAME as c_int, RATE as c_int) };
        let mut pre = Self {
            state: NonNull::new(ptr).ok_or_else(|| "speexdsp preprocessor".to_owned())?,
            level: AGC_LEVEL,
        };
        for request in [SET_DENOISE, SET_AGC] {
            let mut on: i32 = 1;
            pre.ctl(request, (&raw mut on).cast())?;
        }
        let mut level = AGC_LEVEL;
        pre.ctl(SET_AGC_LEVEL, (&raw mut level).cast())?;
        Ok(pre)
    }
    fn ctl(&mut self, request: c_int, value: *mut c_void) -> Result<(), String> {
        // Each request reads one value of the type speex_preprocess.h gives it.
        let result = unsafe { speex_preprocess_ctl(self.state.as_ptr(), request, value) };
        if result != 0 {
            return Err(format!("speexdsp setting {request}: {result}"));
        }
        Ok(())
    }
    /// Process one captured frame in place, aiming at the level of the
    /// game's sensitivity.
    pub fn run(&mut self, pcm: &mut [i16; FRAME], sensitivity: f32) {
        let mut level = agc_level(sensitivity);
        if level != self.level && self.ctl(SET_AGC_LEVEL, (&raw mut level).cast()).is_ok() {
            self.level = level;
        }
        // The state was made for FRAME samples; the frame is exactly that.
        unsafe { speex_preprocess_run(self.state.as_ptr(), pcm.as_mut_ptr()) };
    }
}
impl Drop for Preprocessor {
    fn drop(&mut self) {
        unsafe { speex_preprocess_state_destroy(self.state.as_ptr()) }
    }
}
pub struct Encoder(NonNull<c_void>);
impl Encoder {
    pub fn new() -> Result<Self, String> {
        let mut error = 0;
        // Opus owns the returned object; Drop frees it, and all accesses are exclusive.
        let ptr = unsafe { opus_encoder_create(RATE as c_int, 1, 2048, &mut error) };
        let enc = Self(NonNull::new(ptr).ok_or_else(|| format!("Opus encoder: {error}"))?);
        if error != 0 {
            return Err(format!("Opus encoder: {error}"));
        }
        for (request, value) in [
            (4002, 16000),
            (4006, 1),
            (4012, 0),
            (4016, 0),
            (4024, 3001),
            (4010, 5),
        ] {
            let result = unsafe { opus_encoder_ctl(enc.0.as_ptr(), request, value as c_int) };
            if result != 0 {
                return Err(format!("Opus setting {request}: {result}"));
            }
        }
        Ok(enc)
    }
    pub fn encode(&mut self, pcm: &[i16; FRAME]) -> Result<Vec<u8>, String> {
        // No more room than the relay and the games take: the encoder keeps
        // a frame within it (its own measured at most 93 bytes).
        let mut out = vec![0; crate::wire::MAX_FRAME_BYTES];
        let n = unsafe {
            opus_encode(
                self.0.as_ptr(),
                pcm.as_ptr(),
                FRAME as c_int,
                out.as_mut_ptr(),
                out.len() as c_int,
            )
        };
        if n <= 0 {
            return Err(format!("Opus encode: {n}"));
        }
        out.truncate(n as usize);
        Ok(out)
    }
}
impl Drop for Encoder {
    fn drop(&mut self) {
        unsafe { opus_encoder_destroy(self.0.as_ptr()) }
    }
}
pub struct Decoder(NonNull<c_void>);
impl Decoder {
    pub fn new() -> Result<Self, String> {
        let mut error = 0;
        let ptr = unsafe { opus_decoder_create(RATE as c_int, 1, &mut error) };
        let dec = Self(NonNull::new(ptr).ok_or_else(|| format!("Opus decoder: {error}"))?);
        if error != 0 {
            return Err(format!("Opus decoder: {error}"));
        }
        Ok(dec)
    }
    pub fn reset(&mut self) {
        unsafe {
            opus_decoder_ctl(self.0.as_ptr(), 4028);
        }
    }
    pub fn decode(&mut self, frame: Option<&[u8]>) -> Result<[f32; FRAME], String> {
        let mut out = [0.; FRAME];
        let (ptr, len) = frame.map_or((std::ptr::null(), 0), |f| (f.as_ptr(), f.len()));
        let len = c_int::try_from(len).map_err(|_| "Opus frame too long".to_owned())?;
        let n = unsafe {
            opus_decode_float(
                self.0.as_ptr(),
                ptr,
                len,
                out.as_mut_ptr(),
                FRAME as c_int,
                0,
            )
        };
        if n != FRAME as c_int {
            return Err(format!("Opus decode: {n}"));
        }
        Ok(out)
    }
}
impl Drop for Decoder {
    fn drop(&mut self) {
        unsafe { opus_decoder_destroy(self.0.as_ptr()) }
    }
}
// Opus objects contain no thread-affine resources. Each wrapper owns its
// pointer and requires &mut self for every codec operation; transferring that
// ownership is safe, while sharing concurrent access (Sync) is deliberately not.
unsafe impl Send for Encoder {}
unsafe impl Send for Decoder {}
unsafe impl Send for Preprocessor {}
