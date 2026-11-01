//! HTTP GETs, through Windows' own WinHTTP: its TLS, its certificate
//! store, the system's proxy settings. A file of a list (a patch, the
//! game's archive) follows its redirects and may go on from a byte it got
//! before ([`get_file`]); nothing else does.

use alloc::string::String;
use alloc::vec::Vec;
use core::ffi::c_void;
use core::ptr::{null, null_mut};

use updater_logic::download::{self, PAGE_HEAD, Speed};
use updater_logic::url::{self, MAX_REDIRECTS, Url};
use windows_sys::Win32::Networking::WinHttp::{
    WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
    WINHTTP_DISABLE_COOKIES, WINHTTP_FLAG_BYPASS_PROXY_CACHE, WINHTTP_FLAG_SECURE,
    WINHTTP_FLAG_SECURE_PROTOCOL_TLS1_2, WINHTTP_FLAG_SECURE_PROTOCOL_TLS1_3,
    WINHTTP_OPTION_DISABLE_FEATURE, WINHTTP_OPTION_REDIRECT_POLICY,
    WINHTTP_OPTION_REDIRECT_POLICY_NEVER, WINHTTP_OPTION_SECURE_PROTOCOLS,
    WINHTTP_QUERY_CONTENT_LENGTH, WINHTTP_QUERY_CONTENT_RANGE, WINHTTP_QUERY_CONTENT_TYPE,
    WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_QUERY_LOCATION, WINHTTP_QUERY_STATUS_CODE,
    WinHttpCloseHandle, WinHttpConnect, WinHttpOpen, WinHttpOpenRequest, WinHttpQueryHeaders,
    WinHttpReadData, WinHttpReceiveResponse, WinHttpSendRequest, WinHttpSetOption,
    WinHttpSetTimeouts,
};

use crate::win::{self, Code, from_wide, last_error, wide};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpError {
    /// A step Windows could not do: no network, no such host, no answer, a
    /// certificate it does not trust. The step's name and the error code.
    Network(&'static str, Code),
    /// The server answered, with another status than 200 (or 206 to a
    /// request from a byte on).
    Status(u32),
    /// The answer is longer than the caller allows.
    TooLarge,
    /// The connection ended before the answer did.
    Cut { got: u64, expected: u64 },
    /// The caller's progress function said stop.
    Stopped,
    /// A file of a list answered with a web page (`text/html`), not the
    /// file: the page's title, when its first bytes have one, and the
    /// bytes read of it.
    Html(Option<String>, u64),
    /// A patch's file was redirected more than `url::MAX_REDIRECTS` times.
    Redirects,
    /// A patch's file was redirected to an address a list may not name
    /// (its `Location`, at most 200 characters of it).
    BadRedirect(String),
    /// Slower than `download::MIN_SPEED` over a stretch of
    /// `download::SLOW_WINDOW_MS`: its bytes a second.
    Slow(u64),
    /// A `206 Partial Content` of another range than the one asked for.
    BadRange,
}

/// What a download slower than `download::MIN_SPEED` over a stretch of
/// `download::SLOW_WINDOW_MS` does.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Slow {
    /// Goes on: no other address would give the file (the last of a list's,
    /// and every download of one address).
    Wait,
    /// Ends with `HttpError::Slow`: another address may do better.
    Leave,
}

/// Where a file's body goes: `sink(at, chunk)`, `chunk` the file's bytes
/// from byte `at`.
pub type Sink<'a> = dyn FnMut(u64, &[u8]) -> Result<(), HttpError> + 'a;

struct Internet(*mut c_void);

impl Drop for Internet {
    fn drop(&mut self) {
        unsafe {
            WinHttpCloseHandle(self.0);
        }
    }
}

fn handle(raw: *mut c_void, step: &'static str) -> Result<Internet, HttpError> {
    if raw.is_null() {
        Err(HttpError::Network(step, last_error()))
    } else {
        Ok(Internet(raw))
    }
}

fn set_option(handle: &Internet, option: u32, value: u32) -> bool {
    unsafe { WinHttpSetOption(handle.0, option, (&raw const value).cast::<c_void>(), 4) != 0 }
}

