//! The login exchange as a state machine without IO (`docs/notes/login.md`; PROVEN with real-client captures;
//! `0x0C` layout from the reader at 0x20011F140):
//!
//! ```text
//! C->S 01 sign-in        u8 signin_type, str account, str version, str build, u32 start_token,
//!                        u32 crc32(key DER the client holds, 0 if none), u8 FF, u32 0B, rest
//! S->C 0D request_secure_data   u8 password_type, u8 0 (key follows), u32 len, DER RSAPublicKey, u32 challenge
//! C->S 02 secure data    u32 n, n bytes = RSA PKCS#1 v1.5 block with the server key
//!        plaintext       20 bytes, u8 signin_type, u8 password_type, str digest (40 hex), u32 challenge
//! S->C 0B success        u8 0, u32 session, u32 lobby ip (u32 LE of the host-order value), u16 lobby port
//! S->C 0C failed         u8 reason [, u8, u32 when reason is 2 or 3]
//! ```
//! Strings: `u8 len`, or `FF u16 len`.
//!
//! Steam mode (a client started with `-steam`, sign-in method 2; exe addresses in the docs of [`STEAM_REPLY`] and
//! [`IMPORT`]): the account signs in with its own name and password, typed in the client's Steam "Import" panel.
//!
//! ```text
//! C->S 01 sign-in        u8 2, u64 SteamID64 (in place of the account string), the same tail as method 1
//! S->C 10 04             opens the Import panel (status text "invalid user or password" for 4 s)
//! C->S 04 import         u32 n, n ticket bytes, u8 sub; sub 2: str e-mail, str password (plain text)
//!                                                       sub 1: u8 0x0A, str Steam persona name ("Create new")
//! S->C 0B / 10 04 / 0C   granted / Import panel again / refusal text (ban, lockout, ...) on the panel
//! ```
//! The SteamID and the ticket never grant anything: the SteamID is passed on as information only
//! ([`Attempt::steam_id`]), the ticket is ignored.

use crate::digest::{client_password_digest, form_field, FORM_EMAIL_MAX};
use crate::keys::LoginKey;
use std::fmt;
use std::net::{IpAddr, SocketAddrV4};

pub const SIGN_IN: u8 = 0x01;
pub const SECURE_DATA: u8 = 0x02;
pub const SUCCESS: u8 = 0x0B;
pub const FAILED: u8 = 0x0C;
pub const REQUEST_SECURE_DATA: u8 = 0x0D;
/// Steam-mode import request (`import_account`, 0x200126C7A..0x200126E17, login state 6; "Create new" 0x20011ED10,
/// state 4): `u32 len, len ticket bytes, u8 sub`, then sub 2 `str e-mail, str password` (the plain text of the password
/// field, written raw by 0x20011DF30: no RSA, no hash), sub 1 `u8 0x0A, str persona name`. Sent on the open login
/// connection: neither builder connects (PROVEN, both send through login_client+8 with 0x2005B64F0).
pub const IMPORT: u8 = 0x04;
/// Steam-mode status (dispatcher 0x20011F9F0 case 0x10, 0x20011FBA7): code 4 -> Import panel + text
/// `st_login_menu_status_invalid_user_or_password` for 4 s and the password field cleared (0x2001284D0); code 16 ->
/// Import panel + `st_account_already_linked_to_steam`; another code -> login state 4, no UI change. The login state
/// and the connection stay as they are (PROVEN).
pub const STEAM_REPLY: u8 = 0x10;
/// `10 04`: the Import panel.
pub const STEAM_IMPORT_PANEL: [u8; 2] = [STEAM_REPLY, 4];
/// Sign-in method byte of a `-steam` client (login_client+0x610 = 2, 0x20010B107).
pub const SIGNIN_STEAM: u8 = 2;
/// `04` sub-requests (LIKELY names from their callers `start_create_account` / `import_account`).
pub const IMPORT_SUB_CREATE: u8 = 1;
pub const IMPORT_SUB_IMPORT: u8 = 2;
/// `password_type` passed on for a Steam import: the server computed the type-1 digest (SHA-1 hex) itself.
pub const IMPORT_PASSWORD_TYPE: u8 = 1;

/// Random bytes in front of the secure-data plaintext (meaning not traced; ignored).
pub const PLAIN_PREFIX: usize = 20;

