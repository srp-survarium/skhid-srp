//! The 0.26g client's login server ([`v026g`]) and its sealed sign-in
//! ([`sealed`]). What is here is the part of the sign-in message every
//! version of the game shares; the 0.21d login server (TLS, the presence
//! pings, sign-out) is in `srp-021d/servers/login-server`.

use std::io::{self, Read};
use std::time::Duration;

pub mod sealed;
pub mod v026g;

/// How often sessions unseen for [`session::SESSION_IDLE_LIMIT`] are ended: the
/// original login server's `collect_interval`
/// (`login_server/sources/entry_point.cpp`).
const COLLECT_INTERVAL: Duration = Duration::from_secs(60);

/// How often the games gone without signing out are looked for
/// (`SessionStore::drop_gone_players`).
const GONE_CHECK_INTERVAL: Duration = Duration::from_secs(5);

/// Read until `buffer` holds a whole message, as `complete` measures it from
/// what has arrived (`None`: can't tell yet), and return its length. Whatever
/// else came in the same reads is read too and ignored.
fn read_message(
    stream: &mut impl Read,
    buffer: &mut [u8],
    complete: impl Fn(&[u8]) -> Option<usize>,
) -> io::Result<usize> {
    let mut filled = 0;
    loop {
        if let Some(len) = complete(&buffer[..filled])
            && len <= filled
        {
            return Ok(len);
        }
        if filled == buffer.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "message too long",
            ));
        }
        match stream.read(&mut buffer[filled..])? {
            0 => return Err(io::ErrorKind::UnexpectedEof.into()),
            read => filled += read,
        }
    }
}

/// The 0.100b client's version field: `"0.100b"` and two zeros.
const VERSION_0100B: &[u8] = b"0.100b";

/// A sign-in, after its type byte: the user name (u8 length), then the game
/// version. 0.21d (`sign_in_on_connected` 0x23E759) sends a 12-byte version
/// (`"0.21d"`, zero-padded), a `connect_from_eula_wnd` byte and a u32
/// `online_queue_order_id`; 0.100b sent an 8-byte version and nothing more.
/// Reading only part of it would leave the rest to be taken for the start of
/// the TLS handshake.
fn sign_in_length(buffer: &[u8]) -> Option<usize> {
    let user_name = 1 + usize::from(*buffer.first()?);
    let version = buffer.get(user_name..user_name + VERSION_0100B.len())?;
    Some(
        user_name
            + if version == VERSION_0100B {
                8
            } else {
                12 + 1 + 4
            },
    )
}

/// What a sign-in says besides the password.
#[derive(Debug, PartialEq)]
struct SignIn {
    /// What the player typed as the account (`m_account_email`): any user
    /// name, an email or not.
    user_name: String,
    version: String,
    /// 0.26g: what a patched client says of its patches, 0 from an exe
    /// that says nothing (`docs/notes/client_patch.md` §6).
    /// The version field is 12 bytes, the text and zeros; the patch puts
    /// this into byte 7, past the zero that ends a text of six letters.
    patch_revision: u8,
    /// 0.26g: byte 8 of the version field (`client_patch.md` §3.25). A game
    /// of patch revision 13 or later reports here what it saw of one known
    /// cheat; 0 from any older exe. Bit 0 (1): a stale shell_32.dll in the
    /// temporary folder. Bit 7 (0x80): the check ran. Only logged.
    cheat_flags: u8,
    /// 0.21d: 1 when the player signs in from the EULA window.
    from_eula_window: bool,
    /// 0.21d: the place in the online queue it rejoins, `u32::MAX` for none.
    queue_order_id: u32,
}

