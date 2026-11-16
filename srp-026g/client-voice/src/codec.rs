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
const SET_AGC_INCREMENT: c_int = 26;
const SET_AGC_MAX_GAIN: c_int = 30;
/// The loudness automatic gain aims at, of speexdsp's 1..32768: 0.69's
/// `voice_chat_preprocess_gain` default (speexdsp's own is 8000).
pub const AGC_LEVEL: f32 = 12_000.;
/// The most the automatic gain raises a microphone, dB (speexdsp's own: 30).
/// With 30 a voice was held at its level only down to 30 dB under a loud
/// microphone's; one 40 dB under came out 9 dB quieter than everyone else,
/// one 50 dB under 24 dB quieter: fine beside the listener and poor through
/// the radio, whose static stays where it is (the owner, 2026-11-16, of two
/// players: "it was fine close by", "at a distance with radio it was worse",
/// "it wasn't as loud as when I heard" the other). With 45 the one 40 dB
/// under is within 2 dB (50 under: 12 dB quieter, speexdsp's own floor).
pub const AGC_MAX_GAIN: i32 = 45;
/// How fast the gain rises, dB a second (speexdsp's own: 12): a quiet
/// microphone's first words of a match were 6 dB under its level for
/// seconds; twice as fast they are 2.
pub const AGC_INCREMENT: i32 = 24;
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
        for (request, mut value) in [(SET_AGC_MAX_GAIN, AGC_MAX_GAIN), (SET_AGC_INCREMENT, AGC_INCREMENT)] {
            pre.ctl(request, (&raw mut value).cast())?;
        }
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
/// The encoder's rate, bits a second: a third of it is each frame's copy of
/// the frame before ([`EXPECTED_LOSS`]), the rest about the 16 kbit/s the
/// voice alone had until 2026-11-16.
pub const BITRATE: i32 = 24_000;
/// The loss the encoder is told to expect, per cent: with it (and the rate
/// above SILK's threshold for it, 18.4 kbit/s wide band) every frame carries
/// a coarser copy of the frame before it (Opus' in-band forward error
/// correction), which [`Decoder::recover`] plays when that frame did not
/// come.
pub const EXPECTED_LOSS: i32 = 10;
pub struct Encoder(NonNull<c_void>);
impl Encoder {
    pub fn new() -> Result<Self, String> {
        Self::with(BITRATE, EXPECTED_LOSS)
    }
    /// At `bitrate`, with each frame's copy of the frame before when
    /// `expected_loss` (per cent) is not 0.
    pub fn with(bitrate: i32, expected_loss: i32) -> Result<Self, String> {
        let mut error = 0;
        // Opus owns the returned object; Drop frees it, and all accesses are exclusive.
        let ptr = unsafe { opus_encoder_create(RATE as c_int, 1, 2048, &mut error) };
        let enc = Self(NonNull::new(ptr).ok_or_else(|| format!("Opus encoder: {error}"))?);
        if error != 0 {
            return Err(format!("Opus encoder: {error}"));
        }
        // The bit rate, variable, in-band FEC, the loss expected, no DTX,
        // a voice, complexity 5.
        for (request, value) in [
            (4002, bitrate),
            (4006, 1),
            (4012, i32::from(expected_loss != 0)),
            (4014, expected_loss),
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
        // a frame within it (its own measured at most 114 bytes).
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
    /// The frame's 20 ms; with none, the decoder's concealment of them.
    pub fn decode(&mut self, frame: Option<&[u8]>) -> Result<[f32; FRAME], String> {
        self.run(frame, false)
    }
    /// The 20 ms of a frame that did not come, from the copy of it in the
    /// frame after it, `next` (the decoder conceals them when `next` has no
    /// copy). `next` itself is decoded after, as usual.
    pub fn recover(&mut self, next: &[u8]) -> Result<[f32; FRAME], String> {
        self.run(Some(next), true)
    }
    fn run(&mut self, frame: Option<&[u8]>, fec: bool) -> Result<[f32; FRAME], String> {
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
                c_int::from(fec),
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