/// Why a sign-in failed: the `0x0C` refusal. PROVEN layout (0x20011F140): `u8 code`, and for codes 2 and 3 also
/// `u8 reason, u32 value`. The code -> text table is PROVEN (0x20010BAD0 / 0x20012A060):
/// 4 invalid user or password, 5 attempt interval violated, 2 banned, 3 suspended, 7 already online, 8 invalid
/// version, 9 e-mail not confirmed, 6 access level, 1 hardware ban.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Refusal {
    pub code: u8,
    /// Ban reason 0..15 (codes 2 and 3 only).
    pub reason: u8,
    /// Ban length in minutes (codes 2 and 3 only; LIKELY unit).
    pub value: u32,
}

impl Refusal {
    pub const HARDWARE_BANNED: Refusal = Refusal::plain(1);
    pub const BAD_CREDENTIALS: Refusal = Refusal::plain(4);
    pub const ATTEMPT_INTERVAL: Refusal = Refusal::plain(5);
    pub const ALREADY_ONLINE: Refusal = Refusal::plain(7);
    pub const INVALID_VERSION: Refusal = Refusal::plain(8);

    pub const fn plain(code: u8) -> Refusal {
        Refusal { code, reason: 0, value: 0 }
    }
    pub const fn banned(reason: u8, minutes: u32) -> Refusal {
        Refusal { code: 2, reason, value: minutes }
    }
    pub const fn suspended(reason: u8, minutes: u32) -> Refusal {
        Refusal { code: 3, reason, value: minutes }
    }

    /// The `0C` payload.
    pub fn payload(&self) -> Vec<u8> {
        let mut out = vec![FAILED, self.code];
        if self.code == 2 || self.code == 3 {
            out.push(self.reason);
            out.extend(self.value.to_le_bytes());
        }
        out
    }
}

/// What the authenticator sees: the account name comes from the sign-in request.
#[derive(Debug)]
pub struct Attempt<'a> {
    pub account: &'a str,
    pub signin_type: u8,
    pub password_type: u8,
    /// The 40-hex `account_digest` the client sends.
    pub digest: &'a str,
    pub client_version: &'a str,
    /// The client's address (the TCP peer of the login connection); None when unknown (tests, in-memory runs).
    pub peer: Option<IpAddr>,
    /// Steam mode only: the SteamID64 the client CLAIMS in its sign-in request. Self-declared and forgeable: information
    /// only, never a reason to grant anything. None for every other sign-in method.
    pub steam_id: Option<u64>,
}

/// A granted sign-in: the session id the lobby checks (2E) and where the lobby is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Granted {
    pub session: u32,
    pub lobby: SocketAddrV4,
}

pub trait Authenticator: Send + Sync {
    fn authenticate(&self, attempt: &Attempt<'_>) -> Result<Granted, Refusal>;
}

/// Development authenticator: accepts every account and digest.
pub struct AcceptAll {
    pub session: u32,
    pub lobby: SocketAddrV4,
}

impl Authenticator for AcceptAll {
    fn authenticate(&self, _: &Attempt<'_>) -> Result<Granted, Refusal> {
        Ok(Granted { session: self.session, lobby: self.lobby })
    }
}

/// The sign-in request (message 0x01).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignIn {
    pub signin_type: u8,
    /// The account name (the e-mail typed in the login form). Empty for a Steam sign-in (method 2), which carries
    /// [`SignIn::steam_id`] in its place.
    pub account: String,
    /// Method 2 only: the 8 raw bytes in place of the account string, the SteamID64 (sign-in builder 0x20011F5DE, from
    /// `SteamUser()->GetSteamID()` 0x200120CD0). Claimed by the client, never checked.
    pub steam_id: Option<u64>,
    pub client_version: String,
    pub build: String,
    pub start_token: u32,
    pub key_crc: u32,
    pub tail_flag: u8,
    pub tail_value: u32,
    /// What follows: the client's own public key (base64) with its length fields.
    pub rest: Vec<u8>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum LoginError {
    Empty,
    /// A message of this type in this state.
    Unexpected { state: &'static str, message: u8 },
    Malformed(&'static str),
}

impl fmt::Display for LoginError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoginError::Empty => write!(f, "empty frame"),
            LoginError::Unexpected { state, message } => write!(f, "message {message:#04x} while {state}"),
            LoginError::Malformed(what) => write!(f, "malformed {what}"),
        }
    }
}

impl std::error::Error for LoginError {}