fn number_header(request: &Internet, which: u32) -> Option<u32> {
    let mut value = 0_u32;
    let mut size = 4_u32;
    let ok = unsafe {
        WinHttpQueryHeaders(
            request.0,
            which | WINHTTP_QUERY_FLAG_NUMBER,
            null(),
            (&raw mut value).cast::<c_void>(),
            &mut size,
            null_mut(),
        )
    };
    (ok != 0).then_some(value)
}

/// The `Content-Length` the server named, read as text: a number of the
/// header is 32 bits, a patch or the game's archive may pass 4 GB.
fn content_length(request: &Internet) -> Option<u64> {
    let mut text = [0_u16; 32];
    let mut size = (text.len() * 2) as u32;
    let ok = unsafe {
        WinHttpQueryHeaders(
            request.0,
            WINHTTP_QUERY_CONTENT_LENGTH,
            null(),
            text.as_mut_ptr().cast::<c_void>(),
            &mut size,
            null_mut(),
        )
    };
    if ok == 0 {
        return None;
    }
    let digits = &text[..(size as usize / 2).min(text.len())];
    if digits.is_empty() || digits.len() > 19 || !digits.iter().all(|c| (0x30..=0x39).contains(c)) {
        return None;
    }
    Some(
        digits
            .iter()
            .fold(0_u64, |n, c| n * 10 + u64::from(c - 0x30)),
    )
}

/// GET `path` from the server: the body, if the status is 200 and the body
/// at most `limit` bytes. `progress(done, total)` is called as the bytes
/// come (the total when the server names it); `false` from it stops the
/// download.
pub fn get(
    url: &Url,
    path: &str,
    limit: usize,
    progress: &mut dyn FnMut(u64, Option<u64>) -> bool,
) -> Result<Vec<u8>, HttpError> {
    let mut body = Vec::new();
    get_stream(
        url,
        path,
        limit as u64,
        &mut |chunk| {
            body.extend_from_slice(chunk);
            Ok(())
        },
        progress,
    )?;
    Ok(body)
}

/// A bounded download to a sink; even multi-gigabyte files use one 64 KiB buffer.
/// Redirects and cookies remain disabled, including for the game mirror.
pub fn get_stream(
    url: &Url,
    path: &str,
    limit: u64,
    sink: &mut dyn FnMut(&[u8]) -> Result<(), HttpError>,
    progress: &mut dyn FnMut(u64, Option<u64>) -> bool,
) -> Result<(), HttpError> {
    let session = open_session()?;
    let mut full_path = String::from(&*url.prefix);
    full_path.push_str(path);
    let answer = ask(&session, url, &full_path, 0)?;
    if answer.status != 200 {
        return Err(HttpError::Status(answer.status));
    }
    receive(
        &answer.request,
        0,
        limit,
        Slow::Wait,
        &mut |_, chunk| sink(chunk),
        progress,
    )
}

