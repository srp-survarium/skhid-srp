//! Team voice's portable codec, protocol, jitter buffers and retail radio.
//! The Windows DLL adds capture/playback and a UDP session around these types.
mod codec;
mod radio;
mod receiver;
#[cfg(any(windows, test))]
mod runtime;
mod sender;
mod sounds;
#[cfg(windows)]
mod windows;
pub mod wire;
pub use codec::{Decoder, Encoder};
pub use radio::{Radio, mic_gain};
pub use receiver::{Core, Stats};
pub use sender::Sender;
pub const RATE: usize = 16_000;
pub const FRAME: usize = 320;
pub const SLOTS: usize = 20;
#[cfg(test)]
mod tests;