struct Reader<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let s = self.b.get(self.i..self.i.checked_add(n)?)?;
        self.i += n;
        Some(s)
    }
    fn u8(&mut self) -> Option<u8> {
        self.take(1).map(|b| b[0])
    }
    fn u16(&mut self) -> Option<u16> {
        self.take(2).map(|b| u16::from_le_bytes([b[0], b[1]]))
    }
    fn u32(&mut self) -> Option<u32> {
        self.take(4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn u64(&mut self) -> Option<u64> {
        self.take(8).map(|b| u64::from_le_bytes(b.try_into().unwrap()))
    }
    /// A string's raw bytes (no UTF-8 check).
    fn string_bytes(&mut self) -> Option<&'a [u8]> {
        let mut len = usize::from(self.u8()?);
        if len == 0xFF {
            len = usize::from(self.u16()?);
        }
        self.take(len)
    }
    fn string(&mut self) -> Option<String> {
        String::from_utf8(self.string_bytes()?.to_vec()).ok()
    }
    fn rest(&mut self) -> &'a [u8] {
        let r = &self.b[self.i..];
        self.i = self.b.len();
        r
    }
}

/// Parse a sign-in payload (the frame payload including the type byte 0x01).
pub fn parse_sign_in(payload: &[u8]) -> Result<SignIn, LoginError> {
    let mut r = Reader { b: payload, i: 0 };
    let bad = || LoginError::Malformed("sign-in");
    if r.u8().ok_or_else(bad)? != SIGN_IN {
        return Err(bad());
    }
    let signin_type = r.u8().ok_or_else(bad)?;
    // method 2 (Steam) carries the SteamID64 in place of the account string; every other method carries the string
    let (account, steam_id) = if signin_type == SIGNIN_STEAM {
        (String::new(), Some(r.u64().ok_or_else(bad)?))
    } else {
        (r.string().ok_or_else(bad)?, None)
    };
    Ok(SignIn {
        signin_type,
        account,
        steam_id,
        client_version: r.string().ok_or_else(bad)?,
        build: r.string().ok_or_else(bad)?,
        start_token: r.u32().ok_or_else(bad)?,
        key_crc: r.u32().ok_or_else(bad)?,
        tail_flag: r.u8().ok_or_else(bad)?,
        tail_value: r.u32().ok_or_else(bad)?,
        rest: r.rest().to_vec(),
    })
}

/// The decrypted secure data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SecureData {
    pub prefix: [u8; PLAIN_PREFIX],
    pub signin_type: u8,
    pub password_type: u8,
    pub digest: String,
    pub challenge: u32,
}

pub fn parse_plaintext(plain: &[u8]) -> Result<SecureData, LoginError> {
    let bad = || LoginError::Malformed("secure data plaintext");
    let mut r = Reader { b: plain, i: 0 };
    let mut prefix = [0u8; PLAIN_PREFIX];
    prefix.copy_from_slice(r.take(PLAIN_PREFIX).ok_or_else(bad)?);
    let d = SecureData {
        prefix,
        signin_type: r.u8().ok_or_else(bad)?,
        password_type: r.u8().ok_or_else(bad)?,
        digest: r.string().ok_or_else(bad)?,
        challenge: r.u32().ok_or_else(bad)?,
    };
    if !r.rest().is_empty() {
        return Err(bad());
    }
    Ok(d)
}

/// A Steam-mode import request (`04`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportRequest {
    /// Length of the encrypted app ticket in front (0 unless the server asked for one with `0F`; ignored).
    pub ticket_len: usize,
    pub kind: ImportKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImportKind {
    /// Sub 2, the Import panel: the e-mail and password fields as typed (raw bytes; the password in clear).
    Import { email: Vec<u8>, password: Vec<u8> },
    /// Sub 1, "Create new": the fixed byte (0x0A from the builder, 0x20011ED33; meaning not traced) and the Steam
    /// persona name. No password.
    Create { fixed: u8, name: Vec<u8> },
}

/// Parse an import payload (including the type byte 0x04). Bytes after the last field are ignored.
pub fn parse_import(payload: &[u8]) -> Result<ImportRequest, LoginError> {
    let bad = || LoginError::Malformed("import");
    let mut r = Reader { b: payload, i: 0 };
    if r.u8().ok_or_else(bad)? != IMPORT {
        return Err(bad());
    }
    let ticket_len = r.u32().ok_or_else(bad)? as usize;
    r.take(ticket_len).ok_or_else(bad)?;
    let kind = match r.u8().ok_or_else(bad)? {
        IMPORT_SUB_IMPORT => ImportKind::Import {
            email: r.string_bytes().ok_or_else(bad)?.to_vec(),
            password: r.string_bytes().ok_or_else(bad)?.to_vec(),
        },
        IMPORT_SUB_CREATE => {
            let fixed = r.u8().ok_or_else(bad)?;
            ImportKind::Create { fixed, name: r.string_bytes().ok_or_else(bad)?.to_vec() }
        }
        _ => return Err(LoginError::Malformed("import sub-request")),
    };
    Ok(ImportRequest { ticket_len, kind })
}