/// A file of a list at `url` (an address of the list, `url::patch`: its
/// prefix is the whole path; a patch, the game's archive): up to
/// `url::MAX_REDIRECTS` redirects are followed, each only to an address the
/// list could name (`url::redirect`), and `hop(to)` is told of each. An
/// answer of a web page (`Content-Type: text/html`) is no file: a sign-in
/// page, Google Drive's "cannot scan this file for viruses" or "quota
/// exceeded" page; its title is read from its first bytes.
///
/// `from` > 0: the bytes from there on are asked for (`Range`). A `206`
/// from that byte goes on there; a `200` (a server that does not give a
/// part) is the whole file again, from byte 0. `sink(at, chunk)`: `chunk`
/// is the file's from byte `at`. `progress(done, total)`: the file's bytes
/// up to here, of its size when the answer says it. `limit`: the most bytes
/// the file may have; `slow`: whether a slow download ends.
pub fn get_file(
    url: &Url,
    from: u64,
    limit: u64,
    slow: Slow,
    hop: &mut dyn FnMut(&str),
    sink: &mut Sink,
    progress: &mut dyn FnMut(u64, Option<u64>) -> bool,
) -> Result<(), HttpError> {
    let session = open_session()?;
    let mut url = url.clone();
    let mut redirects = 0;
    let answer = loop {
        let answer = ask(&session, &url, &url.prefix, from)?;
        if !matches!(answer.status, 301 | 302 | 303 | 307 | 308) {
            break answer;
        }
        let Some(location) = text_header(&answer.request, WINHTTP_QUERY_LOCATION, 2048) else {
            return Err(HttpError::Status(answer.status));
        };
        if redirects == MAX_REDIRECTS {
            return Err(HttpError::Redirects);
        }
        redirects += 1;
        let Some((text, next)) = url::redirect(&url, &location) else {
            let mut shown: String = location.chars().take(200).collect();
            if shown.len() < location.len() {
                shown.push_str("...");
            }
            return Err(HttpError::BadRedirect(shown));
        };
        hop(&text);
        url = next;
    };
    let at = match answer.status {
        200 => 0,
        206 if from > 0 => {
            text_header(&answer.request, WINHTTP_QUERY_CONTENT_RANGE, 128)
                .and_then(|range| download::range_start(&range))
                .filter(|&start| start == from)
                .ok_or(HttpError::BadRange)?;
            from
        }
        status => return Err(HttpError::Status(status)),
    };
    if text_header(&answer.request, WINHTTP_QUERY_CONTENT_TYPE, 256).is_some_and(|kind| {
        kind.trim_start()
            .get(..9)
            .is_some_and(|kind| kind.eq_ignore_ascii_case("text/html"))
    }) {
        // Its first bytes, for its title: a page is no file, however long.
        let mut page = Vec::new();
        let _ = receive(
            &answer.request,
            0,
            u64::MAX,
            Slow::Wait,
            &mut |_, chunk| {
                page.extend_from_slice(chunk);
                if page.len() >= PAGE_HEAD {
                    Err(HttpError::TooLarge)
                } else {
                    Ok(())
                }
            },
            &mut |_, _| true,
        );
        let read = page.len() as u64;
        return Err(HttpError::Html(download::title(&page), read));
    }
    receive(&answer.request, at, limit, slow, sink, progress)
}

/// The WinHTTP session of one download: no step waits longer than its
/// timeouts; TLS 1.2 and newer.
fn open_session() -> Result<Internet, HttpError> {
    let agent = wide("SkhidUpdater/1");
    let mut session = unsafe {
        WinHttpOpen(
            agent.as_ptr(),
            WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
            null(),
            null(),
            0,
        )
    };
    if session.is_null() {
        // Windows before 8.1 does not know the automatic setting.
        session = unsafe {
            WinHttpOpen(
                agent.as_ptr(),
                WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
                null(),
                null(),
                0,
            )
        };
    }
    let session = handle(session, "start")?;
    unsafe {
        // Name lookup, connect, send, receive: no step waits longer.
        WinHttpSetTimeouts(session.0, 10_000, 10_000, 30_000, 30_000);
    }
    // TLS 1.2 and newer; a Windows that does not know 1.3 takes 1.2 alone,
    // and one that refuses both keeps its own default.
    if !set_option(
        &session,
        WINHTTP_OPTION_SECURE_PROTOCOLS,
        WINHTTP_FLAG_SECURE_PROTOCOL_TLS1_2 | WINHTTP_FLAG_SECURE_PROTOCOL_TLS1_3,
    ) {
        set_option(
            &session,
            WINHTTP_OPTION_SECURE_PROTOCOLS,
            WINHTTP_FLAG_SECURE_PROTOCOL_TLS1_2,
        );
    }
    Ok(session)
}

/// A request sent and its answer's headers received.
struct Answer {
    request: Internet,
    status: u32,
    // After `request`: a request is closed before its connection.
    _connection: Internet,
}

