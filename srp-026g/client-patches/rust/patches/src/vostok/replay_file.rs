//! The replay reader's own open boundary; no process-wide file-system changes.
use super::{Context, Reg};
use crate::raw;

/// At replay.before_open, EDI owns the local open options. The native path
/// normalizer and Windows open both read its assert_on_failure member.
pub fn return_open_errors(ctx: &mut Context<'_>) {
    // SAFETY: 1F0245 has built EDI from this frame's options at ESP+2Ch;
    // it remains live through the synchronous open immediately after the hook.
    unsafe {
        if let Some(options) = raw::site::replay_before_open::options(ctx) {
            *options.assert_on_failure() = 0;
        }
    }
}

/// Owns a bounded copy of the resolved ANSI path; no engine borrow survives
/// the modal Windows dialog. Constructed only on the native failed-open branch.
pub struct FailedReplay {
    path: [u8; 260],
    len: usize,
}

/// The native open's boolean is still in AL at replay.open_result. Both
/// outcomes reach this site; REPLAY_OPEN_RESULT bypasses the reader assertion.
pub fn failure(ctx: &Context<'_>) -> Option<FailedReplay> {
    if ctx.reg(Reg::Eax) as u8 != 0 {
        None
    } else {
        Some(FailedReplay::at_open_failure(ctx))
    }
}

impl FailedReplay {
    fn at_open_failure(ctx: &Context<'_>) -> Self {
        // SAFETY: this hook holds the caller-owned replay path until return.
        let source = unsafe { raw::site::replay_open_result::path(ctx).map(|p| p.bytes()) };
        let mut result = Self {
            path: [0; 260],
            len: 0,
        };
        let Some(source) = source else { return result };
        for slot in &mut result.path[..259] {
            // SAFETY: ESI is the caller's NUL-terminated resolved replay path.
            // Both callers own a 260-byte path; stop at NUL, at most 259 bytes.
            let byte = unsafe { *source.add(result.len) };
            if byte == 0 {
                break;
            }
            *slot = byte;
            result.len += 1;
        }
        result
    }

    fn message(&self) -> [u8; 768] {
        let mut text = [0; 768];
        let prefix = b"Couldn't open replay:\r\n\r\n";
        let suffix = b"\r\n\r\nCheck that this .sur file exists and you have permission to read it.\r\n\r\nIf you started the game from a command, replace the example path with the actual path to your replay, in double quotes. You can also use Watch replay in the game.\r\n\r\nThis replay window will now close.";
        for (out, byte) in text[..767].iter_mut().zip(
            prefix
                .iter()
                .chain(self.path[..self.len].iter())
                .chain(suffix.iter()),
        ) {
            *out = *byte;
        }
        text
    }

    pub fn tell_and_exit(self) -> ! {
        let text = self.message();
        const MB_ICONERROR: u32 = 16;
        const MB_SETFOREGROUND: u32 = 1 << 16;
        const MB_TOPMOST: u32 = 1 << 18;
        // SAFETY: the generated imports have their SDK stdcall signatures.
        // Text/title are NUL-terminated ANSI, matching the game's path encoding.
        // ExitProcess terminates this process only and is documented not to return.
        unsafe {
            raw::engine_message_box_a(
                core::ptr::null_mut(),
                text.as_ptr().cast(),
                c"Skhid - Couldn't open replay".as_ptr().cast(),
                MB_ICONERROR | MB_SETFOREGROUND | MB_TOPMOST,
            );
            raw::engine_exit_process(1);
            core::hint::unreachable_unchecked()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn longest_path_keeps_complete_guidance_and_terminator() {
        let failure = FailedReplay {
            path: [b'x'; 260],
            len: 259,
        };
        let message = failure.message();
        let end = message.iter().position(|&b| b == 0).unwrap();
        let text = core::str::from_utf8(&message[..end]).unwrap();
        assert!(text.contains(&"x".repeat(259)));
        assert!(text.contains("replace the example path"));
        assert!(text.ends_with("This replay window will now close."));
    }
}