/// What a frame led to.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Waiting for the next frame.
    Continue,
    Granted { account: String, granted: Granted },
    /// Refused (0x0C already in `frames`); close the connection after sending.
    Refused { account: String, reason: Refusal },
}

#[derive(Debug, PartialEq, Eq)]
pub struct Reply {
    /// Payloads to send, each as one frame (see `frame::frame`).
    pub payloads: Vec<Vec<u8>>,
    pub outcome: Outcome,
}

enum State {
    AwaitSignIn,
    AwaitSecure(SignIn),
    /// Steam mode: `10 04` sent, the player types in the Import panel; `04` expected (as often as it fails).
    AwaitImport(SignIn),
    Done,
}

pub struct Handshake<'a> {
    key: &'a LoginKey,
    password_type: u8,
    challenge: u32,
    state: State,
    /// The client's address, passed on in [`Attempt::peer`] (the per name + address sign-in lockout).
    pub peer: Option<IpAddr>,
}

impl<'a> Handshake<'a> {
    /// `password_type`: asked from the client in 0x0D. `challenge`: per connection,
    /// echoed in the secure data.
    pub fn new(key: &'a LoginKey, password_type: u8, challenge: u32) -> Self {
        Self { key, password_type, challenge, state: State::AwaitSignIn, peer: None }
    }

    pub fn on_frame(&mut self, payload: &[u8], auth: &dyn Authenticator) -> Result<Reply, LoginError> {
        let &message = payload.first().ok_or(LoginError::Empty)?;
        match (&self.state, message) {
            (State::AwaitSignIn, SIGN_IN) => {
                let sign_in = parse_sign_in(payload)?;
                if let Some(steam_id) = sign_in.steam_id {
                    log::info!(
                        "steam sign-in (method 2) claiming SteamID64 {steam_id}, client {:?}: answering 10 04 (Import panel; the account signs in with its own name and password)",
                        sign_in.client_version
                    );
                    self.state = State::AwaitImport(sign_in);
                    return Ok(Reply { payloads: vec![STEAM_IMPORT_PANEL.to_vec()], outcome: Outcome::Continue });
                }
                if sign_in.key_crc == self.key.public_crc32() {
                    log::debug!("client already holds the current login key");
                }
                let der = self.key.public_der();
                let mut out = vec![REQUEST_SECURE_DATA, self.password_type, 0];
                out.extend((der.len() as u32).to_le_bytes());
                out.extend(der);
                out.extend(self.challenge.to_le_bytes());
                self.state = State::AwaitSecure(sign_in);
                Ok(Reply { payloads: vec![out], outcome: Outcome::Continue })
            }
            (State::AwaitSecure(_), SECURE_DATA) => {
                let State::AwaitSecure(sign_in) = std::mem::replace(&mut self.state, State::Done) else {
                    unreachable!()
                };
                Ok(self.secure(sign_in, payload, auth)?)
            }
            (State::AwaitImport(_), IMPORT) => self.import(payload, auth),
            (state, _) => Err(LoginError::Unexpected {
                state: match state {
                    State::AwaitSignIn => "waiting for sign-in",
                    State::AwaitSecure(_) => "waiting for secure data",
                    State::AwaitImport(_) => "waiting for the steam import",
                    State::Done => "finished",
                },
                message,
            }),
        }
    }

    /// How long the connection may sit silent now: [`crate::IMPORT_READ_TIMEOUT`] while the Steam Import panel waits
    /// for the player, [`crate::READ_TIMEOUT`] otherwise.
    pub fn read_timeout(&self) -> std::time::Duration {
        match self.state {
            State::AwaitImport(_) => crate::IMPORT_READ_TIMEOUT,
            _ => crate::READ_TIMEOUT,
        }
    }

