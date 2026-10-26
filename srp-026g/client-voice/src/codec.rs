//! The only unsafe portable boundary: owned Opus objects, fixed-size PCM frames.
use crate::{FRAME, RATE};
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
        // Two encoded frames and their headers must fit the relay's 400-byte cap.
        let mut out = vec![0; 192];
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
