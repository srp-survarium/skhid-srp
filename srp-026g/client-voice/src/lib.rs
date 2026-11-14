//! Team voice's portable codec, protocol, jitter buffers and retail radio.
//! The Windows DLL adds capture/playback and a UDP session around these types.
mod codec;
pub mod mic_test;
mod radio;
mod receiver;
#[cfg(any(windows, test))]
mod runtime;
mod sender;
mod sounds;
#[cfg(windows)]
mod windows;
pub mod wire;
pub use codec::{AGC_LEVEL, Decoder, Encoder, Preprocessor, agc_level};
pub use radio::Radio;
pub use receiver::{Core, Heard, Hearing, Pan, Stats, Stereo, gains};
pub use sender::Sender;
pub const RATE: usize = 16_000;
pub const FRAME: usize = 320;
pub const SLOTS: usize = 20;
#[cfg(test)]
mod tests;