    /// `04` in Steam mode. Sub 2: the typed password is hashed the way the normal client does
    /// ([`client_password_digest`]) and checked by the same authenticator (lockout, bans, version, auto-register).
    /// Granted -> `0B` as usual. Wrong name or password (code 4) -> `10 04`, the panel again. Other refusals -> their
    /// `0C`: the client shows its text on the current panel and keeps the connection (0x20011F140 sets login state 0,
    /// 0x20012A060 sets only the status text; no disconnect). The connection stays open after every refusal, so the
    /// player can correct the fields and press Import again; the attempt lockout bounds the guessing.
    /// Sub 1 ("Create new", no password) -> `10 04`.
    fn import(&mut self, payload: &[u8], auth: &dyn Authenticator) -> Result<Reply, LoginError> {
        let req = parse_import(payload)?;
        let State::AwaitImport(sign_in) = &self.state else { unreachable!() };
        let panel = || Reply { payloads: vec![STEAM_IMPORT_PANEL.to_vec()], outcome: Outcome::Continue };
        let (email, password) = match &req.kind {
            ImportKind::Create { fixed, name } => {
                log::info!(
                    "steam import sub 1 (Create new, persona {:?}, byte {fixed:#04x}): refused, answering 10 04 (accounts need a name and password)",
                    String::from_utf8_lossy(name)
                );
                return Ok(panel());
            }
            ImportKind::Import { email, password } => (email, password),
        };
        if req.ticket_len > 0 {
            log::debug!("steam import: {}-byte app ticket ignored", req.ticket_len);
        }
        // the account name as the normal form would send it (99 bytes kept)
        let Ok(account) = std::str::from_utf8(form_field(email, FORM_EMAIL_MAX)) else {
            log::info!("steam import sub 2: the name is not UTF-8; answering 10 04");
            return Ok(panel());
        };
        let digest = client_password_digest(password);
        let steam_id = sign_in.steam_id;
        let attempt = Attempt {
            account,
            signin_type: SIGNIN_STEAM,
            password_type: IMPORT_PASSWORD_TYPE,
            digest: &digest,
            client_version: &sign_in.client_version,
            peer: self.peer,
            steam_id,
        };
        let sid = steam_id.unwrap_or(0);
        match auth.authenticate(&attempt) {
            Ok(granted) => {
                log::info!("steam import sub 2 of {account:?} (claimed SteamID64 {sid}): granted, answering 0B");
                let account = account.to_string();
                self.state = State::Done;
                Ok(Reply { payloads: vec![success_payload(&granted)], outcome: Outcome::Granted { account, granted } })
            }
            Err(reason) if reason == Refusal::BAD_CREDENTIALS => {
                log::info!("steam import sub 2 of {account:?} (claimed SteamID64 {sid}): wrong name or password, answering 10 04");
                Ok(panel())
            }
            Err(reason) => {
                log::info!("steam import sub 2 of {account:?} (claimed SteamID64 {sid}): refused, answering 0C {reason:?}; the panel stays open");
                Ok(Reply { payloads: vec![reason.payload()], outcome: Outcome::Continue })
            }
        }
    }

    fn secure(&self, sign_in: SignIn, payload: &[u8], auth: &dyn Authenticator) -> Result<Reply, LoginError> {
        let mut r = Reader { b: payload, i: 1 };
        let bad = || LoginError::Malformed("secure data");
        let n = r.u32().ok_or_else(bad)? as usize;
        let block = r.take(n).ok_or_else(bad)?;
        if !r.rest().is_empty() {
            return Err(bad());
        }
        let refuse = |reason: Refusal| Reply {
            payloads: vec![reason.payload()],
            outcome: Outcome::Refused { account: sign_in.account.clone(), reason },
        };
        // a block that does not decrypt with our key: wrong key or garbage
        let Some(plain) = (block.len() == self.key.block_size()).then(|| self.key.decrypt(block)).flatten() else {
            log::info!("secure data of {:?} did not decrypt", sign_in.account);
            return Ok(refuse(Refusal::BAD_CREDENTIALS));
        };
        let secure = match parse_plaintext(&plain) {
            Ok(s) => s,
            Err(e) => {
                log::info!("secure data of {:?} did not parse ({e}; {} plaintext bytes)", sign_in.account, plain.len());
                return Ok(refuse(Refusal::BAD_CREDENTIALS));
            }
        };
        if secure.challenge != self.challenge {
            log::info!("secure data of {:?} carries the wrong challenge", sign_in.account);
            return Ok(refuse(Refusal::BAD_CREDENTIALS));
        }
        let attempt = Attempt {
            account: &sign_in.account,
            signin_type: secure.signin_type,
            password_type: secure.password_type,
            digest: &secure.digest,
            client_version: &sign_in.client_version,
            peer: self.peer,
            steam_id: None,
        };
        match auth.authenticate(&attempt) {
            Ok(granted) => {
                Ok(Reply {
                    payloads: vec![success_payload(&granted)],
                    outcome: Outcome::Granted { account: sign_in.account, granted },
                })
            }
            Err(reason) => Ok(refuse(reason)),
        }
    }
}

/// The `0B` payload: `u8 0, u32 session, u32 lobby ip (LE of the host-order value), u16 lobby port`.
fn success_payload(granted: &Granted) -> Vec<u8> {
    let mut out = vec![SUCCESS, 0];
    out.extend(granted.session.to_le_bytes());
    out.extend(u32::from(*granted.lobby.ip()).to_le_bytes());
    out.extend(granted.lobby.port().to_le_bytes());
    out
}