/// Read a whole sign-in, as `sign_in_length` measured it.
fn parse_sign_in(message: &[u8]) -> SignIn {
    let name_end = 1 + usize::from(message[0]);
    let (version, rest) = message[name_end..].split_at((message.len() - name_end).min(12));
    let field = version;
    let version = field.split(|&byte| byte == 0).next().unwrap_or_default();
    // Only past the text's own zero, and only in a whole field.
    let (patch_revision, cheat_flags) = if field.len() == 12 && version.len() < 7 {
        (field[7], field[8])
    } else {
        (0, 0)
    };
    let (from_eula_window, queue_order_id) = match *rest {
        [eula, a, b, c, d] => (eula != 0, u32::from_le_bytes([a, b, c, d])),
        _ => (false, u32::MAX),
    };
    SignIn {
        user_name: String::from_utf8_lossy(&message[1..name_end]).into_owned(),
        version: String::from_utf8_lossy(version).into_owned(),
        patch_revision,
        cheat_flags,
        from_eula_window,
        queue_order_id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Hands out its bytes a few at a time, as a socket may.
    struct Trickle<'a>(&'a [u8]);

    impl Read for Trickle<'_> {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            let n = self.0.len().min(buffer.len()).min(3);
            buffer[..n].copy_from_slice(&self.0[..n]);
            self.0 = &self.0[n..];
            Ok(n)
        }
    }

    /// A 0.21d sign-in after its type byte, as `sign_in_on_connected` writes it.
    fn sign_in_0_21d(email: &[u8], eula: u8, queue: u32) -> Vec<u8> {
        let mut sign_in = vec![email.len() as u8];
        sign_in.extend(email);
        sign_in.extend(b"0.21d\0\0\0\0\0\0\0");
        sign_in.push(eula);
        sign_in.extend(queue.to_le_bytes());
        sign_in
    }

    #[test]
    fn a_sign_in_is_read_whole_however_it_arrives() {
        let mut sign_in = vec![5];
        sign_in.extend(b"a@b.c0.100b\0\0");
        let mut buffer = [0; 64];
        let len = read_message(&mut Trickle(&sign_in), &mut buffer, sign_in_length).unwrap();
        assert_eq!(&buffer[..len], sign_in.as_slice());

        // The TLS handshake follows: nothing of it may be read with the sign-in.
        let mut sign_in = sign_in_0_21d(b"a@b.c", 0, u32::MAX);
        let len_0_21d = sign_in.len();
        sign_in.extend([0x16, 3, 1]);
        let len = read_message(&mut Trickle(&sign_in), &mut buffer, sign_in_length).unwrap();
        assert_eq!(len, len_0_21d);
        assert_eq!(len, 1 + 5 + 17);
    }

    #[test]
    fn a_sign_in_says_its_email_version_and_queue_place() {
        let sign_in = sign_in_0_21d(b"player@e2e.invalid", 1, 7);
        assert_eq!(
            parse_sign_in(&sign_in),
            SignIn {
                user_name: "player@e2e.invalid".into(),
                version: "0.21d".into(),
                patch_revision: 0,
                cheat_flags: 0,
                from_eula_window: true,
                queue_order_id: 7,
            }
        );
        let mut old = vec![3];
        old.extend(b"a@b0.100b\0\0");
        assert_eq!(
            parse_sign_in(&old),
            SignIn {
                user_name: "a@b".into(),
                version: "0.100b".into(),
                patch_revision: 0,
                cheat_flags: 0,
                from_eula_window: false,
                queue_order_id: u32::MAX,
            }
        );
    }

    #[test]
    fn a_patched_0_26g_client_says_its_patch_revision() {
        let read = |field: &[u8; 12]| {
            let mut sign_in = vec![3];
            sign_in.extend(b"a@b");
            sign_in.extend(field);
            sign_in.push(0);
            sign_in.extend(u32::MAX.to_le_bytes());
            let sign_in = parse_sign_in(&sign_in);
            (sign_in.version, sign_in.patch_revision, sign_in.cheat_flags)
        };
        // The stock field, and what the patched `sign_in_on_connected`
        // 0x3807D0 makes of it: the text is the same.
        assert_eq!(read(b"0.26g0\0\0\0\0\0\0"), ("0.26g0".into(), 0, 0));
        assert_eq!(read(b"0.26g0\0\x01\0\0\0\0"), ("0.26g0".into(), 1, 0));
        assert_eq!(read(b"0.26g0\0\x02\0\0\0\0"), ("0.26g0".into(), 2, 0));
        // Revision 13 also sets byte 8 (the cheat check, §3.25): 0x80 ran,
        // 0x81 ran and found the stale shell_32.dll.
        assert_eq!(read(b"0.26g0\0\x0d\x80\0\0\0"), ("0.26g0".into(), 13, 0x80));
        assert_eq!(read(b"0.26g0\0\x0d\x81\0\0\0"), ("0.26g0".into(), 13, 0x81));
        // A text that reaches byte 7 keeps it: no revision, no flags.
        assert_eq!(read(b"0.26g001\0\0\0\0"), ("0.26g001".into(), 0, 0));
        assert_eq!(read(b"0.26g00\0\x01\0\0\0"), ("0.26g00".into(), 0, 0));
        // 0.100b's field is shorter than 12 bytes.
        let mut old = vec![3];
        old.extend(b"a@b0.100b\0\0");
        assert_eq!(parse_sign_in(&old).patch_revision, 0);
        assert_eq!(parse_sign_in(&old).cheat_flags, 0);
    }

    #[test]
    fn a_cut_or_oversized_message_is_an_error_not_a_panic() {
        let mut buffer = [0; 64];
        assert!(read_message(&mut Trickle(&[200, 1, 2]), &mut buffer, sign_in_length).is_err());
        let mut small = [0; 4];
        assert!(read_message(&mut Trickle(&[3, b'a', b'@', b'b', b'0']), &mut small, sign_in_length).is_err());
    }
}