/// GET `full_path` of `url`'s host, from byte `from` on when it is not 0
/// (`Range`): the answer's status and headers.
fn ask(session: &Internet, url: &Url, full_path: &str, from: u64) -> Result<Answer, HttpError> {
    let connection = handle(
        unsafe { WinHttpConnect(session.0, wide(&url.host).as_ptr(), url.port, 0) },
        "connect",
    )?;
    let flags = WINHTTP_FLAG_BYPASS_PROXY_CACHE | if url.secure { WINHTTP_FLAG_SECURE } else { 0 };
    let request = handle(
        unsafe {
            WinHttpOpenRequest(
                connection.0,
                wide("GET").as_ptr(),
                wide(full_path).as_ptr(),
                null(),
                null(),
                null(),
                flags,
            )
        },
        "request",
    )?;
    // WinHTTP follows no redirect itself (`get_file` follows a patch's,
    // checking each address), and no cookie is kept or sent.
    if !set_option(
        &request,
        WINHTTP_OPTION_REDIRECT_POLICY,
        WINHTTP_OPTION_REDIRECT_POLICY_NEVER,
    ) {
        return Err(HttpError::Network("disable redirects", last_error()));
    }
    if !set_option(
        &request,
        WINHTTP_OPTION_DISABLE_FEATURE,
        WINHTTP_DISABLE_COOKIES,
    ) {
        return Err(HttpError::Network("disable cookies", last_error()));
    }
    let range = (from > 0).then(|| wide(&alloc::format!("Range: bytes={from}-")));
    let (headers, length) = match &range {
        // u32::MAX: the headers end at their zero.
        Some(range) => (range.as_ptr(), u32::MAX),
        None => (null(), 0),
    };
    if unsafe { WinHttpSendRequest(request.0, headers, length, null(), 0, 0, 0) } == 0 {
        return Err(HttpError::Network("send", last_error()));
    }
    if unsafe { WinHttpReceiveResponse(request.0, null_mut()) } == 0 {
        return Err(HttpError::Network("receive", last_error()));
    }
    let status = number_header(&request, WINHTTP_QUERY_STATUS_CODE)
        .ok_or_else(|| HttpError::Network("status", last_error()))?;
    Ok(Answer {
        request,
        status,
        _connection: connection,
    })
}

/// A header of the answer as text, if it is there and at most `limit`
/// UTF-16 units.
fn text_header(request: &Internet, which: u32, limit: usize) -> Option<String> {
    let mut text = alloc::vec![0_u16; limit + 1];
    let mut size = (text.len() * 2) as u32;
    let ok = unsafe {
        WinHttpQueryHeaders(
            request.0,
            which,
            null(),
            text.as_mut_ptr().cast::<c_void>(),
            &mut size,
            null_mut(),
        )
    };
    (ok != 0).then(|| from_wide(&text[..(size as usize / 2).min(text.len())]))
}

/// The body of the answer, the file's from byte `at`, into `sink`; with
/// [`Slow::Leave`] a stretch slower than `download::MIN_SPEED` ends it.
fn receive(
    request: &Internet,
    at: u64,
    limit: u64,
    slow: Slow,
    sink: &mut Sink,
    progress: &mut dyn FnMut(u64, Option<u64>) -> bool,
) -> Result<(), HttpError> {
    let expected = content_length(request);
    let total = expected.map(|expected| at.saturating_add(expected));
    if total.is_some_and(|total| total > limit) {
        return Err(HttpError::TooLarge);
    }
    let mut received = 0_u64;
    let mut speed = Speed::new(win::ticks(), 0);
    let mut buffer = alloc::vec![0_u8; 64 * 1024];
    loop {
        if !progress(at + received, total) {
            return Err(HttpError::Stopped);
        }
        if slow == Slow::Leave
            && let Some(speed) = speed.slow(win::ticks(), received)
        {
            return Err(HttpError::Slow(speed));
        }
        let mut read = 0_u32;
        let ok = unsafe {
            WinHttpReadData(
                request.0,
                buffer.as_mut_ptr().cast::<c_void>(),
                buffer.len() as u32,
                &mut read,
            )
        };
        if ok == 0 {
            return Err(HttpError::Network("read", last_error()));
        }
        if read == 0 {
            break;
        }
        if at + received + u64::from(read) > limit {
            return Err(HttpError::TooLarge);
        }
        sink(at + received, &buffer[..read as usize])?;
        received += u64::from(read);
    }
    if let Some(expected) = expected
        && received != expected
    {
        return Err(HttpError::Cut {
            got: at + received,
            expected: at + expected,
        });
    }
    if !progress(at + received, total) {
        return Err(HttpError::Stopped);
    }
    Ok(())
}
