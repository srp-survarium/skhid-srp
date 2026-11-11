//! Patch 139: the sign-in's password goes through srp's own cipher, where
//! the game's protocol had TLS (docs/notes/sign-in-cipher.md).
//!
//! After the server's "user name valid" byte the game starts its stream's
//! TLS handshake, writes the password message and reads the answer, all
//! through OpenSSL's `SSL_connect`, `SSL_write` and `SSL_read`, asking
//! `SSL_get_error` after each (vostok/openssl.rs). The game's code stays as
//! it is; those four calls are answered here by the cipher's machine
//! (`sign_in_cipher::game`): its hello for the handshake, a sealed record
//! for a write, the opened record for a read. Only the login client's
//! stream calls them, on the network's thread.
//!
//! Nothing a match computes.

#![forbid(unsafe_code)]

use sign_in_cipher::{MAX_MESSAGE, SKHID};
use sign_in_cipher::game::{Answer, Call, Session};

use crate::vostok::openssl::{self, SslError, Stream};
use crate::vostok::{
    Context, Flow, Lock, NoExit, SignInCipherOnConnectExit, SignInCipherOnGetErrorExit, SignInCipherOnReadExit,
    SignInCipherOnWriteExit,
};

/// The sign-in under way, and what `SSL_get_error` is to say of the last
/// call.
struct Kept {
    session: Session,
    last: SslError,
}

static KEPT: Lock<Kept> = Lock::new(Kept { session: Session::new(), last: SslError::None });

pub fn on_begin(ctx: &mut Context<'_>) -> Flow<NoExit> {
    KEPT.with(ctx.thread(), |kept| kept.session.begin());
    Flow::Continue
}

/// One call answered: the count when done, and the error kept for
/// `SSL_get_error`.
fn answer(ctx: &Context<'_>, call: Call<'_>, stream: &mut Stream<'_>) -> Option<usize> {
    let answer = KEPT.with(ctx.thread(), |kept| {
        let answer = kept.session.call(call, SKHID, stream);
        kept.last = match answer {
            Answer::Done(_) => SslError::None,
            Answer::WantRead => SslError::WantRead,
            Answer::Failed => SslError::Ssl,
        };
        answer
    });
    match answer {
        Answer::Done(count) => Some(count),
        Answer::WantRead => None,
        Answer::Failed => {
            openssl::queue_failure(ctx);
            None
        }
    }
}

pub fn on_connect(ctx: &mut Context<'_>) -> Flow<SignInCipherOnConnectExit> {
    let Some(mut stream) = Stream::at_connect(ctx) else { return Flow::Continue };
    let done = answer(ctx, Call::Connect, &mut stream);
    openssl::result(ctx, done);
    Flow::Exit(SignInCipherOnConnectExit::Done)
}

pub fn on_read(ctx: &mut Context<'_>) -> Flow<SignInCipherOnReadExit> {
    let Some((mut stream, room)) = Stream::at_read(ctx) else { return Flow::Continue };
    // A record's message is at most MAX_MESSAGE; the game asks for 52h.
    let mut message = [0; MAX_MESSAGE];
    let room = room.min(message.len());
    let done = answer(ctx, Call::Read(&mut message[..room]), &mut stream);
    openssl::read(ctx, done.map(|count| &message[..count]));
    Flow::Exit(SignInCipherOnReadExit::Done)
}

pub fn on_write(ctx: &mut Context<'_>) -> Flow<SignInCipherOnWriteExit> {
    let done = {
        let Some((mut stream, bytes)) = Stream::at_write(ctx) else { return Flow::Continue };
        answer(ctx, Call::Write(bytes), &mut stream)
    };
    openssl::result(ctx, done);
    Flow::Exit(SignInCipherOnWriteExit::Done)
}

pub fn on_get_error(ctx: &mut Context<'_>) -> Flow<SignInCipherOnGetErrorExit> {
    let last = KEPT.with(ctx.thread(), |kept| kept.last);
    openssl::error(ctx, last);
    Flow::Exit(SignInCipherOnGetErrorExit::Done)
}
