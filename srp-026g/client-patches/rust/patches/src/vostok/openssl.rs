//! The game's OpenSSL as the login client's stream drives it (patch 139).
//!
//! boost.asio's SSL engine (one, made with the login client: SSL_new at
//! 0x37BCE2) reaches OpenSSL 1.0.1h through four calls: `SSL_connect`
//! (do_connect 0x38B6D0), `SSL_read` (do_read 0x37FAB0), `SSL_write`
//! (do_write 0x37FAE0) and, after each, `SSL_get_error` (engine::perform
//! 0x37F940). The bytes it moves itself: what the socket read it puts into
//! one half of a BIO pair, the SSL's rbio and wbio being the other, and what
//! the SSL wrote there it takes out and sends. perform reads the answers so:
//! a result above 0 is done; `SSL_get_error` 2 (want read) has it read the
//! socket and call again; more bytes waiting in the pair than before the
//! call are sent first; `SSL_get_error` 1 fails the operation with
//! `ERR_get_error`'s code, which must not be 0 (0 is no error to boost).
//!
//! The sites are the four functions after their prologue (`openssl.connect`,
//! `.read`, `.write`, `.get_error`), each with the function's own return as
//! its exit, EAX the result.

use core::ffi::CStr;
use core::marker::PhantomData;

use sign_in_cipher::Random;
use sign_in_cipher::game::Wire;

use super::context::{Context, Reg};
use crate::raw;

/// The SSL of a call, as the bytes' way to the server and back: the pair's
/// half the engine gave it.
pub struct Stream<'c> {
    ssl: raw::Ssl,
    context: PhantomData<&'c Context<'c>>,
}

/// `SSL_get_error`'s answers the engine acts on.
#[derive(Clone, Copy)]
pub enum SslError {
    /// The call did what it was asked.
    None,
    /// The call failed for good.
    Ssl,
    /// The call needs more of the peer's bytes.
    WantRead,
}

/// What the three calls give for "not now, ask `SSL_get_error`".
const NOT_DONE: u32 = -1_i32 as u32;

/// ERR_LIB_SSL, and a reason of ours (above OpenSSL's own).
const LIBRARY_SSL: u32 = 20;
const REASON_SIGN_IN_CIPHER: u32 = 999;

fn stream<'c>(frame: Option<raw::SslCallFrame>) -> Option<Stream<'c>> {
    // SAFETY: the frame is the call's (EBP after the prologue), whose first
    // argument is the SSL, alive for the call.
    let ssl = unsafe { frame?.ssl()? };
    Some(Stream { ssl, context: PhantomData })
}

impl<'c> Stream<'c> {
    /// At `openssl.connect`. None for a call without an SSL (OpenSSL's own
    /// code is left to refuse it).
    pub fn at_connect(ctx: &'c Context<'_>) -> Option<Stream<'c>> {
        // SAFETY: the context is this site's.
        stream(unsafe { raw::site::openssl_connect::frame(ctx) })
    }

    /// At `openssl.read`: the stream and how many bytes the caller has room
    /// for ([`read`] gives them).
    pub fn at_read(ctx: &'c Context<'_>) -> Option<(Stream<'c>, usize)> {
        // SAFETY: the context is this site's: SSL_read's frame, whose third
        // argument is the length of the caller's buffer.
        unsafe {
            let frame = raw::site::openssl_read::frame(ctx);
            Some((stream(frame)?, *frame?.length() as usize))
        }
    }

    /// At `openssl.write`: the stream and the caller's bytes.
    pub fn at_write(ctx: &'c Context<'_>) -> Option<(Stream<'c>, &'c [u8])> {
        // SAFETY: as at_read, at SSL_write's frame: the bytes asio's do_write
        // gives, the write operation's, unchanged during the call.
        unsafe {
            let frame = raw::site::openssl_write::frame(ctx);
            let stream = stream(frame)?;
            let (bytes, length) = (frame?.bytes(), *frame?.length() as usize);
            (!bytes.is_null()).then(|| (stream, core::slice::from_raw_parts(bytes.cast_const(), length)))
        }
    }
}

impl Wire for Stream<'_> {
    fn receive(&mut self, into: &mut [u8]) -> usize {
        // SAFETY: the SSL's rbio, alive with it; BIO_read (0x44C630) copies
        // at most the length into the room and answers the count, or -1 when
        // the pair's half is empty.
        let read = unsafe { raw::engine_bio_read(self.ssl.rbio(), into.as_mut_ptr(), into.len() as u32) } as i32;
        usize::try_from(read).unwrap_or(0).min(into.len())
    }

    fn send(&mut self, bytes: &[u8]) -> bool {
        // SAFETY: the SSL's wbio, alive with it; BIO_write (0x44C740) copies
        // the bytes into the pair (17 KiB of room) and answers the count.
        let written = unsafe { raw::engine_bio_write(self.ssl.wbio(), bytes.as_ptr(), bytes.len() as u32) };
        written as usize == bytes.len()
    }

    fn random(&mut self) -> Option<Random> {
        let mut bytes = [0; 32];
        // SAFETY: ssleay_rand_bytes (0x46B6E0), OpenSSL's generator, which
        // the TLS handshake drew its secrets from on this same thread: 32
        // bytes into the room, 1 when they are random.
        let random = unsafe { raw::engine_rand_bytes(bytes.as_mut_ptr(), 32) } == 1;
        random.then(|| Random::from_generator(bytes))
    }
}

/// `SSL_read`'s result, at `openssl.read`: the bytes read, copied into the
/// caller's room (as many as it holds), or "ask `SSL_get_error`".
pub fn read(ctx: &mut Context<'_>, done: Option<&[u8]>) {
    let copied = done.and_then(|bytes| {
        // SAFETY: the context is this site's: SSL_read's frame, whose second
        // and third arguments are the caller's buffer and its length (asio's
        // do_read 0x37FAB0: the read operation's buffer), written by nothing
        // else during the call; no more than the length is copied.
        unsafe {
            let frame = raw::site::openssl_read::frame(&*ctx)?;
            let (room, length) = (frame.bytes(), *frame.length() as usize);
            let count = bytes.len().min(length);
            (!room.is_null()).then(|| {
                core::ptr::copy_nonoverlapping(bytes.as_ptr(), room, count);
                count
            })
        }
    });
    result(ctx, copied);
}

/// The result of `SSL_connect` or `SSL_write`, at its site: the count when
/// done, else "ask `SSL_get_error`".
pub fn result(ctx: &mut Context<'_>, done: Option<usize>) {
    let value = done.and_then(|count| i32::try_from(count).ok()).map_or(NOT_DONE, |count| count as u32);
    ctx.set_reg(Reg::Eax, value);
}

/// Queues the error a failed call leaves for the engine's `ERR_get_error`:
/// without one boost takes the failure for a success.
pub fn queue_failure(_: &Context<'_>) {
    const FILE: &CStr = c"skhid sign-in cipher";
    // SAFETY: ERR_put_error (0x44A270) on a game thread, as OpenSSL's own
    // functions call it; the file's name is a static.
    unsafe { raw::engine_err_put_error(LIBRARY_SSL, 0, REASON_SIGN_IN_CIPHER, FILE.as_ptr().cast(), 0) };
}

/// `SSL_get_error`'s result, at `openssl.get_error`.
pub fn error(ctx: &mut Context<'_>, error: SslError) {
    let code = match error {
        SslError::None => 0,
        SslError::Ssl => 1,
        SslError::WantRead => 2,
    };
    ctx.set_reg(Reg::Eax, code);
}
